use std::env;

fn main() {
    // Tell Cargo to link against SDL2 and OpenGL ES libraries
    println!("cargo:rustc-link-lib=SDL2");
    println!("cargo:rustc-link-lib=GLESv2");
    
    // For cross-compilation to Raspberry Pi
    if env::var("TARGET").unwrap_or_default().contains("armv7-unknown-linux-gnueabihf") {
        // Add additional library paths for cross-compilation if needed
        println!("cargo:rustc-link-search=native=/usr/lib/arm-linux-gnueabihf");
    }
    
    // Tell Cargo to re-run this build script if these files change
    println!("cargo:rerun-if-changed=build.rs");
}
