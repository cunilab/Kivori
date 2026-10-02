//! Physical HW-040 [`InputSource`].
//!
//! This adapter does NOTHING but read pin levels. All conditioning and semantics live above
//! the port in `input::quadrature` and `input::gesture`, which is exactly what makes them
//! provable without hardware.
//!
//! HW-040 modules are open-collector with the common pin to ground, so inputs use internal
//! pull-ups and read ACTIVE-LOW. `sample()` inverts to logical levels. The concrete GPIO
//! assignments live in [`crate::profile::physical_st7789::RotaryProfile`], not here.
//!
//! The run loop only gets back to input between render passes, and composing plus flushing a
//! frame takes tens of milliseconds — longer than the quarter-steps of one detent. So every edge
//! on any of the three pins raises the GPIO interrupt, whose handler stores one timestamped level
//! snapshot in a bounded queue that [`InputSource::drain`] empties. The handler still only reads
//! pins: decoding stays above the port.

use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::gpio::{Event, Input, Io};
use heapless::Deque;
use kivori_model::input::InputLevels;
use kivori_model::ElapsedMs;

use crate::clock::EspClock;
use crate::ports::{Clock, InputSource};

/// Edge snapshots buffered between two drains. One detent is four edges; 64 covers a fast spin
/// across the longest render pass. On overflow new edges are dropped: the decoder then sees a
/// jump it counts as an invalid transition, which loses that detent but never invents one.
const EDGE_QUEUE: usize = 64;

type Pins = (Input<'static>, Input<'static>, Input<'static>);

static PINS: Mutex<RefCell<Option<Pins>>> = Mutex::new(RefCell::new(None));
static EDGES: Mutex<RefCell<Deque<(InputLevels, ElapsedMs), EDGE_QUEUE>>> =
    Mutex::new(RefCell::new(Deque::new()));

fn read(pins: &Pins) -> InputLevels {
    InputLevels {
        a: pins.0.is_low(),
        b: pins.1.is_low(),
        sw: pins.2.is_low(),
    }
}

#[esp_hal::handler]
fn on_edge() {
    critical_section::with(|cs| {
        let mut pins = PINS.borrow_ref_mut(cs);
        let Some(pins) = pins.as_mut() else {
            return;
        };
        pins.0.clear_interrupt();
        pins.1.clear_interrupt();
        pins.2.clear_interrupt();
        let _dropped_when_full = EDGES
            .borrow_ref_mut(cs)
            .push_back((read(pins), EspClock::new().now_ms()));
    });
}

/// Reads the HW-040 CLK/DT/SW lines and nothing else.
///
/// No debounce and no decoding: `sample()` is a direct, inverted read of the three pins, and the
/// interrupt handler records the same read on every edge.
pub struct PhysicalRotary;

impl PhysicalRotary {
    /// Takes ownership of already-configured `clk`/`dt`/`sw` inputs and starts capturing edges.
    ///
    /// Callers are expected to have configured each pin with an internal pull-up (the HW-040
    /// lines are active-low), matching [`crate::profile::physical_st7789::ROTARY`].
    #[must_use]
    pub fn new(
        io: &mut Io<'_>,
        mut clk: Input<'static>,
        mut dt: Input<'static>,
        mut sw: Input<'static>,
    ) -> Self {
        io.set_interrupt_handler(on_edge);
        critical_section::with(|cs| {
            clk.listen(Event::AnyEdge);
            dt.listen(Event::AnyEdge);
            sw.listen(Event::AnyEdge);
            PINS.borrow_ref_mut(cs).replace((clk, dt, sw));
        });
        Self
    }
}

impl InputSource for PhysicalRotary {
    fn sample(&mut self) -> InputLevels {
        critical_section::with(|cs| {
            PINS.borrow_ref(cs).as_ref().map_or(
                InputLevels {
                    a: false,
                    b: false,
                    sw: false,
                },
                read,
            )
        })
    }

    fn drain(&mut self, now_ms: ElapsedMs, f: &mut dyn FnMut(InputLevels, ElapsedMs)) {
        // Pop one at a time so the handler is never blocked for longer than one pop.
        while let Some((levels, at_ms)) =
            critical_section::with(|cs| EDGES.borrow_ref_mut(cs).pop_front())
        {
            f(levels, at_ms.min(now_ms));
        }
        f(self.sample(), now_ms);
    }
}
