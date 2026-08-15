use std::process::Command as StdCommand;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const DEFAULT_MIN_AVAILABLE_MEMORY_BYTES: u64 = 1536 * MIB;
const ESTIMATED_NEW_TRIGGER_MEMORY_BYTES: u64 = 512 * MIB;

pub(super) fn default_resource_concurrency_limit() -> usize {
    let logical_cpus = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2);
    resource_concurrency_limit_for(logical_cpus, total_memory_bytes())
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
    command_output("sysctl", &["-n", "hw.memsize"])?
        .trim()
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
fn platform_available_memory_bytes() -> Option<u64> {
    parse_vm_stat_available_bytes(&command_output("vm_stat", &[])?)
}

#[cfg(target_os = "macos")]
fn parse_vm_stat_available_bytes(contents: &str) -> Option<u64> {
    let page_size = contents
        .lines()
        .next()?
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    let available_pages = contents.lines().skip(1).filter_map(|line| {
        let (name, value) = line.split_once(':')?;
        matches!(name, "Pages free" | "Pages inactive" | "Pages speculative")
            .then(|| value.trim().trim_end_matches('.').parse::<u64>().ok())
            .flatten()
    });
    available_pages.sum::<u64>().checked_mul(page_size)
}

#[cfg(target_os = "windows")]
fn total_memory_bytes() -> Option<u64> {
    windows_memory_kib("TotalVisibleMemorySize")?.checked_mul(1024)
}

#[cfg(target_os = "windows")]
fn platform_available_memory_bytes() -> Option<u64> {
    windows_memory_kib("FreePhysicalMemory")?.checked_mul(1024)
}

#[cfg(target_os = "windows")]
fn windows_memory_kib(field: &str) -> Option<u64> {
    let query = format!("(Get-CimInstance Win32_OperatingSystem).{field}");
    command_output("powershell.exe", &["-NoProfile", "-Command", &query])?
        .lines()
        .find_map(|line| line.trim().parse().ok())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn total_memory_bytes() -> Option<u64> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_available_memory_bytes() -> Option<u64> {
    None
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn command_output(command: &str, args: &[&str]) -> Option<String> {
    let output = StdCommand::new(command).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_meminfo_parser_reads_kib_values() {
        let contents = "MemTotal:       16384 kB\nMemAvailable:    2048 kB\n";
        assert_eq!(parse_meminfo_bytes(contents, "MemTotal"), Some(16 * MIB));
        assert_eq!(parse_meminfo_bytes(contents, "MemAvailable"), Some(2 * MIB));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_vm_stat_parser_counts_reclaimable_pages() {
        let contents = "Mach Virtual Memory Statistics: (page size of 4096 bytes)\nPages free: 10.\nPages active: 99.\nPages inactive: 20.\nPages speculative: 5.\n";
        assert_eq!(parse_vm_stat_available_bytes(contents), Some(35 * 4096));
    }
}
