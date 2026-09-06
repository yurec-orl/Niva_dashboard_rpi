use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static BINARY_UPDATED: AtomicBool = AtomicBool::new(false);
static RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);
static CONFIG_UPDATED: AtomicBool = AtomicBool::new(false);

/// Exit code `main` returns when the dashboard should be relaunched immediately by the
/// auto-start login script — the on-disk binary was rebuilt, the sensor config file was
/// edited (see watch_for_updates), or a restart was explicitly requested (SIGUSR1, see
/// restart_dashboard.sh). Distinct from both a clean exit (0) and a crash (1), so the
/// login script's retry loop relaunches right away without that counting against the
/// crash-retry budget or waiting out the crash retry delay.
pub const RESTART_EXIT_CODE: u8 = 42;

extern "C" fn handle_shutdown_signal(_signal: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

extern "C" fn handle_restart_signal(_signal: libc::c_int) {
    RESTART_REQUESTED.store(true, Ordering::SeqCst);
}

/// Clean exit on SIGTERM or SIGINT; immediate relaunch on SIGUSR1 (see
/// restart_dashboard.sh — a manual equivalent of the auto-detected binary-rebuild restart).
pub fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, handle_shutdown_signal as *const () as usize);
        libc::signal(libc::SIGINT, handle_shutdown_signal as *const () as usize);
        libc::signal(libc::SIGUSR1, handle_restart_signal as *const () as usize);
    }
}

pub fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

pub fn binary_updated() -> bool {
    BINARY_UPDATED.load(Ordering::SeqCst)
}

pub fn restart_requested() -> bool {
    RESTART_REQUESTED.load(Ordering::SeqCst)
}

pub fn config_updated() -> bool {
    CONFIG_UPDATED.load(Ordering::SeqCst)
}

/// Detect if the on-disk binary has been rebuilt or the sensor config file has been
/// edited, either of which should trigger the same immediate-relaunch path (see
/// RESTART_EXIT_CODE) -- one poll loop/thread for both, rather than two nearly-identical
/// ones, since either check missing its baseline (binary: `current_exe()` fails; config:
/// `path` doesn't exist yet) just skips that half and keeps checking the other.
///
/// Binary: inode comparison, since a rebuild replaces the file via an atomic rename.
/// Config: mtime comparison, since a text editor saving over it commonly keeps the same
/// inode. The config loader's own fail-fast error is what surfaces a missing config file,
/// not this -- a missing `path` here just means that half of the watch never arms.
pub fn watch_for_updates(config_path: std::path::PathBuf) {
    let exe_path = std::env::current_exe().ok();
    let original_ino = exe_path.as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.ino());
    let original_mtime = std::fs::metadata(&config_path).and_then(|m| m.modified()).ok();

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));

        if let Some(original_ino) = original_ino {
            let current_ino = exe_path.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.ino());
            if current_ino != Some(original_ino) {
                BINARY_UPDATED.store(true, Ordering::SeqCst);
                break;
            }
        }

        if let Some(original_mtime) = original_mtime {
            let current_mtime = std::fs::metadata(&config_path).and_then(|m| m.modified()).ok();
            if current_mtime != Some(original_mtime) {
                CONFIG_UPDATED.store(true, Ordering::SeqCst);
                break;
            }
        }
    });
}
