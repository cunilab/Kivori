//! Modelled ESP32-C3 time for the virtual Pip (docs/virtual-pip.md, "Modelled timing and internals").
//!
//! The production `Runtime::step` measures itself with the `Clock` port. On the host every read used
//! to return the request's time, so compose and flush cost 0 ms. [`SimClock`] keeps microseconds and,
//! when modelled, is charged what the chip would spend: a per-row compose cost from the committed
//! frame-cost fixture (`row_composed`, called by the runtime under `host-sim` only) and the SPI
//! transfer time of every panel window ([`SharedDisplay`]). Everything here is a pure function of the
//! frame the runtime draws, so a modelled run is deterministic.
use std::{cell::Cell, cell::RefCell, rc::Rc};

use kivori_firmware::{
    ports::{Clock, DisplaySink},
    profile::physical_st7789::SPI_CLOCK_HZ,
    render::TILE_ROWS,
    sim::CaptureDisplay,
};
use kivori_model::{
    desk::{DeskView, DisplayMode},
    Rect, Rgb565,
};
use serde::Deserialize;

/// The committed output of `firmware/pip/frame-cost` (an RV32IMC simulation of the real renderer).
const FRAME_COST_JSON: &str = include_str!("../../frame-cost/fixtures/frame-cost.json");
/// Time the device loop charges for a pass that did nothing: the real loop spins, the host cannot,
/// so an idle pass advances one millisecond (the resolution of input timestamps).
pub const IDLE_PASS_US: u64 = 1_000;
/// What the run loop spends at each service point inside a frame (between tile rows and display
/// windows: draining the input queue, moving USB bytes to RAM, pushing queued output). An assumption,
/// fitted only so a frame that composes in 79 ms and flushes in 0 to 1 ms lasts 80 ms or more, as
/// the unit's 12 fps requires. Input is serviced between rows on the unit, so it is not free.
pub const SERVICE_POINT_US: u64 = 150;
/// Panel commands around each window's pixels: CASET (1 + 4 bytes), RASET (1 + 4) and RAMWR (1).
const WINDOW_COMMAND_BYTES: u64 = 11;
/// RGB565 on the wire.
const BITS_PER_PIXEL: u64 = 16;

/// What the runtime is drawing, as the closest scenario of the frame-cost fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    IdleMascot,
    ConnectedDesk,
    MediaView,
    SystemView,
    ClockView,
    BuddyOverlay,
    SwitchBuddySystem,
}

