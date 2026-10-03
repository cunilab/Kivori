//! CPU, RAM and local time from Mach host statistics and libc.
//!
//! RAM is Activity Monitor's "Memory Used": app memory (`internal - purgeable`) + wired +
//! compressed, in pages of the host page size, over `hw.memsize`. Same formula as htop's macOS
//! port (htop-dev/htop#631) and other Activity-Monitor-matching tools.
//!
//! CPU comes from per-CPU `host_processor_info(PROCESSOR_CPU_LOAD_INFO)`, NOT
//! `host_statistics(HOST_CPU_LOAD_INFO)`: XNU rate-limits `host_statistics*` for third-party
//! processes (`rate_limit_host_statistics` in osfmk/kern/host.c: 2..=10 calls per 1 s window,
//! system-wide, then a cached copy), and that cache was observed here returning identical tick
//! counts 200 ms apart, i.e. CPU unknown. The aggregate counters are also u32 and wrap after
//! ~60 days on 8 cores; per-CPU counters summed in u64 do not. RAM is a level, not a delta, so a
//! cached `HOST_VM_INFO64` reply costs at most a second of staleness and stays on host_statistics.

use std::sync::OnceLock;

use kivori_model::desk::ClockTime;

use crate::platform::system::{CpuTimes, SystemProbe};

// `libc` marks its Mach port items deprecated ("use mach2"), so the ones we need are
// declared here. Signatures from <mach/mach_init.h> and <mach/mach_host.h>.
extern "C" {
    fn mach_host_self() -> libc::mach_port_t;
    fn host_page_size(host: libc::mach_port_t, out_page_size: *mut libc::vm_size_t) -> i32;
    static mach_task_self_: libc::mach_port_t;
}

