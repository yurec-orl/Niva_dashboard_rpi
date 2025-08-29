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
    
    // Tell Cargo to re-run this build script if these files change
    println!("cargo:rerun-if-changed=build.rs");
}
