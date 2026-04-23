use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use windows::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
        Threading::{GetCurrentProcess, GetCurrentProcessId, GetProcessTimes},
    },
};

pub type LogPerformanceSnapshotFn = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceSnapshot {
    pub cpu_percent: Option<f64>,
    pub working_set_bytes: usize,
    pub private_bytes: usize,
    pub thread_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CpuSample {
    captured_at: Instant,
    process_total_100ns: u64,
}

#[derive(Debug)]
pub struct ProcessCpuSampler {
    previous_sample: Mutex<Option<CpuSample>>,
    logical_processors: f64,
}

impl ProcessCpuSampler {
    pub fn new() -> Self {
        let logical_processors = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1) as f64;
        Self {
            previous_sample: Mutex::new(None),
            logical_processors,
        }
    }

    pub fn current_percent(&self) -> Result<Option<f64>, String> {
        let current = CpuSample {
            captured_at: Instant::now(),
            process_total_100ns: current_process_total_cpu_time_100ns()?,
        };
        let mut previous_sample = self
            .previous_sample
            .lock()
            .map_err(|err| format!("CPU sampler lock poisoned: {err}"))?;
        let percent = compute_cpu_percent(*previous_sample, current, self.logical_processors);
        *previous_sample = Some(current);
        Ok(percent)
    }
}

pub fn process_performance_snapshot_logger() -> LogPerformanceSnapshotFn {
    let sampler = Arc::new(ProcessCpuSampler::new());
    Arc::new(move || {
        let snapshot = capture_process_performance_snapshot(&sampler)?;
        log::info!("{}", format_performance_snapshot(&snapshot));
        Ok(())
    })
}

pub fn capture_process_performance_snapshot(
    sampler: &ProcessCpuSampler,
) -> Result<PerformanceSnapshot, String> {
    let (working_set_bytes, private_bytes) = current_process_memory_bytes()?;
    Ok(PerformanceSnapshot {
        cpu_percent: sampler.current_percent()?,
        working_set_bytes,
        private_bytes,
        thread_count: current_process_thread_count(),
    })
}

pub fn current_process_private_bytes() -> Option<usize> {
    current_process_memory_bytes()
        .ok()
        .map(|(_, private_bytes)| private_bytes)
}

pub fn current_process_thread_count() -> Option<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).ok()? };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let process_id = unsafe { GetCurrentProcessId() };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut count = 0_u32;

    unsafe {
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == process_id {
                    count += 1;
                }

                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    Some(count)
}

fn current_process_memory_bytes() -> Result<(usize, usize), String> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };

    unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
        .ok()
        .map_err(|err| format!("Could not read process memory counters: {err}"))?;
    }

    Ok((counters.WorkingSetSize, counters.PrivateUsage))
}

fn current_process_total_cpu_time_100ns() -> Result<u64, String> {
    let mut creation = Default::default();
    let mut exit = Default::default();
    let mut kernel = Default::default();
    let mut user = Default::default();

    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
        .ok()
        .ok_or_else(|| "Could not read process CPU times".to_string())?;
    }

    Ok(file_time_to_u64(kernel) + file_time_to_u64(user))
}

fn file_time_to_u64(file_time: windows::Win32::Foundation::FILETIME) -> u64 {
    ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64
}

fn format_cpu_percent(cpu_percent: Option<f64>) -> String {
    match cpu_percent {
        Some(percent) => format!("{percent:.1}%"),
        None => "n/a".to_string(),
    }
}

fn format_optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_performance_snapshot(snapshot: &PerformanceSnapshot) -> String {
    format!(
        "Performance snapshot: cpu={}, working_set={}, private_bytes={}, threads={}",
        format_cpu_percent(snapshot.cpu_percent),
        snapshot.working_set_bytes,
        snapshot.private_bytes,
        format_optional_u32(snapshot.thread_count),
    )
}

fn compute_cpu_percent(
    previous: Option<CpuSample>,
    current: CpuSample,
    logical_processors: f64,
) -> Option<f64> {
    let previous = previous?;
    let elapsed_wall_seconds = current
        .captured_at
        .saturating_duration_since(previous.captured_at)
        .as_secs_f64();
    if elapsed_wall_seconds <= f64::EPSILON || logical_processors <= f64::EPSILON {
        return None;
    }

    let elapsed_process_100ns = current
        .process_total_100ns
        .saturating_sub(previous.process_total_100ns);
    let elapsed_process_seconds = elapsed_process_100ns as f64 / 10_000_000.0;
    Some((elapsed_process_seconds / (elapsed_wall_seconds * logical_processors)) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_cpu_sample_returns_none() {
        let now = Instant::now();
        let current = CpuSample {
            captured_at: now,
            process_total_100ns: 1_000_000,
        };

        assert_eq!(compute_cpu_percent(None, current, 8.0), None);
    }

    #[test]
    fn cpu_percent_uses_process_delta_over_wall_time_and_cpu_count() {
        let start = Instant::now();
        let previous = CpuSample {
            captured_at: start,
            process_total_100ns: 0,
        };
        let current = CpuSample {
            captured_at: start + std::time::Duration::from_secs(1),
            process_total_100ns: 5_000_000,
        };

        let percent =
            compute_cpu_percent(Some(previous), current, 4.0).expect("cpu percent should exist");

        assert!((percent - 12.5).abs() < 0.1);
    }
}
