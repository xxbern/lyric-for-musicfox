//! P5 Custom Non-blocking Logger with File Rotation and Exit Drain

use crossbeam_channel::Sender;
use log::{LevelFilter, Log, Metadata, Record};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

pub struct LoggerGuard {
    tx: Option<Sender<LogMsg>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for LoggerGuard {
    fn drop(&mut self) {
        // Clear the global logger's Sender so no new logs are accepted
        if let Ok(mut guard) = LOGGER.tx.lock() {
            *guard = None;
        }
        // Drop the local Sender so that the channel receives the disconnect signal
        self.tx.take();
        // Wait for the background thread to finish draining queue and flushing files
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

enum LogMsg {
    Record {
        level: log::Level,
        target: String,
        body: String,
        timestamp: String,
    },
    Flush(Sender<()>),
}

struct FileLogger {
    tx: Mutex<Option<Sender<LogMsg>>>,
}

static LOGGER: FileLogger = FileLogger {
    tx: Mutex::new(None),
};

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(guard) = self.tx.lock() {
            if let Some(ref tx) = *guard {
                let timestamp =
                    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
                let msg = LogMsg::Record {
                    level: record.level(),
                    target: record.target().to_string(),
                    body: record.args().to_string(),
                    timestamp,
                };
                let _ = tx.send(msg);
            }
        }
    }

    fn flush(&self) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let sent = {
            if let Ok(guard) = self.tx.lock() {
                if let Some(ref sender) = *guard {
                    sender.send(LogMsg::Flush(tx)).is_ok()
                } else {
                    false
                }
            } else {
                false
            }
        };
        if sent {
            let _ = rx.recv();
        }
    }
}

fn process_msg(msg: LogMsg, file_opt: &mut Option<File>, log_path: &Path, current_size: &mut u64) {
    match msg {
        LogMsg::Record {
            level,
            target,
            body,
            timestamp,
        } => {
            let line = format!(
                "{} {:5} [{}] {}\n",
                timestamp,
                level.to_string(),
                target,
                body
            );

            // Double write to stderr
            let _ = std::io::stderr().write_all(line.as_bytes());

            // Write to file
            if let Some(file) = file_opt {
                if let Ok(()) = file.write_all(line.as_bytes()) {
                    *current_size += line.len() as u64;

                    // Check size limit: 5MB (5 * 1024 * 1024)
                    if *current_size >= 5 * 1024 * 1024 {
                        let _ = file.flush();
                        let _ = file.sync_all();
                        *file_opt = None; // Close current file handle

                        let mut old_path = log_path.to_path_buf();
                        old_path.set_extension("txt.old");

                        let _ = std::fs::remove_file(&old_path);
                        let _ = std::fs::rename(log_path, &old_path);

                        if let Ok(new_file) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(log_path)
                        {
                            *current_size = new_file.metadata().map(|m| m.len()).unwrap_or(0);
                            *file_opt = Some(new_file);
                        } else {
                            *current_size = 0;
                        }
                    }
                }
            } else {
                // If log file was not previously open, try to open it
                if let Ok(new_file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                {
                    *current_size = new_file.metadata().map(|m| m.len()).unwrap_or(0);
                    *file_opt = Some(new_file);
                    // Try writing again
                    let _ = file_opt.as_mut().unwrap().write_all(line.as_bytes());
                    *current_size += line.len() as u64;
                }
            }
        }
        LogMsg::Flush(tx) => {
            if let Some(file) = file_opt {
                let _ = file.flush();
                let _ = file.sync_all();
            }
            let _ = tx.send(());
        }
    }
}

pub fn init(log_path: &Path, default_level: LevelFilter, enabled: bool) -> LoggerGuard {
    if !enabled {
        log::set_max_level(LevelFilter::Off);
        let _ = log::set_logger(&LOGGER);
        return LoggerGuard {
            tx: None,
            thread_handle: None,
        };
    }

    let (tx, rx) = crossbeam_channel::unbounded();

    let mut level = default_level;
    if let Ok(env_val) = std::env::var("LYRIC_LOG") {
        match env_val.to_uppercase().as_str() {
            "OFF" => level = LevelFilter::Off,
            "ERROR" => level = LevelFilter::Error,
            "WARN" => level = LevelFilter::Warn,
            "INFO" => level = LevelFilter::Info,
            "DEBUG" => level = LevelFilter::Debug,
            "TRACE" => level = LevelFilter::Trace,
            _ => {}
        }
    }

    log::set_max_level(level);
    let _ = log::set_logger(&LOGGER);

    if let Ok(mut guard) = LOGGER.tx.lock() {
        *guard = Some(tx.clone());
    }

    let log_path_buf = log_path.to_path_buf();
    let thread_handle = std::thread::spawn(move || {
        if let Some(parent) = log_path_buf.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_buf)
            .ok();

        let mut current_size = file
            .as_ref()
            .and_then(|f| f.metadata().map(|m| m.len()).ok())
            .unwrap_or(0);

        loop {
            match rx.recv() {
                Ok(msg) => {
                    process_msg(msg, &mut file, &log_path_buf, &mut current_size);
                }
                Err(_) => {
                    // Drain remaining logs
                    while let Ok(msg) = rx.try_recv() {
                        process_msg(msg, &mut file, &log_path_buf, &mut current_size);
                    }
                    break;
                }
            }
        }
    });

    LoggerGuard {
        tx: Some(tx),
        thread_handle: Some(thread_handle),
    }
}

pub use log::{debug, error, info, warn};
