mod hardware;
mod graphics;
mod page_framework;
mod test;

use crate::test::run_test::run_test;
use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::PageManager;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    print!("Niva Dashboard - Raspberry Pi Version (KMS/DRM Backend)\r\n");
    print!("Available test modes:\r\n");
    print!("1. Basic OpenGL triangle test\r\n");
    print!("2. OpenGL text rendering test with FreeType\r\n");
    print!("3. Dashboard performance test (9 animated gauges)\r\n");
    print!("4. Rotating needle gauge test (circular gauge with numbers)\r\n");
    print!("5. GPIO input test\r\n");
    print!("Usage: cargo run -- [test={{basic|gltext|dashboard|needle|gpio}}]\r\n");

    for arg in args {
        let parm = arg.split("=").collect::<Vec<&str>>();
        if parm.len() == 2 {
            match parm[0] {
                "test" => {
                    run_test(parm[1]);
                    return;
                }
                _ => {
                    eprintln!("Unknown argument: {}", parm[0]);
                }
            }
        }
    }

    let context = GraphicsContext::new_dashboard("Niva Dashboard").expect("Failed to create graphics context");
    
    // Hide mouse cursor for dashboard application
    if let Err(e) = context.hide_cursor() {
        eprintln!("Warning: Failed to hide cursor: {}", e);
    } else {
        print!("✓ Mouse cursor hidden for dashboard mode\r\n");
    }

    let mut mgr = PageManager::new(context);
    match mgr.start() {
        Ok(()) => print!("Dashboard finished successfully!\r\n"),
        Err(e) => eprintln!("Failed to start dashboard: {}", e),
    }
}