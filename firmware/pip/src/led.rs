//! The RGB LED engine (capability `RGB_LED_V1`): hardware-neutral and pure.
//!
//! The desktop sends one [`LedSettings`] on every session start and whenever it changes; the device
//! keeps them until it restarts and runs everything else itself, so the LED reacts at once: the
//! breathing and colour wheel, the buddy's mood, and in Status mode the muted microphone, a lost
//! host and a press. Before any settings arrive (no app yet) the LED stays dark.
//!
//! [`LedEngine::frame`] is a function of the engine's state and the time only, so the physical
//! board can compute it from a timer interrupt (the run loop's passes are too slow for a smooth
//! fade) and host tests can sample it at any instant. Integer math only: no FPU, no libm.

use kivori_model::led::{LedMode, LedSpeed};
use kivori_model::MascotExpression;
use kivori_protocol::LedSettings;

/// How long the white flash of a press lasts (Status mode).
pub const FLASH_MS: u32 = 120;

/// Full level of a colour (the `level` axis runs `0..=255`).
const FULL: u8 = 255;
/// Breathe never goes fully dark: about 10 % of full.
const BREATHE_FLOOR: u8 = 26;
/// Mood's soft breathe runs between about 50 % and full, over 5 s.
const MOOD_FLOOR: u8 = 128;
const MOOD_PERIOD_MS: u32 = 5_000;
/// Sleeping breathes slower and dimmer: 8 s between about 20 % and 40 %.
const SLEEP_FLOOR: u8 = 51;
const SLEEP_CEIL: u8 = 102;
const SLEEP_PERIOD_MS: u32 = 8_000;
/// Offline and booting: a dim steady glow, about 15 %.
const DIM: u8 = 38;
/// No host: a dim amber pulse, between about 8 % and 40 % over 3 s.
const AMBER: [u8; 3] = [255, 160, 0];
const AMBER_FLOOR: u8 = 20;
const AMBER_CEIL: u8 = 100;
const AMBER_PERIOD_MS: u32 = 3_000;
/// The microphone is muted.
const RED: [u8; 3] = [255, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// How the LED moves in Mood mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Motion {
    /// The soft 5 s breathe.
    Soft,
    /// Sleeping: slower and dimmer.
    Sleep,
    /// Dim and steady.
    Dim,
}

/// The colour and motion of one expression. Exhaustive on purpose: a new expression does not
/// compile until it has a colour here.
const fn mood(expression: MascotExpression) -> ([u8; 3], Motion) {
    use MascotExpression as E;
    match expression {
        E::Booting => ([150, 160, 200], Motion::Dim),
        E::Idle => ([46, 196, 182], Motion::Soft),
        E::Happy => ([255, 200, 40], Motion::Soft),
        E::Busy => ([255, 80, 10], Motion::Soft),
        E::Sleeping => ([20, 30, 160], Motion::Sleep),
        E::Offline => ([110, 120, 140], Motion::Dim),
        E::Delighted => ([255, 240, 120], Motion::Soft),
        E::Affectionate => ([255, 100, 170], Motion::Soft),
        E::Laughing => ([255, 60, 130], Motion::Soft),
        E::Surprised => ([230, 240, 255], Motion::Soft),
        E::Reassuring => ([90, 200, 150], Motion::Soft),
        E::Error => ([255, 20, 20], Motion::Soft),
        E::Strained => ([255, 140, 0], Motion::Soft),
        E::Muted => ([180, 60, 70], Motion::Soft),
        E::Listening => ([60, 220, 90], Motion::Soft),
        E::Attentive => ([50, 130, 255], Motion::Soft),
        E::MicMuted => ([230, 30, 70], Motion::Soft),
    }
}

/// Duty for a perceived level: LEDs look far brighter at a low duty than the duty says, so the
/// output is the square of the level (rounded up, so any non-zero level still lights).
const GAMMA: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 1;
    while i < 256 {
        table[i] = (i * i).div_ceil(255) as u8;
        i += 1;
    }
    table
};

const fn breathe_period(speed: LedSpeed) -> u32 {
    match speed {
        LedSpeed::Slow => 6_000,
        LedSpeed::Medium => 4_000,
        LedSpeed::Fast => 2_000,
    }
}

