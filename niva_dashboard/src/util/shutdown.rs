use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static BINARY_UPDATED: AtomicBool = AtomicBool::new(false);
static RESTART_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Exit code `main` returns when the dashboard should be relaunched immediately by the
/// auto-start login script — either the on-disk binary was rebuilt (see
/// watch_for_binary_update) or a restart was explicitly requested (SIGUSR1, see
/// restart_dashboard.sh). Distinct from both a clean exit (0) and a crash (1), so the login
/// script's retry loop relaunches right away without that counting against the crash-retry
/// budget or waiting out the crash retry delay.
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

/// Detect if the binary has been updated
pub fn watch_for_binary_update() {
    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };
    let Ok(original_ino) = std::fs::metadata(&exe_path).map(|m| m.ino()) else {
        return;
    };

    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));
        if let Ok(meta) = std::fs::metadata(&exe_path) {
            if meta.ino() != original_ino {
                BINARY_UPDATED.store(true, Ordering::SeqCst);
                break;
            }
        }
    });
}
