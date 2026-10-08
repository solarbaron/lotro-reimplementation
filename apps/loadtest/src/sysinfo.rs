// SPDX-License-Identifier: GPL-3.0-or-later
//! What the report says about the machine and the process (`test-server.md` §13.3, §13.4):
//! resident memory, CPU time, load average, CPU model, cores and memory, read from Linux's
//! `/proc`. On other systems (or when a file is missing) every reading is `None`.
//!
//! The parsers take the file's text and never panic: a line they do not understand is
//! skipped.

/// Bytes of a `/proc/self/status` field given in kB (`VmRSS`, `VmHWM`, ...).
#[must_use]
pub fn parse_status_kib(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(key)?.strip_prefix(':')?;
        let mut parts = rest.split_whitespace();
        let value: u64 = parts.next()?.parse().ok()?;
        match parts.next() {
            Some("kB") | None => value.checked_mul(1024),
            Some(_) => None,
        }
    })
}

/// The 1-minute load average from the text of `/proc/loadavg`.
#[must_use]
pub fn parse_loadavg(text: &str) -> Option<f64> {
    let v: f64 = text.split_whitespace().next()?.parse().ok()?;
    (v.is_finite() && v >= 0.0).then_some(v)
}

/// User plus system CPU time in clock ticks from the text of `/proc/self/stat` (fields 14
/// and 15, counted after the parenthesised command name, which may itself hold spaces).
#[must_use]
pub fn parse_stat_cpu_ticks(text: &str) -> Option<u64> {
    let after = &text[text.rfind(')')? + 1..];
    let fields: Vec<&str> = after.split_whitespace().take(13).collect();
    // After ")" the fields start at field 3 (state); utime is field 14, stime field 15.
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    utime.checked_add(stime)
}

/// The first `model name` of `/proc/cpuinfo`.
#[must_use]
pub fn parse_cpu_model(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim() == "model name").then(|| v.trim().chars().take(128).collect())
    })
}

fn read(path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    // These files are small; refuse anything absurd rather than hold it.
    (text.len() <= 4 << 20).then_some(text)
}

/// This process's resident memory, bytes.
#[must_use]
pub fn process_rss() -> Option<u64> {
    parse_status_kib(&read("/proc/self/status")?, "VmRSS")
}

/// Resident memory of another process on this machine, bytes (`/proc/<pid>/status`).
#[must_use]
pub fn pid_rss(pid: u32) -> Option<u64> {
    parse_status_kib(&read(&format!("/proc/{pid}/status"))?, "VmRSS")
}

/// This process's peak resident memory, bytes.
#[must_use]
pub fn process_peak_rss() -> Option<u64> {
    parse_status_kib(&read("/proc/self/status")?, "VmHWM")
}

/// This process's CPU time (user + system), seconds. Linux reports clock ticks of
/// `USER_HZ`, which is 100 on every architecture Tessera targets.
#[must_use]
pub fn process_cpu_seconds() -> Option<f64> {
    parse_stat_cpu_ticks(&read("/proc/self/stat")?).map(|t| t as f64 / 100.0)
}

/// The 1-minute load average.
#[must_use]
pub fn load_average() -> Option<f64> {
    parse_loadavg(&read("/proc/loadavg")?)
}

/// What the report records about the machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    /// CPU model, if known.
    pub cpu: Option<String>,
    /// Logical cores available to this process.
    pub cores: usize,
    /// Total memory, bytes.
    pub memory: Option<u64>,
}

impl Machine {
    /// Reads the machine description.
    #[must_use]
    pub fn detect() -> Self {
        Self {
            cpu: read("/proc/cpuinfo").and_then(|t| parse_cpu_model(&t)),
            cores: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
            memory: read("/proc/meminfo").and_then(|t| parse_status_kib(&t, "MemTotal")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_fields() {
        let t = "Name:\tx\nVmHWM:\t  2048 kB\nVmRSS:\t  1024 kB\n";
        assert_eq!(parse_status_kib(t, "VmRSS"), Some(1024 * 1024));
        assert_eq!(parse_status_kib(t, "VmHWM"), Some(2048 * 1024));
        assert_eq!(parse_status_kib(t, "VmSwap"), None);
        assert_eq!(parse_status_kib("VmRSS: x kB", "VmRSS"), None);
        assert_eq!(parse_status_kib("VmRSS: 1 MB", "VmRSS"), None);
        assert_eq!(
            parse_status_kib("VmRSS: 18446744073709551615 kB", "VmRSS"),
            None
        );
        // A key that only starts with the wanted one does not match.
        assert_eq!(parse_status_kib("VmRSSx: 1 kB", "VmRSS"), None);
    }

    #[test]
    fn loadavg_and_stat() {
        assert_eq!(parse_loadavg("1.50 2.00 3.00 1/100 42\n"), Some(1.5));
        assert_eq!(parse_loadavg(""), None);
        assert_eq!(parse_loadavg("-1 2 3"), None);
        let stat = "42 (a b) c) S 1 2 3 4 5 6 7 8 9 10 250 50 0 0 20 0";
        assert_eq!(parse_stat_cpu_ticks(stat), Some(300));
        assert_eq!(parse_stat_cpu_ticks("42 (x) S 1"), None);
        assert_eq!(parse_stat_cpu_ticks("no paren"), None);
    }

    #[test]
    fn cpu_model() {
        let t = "processor\t: 0\nmodel name\t: Some CPU @ 2.40GHz\n";
        assert_eq!(parse_cpu_model(t).as_deref(), Some("Some CPU @ 2.40GHz"));
        assert_eq!(parse_cpu_model("nothing"), None);
    }

    #[test]
    fn parsers_never_panic_on_noise() {
        let mut rng = crate::rng::Rng::new(1);
        for n in 0..2_000 {
            let bytes = rng.bytes(n % 97);
            let text = String::from_utf8_lossy(&bytes);
            let _ = parse_status_kib(&text, "VmRSS");
            let _ = parse_loadavg(&text);
            let _ = parse_stat_cpu_ticks(&text);
            let _ = parse_cpu_model(&text);
            let with_paren = format!("1 ({text}) {text}");
            let _ = parse_stat_cpu_ticks(&with_paren);
        }
    }

    #[test]
    fn this_machine() {
        let m = Machine::detect();
        assert!(m.cores >= 1);
        if cfg!(target_os = "linux") {
            assert!(process_rss().is_some_and(|r| r > 0));
            assert!(pid_rss(std::process::id()).is_some_and(|r| r > 0));
            assert!(load_average().is_some());
        }
    }
}
