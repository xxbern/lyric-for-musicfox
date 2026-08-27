//! 内存自诊断：阶段打点与周期采样（设置环境变量 `LFM_MEM_DIAG=1` 启用）。

#[cfg(windows)]
mod imp {
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENABLED: AtomicBool = AtomicBool::new(false);
    const CSV_NAME: &str = "mem_diag.csv";

    fn enabled() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    /// 进程工作集（字节）。
    fn working_set_bytes() -> Option<(u64, u64)> {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;

        unsafe {
            let mut pmc = PROCESS_MEMORY_COUNTERS {
                cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                ..Default::default()
            };
            let proc_handle = HANDLE(GetCurrentProcess().0);
            if GetProcessMemoryInfo(proc_handle, &mut pmc, pmc.cb).is_ok() {
                Some((pmc.WorkingSetSize as u64, pmc.PagefileUsage as u64))
            } else {
                None
            }
        }
    }

    pub fn record(label: &str) {
        if !enabled() {
            return;
        }
        let Some((ws, commit)) = working_set_bytes() else {
            return;
        };
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let fresh = !std::path::Path::new(CSV_NAME).exists();
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(CSV_NAME)
        {
            if fresh {
                let _ = writeln!(f, "epoch_s,label,working_set_kb,commit_kb");
            }
            let _ = writeln!(f, "{ts},{label},{},{}", ws / 1024, commit / 1024);
        }
        log::info!(
            "[mem-diag] {label}: working_set={}KB commit={}KB",
            ws / 1024,
            commit / 1024
        );
    }

    pub fn spawn_sampler(interval_secs: u64) {
        if !enabled() {
            return;
        }
        record("sampler_start");
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
            record("sample");
        });
    }

    pub fn init_from_env() {
        let on = std::env::var("LFM_MEM_DIAG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        ENABLED.store(on, Ordering::Relaxed);
    }
}

#[cfg(windows)]
pub use imp::{init_from_env, record, spawn_sampler};

#[cfg(not(windows))]
mod stub {
    pub fn init_from_env() {}
    pub fn record(_label: &str) {}
    pub fn spawn_sampler(_interval_secs: u64) {}
}

#[cfg(not(windows))]
pub use stub::{init_from_env, record, spawn_sampler};
