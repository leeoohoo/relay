#[cfg(target_os = "windows")]
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const DEFAULT_MIN_AVAILABLE_MEMORY_BYTES: u64 = 1536 * MIB;
const ESTIMATED_NEW_TRIGGER_MEMORY_BYTES: u64 = 512 * MIB;
const LOAD_THROTTLE_RATIO: f64 = 0.70;
const LOAD_PAUSE_RATIO: f64 = 1.50;

#[derive(Debug)]
pub(super) struct ResourcePressureState {
    available_memory_bytes: Option<u64>,
    one_minute_load_average: Option<f64>,
    memory_claims_paused: bool,
    load_claim_slots: usize,
}

impl Default for ResourcePressureState {
    fn default() -> Self {
        Self {
            available_memory_bytes: None,
            one_minute_load_average: None,
            memory_claims_paused: false,
            load_claim_slots: 2,
        }
    }
}

impl ResourcePressureState {
    pub(super) fn refresh(
        &mut self,
        minimum_available_memory_bytes: u64,
        logical_cpus: usize,
    ) -> bool {
        let was_aggressive = self.aggressive_browser_reclaim(minimum_available_memory_bytes);
        self.available_memory_bytes = available_memory_bytes();
        self.one_minute_load_average = one_minute_load_average();

        let memory_paused = memory_bounded_claim_slots(
            1,
            self.available_memory_bytes,
            minimum_available_memory_bytes,
        ) == 0;
        if memory_paused != self.memory_claims_paused {
            if memory_paused {
                tracing::warn!(
                    available_memory_mb = self.available_memory_bytes.map(|bytes| bytes / MIB),
                    minimum_available_memory_mb = minimum_available_memory_bytes / MIB,
                    "memory pressure paused new Agent Trigger claims"
                );
            } else {
                tracing::info!(
                    available_memory_mb = self.available_memory_bytes.map(|bytes| bytes / MIB),
                    "memory pressure recovered; Agent Trigger claims resumed"
                );
            }
        }
        self.memory_claims_paused = memory_paused;

        let load_claim_slots =
            load_bounded_claim_slots(2, self.one_minute_load_average, logical_cpus);
        if load_claim_slots != self.load_claim_slots {
            match load_claim_slots {
                0 => tracing::warn!(
                    one_minute_load = self.one_minute_load_average,
                    logical_cpus,
                    "CPU load paused new Agent Trigger claims"
                ),
                1 => tracing::info!(
                    one_minute_load = self.one_minute_load_average,
                    logical_cpus,
                    "CPU load limited new Agent Trigger claims to one at a time"
                ),
                _ if self.load_claim_slots < 2 => tracing::info!(
                    one_minute_load = self.one_minute_load_average,
                    logical_cpus,
                    "CPU load recovered; normal Agent Trigger concurrency resumed"
                ),
                _ => {}
            }
        }
        self.load_claim_slots = load_claim_slots;

        !was_aggressive && self.aggressive_browser_reclaim(minimum_available_memory_bytes)
    }

    pub(super) fn claim_slots(
        &self,
        concurrency_slots: usize,
        minimum_available_memory_bytes: u64,
        logical_cpus: usize,
    ) -> usize {
        let memory_slots = memory_bounded_claim_slots(
            concurrency_slots,
            self.available_memory_bytes,
            minimum_available_memory_bytes,
        );
        load_bounded_claim_slots(memory_slots, self.one_minute_load_average, logical_cpus)
    }

    pub(super) fn aggressive_browser_reclaim(&self, minimum_available_memory_bytes: u64) -> bool {
        should_aggressively_reclaim_browser(
            self.available_memory_bytes,
            minimum_available_memory_bytes,
        )
    }
}

pub(super) fn default_resource_concurrency_limit() -> usize {
    let logical_cpus = logical_cpu_count();
    resource_concurrency_limit_for(logical_cpus, total_memory_bytes())
}

pub(super) fn logical_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2)
}

pub(super) fn resource_concurrency_limit_for(
    logical_cpus: usize,
    total_memory_bytes: Option<u64>,
) -> usize {
    let cpu_limit = (logical_cpus / 4).clamp(1, 4);
    let memory_limit = match total_memory_bytes {
        Some(bytes) if bytes <= 8 * GIB => 1,
        Some(bytes) if bytes <= 16 * GIB => 2,
        Some(bytes) if bytes <= 32 * GIB => 3,
        Some(_) | None => 4,
    };
    cpu_limit.min(memory_limit)
}

