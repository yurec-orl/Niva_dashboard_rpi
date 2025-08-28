mod hardware;
mod graphics;
mod test;

use crate::test::run_test::run_test;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    println!("Niva Dashboard - Raspberry Pi Version");
    println!("Available test modes:");
    println!("1. Basic OpenGL triangle test");
    println!("2. Simple moving needle test");
    println!("3. Multi-gauge dashboard test (OpenGL)");
    println!("4. Text rendering test with multiple fonts and sizes (SDL2 TTF)");
    println!("5. SDL2 high-level gauge rendering test");
    println!("6. SDL2 advanced needle rendering methods (rectangles, polygons, textures)");
    println!("7. OpenGL rotating needles demo with antialiasing and variable thickness");
    println!("8. OpenGL text rendering test with FreeType");
    println!("9. GPIO input test");
    println!("10. Combined graphics test (shared context)");
    println!("Usage: cargo run -- [test={{basic|needle|gauges|text|sdl2|advanced|rotating|gltext|gpio|all}}]");

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