const fn cycle_period(speed: LedSpeed) -> u32 {
    match speed {
        LedSpeed::Slow => 20_000,
        LedSpeed::Medium => 10_000,
        LedSpeed::Fast => 5_000,
    }
}

/// A smooth triangle through `0..=1024` and back over `period_ms`, starting low.
const fn wave(now_ms: u32, period_ms: u32) -> u32 {
    let phase = now_ms % period_ms;
    let half = period_ms / 2;
    let t = if phase < half {
        phase * 1024 / half
    } else {
        (period_ms - phase) * 1024 / (period_ms - half)
    };
    // Smoothstep, so the fade lingers at both ends like a breath.
    (t * t * (3 * 1024 - 2 * t)) >> 20
}

/// The level of a breathe between `floor` and `ceil`.
const fn breathe(now_ms: u32, period_ms: u32, floor: u8, ceil: u8) -> u8 {
    (floor as u32 + (ceil - floor) as u32 * wave(now_ms, period_ms) / 1024) as u8
}

/// The fully saturated colour `period_ms` into a turn of the wheel, red to red.
const fn wheel(now_ms: u32, period_ms: u32) -> [u8; 3] {
    let pos = (now_ms % period_ms) * 1536 / period_ms;
    let (segment, f) = (pos / 256, (pos % 256) as u8);
    match segment {
        0 => [255, f, 0],
        1 => [255 - f, 255, 0],
        2 => [0, 255, f],
        3 => [0, 255 - f, 255],
        4 => [f, 0, 255],
        _ => [255, 0, 255 - f],
    }
}

/// Scales `rgb` by `level` (`0..=255`) and `brightness` (percent), then applies the gamma.
fn shape(rgb: [u8; 3], level: u8, brightness: u8) -> [u8; 3] {
    let scale = u32::from(level) * u32::from(brightness.min(100));
    // Divisor 255 * 100, rounded to nearest.
    rgb.map(|c| GAMMA[((u32::from(c) * scale + 12_750) / 25_500) as usize])
}

/// The LED's state: the desktop's settings plus the device's own inputs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LedEngine {
    mode: LedMode,
    color: [u8; 3],
    brightness: u8,
    speed: LedSpeed,
    expression: MascotExpression,
    mic_muted: bool,
    connected: bool,
    flash_at: Option<u32>,
    revision: u32,
}

impl LedEngine {
    /// Off, with no host: what a fresh boot shows.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: LedMode::Off,
            color: [0; 3],
            brightness: 0,
            speed: LedSpeed::Medium,
            expression: MascotExpression::Booting,
            mic_muted: false,
            connected: false,
            flash_at: None,
            revision: 0,
        }
    }

    /// Adopts the desktop's settings (they stay until restart).
    pub fn configure(&mut self, settings: &LedSettings) {
        self.mode = settings.mode;
        self.color = settings.color;
        self.brightness = settings.brightness.min(100);
        self.speed = settings.speed;
        self.revision = self.revision.wrapping_add(1);
    }

    /// The device's own state: the face on screen, whether every microphone is muted, and whether
    /// a host session is live.
    pub fn set_inputs(&mut self, expression: MascotExpression, mic_muted: bool, connected: bool) {
        if (self.expression, self.mic_muted, self.connected) != (expression, mic_muted, connected) {
            self.expression = expression;
            self.mic_muted = mic_muted;
            self.connected = connected;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// A knob or key press at `now_ms`: Status mode flashes white for [`FLASH_MS`].
    pub fn note_input(&mut self, now_ms: u32) {
        self.flash_at = Some(now_ms);
        self.revision = self.revision.wrapping_add(1);
    }

    /// The mode the desktop last set (`Off` until it sets one).
    #[must_use]
    pub const fn mode(&self) -> LedMode {
        self.mode
    }

    /// Counts every change, so a caller that mirrors the engine knows when to copy it again.
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }

    /// The duty to drive, red, green and blue (`0..=255`, common cathode), at `now_ms`.
    #[must_use]
    pub fn frame(&self, now_ms: u32) -> [u8; 3] {
        let (rgb, level) = match self.mode {
            LedMode::Off => return [0; 3],
            LedMode::Solid => (self.color, FULL),
            LedMode::Breathe => (
                self.color,
                breathe(now_ms, breathe_period(self.speed), BREATHE_FLOOR, FULL),
            ),
            LedMode::Cycle => (wheel(now_ms, cycle_period(self.speed)), FULL),
            LedMode::Status => self.status(now_ms),
            LedMode::Mood => self.mood(now_ms),
        };
        shape(rgb, level, self.brightness)
    }

    /// Status: the base colour, overridden by no host (amber pulse), then a muted microphone (red),
    /// with a press flashing white on top of either.
    fn status(&self, now_ms: u32) -> ([u8; 3], u8) {
        if self
            .flash_at
            .is_some_and(|at| now_ms.wrapping_sub(at) < FLASH_MS)
        {
            (WHITE, FULL)
        } else if !self.connected {
            (
                AMBER,
                breathe(now_ms, AMBER_PERIOD_MS, AMBER_FLOOR, AMBER_CEIL),
            )
        } else if self.mic_muted {
            (RED, FULL)
        } else {
            (self.color, FULL)
        }
    }

    /// Mood: the colour of the face on screen, moving the way that face does.
    fn mood(&self, now_ms: u32) -> ([u8; 3], u8) {
        let (rgb, motion) = mood(self.expression);
        let level = match motion {
            Motion::Soft => breathe(now_ms, MOOD_PERIOD_MS, MOOD_FLOOR, FULL),
            Motion::Sleep => breathe(now_ms, SLEEP_PERIOD_MS, SLEEP_FLOOR, SLEEP_CEIL),
            Motion::Dim => DIM,
        };
        (rgb, level)
    }
}

