//! CPU and RAM from `/proc/stat` and `/proc/meminfo`, and the local clock from `localtime_r`.
//!
//! RAM is "used" in the sense of `free`/GNOME System Monitor: `MemTotal - MemAvailable`. A kernel
//! without `MemAvailable` (before 3.14) gives unknown, never a guess from `MemFree`, which counts
//! reclaimable page cache as used.

use kivori_model::desk::ClockTime;

use crate::platform::system::{CpuTimes, SystemProbe};

#[derive(Debug, Clone, Copy, Default)]
pub struct LinuxSystemProbe;

impl SystemProbe for LinuxSystemProbe {
    fn cpu_times(&self) -> Option<CpuTimes> {
        parse_cpu_times(&std::fs::read_to_string("/proc/stat").ok()?)
    }

    fn ram_percent(&self) -> Option<u8> {
        parse_ram_percent(&std::fs::read_to_string("/proc/meminfo").ok()?)
    }
}

/// The aggregate `cpu` line of `/proc/stat`: `user nice system idle iowait irq softirq steal ...`
/// in USER_HZ ticks. `guest` and `guest_nice` are already inside `user` and `nice`, so they are not
/// added again. Idle time is `idle + iowait`. A line with fewer than the first four counters, or a
/// non-number, is unknown.
fn parse_cpu_times(stat: &str) -> Option<CpuTimes> {
    let line = stat.lines().find(|line| {
        let mut words = line.split_whitespace();
        words.next() == Some("cpu")
    })?;
    let ticks: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if ticks.len() < 4 {
        return None;
    }
    // [user, nice, system, idle, iowait, irq, softirq, steal]; kernels before 2.6.0/2.6.11 omit
    // the tail, which then counts as zero.
    let at = |index: usize| ticks.get(index).copied().unwrap_or(0);
    Some(CpuTimes {
        idle: at(3) + at(4),
        total: ticks.iter().sum(),
    })
}

/// Memory in use as a percentage of `MemTotal`, rounded, from `/proc/meminfo` text.
fn parse_ram_percent(meminfo: &str) -> Option<u8> {
    let field = |name: &str| -> Option<u64> {
        meminfo.lines().find_map(|line| {
            let rest = line.strip_prefix(name)?.strip_prefix(':')?;
            rest.split_whitespace().next()?.parse().ok()
        })
    };
    let total = field("MemTotal")?;
    let available = field("MemAvailable")?;
    if total == 0 {
        return None;
    }
    let used = total.saturating_sub(available);
    let percent = (u128::from(used) * 100 + u128::from(total) / 2) / u128::from(total);
    Some(percent.min(100) as u8)
}

/// Local wall-clock time of day, via `time` + `localtime_r` (honours the user's time zone).
pub fn local_time() -> Option<ClockTime> {
    // SAFETY: `time(NULL)` only returns the current time.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now == -1 {
        return None;
    }
    // SAFETY: all-zero is a valid `tm` (integers plus a null `tm_zone` pointer).
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the call; `localtime_r` is the thread-safe variant.
    if unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
        return None;
    }
    Some(ClockTime {
        hour: u8::try_from(tm.tm_hour).ok()?,
        minute: u8::try_from(tm.tm_min).ok()?,
        // `tm_sec` can be 60 on a leap second; the device clock is 0..=59.
        second: u8::try_from(tm.tm_sec).ok()?.min(59),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  4705 150 1120 16250856 2000 10 30 5 99 1\n\
        cpu0 1393 76 459 8123456 1000 5 15 2 0 0\n\
        cpu1 3312 74 661 8127400 1000 5 15 3 0 0\n\
        intr 1462898\nctxt 115315\n";

    #[test]
    fn cpu_total_is_the_eight_counters_and_idle_includes_iowait() {
        // user+nice+system+idle+iowait+irq+softirq+steal; guest (99, 1) is already in user/nice.
        let t = parse_cpu_times(STAT).unwrap();
        assert_eq!(t.idle, 16_250_856 + 2_000);
        assert_eq!(t.total, 4705 + 150 + 1120 + 16_250_856 + 2000 + 10 + 30 + 5);
    }

    #[test]
    fn only_the_aggregate_line_counts() {
        // A file that starts with a per-CPU line must not be read as the aggregate.
        assert_eq!(parse_cpu_times("cpu0 1 2 3 4\nintr 5\n"), None);
    }

    #[test]
    fn an_old_kernel_with_four_counters_still_parses() {
        let t = parse_cpu_times("cpu  10 0 5 85\n").unwrap();
        assert_eq!(
            t,
            CpuTimes {
                idle: 85,
                total: 100
            }
        );
    }

    #[test]
    fn malformed_stat_is_unknown_not_zero() {
        assert_eq!(parse_cpu_times(""), None);
        assert_eq!(parse_cpu_times("cpu  1 2 3\n"), None, "too few counters");
        assert_eq!(parse_cpu_times("cpu  1 2 x 4\n"), None, "not a number");
        assert_eq!(parse_cpu_times("cpu  1 2 3 -4\n"), None, "negative");
    }

    #[test]
    fn two_samples_give_a_percentage() {
        use crate::platform::system::cpu_percent;
        let a = parse_cpu_times("cpu  100 0 100 800 0 0 0 0\n").unwrap();
        let b = parse_cpu_times("cpu  200 0 150 850 0 0 0 0\n").unwrap();
        // 150 busy over 200 total ticks.
        assert_eq!(cpu_percent(a, b), Some(75));
    }

    const MEMINFO: &str = "MemTotal:       16384000 kB\n\
        MemFree:         1000000 kB\n\
        MemAvailable:    4096000 kB\n\
        Buffers:          500000 kB\n";

    #[test]
    fn ram_is_total_minus_available_over_total() {
        // (16384000 - 4096000) / 16384000 = 75 %.
        assert_eq!(parse_ram_percent(MEMINFO), Some(75));
    }

    #[test]
    fn ram_rounds_and_clamps() {
        assert_eq!(
            parse_ram_percent("MemTotal: 1000 kB\nMemAvailable: 335 kB\n"),
            Some(67),
            "66.5 rounds up"
        );
        assert_eq!(
            parse_ram_percent("MemTotal: 1000 kB\nMemAvailable: 5000 kB\n"),
            Some(0),
            "available above total saturates"
        );
    }

    #[test]
    fn ram_without_mem_available_is_unknown() {
        // MemFree would count the page cache as used: not a substitute.
        assert_eq!(
            parse_ram_percent("MemTotal: 1000 kB\nMemFree: 100 kB\n"),
            None
        );
        assert_eq!(parse_ram_percent(""), None);
        assert_eq!(
            parse_ram_percent("MemTotal: 0 kB\nMemAvailable: 0 kB\n"),
            None
        );
        // `MemTotalX:` is not `MemTotal:`.
        assert_eq!(
            parse_ram_percent("MemTotalX: 5 kB\nMemAvailable: 1 kB\n"),
            None
        );
    }

    #[test]
    fn the_live_proc_files_parse() {
        let probe = LinuxSystemProbe;
        assert!(probe.cpu_times().is_some());
        assert!(probe.ram_percent().is_some_and(|p| p <= 100));
    }

    #[test]
    fn the_local_clock_is_a_valid_time_of_day() {
        let t = local_time().expect("localtime_r");
        assert!(t.hour < 24 && t.minute < 60 && t.second < 60);
    }
}