impl Scenario {
    const ALL: [(Self, &'static str); 7] = [
        (Self::IdleMascot, "idle_mascot"),
        (Self::ConnectedDesk, "connected_desk"),
        (Self::MediaView, "media_view"),
        (Self::SystemView, "system_view"),
        (Self::ClockView, "clock_view"),
        (Self::BuddyOverlay, "buddy_overlay"),
        (Self::SwitchBuddySystem, "switch_buddy_system"),
    ];

    /// The scenario whose frame cost best matches `view` drawn with or without a volume `overlay`.
    ///
    /// Only the fixture's seven situations are measured. A frame outside them falls back to
    /// `ConnectedDesk` (the app's normal desk): the recovery, update and Volume views, which are
    /// cheap or unmeasured. A view-switch slide that is not Buddy and System takes the dearer of the
    /// two views it shows, an approximation.
    #[must_use]
    pub fn of(view: &DeskView, overlay: bool, costs: &FrameCosts) -> Self {
        if view.recovery_percent.is_some() || view.updating {
            return Self::ConnectedDesk;
        }
        let mode = view.status.mode;
        match view.previous_mode {
            Some(previous) if previous != mode => {
                return if matches!(
                    (previous, mode),
                    (DisplayMode::Buddy, DisplayMode::System)
                        | (DisplayMode::System, DisplayMode::Buddy)
                ) {
                    Self::SwitchBuddySystem
                } else {
                    let (a, b) = (
                        Self::of_mode(previous, view, overlay),
                        Self::of_mode(mode, view, overlay),
                    );
                    if costs.total_us(a) >= costs.total_us(b) {
                        a
                    } else {
                        b
                    }
                };
            }
            _ => {}
        }
        Self::of_mode(mode, view, overlay)
    }

    fn of_mode(mode: DisplayMode, view: &DeskView, overlay: bool) -> Self {
        match mode {
            DisplayMode::Buddy => {
                if overlay || view.feedback.is_some() || view.button_down {
                    Self::BuddyOverlay
                } else if view.controls.is_some()
                    || view.media_info.is_some()
                    || view.status.clock.is_some()
                {
                    Self::ConnectedDesk
                } else {
                    Self::IdleMascot
                }
            }
            DisplayMode::Clock => Self::ClockView,
            DisplayMode::Media => Self::MediaView,
            DisplayMode::System => Self::SystemView,
            DisplayMode::Volume => Self::ConnectedDesk,
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Deserialize)]
struct Fixture {
    scenarios: std::collections::BTreeMap<String, FixtureScenario>,
}

#[derive(Deserialize)]
struct FixtureScenario {
    row_ms_mean: Vec<f64>,
}

/// Per tile row compose time of every scenario, in microseconds.
#[derive(Clone, Debug)]
pub struct FrameCosts {
    rows_us: [[u64; TILE_ROWS]; 7],
}

impl FrameCosts {
    /// The costs in the committed fixture.
    ///
    /// # Panics
    /// If the fixture is malformed or lacks a scenario or a row: a build-time inconsistency.
    #[must_use]
    pub fn committed() -> Self {
        Self::parse(FRAME_COST_JSON).expect("committed frame-cost fixture")
    }

    fn parse(json: &str) -> Result<Self, String> {
        let fixture: Fixture = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut rows_us = [[0; TILE_ROWS]; 7];
        for (scenario, name) in Scenario::ALL {
            let rows = &fixture
                .scenarios
                .get(name)
                .ok_or_else(|| format!("fixture lacks scenario {name}"))?
                .row_ms_mean;
            if rows.len() != TILE_ROWS {
                return Err(format!("{name}: {} rows, expected {TILE_ROWS}", rows.len()));
            }
            for (slot, ms) in rows_us[scenario.index()].iter_mut().zip(rows) {
                *slot = (ms * 1000.0).round() as u64;
            }
        }
        Ok(Self { rows_us })
    }

    /// Compose time of tile row `row` of a frame of `scenario`.
    #[must_use]
    pub fn row_us(&self, scenario: Scenario, row: usize) -> u64 {
        self.rows_us[scenario.index()]
            .get(row)
            .copied()
            .unwrap_or(0)
    }

    /// Compose time of a whole frame of `scenario`.
    #[must_use]
    pub fn total_us(&self, scenario: Scenario) -> u64 {
        self.rows_us[scenario.index()].iter().sum()
    }
}

/// Microseconds the SPI link needs for one panel window of `pixels` pixels: the pixels plus the
/// address-window commands, at the product's SPI clock. DMA set-up and CPU time around it are not
/// modelled.
#[must_use]
pub fn window_transfer_us(pixels: usize) -> u64 {
    let bits = pixels as u64 * BITS_PER_PIXEL + WINDOW_COMMAND_BYTES * 8;
    (bits * 1_000_000).div_ceil(u64::from(SPI_CLOCK_HZ))
}

/// The virtual device clock: microseconds, settable like `VirtualClock`, and charged modelled time
/// when it has frame costs.
pub struct SimClock {
    us: Cell<u64>,
    costs: Option<FrameCosts>,
}

impl SimClock {
    /// A clock at 0 that only moves when told to (zero-time frames).
    #[must_use]
    pub fn ideal() -> Self {
        Self {
            us: Cell::new(0),
            costs: None,
        }
    }

    /// A clock at 0 that charges compose time per row and (through [`SharedDisplay`]) transfer time.
    #[must_use]
    pub fn modelled() -> Self {
        Self {
            us: Cell::new(0),
            costs: Some(FrameCosts::committed()),
        }
    }

    /// Whether this clock models the chip's time.
    #[must_use]
    pub fn is_modelled(&self) -> bool {
        self.costs.is_some()
    }

    /// Current time in microseconds.
    #[must_use]
    pub fn us(&self) -> u64 {
        self.us.get()
    }

    /// Sets the time to `ms` milliseconds.
    pub fn set_ms(&self, ms: u32) {
        self.us.set(u64::from(ms) * 1000);
    }

    /// Advances the clock by `us` microseconds.
    pub fn advance_us(&self, us: u64) {
        self.us.set(self.us.get().saturating_add(us));
    }
}

impl Clock for SimClock {
    fn now_ms(&self) -> u32 {
        (self.us.get() / 1000) as u32
    }

    fn row_composed(&self, row: usize, view: &DeskView, overlay: bool) {
        if let Some(costs) = &self.costs {
            self.advance_us(costs.row_us(Scenario::of(view, overlay, costs), row));
        }
    }
}

/// A [`DisplaySink`] over a shared capture buffer that charges each window's SPI time to the clock
/// when the clock is modelled.
pub struct SharedDisplay {
    capture: Rc<RefCell<CaptureDisplay>>,
    clock: Rc<SimClock>,
}

impl SharedDisplay {
    #[must_use]
    pub fn new(capture: Rc<RefCell<CaptureDisplay>>, clock: Rc<SimClock>) -> Self {
        Self { capture, clock }
    }

    fn charge(&self, pixels: usize) {
        if self.clock.is_modelled() {
            self.clock.advance_us(window_transfer_us(pixels));
        }
    }
}

impl DisplaySink for SharedDisplay {
    type Error = std::convert::Infallible;

    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.charge(pixels.len());
        self.capture.borrow_mut().blit_tile(rect, pixels)
    }

    fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), Self::Error> {
        self.charge(tiles.len());
        self.capture.borrow_mut().blit_tiles(rect, tiles, cols)
    }