/// The host port, acquired once per process. Every `mach_host_self()` call adds a user reference
/// to the same send right, so calling it once a second would grow that count without bound.
fn host() -> libc::mach_port_t {
    static HOST: OnceLock<libc::mach_port_t> = OnceLock::new();
    // SAFETY: `mach_host_self` takes no arguments and always returns this task's host port name.
    *HOST.get_or_init(|| unsafe { mach_host_self() })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MacSystemProbe;

/// Per-CPU `processor_cpu_load_info` ticks (`[user, system, idle, nice]` per CPU) summed over
/// every CPU: all states as total, the idle state as idle.
fn cpu_times_from_ticks(ticks: &[u32]) -> CpuTimes {
    let states = libc::CPU_STATE_MAX as usize;
    let idle_slot = libc::CPU_STATE_IDLE as usize;
    ticks
        .chunks_exact(states)
        .fold(CpuTimes { idle: 0, total: 0 }, |acc, cpu| CpuTimes {
            idle: acc.idle + u64::from(cpu[idle_slot]),
            total: acc.total + cpu.iter().map(|&t| u64::from(t)).sum::<u64>(),
        })
}

/// Activity Monitor "Memory Used" as a percentage of physical memory, rounded and clamped.
fn ram_percent_from_pages(
    internal: u64,
    purgeable: u64,
    wired: u64,
    compressed: u64,
    page_size: u64,
    physical: u64,
) -> Option<u8> {
    if physical == 0 {
        return None;
    }
    let used_pages = internal.saturating_sub(purgeable) + wired + compressed;
    let used = u128::from(used_pages) * u128::from(page_size);
    let pct = (used * 100 + u128::from(physical) / 2) / u128::from(physical);
    Some(pct.min(100) as u8)
}

impl SystemProbe for MacSystemProbe {
    fn cpu_times(&self) -> Option<CpuTimes> {
        let mut cpus: libc::natural_t = 0;
        let mut info: libc::processor_info_array_t = std::ptr::null_mut();
        let mut count: libc::mach_msg_type_number_t = 0;
        // SAFETY: all three are valid out-pointers; on success the kernel maps a fresh array of
        // `count` `integer_t`s into this task and stores its address in `info`.
        let kr = unsafe {
            libc::host_processor_info(
                host(),
                libc::PROCESSOR_CPU_LOAD_INFO,
                &mut cpus,
                &mut info,
                &mut count,
            )
        };
        if kr != libc::KERN_SUCCESS || info.is_null() {
            return None;
        }
        // SAFETY: the kernel guarantees `info` points at `count` initialised `integer_t`s, which
        // stay mapped until the `vm_deallocate` below; the slice is not used after it.
        let ticks = unsafe { std::slice::from_raw_parts(info.cast::<u32>(), count as usize) };
        let times = cpu_times_from_ticks(ticks);
        // SAFETY: frees exactly the region `host_processor_info` allocated in this task.
        unsafe {
            libc::vm_deallocate(
                mach_task_self_,
                info as libc::vm_address_t,
                count as usize * std::mem::size_of::<libc::integer_t>(),
            );
        }
        Some(times)
    }

    fn ram_percent(&self) -> Option<u8> {
        // SAFETY: all-zero is a valid `vm_statistics64` (plain integers).
        let mut vm: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
        let mut count = libc::HOST_VM_INFO64_COUNT;
        // SAFETY: `vm` is a writable `vm_statistics64` and `count` is its size in `integer_t`
        // units, exactly what HOST_VM_INFO64 expects.
        let kr = unsafe {
            libc::host_statistics64(
                host(),
                libc::HOST_VM_INFO64,
                std::ptr::addr_of_mut!(vm).cast(),
                &mut count,
            )
        };
        if kr != libc::KERN_SUCCESS {
            return None;
        }

        let mut page_size: libc::vm_size_t = 0;
        // SAFETY: `page_size` is a valid out-pointer for the duration of the call.
        if unsafe { host_page_size(host(), &mut page_size) } != libc::KERN_SUCCESS {
            return None;
        }

        let mut physical: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: NUL-terminated name; `physical`/`len` describe a writable u64, which is the
        // documented type of `hw.memsize`; no new value is written.
        let rc = unsafe {
            libc::sysctlbyname(
                c"hw.memsize".as_ptr(),
                std::ptr::addr_of_mut!(physical).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return None;
        }

        // Copy out of the packed struct before use (no references into it).
        let (internal, purgeable, wired, compressed) = (
            vm.internal_page_count,
            vm.purgeable_count,
            vm.wire_count,
            vm.compressor_page_count,
        );
        ram_percent_from_pages(
            u64::from(internal),
            u64::from(purgeable),
            u64::from(wired),
            u64::from(compressed),
            page_size as u64,
            physical,
        )
    }
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

    #[test]
    fn cpu_total_sums_every_state_of_every_cpu_and_idle_is_the_idle_slot() {
        // Two CPUs, each [user, system, idle, nice].
        let t = cpu_times_from_ticks(&[10, 20, 300, 5, 1, 2, 30, 0]);
        assert_eq!(
            t,
            CpuTimes {
                idle: 330,
                total: 368
            }
        );
        let t = cpu_times_from_ticks(&[u32::MAX; 8]);
        assert_eq!(t.total, 8 * u64::from(u32::MAX), "no u32 overflow");
        assert_eq!(cpu_times_from_ticks(&[]), CpuTimes { idle: 0, total: 0 });
    }

    #[test]
    fn ram_is_app_plus_wired_plus_compressed_over_physical() {
        // 16 KiB pages, 16 GiB machine = 1_048_576 pages.
        let page = 16_384;
        let phys = 16 * 1024 * 1024 * 1024;
        // (600k - 100k) + 200k + 50k = 750k pages = 71.5 % -> 72.
        assert_eq!(
            ram_percent_from_pages(600_000, 100_000, 200_000, 50_000, page, phys),
            Some(72)
        );
        assert_eq!(
            ram_percent_from_pages(1, 5, 0, 0, page, phys),
            Some(0),
            "saturates"
        );
        assert_eq!(
            ram_percent_from_pages(u64::from(u32::MAX), 0, 0, 0, page, phys),
            Some(100)
        );
        assert_eq!(ram_percent_from_pages(1, 0, 0, 0, page, 0), None);
    }
}
