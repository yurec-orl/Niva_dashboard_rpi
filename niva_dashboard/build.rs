use std::env;

fn main() {
    // Tell Cargo to link against OpenGL ES, EGL, DRM, and GBM libraries
    println!("cargo:rustc-link-lib=GLESv2");
    println!("cargo:rustc-link-lib=EGL");
    println!("cargo:rustc-link-lib=drm");
    println!("cargo:rustc-link-lib=gbm");
    println!("cargo:rustc-link-lib=freetype");
    
    // For cross-compilation to Raspberry Pi
    if env::var("TARGET").unwrap_or_default().contains("armv7-unknown-linux-gnueabihf") {
        // Add additional library paths for cross-compilation if needed
        println!("cargo:rustc-link-search=native=/usr/lib/arm-linux-gnueabihf");
        println!("cargo:rustc-link-search=native=/opt/vc/lib");
    }
    
    // Embed build identity for the diagnostics page — lets you confirm on-device which
    // build is actually running, which matters because this project self-restarts onto a
    // freshly built binary while running (see util::shutdown::binary_updated).
    let git_hash = command_stdout("git", &["rev-parse", "--short", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=NIVA_GIT_HASH={}", git_hash);

    let build_time = command_stdout("date", &["+%Y-%m-%d %H:%M:%S"])
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=NIVA_BUILD_TIME={}", build_time);

    // Re-run if HEAD moves (new commit, checkout, merge) so the embedded hash stays fresh.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs");

    // Tell Cargo to re-run this build script if these files change
    println!("cargo:rerun-if-changed=build.rs");
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
}