    fn set_brightness(&mut self, step: u8) {
        self.capture.borrow_mut().set_brightness(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_fixture_gives_the_documented_row_costs() {
        let costs = FrameCosts::committed();
        let desk: Vec<u64> = (0..TILE_ROWS)
            .map(|r| costs.row_us(Scenario::ConnectedDesk, r))
            .collect();
        assert_eq!(desk, vec![4337, 8555, 20774, 23785, 14565, 7013]);
        let total = costs.total_us(Scenario::ConnectedDesk);
        assert!((78_900..=79_100).contains(&total), "{total}");
        assert!(costs.total_us(Scenario::MediaView) < total);
        assert!(costs.total_us(Scenario::SystemView) > total);
    }

    #[test]
    fn a_malformed_fixture_is_refused() {
        assert!(FrameCosts::parse("{}").is_err());
        assert!(FrameCosts::parse(r#"{"scenarios":{}}"#).is_err());
    }

    fn view(mode: DisplayMode) -> DeskView {
        let mut v = DeskView::default();
        v.status.mode = mode;
        v
    }

    #[test]
    fn frames_map_to_the_closest_measured_scenario() {
        let costs = FrameCosts::committed();
        let of = |v: &DeskView, overlay| Scenario::of(v, overlay, &costs);
        assert_eq!(of(&view(DisplayMode::Buddy), false), Scenario::IdleMascot);
        let mut desk = view(DisplayMode::Buddy);
        desk.controls = Some(Default::default());
        assert_eq!(of(&desk, false), Scenario::ConnectedDesk);
        assert_eq!(of(&desk, true), Scenario::BuddyOverlay);
        let mut pressed = desk;
        pressed.button_down = true;
        assert_eq!(of(&pressed, false), Scenario::BuddyOverlay);
        assert_eq!(of(&view(DisplayMode::Clock), false), Scenario::ClockView);
        assert_eq!(of(&view(DisplayMode::Media), false), Scenario::MediaView);
        assert_eq!(of(&view(DisplayMode::System), false), Scenario::SystemView);
        assert_eq!(
            of(&view(DisplayMode::Volume), false),
            Scenario::ConnectedDesk
        );
        let mut slide = view(DisplayMode::System);
        slide.previous_mode = Some(DisplayMode::Buddy);
        assert_eq!(of(&slide, false), Scenario::SwitchBuddySystem);
        slide.status.mode = DisplayMode::Media;
        slide.previous_mode = Some(DisplayMode::System);
        assert_eq!(of(&slide, false), Scenario::SystemView);
        let mut recovery = view(DisplayMode::System);
        recovery.recovery_percent = Some(10);
        assert_eq!(of(&recovery, false), Scenario::ConnectedDesk);
    }

    #[test]
    fn the_modelled_clock_charges_rows_and_the_ideal_clock_does_not() {
        let desk = {
            let mut v = view(DisplayMode::Buddy);
            v.controls = Some(Default::default());
            v
        };
        let modelled = SimClock::modelled();
        for row in 0..TILE_ROWS {
            modelled.row_composed(row, &desk, false);
        }
        assert_eq!(
            modelled.us(),
            FrameCosts::committed().total_us(Scenario::ConnectedDesk)
        );
        let ideal = SimClock::ideal();
        ideal.row_composed(0, &desk, false);
        assert_eq!(ideal.us(), 0);
    }

    #[test]
    fn a_window_takes_its_pixels_at_the_spi_clock() {
        // A full tile row (240 x 40) at 20 MHz and 16 bits per pixel: 7.68 ms plus the commands.
        let row = window_transfer_us(240 * 40);
        assert!((7_680..7_700).contains(&row), "{row}");
        assert!(window_transfer_us(1600) < window_transfer_us(3200));
    }
}
