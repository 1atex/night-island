use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};

const MB: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage {
    pub working_set: u64,
    pub private: u64,
}

impl Usage {
    pub fn working_mb(self) -> u64 {
        self.working_set / MB
    }

    pub fn private_mb(self) -> u64 {
        self.private / MB
    }
}

pub fn usage() -> Option<Usage> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    let size = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    counters.cb = size;
    unsafe {
        GetProcessMemoryInfo(GetCurrentProcess(), (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(), size)
            .ok()?;
    }
    Some(Usage { working_set: counters.WorkingSetSize as u64, private: counters.PrivateUsage as u64 })
}

pub fn trim() {
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

pub fn worth_logging(before: Usage, after: Usage) -> bool {
    before.working_set.saturating_sub(after.working_set) >= 10 * MB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Needs the Windows process memory counters, which wine does not report"]
    fn the_process_reports_a_plausible_memory_use() {
        let usage = usage().expect("memory info");
        assert!(usage.working_mb() >= 1 && usage.working_mb() < 4096);
        assert!(usage.private >= MB);
    }

    #[test]
    #[ignore = "Needs the Windows process memory counters, which wine does not report"]
    fn trimming_gives_pages_back_to_windows() {
        let hog = vec![1u8; 48 << 20];
        assert_eq!(hog.iter().map(|b| u64::from(*b)).sum::<u64>(), 48 << 20);
        let before = usage().unwrap();
        trim();
        let after = usage().unwrap();
        assert!(after.working_set < before.working_set, "{before:?} -> {after:?}");
        assert!(worth_logging(before, after));
        assert_eq!(hog.len(), 48 << 20);
    }

    #[test]
    fn only_a_big_drop_is_worth_a_log_line() {
        let a = Usage { working_set: 100 * MB, private: 100 * MB };
        let small = Usage { working_set: 95 * MB, private: 100 * MB };
        let big = Usage { working_set: 60 * MB, private: 100 * MB };
        assert!(!worth_logging(a, small) && worth_logging(a, big) && !worth_logging(big, a));
    }
}
