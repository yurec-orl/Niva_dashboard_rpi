use flexi_logger::{
    Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, Logger, LoggerHandle, Naming, Record,
    TS_DASHES_BLANK_COLONS_DOT_BLANK,
};
use std::env;

const MAX_LOG_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024; // Rotate once a log file reaches 5 MB
const MAX_LOG_FILES: usize = 10; // Keep the 10 most recent log files, delete older ones

/// Same fields as flexi_logger's `default_format` (level, module path, message), with a
/// timestamp prepended — needed to correlate log lines against events happening elsewhere
/// (e.g. matching a UPS shutdown against how long a build had been running).
fn timestamped_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {} [{}] {}",
        now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args(),
    )
}

/// Sets up file + terminal logging. Log lines go to both stdout (like the previous
/// `print!` calls) and to a rotating file under `~/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs`.
/// The returned handle must be kept alive for the lifetime of `main` — dropping it early
/// would tear down the logger's background writer thread.
pub fn init_logging() -> LoggerHandle {
    let home = env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
    let log_dir = format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs");

    let handle = Logger::try_with_env_or_str("info")
        .expect("Invalid log level filter")
        .format(timestamped_format)
        .log_to_file(FileSpec::default().directory(log_dir).basename("niva_dashboard"))
        .append()
        .duplicate_to_stdout(Duplicate::All)
        .rotate(
            Criterion::Size(MAX_LOG_FILE_SIZE_BYTES),
            Naming::Numbers,
            Cleanup::KeepLogFiles(MAX_LOG_FILES),
        )
        .start()
        .expect("Failed to initialize logger");

    // The file writer only opens its output file lazily, on the first write, so
    // trigger_rotation() would be a no-op if called before anything is logged. Force
    // the open first, then rotate away whatever _rCURRENT.log carried over (via
    // .append()) from the previous run, so each run starts with a fresh file.
    // Side effect: the first log entry will always be in older log.
    log::info!("Rotating log file");
    handle
        .trigger_rotation()
        .expect("Failed to rotate log file on startup");

    handle
}
