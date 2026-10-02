//! CPU and RAM monitoring: OS probes supply raw counters, the math here turns them into the
//! percentages and the high-load cue the device shows.

/// Cumulative CPU time counters since boot, in any unit, as long as both samples use the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    /// Time spent idle.
    pub idle: u64,
    /// Total time (idle included).
    pub total: u64,
}

/// Whole-machine busy percentage between two samples, rounded to the nearest point. `None` when
/// no time passed or a counter went backwards (a reset), so a bad sample is unknown, never 0 %.
#[must_use]
pub fn cpu_percent(prev: CpuTimes, now: CpuTimes) -> Option<u8> {
    let total = now.total.checked_sub(prev.total)?;
    let idle = now.idle.checked_sub(prev.idle)?;
    if total == 0 || idle > total {
        return None;
    }
    let busy = total - idle;
    Some(((busy * 100 + total / 2) / total) as u8)
}

/// Raw OS readings. Each returns `None` when the OS call fails.
pub trait SystemProbe: Send {
    fn cpu_times(&self) -> Option<CpuTimes>;
    /// Physical memory in use, 0..=100.
    fn ram_percent(&self) -> Option<u8>;
}

/// A probe for targets without an implementation: everything unknown.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSystemProbe;

impl SystemProbe for NoSystemProbe {
    fn cpu_times(&self) -> Option<CpuTimes> {
        None
    }
    fn ram_percent(&self) -> Option<u8> {
        None
    }
}

/// CPU at or above this, for [`HIGH_LOAD_SAMPLES`] samples in a row, turns the load cue on.
pub const HIGH_LOAD_ON_PERCENT: u8 = 85;
/// CPU below this turns it off again. The gap keeps the cue from flickering around one value.
pub const HIGH_LOAD_OFF_PERCENT: u8 = 70;
/// Consecutive high samples (one per second) before the cue shows.
pub const HIGH_LOAD_SAMPLES: u8 = 3;

/// One monitoring sample, as shown to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SystemSample {
    pub cpu_percent: Option<u8>,
    pub ram_percent: Option<u8>,
    pub high_load: bool,
}

/// Turns successive probe readings into [`SystemSample`]s. Call [`Self::sample`] about once a
/// second; the first call only primes the CPU counters.
pub struct SystemMonitor<P> {
    probe: P,
    prev: Option<CpuTimes>,
    high_streak: u8,
    high_load: bool,
}

impl<P: SystemProbe> SystemMonitor<P> {
    pub const fn new(probe: P) -> Self {
        Self {
            probe,
            prev: None,
            high_streak: 0,
            high_load: false,
        }
    }

    pub fn sample(&mut self) -> SystemSample {
        let now = self.probe.cpu_times();
        let cpu = match (self.prev, now) {
            (Some(prev), Some(now)) => cpu_percent(prev, now),
            _ => None,
        };
        self.prev = now;
        self.update_high_load(cpu);
        SystemSample {
            cpu_percent: cpu,
            ram_percent: self.probe.ram_percent(),
            high_load: self.high_load,
        }
    }

    /// Unknown CPU clears the cue: Kivori never keeps showing load it can no longer observe.
    fn update_high_load(&mut self, cpu: Option<u8>) {
        match cpu {
            Some(cpu) if cpu >= HIGH_LOAD_ON_PERCENT => {
                self.high_streak = self.high_streak.saturating_add(1);
                if self.high_streak >= HIGH_LOAD_SAMPLES {
                    self.high_load = true;
                }
            }
            Some(cpu) if cpu >= HIGH_LOAD_OFF_PERCENT => self.high_streak = 0,
            _ => {
                self.high_streak = 0;
                self.high_load = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn cpu_percent_is_busy_over_total_between_samples() {
        let a = CpuTimes {
            idle: 100,
            total: 200,
        };
        assert_eq!(
            cpu_percent(
                a,
                CpuTimes {
                    idle: 175,
                    total: 300
                }
            ),
            Some(25)
        );
        assert_eq!(cpu_percent(a, a), None, "no time passed");
        assert_eq!(
            cpu_percent(
                a,
                CpuTimes {
                    idle: 50,
                    total: 300
                }
            ),
            None,
            "counter reset"
        );
        assert_eq!(
            cpu_percent(
                a,
                CpuTimes {
                    idle: 100,
                    total: 203
                }
            ),
            Some(100)
        );
        assert_eq!(
            cpu_percent(
                a,
                CpuTimes {
                    idle: 102,
                    total: 203
                }
            ),
            Some(33),
            "rounds to nearest"
        );
    }

    /// Replays a fixed CPU percentage per sample (out of 100 ticks) and 40 % RAM.
    struct Script(RefCell<(u64, u64, Vec<Option<u8>>)>);

    impl SystemProbe for Script {
        fn cpu_times(&self) -> Option<CpuTimes> {
            let mut s = self.0.borrow_mut();
            let busy = s.2.remove(0)?;
            s.0 += 100 - u64::from(busy);
            s.1 += 100;
            Some(CpuTimes {
                idle: s.0,
                total: s.1,
            })
        }
        fn ram_percent(&self) -> Option<u8> {
            Some(40)
        }
    }

    fn run(cpu: &[Option<u8>]) -> Vec<(Option<u8>, bool)> {
        let mut script = vec![Some(0)];
        script.extend_from_slice(cpu);
        let mut monitor = SystemMonitor::new(Script(RefCell::new((0, 0, script))));
        assert_eq!(monitor.sample().cpu_percent, None, "first sample primes");
        cpu.iter()
            .map(|_| {
                let s = monitor.sample();
                (s.cpu_percent, s.high_load)
            })
            .collect()
    }

    #[test]
    fn high_load_needs_a_sustained_streak_and_clears_with_hysteresis() {
        let flags: Vec<bool> = run(&[
            Some(90),
            Some(95),
            Some(80), // streak broken, still not on
            Some(90),
            Some(90),
            Some(90), // third in a row: on
            Some(75), // inside the gap: stays on
            Some(60), // below the off threshold: off
        ])
        .into_iter()
        .map(|(_, high)| high)
        .collect();
        assert_eq!(
            flags,
            [false, false, false, false, false, true, true, false]
        );
    }

    #[test]
    fn an_unknown_sample_clears_the_cue() {
        let samples = run(&[Some(99), Some(99), Some(99), None]);
        assert_eq!(samples[2], (Some(99), true));
        assert_eq!(samples[3], (None, false));
    }
}
