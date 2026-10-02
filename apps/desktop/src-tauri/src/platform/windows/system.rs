//! Windows CPU, RAM and local time via plain Win32 calls. Every failed call is `None`: unknown,
//! never a guessed 0.

use kivori_model::desk::ClockTime;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{
    GetLocalTime, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::GetSystemTimes;

use crate::platform::system::{CpuTimes, SystemProbe};

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsSystemProbe;

/// `FILETIME` (two 32-bit halves) -> 100 ns ticks.
fn filetime_ticks(ft: FILETIME) -> u64 {
    (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime)
}

/// `GetSystemTimes` reports kernel time INCLUDING idle time, so total is kernel + user.
fn compose_cpu_times(idle: u64, kernel: u64, user: u64) -> CpuTimes {
    CpuTimes {
        idle,
        total: kernel.saturating_add(user),
    }
}

impl SystemProbe for WindowsSystemProbe {
    fn cpu_times(&self) -> Option<CpuTimes> {
        let (mut idle, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        // SAFETY: the three pointers are valid, writable `FILETIME`s that outlive the call.
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
        Some(compose_cpu_times(
            filetime_ticks(idle),
            filetime_ticks(kernel),
            filetime_ticks(user),
        ))
    }

    fn ram_percent(&self) -> Option<u8> {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: `status` is a valid, writable MEMORYSTATUSEX with `dwLength` set as required.
        unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
        Some(status.dwMemoryLoad.min(100) as u8)
    }
}

/// The local wall-clock time of day.
pub fn local_time() -> Option<ClockTime> {
    // SAFETY: `GetLocalTime` takes no arguments and cannot fail.
    let t = unsafe { GetLocalTime() };
    clock_from_parts(t.wHour, t.wMinute, t.wSecond)
}

fn clock_from_parts(hour: u16, minute: u16, second: u16) -> Option<ClockTime> {
    // A leap second can report 60; clamp so the wire contract (0..=59) holds.
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(ClockTime {
        hour: hour as u8,
        minute: minute as u8,
        second: second.min(59) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_joins_high_and_low_halves() {
        let ft = FILETIME {
            dwLowDateTime: 0x0000_0002,
            dwHighDateTime: 0x0000_0001,
        };
        assert_eq!(filetime_ticks(ft), 0x1_0000_0002);
        assert_eq!(filetime_ticks(FILETIME::default()), 0);
        let max = FILETIME {
            dwLowDateTime: u32::MAX,
            dwHighDateTime: u32::MAX,
        };
        assert_eq!(filetime_ticks(max), u64::MAX);
    }

    #[test]
    fn kernel_time_includes_idle_so_total_is_kernel_plus_user() {
        let t = compose_cpu_times(300, 500, 200);
        assert_eq!(t.idle, 300);
        assert_eq!(t.total, 700, "idle must not be added a second time");
    }

    #[test]
    fn clock_parts_are_validated_and_leap_second_clamped() {
        let t = clock_from_parts(23, 59, 60).expect("leap second");
        assert_eq!((t.hour, t.minute, t.second), (23, 59, 59));
        assert!(clock_from_parts(24, 0, 0).is_none());
        assert!(clock_from_parts(0, 60, 0).is_none());
    }
}