impl Default for LedEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MascotExpression as E;

    const ALL: [MascotExpression; 17] = [
        E::Booting,
        E::Idle,
        E::Happy,
        E::Busy,
        E::Sleeping,
        E::Offline,
        E::Delighted,
        E::Affectionate,
        E::Laughing,
        E::Surprised,
        E::Reassuring,
        E::Error,
        E::Strained,
        E::Muted,
        E::Listening,
        E::Attentive,
        E::MicMuted,
    ];

    fn engine(mode: LedMode, color: [u8; 3], brightness: u8, speed: LedSpeed) -> LedEngine {
        let mut led = LedEngine::new();
        led.configure(&LedSettings {
            session: 1,
            mode,
            color,
            brightness,
            speed,
        });
        led
    }

    /// The (min, max) of one channel over `[0, span_ms)`, sampled every 10 ms.
    fn range(led: &LedEngine, channel: usize, span_ms: u32) -> (u8, u8) {
        (0..span_ms / 10)
            .map(|i| led.frame(i * 10)[channel])
            .fold((255, 0), |(lo, hi), v| (lo.min(v), hi.max(v)))
    }

    #[test]
    fn the_led_is_dark_until_the_app_sends_settings() {
        let mut led = LedEngine::new();
        for now in [0, 500, 10_000] {
            assert_eq!(led.frame(now), [0; 3]);
        }
        // Inputs alone light nothing either.
        led.set_inputs(E::Happy, true, true);
        led.note_input(10);
        assert_eq!(led.frame(20), [0; 3]);
    }

    #[test]
    fn off_is_dark_whatever_else_is_set() {
        let mut led = engine(LedMode::Off, [255; 3], 100, LedSpeed::Fast);
        led.note_input(0);
        led.set_inputs(E::Error, true, false);
        assert_eq!(led.frame(30), [0; 3]);
    }

    #[test]
    fn solid_is_the_colour_steady() {
        let led = engine(LedMode::Solid, [255, 128, 0], 100, LedSpeed::Medium);
        let first = led.frame(0);
        assert_eq!(first[0], 255);
        assert_eq!(first[2], 0);
        assert_eq!(first[1], GAMMA[128]);
        for now in [1, 999, 12_345, 4_000_000] {
            assert_eq!(led.frame(now), first);
        }
    }

    #[test]
    fn breathe_period_follows_the_speed_and_never_goes_dark() {
        for (speed, period) in [
            (LedSpeed::Slow, 6_000),
            (LedSpeed::Medium, 4_000),
            (LedSpeed::Fast, 2_000),
        ] {
            let led = engine(LedMode::Breathe, [255, 255, 255], 100, speed);
            // Periodic.
            for now in [0, 137, 1_001] {
                assert_eq!(led.frame(now), led.frame(now + period), "{speed:?}");
            }
            // Low at the start, full at the half period, and one rise and fall in between.
            let low = led.frame(0)[0];
            let high = led.frame(period / 2)[0];
            assert_eq!(high, 255, "{speed:?}");
            assert!(low > 0 && low < 20, "{speed:?} floor {low}");
            let (min, max) = range(&led, 0, period);
            assert_eq!((min, max), (low, 255), "{speed:?}");
            // One smooth rise to the peak at half the period, then one fall.
            let at = |i: u32| led.frame(i * 10)[0];
            let half = period / 20;
            assert!((0..half).all(|i| at(i) <= at(i + 1)), "rises: {speed:?}");
            assert!(
                (half..period / 10).all(|i| at(i) >= at(i + 1)),
                "falls: {speed:?}"
            );
        }
    }

    #[test]
    fn cycle_walks_the_whole_wheel_once_per_period() {
        for (speed, period) in [
            (LedSpeed::Slow, 20_000),
            (LedSpeed::Medium, 10_000),
            (LedSpeed::Fast, 5_000),
        ] {
            // The chosen colour is not used.
            let led = engine(LedMode::Cycle, [1, 2, 3], 100, speed);
            assert_eq!(led.frame(0), led.frame(period), "{speed:?}");
            // The six corners of the wheel, in order, at multiples of period / 6.
            let corners = [
                [255, 0, 0],
                [255, 255, 0],
                [0, 255, 0],
                [0, 255, 255],
                [0, 0, 255],
                [255, 0, 255],
            ];
            for (i, corner) in corners.into_iter().enumerate() {
                let at = period / 6 * i as u32;
                assert_eq!(
                    led.frame(at),
                    corner.map(|c| GAMMA[c]),
                    "{speed:?} corner {i}"
                );
            }
            // Every channel reaches full and zero within a turn.
            for channel in 0..3 {
                assert_eq!(range(&led, channel, period), (0, 255), "{speed:?}");
            }
        }
    }

    #[test]
    fn brightness_scales_the_output_from_dark_to_full() {
        let at = |brightness| {
            engine(
                LedMode::Solid,
                [255, 255, 255],
                brightness,
                LedSpeed::Medium,
            )
            .frame(0)
        };
        assert_eq!(at(0), [0; 3]);
        assert_eq!(at(100), [255; 3]);
        // Above 100 counts as 100.
        assert_eq!(at(250), [255; 3]);
        let mut last = 0;
        for b in 0..=100 {
            let v = at(b)[0];
            assert!(v >= last, "brightness {b}");
            last = v;
        }
        assert!(at(1)[0] >= 1, "the dimmest setting still lights");
        assert!(
            at(50)[0] < 128,
            "gamma keeps half brightness well under half duty"
        );
        // Mid-grey in the output and the 1 % step are usable: distinct, non-zero levels.
        assert!(at(1)[0] < at(10)[0] && at(10)[0] < at(50)[0]);
    }

    #[test]
    fn gamma_is_monotonic_and_pins_the_ends() {
        assert_eq!(GAMMA[0], 0);
        assert_eq!(GAMMA[255], 255);
        assert!(GAMMA.windows(2).all(|w| w[0] <= w[1]));
        assert!(GAMMA[1..].iter().all(|&v| v >= 1));
        assert!(GAMMA[128] < 128);
    }

    #[test]
    fn status_shows_the_base_colour_when_all_is_well() {
        let mut led = engine(LedMode::Status, [10, 200, 30], 100, LedSpeed::Medium);
        led.set_inputs(E::Idle, false, true);
        let expect = [10, 200, 30].map(|c| GAMMA[c]);
        for now in [0, 777, 90_000] {
            assert_eq!(led.frame(now), expect);
        }
    }

    #[test]
    fn status_pulses_amber_without_a_host_and_that_outranks_a_muted_mic() {
        let mut led = engine(LedMode::Status, [10, 200, 30], 100, LedSpeed::Medium);
        led.set_inputs(E::Offline, true, false);
        let (lo, hi) = range(&led, 0, AMBER_PERIOD_MS);
        assert!(lo < hi, "it pulses");
        assert!(hi < 128, "and stays dim: {hi}");
        for now in (0..AMBER_PERIOD_MS).step_by(10) {
            let [r, g, b] = led.frame(now);
            assert!(r > g && g > 0 && b == 0, "amber at {now}: {r},{g},{b}");
        }
        // The pulse repeats every 3 s.
        assert_eq!(led.frame(100), led.frame(100 + AMBER_PERIOD_MS));
    }

    #[test]
    fn status_shows_solid_red_when_the_mic_is_muted() {
        let mut led = engine(LedMode::Status, [10, 200, 30], 100, LedSpeed::Medium);
        led.set_inputs(E::MicMuted, true, true);
        for now in [0, 1_500, 9_999] {
            assert_eq!(led.frame(now), [255, 0, 0]);
        }
        led.set_inputs(E::Idle, false, true);
        assert_eq!(led.frame(0), [10, 200, 30].map(|c| GAMMA[c]));
    }

    #[test]
    fn a_press_flashes_white_for_120_ms_over_everything_in_status() {
        for (muted, connected) in [(false, true), (true, true), (false, false), (true, false)] {
            let mut led = engine(LedMode::Status, [10, 200, 30], 100, LedSpeed::Medium);
            led.set_inputs(E::Idle, muted, connected);
            let before = led.frame(1_000);
            led.note_input(1_000);
            for at in [1_000, 1_060, 1_119] {
                assert_eq!(
                    led.frame(at),
                    [255; 3],
                    "mic {muted} host {connected} at {at}"
                );
            }
            assert_ne!(led.frame(1_120), [255; 3], "over at 120 ms");
            if connected {
                assert_eq!(led.frame(1_120), before);
            }
        }
    }

    #[test]
    fn a_press_does_nothing_outside_status() {
        for mode in [
            LedMode::Solid,
            LedMode::Breathe,
            LedMode::Cycle,
            LedMode::Mood,
        ] {
            let mut led = engine(mode, [10, 20, 30], 100, LedSpeed::Medium);
            led.set_inputs(E::Idle, false, true);
            let plain = led.frame(1_050);
            led.note_input(1_000);
            assert_eq!(led.frame(1_050), plain, "{mode:?}");
        }
    }

    #[test]
    fn the_flash_is_brightness_scaled_and_survives_a_clock_wrap() {
        let mut led = engine(LedMode::Status, [10, 200, 30], 50, LedSpeed::Medium);
        led.set_inputs(E::Idle, false, true);
        let half = GAMMA[((255u32 * 255 * 50 + 12_750) / 25_500) as usize];
        led.note_input(u32::MAX - 20);
        assert_eq!(led.frame(u32::MAX - 20), [half; 3]);
        assert_eq!(led.frame(50), [half; 3], "70 ms later, across the wrap");
        assert_ne!(led.frame(200), [half; 3]);
    }

    #[test]
    fn every_expression_has_a_lit_colour_and_the_families_stay_apart() {
        for expression in ALL {
            let mut led = engine(LedMode::Mood, [0; 3], 100, LedSpeed::Medium);
            led.set_inputs(expression, false, true);
            let lit = (0..800).any(|i| led.frame(i * 10).iter().any(|&v| v > 0));
            assert!(lit, "{expression:?} is dark");
        }
        // No two expressions share a colour.
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(mood(*a).0, mood(*b).0, "{a:?} and {b:?}");
            }
        }
        // The brief's families.
        let c = |e| mood(e).0;
        assert!(c(E::Error)[0] > 200 && c(E::Error)[1] < 60 && c(E::Error)[2] < 60);
        assert!(c(E::Muted)[0] > c(E::Muted)[1] + 80);
        assert!(c(E::Sleeping)[2] > 100 && c(E::Sleeping)[0] < 60);
        assert!(c(E::Happy)[0] > 200 && c(E::Happy)[1] > 150 && c(E::Happy)[2] < 100);
        assert!(c(E::Delighted)[0] > 200 && c(E::Delighted)[1] > 150);
        assert!(c(E::Listening)[1] > 180 && c(E::Listening)[0] < 100);
        assert!(c(E::Busy)[0] > 200 && (50..130).contains(&c(E::Busy)[1]));
        assert!(c(E::Idle)[1] > 150 && c(E::Idle)[2] > 150 && c(E::Idle)[0] < 80);
        assert!(c(E::Surprised).iter().all(|&v| v > 200));
        assert!(c(E::Affectionate)[0] > 200 && c(E::Affectionate)[2] > 120);
        assert!(c(E::Laughing)[0] > 200 && c(E::Laughing)[2] > 100);
        assert!(c(E::Strained)[0] > 200 && (100..180).contains(&c(E::Strained)[1]));
    }

    #[test]
    fn mood_breathes_softly_over_five_seconds() {
        let mut led = engine(LedMode::Mood, [0; 3], 100, LedSpeed::Fast);
        led.set_inputs(E::Idle, false, true);
        // The speed setting does not change mood's period.
        assert_eq!(led.frame(300), led.frame(300 + MOOD_PERIOD_MS));
        let (lo, hi) = range(&led, 1, MOOD_PERIOD_MS);
        assert_eq!(hi, GAMMA[196]);
        assert!(lo > 0 && lo < hi, "soft: {lo}..{hi}");
        assert!(lo >= hi / 8, "soft, not a blink: {lo}..{hi}");
    }

    #[test]
    fn sleeping_breathes_slower_and_dimmer_and_offline_is_dim_and_steady() {
        let mut led = engine(LedMode::Mood, [0; 3], 100, LedSpeed::Medium);
        led.set_inputs(E::Sleeping, false, true);
        assert_eq!(led.frame(500), led.frame(500 + SLEEP_PERIOD_MS));
        assert_ne!(led.frame(500), led.frame(500 + MOOD_PERIOD_MS));
        let (_, sleeping_peak) = range(&led, 2, SLEEP_PERIOD_MS);
        led.set_inputs(E::Idle, false, true);
        let (_, idle_peak) = range(&led, 2, MOOD_PERIOD_MS);
        // 40 % level at its brightest, against the full level of an awake face.
        assert!(sleeping_peak < idle_peak);
        assert_eq!(sleeping_peak, shape([0, 0, 160], SLEEP_CEIL, 100)[2]);
        // The sleeping breath is the slower one: less change over the same short window.
        let travel = |led: &LedEngine, ch: usize| {
            (1..100)
                .map(|i| u32::from(led.frame(i * 10)[ch].abs_diff(led.frame(i * 10 - 10)[ch])))
                .sum::<u32>()
        };
        let mut sleeping = engine(LedMode::Mood, [0; 3], 100, LedSpeed::Medium);
        sleeping.set_inputs(E::Sleeping, false, true);
        let mut awake = sleeping;
        awake.set_inputs(E::Idle, false, true);
        assert!(travel(&sleeping, 2) < travel(&awake, 1));

        for steady in [E::Offline, E::Booting] {
            led.set_inputs(steady, false, false);
            let first = led.frame(0);
            for now in [1, 2_500, 77_777] {
                assert_eq!(led.frame(now), first, "{steady:?}");
            }
            let awake_value = u32::from(mood(steady).0.into_iter().max().unwrap());
            assert!(u32::from(first.into_iter().max().unwrap()) < awake_value / 2);
        }
    }

    #[test]
    fn mood_ignores_the_mic_and_the_host_which_the_face_already_shows() {
        let mut led = engine(LedMode::Mood, [0; 3], 100, LedSpeed::Medium);
        led.set_inputs(E::Happy, false, true);
        let calm = led.frame(1_234);
        led.set_inputs(E::Happy, true, false);
        assert_eq!(led.frame(1_234), calm);
        led.note_input(1_200);
        assert_eq!(led.frame(1_234), calm);
    }

    #[test]
    fn settings_persist_and_a_new_session_does_not_reset_them() {
        let mut led = engine(LedMode::Solid, [9, 9, 9], 100, LedSpeed::Medium);
        let before = led.frame(0);
        led.set_inputs(E::Idle, false, false);
        assert_eq!(led.frame(1), before);
    }

    #[test]
    fn the_revision_moves_only_when_something_changed() {
        let mut led = LedEngine::new();
        let r0 = led.revision();
        led.set_inputs(E::Booting, false, false);
        assert_eq!(led.revision(), r0, "same inputs");
        led.set_inputs(E::Idle, false, false);
        assert_ne!(led.revision(), r0);
        let r1 = led.revision();
        led.note_input(5);
        assert_ne!(led.revision(), r1);
    }
}