pub(super) fn minimum_available_memory_bytes_from_env() -> u64 {
    std::env::var("AGENT_TRIGGER_MIN_AVAILABLE_MEMORY_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.clamp(512, 65_536) * MIB)
        .unwrap_or(DEFAULT_MIN_AVAILABLE_MEMORY_BYTES)
}

pub(super) fn memory_bounded_claim_slots(
    concurrency_slots: usize,
    available_memory_bytes: Option<u64>,
    minimum_available_memory_bytes: u64,
) -> usize {
    let Some(available_memory_bytes) = available_memory_bytes else {
        return concurrency_slots;
    };
    if available_memory_bytes < minimum_available_memory_bytes {
        return 0;
    }
    let headroom = available_memory_bytes.saturating_sub(minimum_available_memory_bytes);
    let memory_slots = (headroom / ESTIMATED_NEW_TRIGGER_MEMORY_BYTES)
        .saturating_add(1)
        .min(usize::MAX as u64) as usize;
    concurrency_slots.min(memory_slots)
}

pub(super) fn available_memory_bytes() -> Option<u64> {
    platform_available_memory_bytes()
}

pub(super) fn should_aggressively_reclaim_browser(
    available_memory_bytes: Option<u64>,
    minimum_available_memory_bytes: u64,
) -> bool {
    available_memory_bytes
        .is_some_and(|available| available < minimum_available_memory_bytes.saturating_mul(2))
}

pub(super) fn load_bounded_claim_slots(
    concurrency_slots: usize,
    one_minute_load_average: Option<f64>,
    logical_cpus: usize,
) -> usize {
    let Some(load_average) = one_minute_load_average else {
        return concurrency_slots;
    };
    let load_ratio = load_average / logical_cpus.max(1) as f64;
    if load_ratio >= LOAD_PAUSE_RATIO {
        0
    } else if load_ratio >= LOAD_THROTTLE_RATIO {
        concurrency_slots.min(1)
    } else {
        concurrency_slots
    }
}

pub(super) fn one_minute_load_average() -> Option<f64> {
    platform_one_minute_load_average()
}

#[cfg(target_os = "linux")]
fn total_memory_bytes() -> Option<u64> {
    parse_meminfo_bytes(&std::fs::read_to_string("/proc/meminfo").ok()?, "MemTotal")
}

#[cfg(target_os = "linux")]
fn platform_available_memory_bytes() -> Option<u64> {
    parse_meminfo_bytes(
        &std::fs::read_to_string("/proc/meminfo").ok()?,
        "MemAvailable",
    )
}

#[cfg(target_os = "linux")]
fn platform_one_minute_load_average() -> Option<f64> {
    std::fs::read_to_string("/proc/loadavg")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[cfg(target_os = "linux")]
fn parse_meminfo_bytes(contents: &str, key: &str) -> Option<u64> {
    contents.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name == key)
            .then(|| value.split_whitespace().next()?.parse::<u64>().ok())
            .flatten()
            .and_then(|value| value.checked_mul(1024))
    })
}

#[cfg(target_os = "macos")]
fn total_memory_bytes() -> Option<u64> {
    let name = b"hw.memsize\0";
    let mut value = 0_u64;
    let mut size = std::mem::size_of::<u64>();
    // SAFETY: `name` is NUL terminated and the output buffer and length match a u64.
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr().cast(),
            (&mut value as *mut u64).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (status == 0 && size == std::mem::size_of::<u64>()).then_some(value)
}

