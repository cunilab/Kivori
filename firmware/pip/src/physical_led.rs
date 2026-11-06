//! The RGB LED of the physical board, driven from a 50 Hz timer interrupt.
//!
//! A run-loop pass takes up to ~140 ms on the RP2040 while a frame is composed and flushed, far too
//! slow for a smooth fade, so the LED does not wait for it. The run loop hands the
//! [`LedEngine`](crate::led::LedEngine) over through [`publish`] whenever its settings or inputs
//! change; the alarm interrupt computes [`LedEngine::frame`](crate::led::LedEngine::frame) for the
//! current time and writes the three PWM duties. The handler is short: a few integer operations,
//! and PWM writes only when the colour changed.

use core::cell::RefCell;

use critical_section::Mutex;
use embedded_hal::pwm::SetDutyCycle;
use rp2040_hal::fugit::ExtU32;
use rp2040_hal::pac::{self, interrupt};
use rp2040_hal::pwm::{Channel, FreeRunning, Pwm4, Pwm5, Slice, A, B};
use rp2040_hal::timer::{Alarm, Alarm0};

use crate::clock::TimerClock;
use crate::led::LedEngine;
use crate::ports::{Clock, LedSink};

/// Refresh period: 50 Hz is smooth for a fade and costs the core almost nothing.
const TICK_US: u32 = 20_000;

/// The RGB LED's three PWM channels (GP9 = slice 4 B, GP10 = slice 5 A, GP11 = slice 5 B).
// Brightness and resistor values are pending a hardware test (issue #179).
pub struct RgbLed {
    /// Red, slice 4 channel B.
    pub red: Channel<Slice<Pwm4, FreeRunning>, B>,
    /// Green, slice 5 channel A.
    pub green: Channel<Slice<Pwm5, FreeRunning>, A>,
    /// Blue, slice 5 channel B.
    pub blue: Channel<Slice<Pwm5, FreeRunning>, B>,
}

impl LedSink for RgbLed {
    /// Sets the three colour levels (0 = off, 255 = fully on; common cathode, so active high).
    fn set_rgb(&mut self, [r, g, b]: [u8; 3]) {
        // A failed duty write leaves the previous colour: harmless on an indicator.
        let _ = self.red.set_duty_cycle_fraction(u16::from(r), 255);
        let _ = self.green.set_duty_cycle_fraction(u16::from(g), 255);
        let _ = self.blue.set_duty_cycle_fraction(u16::from(b), 255);
    }
}

/// What the interrupt owns: its alarm, the LED, the engine copy and the colour on the pins.
struct Driver {
    alarm: Alarm0,
    led: RgbLed,
    engine: LedEngine,
    shown: [u8; 3],
}

static DRIVER: Mutex<RefCell<Option<Driver>>> = Mutex::new(RefCell::new(None));

/// Starts the 50 Hz refresh. The LED must already be dark (the engine starts Off).
pub fn start(mut alarm: Alarm0, led: RgbLed) {
    critical_section::with(|cs| {
        alarm.enable_interrupt();
        let _ = alarm.schedule(TICK_US.micros());
        DRIVER.borrow_ref_mut(cs).replace(Driver {
            alarm,
            led,
            engine: LedEngine::new(),
            shown: [0; 3],
        });
    });
    // SAFETY: the handler only touches state behind a critical-section mutex, which is set up.
    unsafe { pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0) };
}

/// Gives the interrupt the engine's current state (the run loop's [`crate::runtime::LedPublish`]).
pub fn publish(engine: &LedEngine) {
    critical_section::with(|cs| {
        if let Some(driver) = DRIVER.borrow_ref_mut(cs).as_mut() {
            driver.engine = *engine;
        }
    });
}

#[interrupt]
fn TIMER_IRQ_0() {
    critical_section::with(|cs| {
        let mut driver = DRIVER.borrow_ref_mut(cs);
        let Some(driver) = driver.as_mut() else {
            return;
        };
        driver.alarm.clear_interrupt();
        let _ = driver.alarm.schedule(TICK_US.micros());
        let rgb = driver.engine.frame(TimerClock::new().now_ms());
        if rgb != driver.shown {
            driver.shown = rgb;
            driver.led.set_rgb(rgb);
        }
    });
}
