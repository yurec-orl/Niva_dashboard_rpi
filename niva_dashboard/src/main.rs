mod hardware;
mod graphics;
mod test;

use crate::test::run_test::run_test;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    println!("Niva Dashboard - Raspberry Pi Version (KMS/DRM Backend)");
    println!("Available test modes:");
    println!("1. Basic OpenGL triangle test");
    println!("2. OpenGL text rendering test with FreeType");
    println!("3. Dashboard performance test (9 animated gauges)");
    println!("4. Rotating needle gauge test (circular gauge with numbers)");
    println!("5. GPIO input test");
    println!("Note: SDL2-based tests are disabled after migration to KMS/DRM backend");
    println!("Usage: cargo run -- [test={{basic|gltext|dashboard|needle|gpio}}]");

    for arg in args {
        let parm = arg.split("=").collect::<Vec<&str>>();
        if parm.len() == 2 {
            match parm[0] {
                "test" => {
                    run_test(parm[1]);
                }
                _ => {
                    eprintln!("Unknown argument: {}", parm[0]);
                }
            }
        }
    }

}