//! Physical HW-040 + contextual buttons [`InputSource`].
//!
//! This adapter does NOTHING but read pin levels. All conditioning and semantics live above
//! the port in `input::quadrature` and `input::gesture`, which is exactly what makes them
//! provable without hardware.
//!
//! HW-040 modules switch the common pin to ground, so the lines read ACTIVE-LOW (the module has
//! its own pull-ups; the internal ones are enabled too, which is harmless). `sample()` inverts to
//! logical levels. The concrete GPIO assignments live in
//! [`crate::profile::physical_st7789::RotaryProfile`], not here.
//!
//! The run loop only gets back to input between display windows and render passes, and composing
//! plus flushing a frame takes tens of milliseconds — longer than the quarter-steps of one detent. So every edge
//! on any of the three pins raises the GPIO interrupt, whose handler stores one timestamped level
//! snapshot in a bounded queue that [`InputSource::drain`] empties. The handler still only reads
//! pins: decoding stays above the port.

use core::cell::{Cell, RefCell};

use critical_section::Mutex;
use embedded_hal::digital::InputPin;
use heapless::Deque;
use kivori_model::input::InputLevels;
use kivori_model::ElapsedMs;
use rp2040_hal::gpio::{DynPinId, FunctionSio, Interrupt, Pin, PullUp, SioInput};
use rp2040_hal::pac::{self, interrupt};

use crate::clock::TimerClock;
use crate::ports::{Clock, InputSource};

/// Edge snapshots buffered between two drains. One detent is four edges; 128 covers a fast spin
/// plus switch bounce across the longest render pass. On overflow the OLDEST edge is dropped, so
/// the latest edges (a switch release) keep their timing. Runtime observes the loss counter and
/// suppresses rotary decoding for the affected drain instead of joining phases across a gap.
const EDGE_QUEUE: usize = 128;

/// An encoder or button line: an input with the internal pull-up, its pin number erased so the
/// six lines share one type.
pub type In = Pin<DynPinId, FunctionSio<SioInput>, PullUp>;
type Pins = (In, In, In, [In; 3]);

static PINS: Mutex<RefCell<Option<Pins>>> = Mutex::new(RefCell::new(None));
static EDGES: Mutex<RefCell<Deque<(InputLevels, ElapsedMs), EDGE_QUEUE>>> =
    Mutex::new(RefCell::new(Deque::new()));

/// Edges dropped because `EDGES` was full (cumulative); shown by the `latency-probe` readout.
static DROPPED: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));

/// The most edges `EDGES` has held at once (since boot).
static HIGH_WATER: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));

/// Pin reads cannot fail on the RP2040 (`Error = Infallible`), so a failed read counts as released.
fn low(pin: &mut In) -> bool {
    pin.is_low().unwrap_or(false)
}

fn read(pins: &mut Pins) -> InputLevels {
    // (clk, dt, sw); `input::hw040` decides which pin is which quadrature channel.
    let [b0, b1, b2] = &mut pins.3;
    crate::input::hw040::levels(
        low(&mut pins.0),
        low(&mut pins.1),
        low(&mut pins.2),
        [low(b0), low(b1), low(b2)],
    )
}

fn clear_edges(pin: &mut In) {
    pin.clear_interrupt(Interrupt::EdgeLow);
    pin.clear_interrupt(Interrupt::EdgeHigh);
}

fn listen(pin: &mut In) {
    pin.set_interrupt_enabled(Interrupt::EdgeLow, true);
    pin.set_interrupt_enabled(Interrupt::EdgeHigh, true);
}

#[interrupt]
fn IO_IRQ_BANK0() {
    critical_section::with(|cs| {
        let mut pins = PINS.borrow_ref_mut(cs);
        let Some(pins) = pins.as_mut() else {
            return;
        };
        clear_edges(&mut pins.0);
        clear_edges(&mut pins.1);
        clear_edges(&mut pins.2);
        for key in &mut pins.3 {
            clear_edges(key);
        }
        let mut edges = EDGES.borrow_ref_mut(cs);
        if edges.is_full() {
            let _ = edges.pop_front();
            let dropped = DROPPED.borrow(cs);
            dropped.set(dropped.get().saturating_add(1));
        }
        let _ = edges.push_back((read(pins), TimerClock::new().now_ms()));
        let high_water = HIGH_WATER.borrow(cs);
        high_water.set(high_water.get().max(edges.len() as u32));
    });
}

/// Reads the HW-040 CLK/DT/SW lines and nothing else.
///
/// No debounce and no decoding: `sample()` is a direct, inverted read of the three pins, and the
/// interrupt handler records the same read on every edge.
pub struct PhysicalRotary;

impl PhysicalRotary {
    /// Takes ownership of already-configured `clk`/`dt`/`sw` and contextual-button inputs and
    /// starts capturing edges.
    ///
    /// The pins are active-low inputs, matching [`crate::profile::physical_st7789::ROTARY`].
    #[must_use]
    pub fn new(mut clk: In, mut dt: In, mut sw: In, mut keys: [In; 3]) -> Self {
        critical_section::with(|cs| {
            listen(&mut clk);
            listen(&mut dt);
            listen(&mut sw);
            for key in &mut keys {
                listen(key);
            }
            PINS.borrow_ref_mut(cs).replace((clk, dt, sw, keys));
        });
        // SAFETY: the handler only touches state behind critical-section mutexes, which are set up.
        unsafe { pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0) };
        Self
    }
}

impl InputSource for PhysicalRotary {
    fn sample(&mut self) -> InputLevels {
        critical_section::with(|cs| {
            PINS.borrow_ref_mut(cs)
                .as_mut()
                .map_or(InputLevels::default(), read)
        })
    }

    fn dropped_edges(&self) -> u32 {
        critical_section::with(|cs| DROPPED.borrow(cs).get())
    }

    fn edge_high_water(&self) -> u32 {
        critical_section::with(|cs| HIGH_WATER.borrow(cs).get())
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
