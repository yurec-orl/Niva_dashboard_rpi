#![allow(dead_code)]
//! Read-only system/hardware diagnostics for the Diagnostics page: OS/kernel identity,
//! disk usage, and Raspberry Pi power/clock health (`vcgencmd`). Everything here is a
//! best-effort snapshot — every accessor returns `None` rather than erroring, since none of
//! this is required for the dashboard to function, only to display extra context.

use std::process::Command;

// vcgencmd is invoked by absolute path (like uhubctl in adc_data_provider.rs) so it doesn't
// depend on the launching process's PATH.
const VCGENCMD: &str = "/usr/bin/vcgencmd";
const DF: &str = "/usr/bin/df";

/// Decoded `vcgencmd get_throttled` bitmask. Bits 0-3 are "currently happening", bits
/// 16-19 are "happened at least once since boot" — see `raspi-config`/firmware docs.
pub struct ThrottleStatus {
    pub under_voltage_now: bool,
    pub freq_capped_now: bool,
    pub throttled_now: bool,
    pub soft_temp_limit_now: bool,
    pub under_voltage_occurred: bool,
    pub freq_capped_occurred: bool,
    pub throttled_occurred: bool,
    pub soft_temp_limit_occurred: bool,
}

impl ThrottleStatus {
    fn from_bits(bits: u32) -> Self {
        ThrottleStatus {
            under_voltage_now: bits & 0x1 != 0,
            freq_capped_now: bits & 0x2 != 0,
            throttled_now: bits & 0x4 != 0,
            soft_temp_limit_now: bits & 0x8 != 0,
            under_voltage_occurred: bits & 0x10000 != 0,
            freq_capped_occurred: bits & 0x20000 != 0,
            throttled_occurred: bits & 0x40000 != 0,
            soft_temp_limit_occurred: bits & 0x80000 != 0,
        }
    }

    /// Short human-readable summary, e.g. "OK", "ACTIVE: UNDERVOLT", or
    /// "history: UNDERVOLT,THROTTLED" (flagged at some point since boot, not right now).
    pub fn summary(&self) -> String {
        let mut active = Vec::new();
        if self.under_voltage_now { active.push("UNDERVOLT"); }
        if self.freq_capped_now { active.push("FREQ_CAP"); }
        if self.throttled_now { active.push("THROTTLED"); }
        if self.soft_temp_limit_now { active.push("TEMP_LIMIT"); }

        let mut history = Vec::new();
        if self.under_voltage_occurred { history.push("UNDERVOLT"); }
        if self.freq_capped_occurred { history.push("FREQ_CAP"); }
        if self.throttled_occurred { history.push("THROTTLED"); }
        if self.soft_temp_limit_occurred { history.push("TEMP_LIMIT"); }

        if active.is_empty() && history.is_empty() {
            return "OK".to_string();
        }
        let mut parts = Vec::new();
        if !active.is_empty() {
            parts.push(format!("ACTIVE: {}", active.join(",")));
        }
        if !history.is_empty() {
            parts.push(format!("history: {}", history.join(",")));
        }
        parts.join("  ")
    }
}

/// Kernel release string (e.g. "6.12.25+rpt-rpi-v8"), read directly from procfs.
pub fn kernel_version() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
}

/// OS pretty name (e.g. "Debian GNU/Linux 12 (bookworm)") from /etc/os-release.
pub fn os_pretty_name() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// Root filesystem usage in MB: (total, available).
pub fn root_disk_usage_mb() -> Option<(u64, u64)> {
    let output = Command::new(DF).args(["-B1", "--output=size,avail", "/"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let data_line = text.lines().nth(1)?; // first line is the column header
    let mut fields = data_line.split_whitespace();
    let total: u64 = fields.next()?.parse().ok()?;
    let avail: u64 = fields.next()?.parse().ok()?;
    const BYTES_PER_MB: u64 = 1024 * 1024;
    Some((total / BYTES_PER_MB, avail / BYTES_PER_MB))
}

/// Raspberry Pi under-voltage/throttling flags via `vcgencmd get_throttled`. `None` if
/// vcgencmd isn't available (e.g. not running on a Pi).
pub fn throttle_status() -> Option<ThrottleStatus> {
    let text = command_stdout(VCGENCMD, &["get_throttled"])?;
    let hex = text.strip_prefix("throttled=0x")?;
    let bits = u32::from_str_radix(hex, 16).ok()?;
    Some(ThrottleStatus::from_bits(bits))
}

/// Core voltage in volts, via `vcgencmd measure_volts`.
pub fn core_voltage() -> Option<f32> {
    let text = command_stdout(VCGENCMD, &["measure_volts"])?;
    text.strip_prefix("volt=")?.strip_suffix('V')?.parse().ok()
}

/// ARM core clock speed in MHz, via `vcgencmd measure_clock arm`.
pub fn arm_clock_mhz() -> Option<u32> {
    let text = command_stdout(VCGENCMD, &["measure_clock", "arm"])?;
    let hz: u64 = text.split('=').nth(1)?.parse().ok()?;
    Some((hz / 1_000_000) as u32)
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
}
