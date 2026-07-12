use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static BINARY_UPDATED: AtomicBool = AtomicBool::new(false);

/// Exit code `main` returns when the on-disk binary was rebuilt while this process was
/// running. Distinct from both a clean exit (0) and a crash (1), so the auto-start login
/// script can relaunch immediately with the new binary, without that relaunch counting
/// against the crash-retry budget or waiting out the crash retry delay.
pub const BINARY_UPDATED_EXIT_CODE: u8 = 42;

extern "C" fn handle_shutdown_signal(_signal: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Clean exit on SIGTERM or SIGINT.
pub fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, handle_shutdown_signal as usize);
        libc::signal(libc::SIGINT, handle_shutdown_signal as usize);
    }
}

pub fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

pub fn binary_updated() -> bool {
    BINARY_UPDATED.load(Ordering::SeqCst)
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