#[cfg(target_os = "macos")]
fn platform_available_memory_bytes() -> Option<u64> {
    let mut statistics = std::mem::MaybeUninit::<libc::vm_statistics64_data_t>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: Mach writes at most `count` integer slots into the correctly sized structure.
    let status = unsafe {
        libc::host_statistics64(
            macos_host_self(),
            libc::HOST_VM_INFO64,
            statistics.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if status != libc::KERN_SUCCESS {
        return None;
    }
    // SAFETY: a successful host_statistics64 call initialized the output structure.
    let statistics = unsafe { statistics.assume_init() };
    // SAFETY: sysconf has no memory-safety preconditions for this constant.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        return None;
    }
    u64::from(statistics.free_count)
        .checked_add(u64::from(statistics.inactive_count))?
        .checked_add(u64::from(statistics.speculative_count))?
        .checked_mul(page_size as u64)
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn macos_host_self() -> libc::mach_port_t {
    // libc deprecates this binding in favor of a larger Mach wrapper crate;
    // the system call itself remains the stable API required here.
    unsafe { libc::mach_host_self() }
}

#[cfg(target_os = "macos")]
fn platform_one_minute_load_average() -> Option<f64> {
    let mut load = [0.0_f64; 1];
    // SAFETY: the buffer contains exactly one f64 and `nelem` matches its length.
    (unsafe { libc::getloadavg(load.as_mut_ptr(), 1) } == 1).then_some(load[0])
}

#[cfg(target_os = "windows")]
fn total_memory_bytes() -> Option<u64> {
    windows_memory_status().map(|status| status.ullTotalPhys)
}

#[cfg(target_os = "windows")]
fn platform_available_memory_bytes() -> Option<u64> {
    windows_memory_status().map(|status| status.ullAvailPhys)
}

#[cfg(target_os = "windows")]
fn platform_one_minute_load_average() -> Option<f64> {
    None
}

#[cfg(target_os = "windows")]
fn windows_memory_status() -> Option<MEMORYSTATUSEX> {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    // SAFETY: `status` has the exact Windows MEMORYSTATUSEX layout and a valid length field.
    (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn total_memory_bytes() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_available_memory_bytes() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_one_minute_load_average() -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrency_limit_combines_cpu_and_total_memory() {
        assert_eq!(resource_concurrency_limit_for(1, Some(64 * GIB)), 1);
        assert_eq!(resource_concurrency_limit_for(8, Some(64 * GIB)), 2);
        assert_eq!(resource_concurrency_limit_for(64, Some(8 * GIB)), 1);
        assert_eq!(resource_concurrency_limit_for(64, Some(16 * GIB)), 2);
        assert_eq!(resource_concurrency_limit_for(64, Some(32 * GIB)), 3);
        assert_eq!(resource_concurrency_limit_for(64, Some(64 * GIB)), 4);
        assert_eq!(resource_concurrency_limit_for(64, None), 4);
    }

    #[test]
    fn memory_pressure_stops_only_new_claims_and_bounds_bursts() {
        let reserve = DEFAULT_MIN_AVAILABLE_MEMORY_BYTES;
        assert_eq!(memory_bounded_claim_slots(4, None, reserve), 4);
        assert_eq!(memory_bounded_claim_slots(4, Some(reserve - 1), reserve), 0);
        assert_eq!(memory_bounded_claim_slots(4, Some(reserve), reserve), 1);
        assert_eq!(
            memory_bounded_claim_slots(4, Some(reserve + 512 * MIB), reserve),
            2
        );
        assert_eq!(
            memory_bounded_claim_slots(4, Some(reserve + 4 * GIB), reserve),
            4
        );
    }

    #[test]
    fn browser_reclaim_starts_before_new_claims_are_fully_paused() {
        let reserve = DEFAULT_MIN_AVAILABLE_MEMORY_BYTES;
        assert!(!should_aggressively_reclaim_browser(None, reserve));
        assert!(!should_aggressively_reclaim_browser(
            Some(reserve * 2),
            reserve
        ));
        assert!(should_aggressively_reclaim_browser(
            Some(reserve * 2 - 1),
            reserve
        ));
    }

    #[test]
    fn load_pressure_throttles_then_pauses_only_new_claims() {
        assert_eq!(load_bounded_claim_slots(4, None, 8), 4);
        assert_eq!(load_bounded_claim_slots(4, Some(5.5), 8), 4);
        assert_eq!(load_bounded_claim_slots(4, Some(5.6), 8), 1);
        assert_eq!(load_bounded_claim_slots(4, Some(7.2), 8), 1);
        assert_eq!(load_bounded_claim_slots(4, Some(12.0), 8), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_meminfo_parser_reads_kib_values() {
        let contents = "MemTotal:       16384 kB\nMemAvailable:    2048 kB\n";
        assert_eq!(parse_meminfo_bytes(contents, "MemTotal"), Some(16 * MIB));
        assert_eq!(parse_meminfo_bytes(contents, "MemAvailable"), Some(2 * MIB));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_resource_probe_uses_native_system_apis() {
        let total = total_memory_bytes().expect("total memory");
        let available = platform_available_memory_bytes().expect("available memory");
        let load = platform_one_minute_load_average().expect("load average");
        assert!(total > 0);
        assert!(available > 0);
        assert!(load >= 0.0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_resource_probe_uses_native_system_apis() {
        let total = total_memory_bytes().expect("total memory");
        let available = platform_available_memory_bytes().expect("available memory");
        assert!(total > 0);
        assert!(available > 0);
        assert!(available <= total);
    }
}
