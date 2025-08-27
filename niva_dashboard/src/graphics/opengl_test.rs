#![allow(unused)]
use std::ffi::CString;

// SDL2 and OpenGL ES bindings
extern "C" {
    // SDL2 functions
    fn SDL_Init(flags: u32) -> i32;
    fn SDL_CreateWindow(title: *const std::ffi::c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut std::ffi::c_void;
    fn SDL_GL_CreateContext(window: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn SDL_GL_SetAttribute(attr: i32, value: i32) -> i32;
    fn SDL_GL_SwapWindow(window: *mut std::ffi::c_void);
    fn SDL_PollEvent(event: *mut SDL_Event) -> i32;
    fn SDL_Quit();
    fn SDL_DestroyWindow(window: *mut std::ffi::c_void);
    fn SDL_GL_DeleteContext(context: *mut std::ffi::c_void);
    
    // OpenGL ES functions
    fn glClear(mask: u32);
    fn glClearColor(red: f32, green: f32, blue: f32, alpha: f32);
    fn glViewport(x: i32, y: i32, width: i32, height: i32);
    fn glFlush();
    fn glCreateShader(shader_type: u32) -> u32;
    fn glShaderSource(shader: u32, count: i32, string: *const *const i8, length: *const i32);
    fn glCompileShader(shader: u32);
    fn glCreateProgram() -> u32;
    fn glAttachShader(program: u32, shader: u32);
    fn glLinkProgram(program: u32);
    fn glUseProgram(program: u32);
    fn glGenBuffers(n: i32, buffers: *mut u32);
    fn glBindBuffer(target: u32, buffer: u32);
    fn glBufferData(target: u32, size: isize, data: *const std::ffi::c_void, usage: u32);
    fn glGetAttribLocation(program: u32, name: *const i8) -> i32;
    fn glEnableVertexAttribArray(index: u32);
    fn glVertexAttribPointer(index: u32, size: i32, type_: u32, normalized: u8, stride: i32, pointer: *const std::ffi::c_void);
    fn glDrawArrays(mode: u32, first: i32, count: i32);
    fn glGetUniformLocation(program: u32, name: *const i8) -> i32;
    fn glUniform3f(location: i32, v0: f32, v1: f32, v2: f32);
    fn glUniform1f(location: i32, v0: f32);
    fn glGetShaderiv(shader: u32, pname: u32, params: *mut i32);
    fn glGetProgramiv(program: u32, pname: u32, params: *mut i32);
    fn glGetShaderInfoLog(shader: u32, bufSize: i32, length: *mut i32, infoLog: *mut i8);
    fn glGetProgramInfoLog(program: u32, bufSize: i32, length: *mut i32, infoLog: *mut i8);
    fn glGetError() -> u32;
}

// SDL2 constants
const SDL_INIT_VIDEO: u32 = 0x00000020;
const SDL_WINDOW_OPENGL: u32 = 0x00000002;
const SDL_WINDOWPOS_CENTERED: i32 = 0x2FFF0000;

// SDL2 GL attributes
const SDL_GL_CONTEXT_MAJOR_VERSION: i32 = 17;
const SDL_GL_CONTEXT_MINOR_VERSION: i32 = 18;
const SDL_GL_CONTEXT_PROFILE_MASK: i32 = 21;
const SDL_GL_CONTEXT_PROFILE_ES: i32 = 4;

// OpenGL constants
const GL_COLOR_BUFFER_BIT: u32 = 0x00004000;
const GL_VERTEX_SHADER: u32 = 0x8B31;
const GL_FRAGMENT_SHADER: u32 = 0x8B30;
const GL_ARRAY_BUFFER: u32 = 0x8892;
const GL_STATIC_DRAW: u32 = 0x88E4;
const GL_TRIANGLES: u32 = 0x0004;
const GL_LINES: u32 = 0x0001;
const GL_FLOAT: u32 = 0x1406;
const GL_COMPILE_STATUS: u32 = 0x8B81;
const GL_LINK_STATUS: u32 = 0x8B82;
const GL_NO_ERROR: u32 = 0;

// SDL Event structure (simplified)
#[repr(C)]
#[allow(non_camel_case_types)]
struct SDL_Event {
    type_: u32,
    padding: [u8; 52], // Simplified event structure
}

const SDL_QUIT: u32 = 0x100;

pub fn run_opengl_test() -> Result<(), String> {
    println!("Starting OpenGL ES test for Raspberry Pi Dashboard...");
    
    unsafe {
        // Initialize SDL2
        if SDL_Init(SDL_INIT_VIDEO) < 0 {
            return Err("Failed to initialize SDL2".to_string());
        }
        
        // Set OpenGL ES attributes
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 2);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 0);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_ES);
        
        // Create window (dashboard size - adjust as needed)
        let title = CString::new("Niva Dashboard - OpenGL Test").unwrap();
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            800,  // Width - adjust for your display
            480,  // Height - adjust for your display
            SDL_WINDOW_OPENGL
        );
        
        if window.is_null() {
            SDL_Quit();
            return Err("Failed to create SDL2 window".to_string());
        }
        
        // Create OpenGL context
        let gl_context = SDL_GL_CreateContext(window);
        if gl_context.is_null() {
            SDL_DestroyWindow(window);
            SDL_Quit();
            return Err("Failed to create OpenGL context".to_string());
        }
        
        // Set viewport
        glViewport(0, 0, 800, 480);
        
        println!("OpenGL ES context created successfully!");
        println!("Setting up simple triangle rendering...");
        
        // Create and compile shaders
        println!("Creating shader program...");
        let shader_program = create_shader_program()?;
        println!("Shader program created with ID: {}", shader_program);
        if shader_program == 0 {
            SDL_GL_DeleteContext(gl_context);
            SDL_DestroyWindow(window);
            SDL_Quit();
            return Err("Failed to create shader program".to_string());
        }
        
        // Create triangle vertices
        let mut vbo = 0u32;
        glGenBuffers(1, &mut vbo);
        glBindBuffer(GL_ARRAY_BUFFER, vbo);
        
        // Triangle vertices (x, y, r, g, b)
        let vertices: [f32; 15] = [
            // Dashboard-style triangle (like a speed indicator)
             0.0,  0.6, 1.0, 0.0, 0.0,  // Top vertex - red
            -0.5, -0.3, 0.0, 1.0, 0.0,  // Bottom left - green  
             0.5, -0.3, 0.0, 0.0, 1.0,  // Bottom right - blue
        ];
        
        glBufferData(
            GL_ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const std::ffi::c_void,
            GL_STATIC_DRAW,
        );
        
        // Get attribute locations
        let pos_attr = glGetAttribLocation(shader_program, b"position\0".as_ptr() as *const i8);
        let color_attr = glGetAttribLocation(shader_program, b"color\0".as_ptr() as *const i8);
        
        println!("Shader attribute locations - position: {}, color: {}", pos_attr, color_attr);
        
        if pos_attr == -1 || color_attr == -1 {
            SDL_GL_DeleteContext(gl_context);
            SDL_DestroyWindow(window);
            SDL_Quit();
            return Err("Failed to get shader attribute locations".to_string());
        }
        
        println!("Running dashboard visualization test...");
        
        // Main render loop
        let mut running = true;
        let mut frame_count = 0;
        
        while running {
            // Poll events
            let mut event = SDL_Event { type_: 0, padding: [0; 52] };
            while SDL_PollEvent(&mut event) != 0 {
                if event.type_ == SDL_QUIT {
                    running = false;
                }
            }
            
            // Render test graphics
            render_dashboard_frame(frame_count, shader_program, vbo, pos_attr, color_attr);
            
            // Swap buffers
            SDL_GL_SwapWindow(window);
            
            frame_count += 1;
            
            // Run for 300 frames (~5 seconds at 60fps)
            if frame_count > 300 {
                running = false;
            }
            
            // Simple frame rate control
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
        // Cleanup
        SDL_GL_DeleteContext(gl_context);
        SDL_DestroyWindow(window);
        SDL_Quit();
        
        println!("OpenGL ES test completed successfully!");
    }
    
    Ok(())
}

unsafe fn render_test_frame(frame: i32) {
    // Clear screen with dark blue background (dashboard-like)
    glClearColor(0.1, 0.1, 0.2, 1.0);
    glClear(GL_COLOR_BUFFER_BIT);
    
    // Calculate animation values
    let time = frame as f32 * 0.02;
    let _pulse = (time.sin() + 1.0) * 0.5; // 0.0 to 1.0
    
    // Note: This is a minimal OpenGL ES test that just clears the screen
    // To draw actual geometry in OpenGL ES, we would need:
    // 1. Vertex shaders and fragment shaders
    // 2. Vertex buffer objects (VBOs)
    // 3. Vertex array objects (VAOs)
    // 4. Proper attribute binding
    // This simplified version just demonstrates the SDL2/OpenGL ES context creation
    
    println!("Frame {} - OpenGL ES context is working! Background color cycles...", frame);
    
    glFlush();
}

unsafe fn create_shader_program() -> Result<u32, String> {
    println!("Creating shaders...");
    
    // Vertex shader source - simplified
    let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
varying vec3 v_color;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    v_color = color;
}
\0";
    
    // Fragment shader source - simplified  
    let fragment_shader_source = b"
precision mediump float;
varying vec3 v_color;

void main() {
    gl_FragColor = vec4(v_color, 1.0);
}
\0";
    
    // Create and compile vertex shader
    println!("Creating vertex shader...");
    let vertex_shader = glCreateShader(GL_VERTEX_SHADER);
    if vertex_shader == 0 {
        return Err("Failed to create vertex shader".to_string());
    }
    
    let vertex_src_ptr = vertex_shader_source.as_ptr() as *const i8;
    glShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
    glCompileShader(vertex_shader);
    
    // Check vertex shader compilation
    let mut compile_status = 0i32;
    glGetShaderiv(vertex_shader, GL_COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Vertex shader compilation failed".to_string());
    }
    println!("Vertex shader compiled successfully");
    
    // Create and compile fragment shader
    println!("Creating fragment shader...");
    let fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
    if fragment_shader == 0 {
        return Err("Failed to create fragment shader".to_string());
    }
    
    let fragment_src_ptr = fragment_shader_source.as_ptr() as *const i8;
    glShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
    glCompileShader(fragment_shader);
    
    // Check fragment shader compilation
    let mut compile_status = 0i32;
    glGetShaderiv(fragment_shader, GL_COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Fragment shader compilation failed".to_string());
    }
    println!("Fragment shader compiled successfully");
    
    // Create shader program
    println!("Creating shader program...");
    let program = glCreateProgram();
    if program == 0 {
        return Err("Failed to create shader program".to_string());
    }
    
    glAttachShader(program, vertex_shader);
    glAttachShader(program, fragment_shader);
    glLinkProgram(program);
    
    // Check program linking
    let mut link_status = 0i32;
    glGetProgramiv(program, GL_LINK_STATUS, &mut link_status);
    if link_status == 0 {
        return Err("Shader program linking failed".to_string());
    }
    
    println!("Shader program created and linked successfully!");
    Ok(program)
}

unsafe fn render_dashboard_frame(frame: i32, shader_program: u32, vbo: u32, pos_attr: i32, color_attr: i32) {
    // Clear screen with dark dashboard background
    glClearColor(0.05, 0.05, 0.15, 1.0);
    glClear(GL_COLOR_BUFFER_BIT);
    
    // Use our shader program
    glUseProgram(shader_program);
    
    // Bind the vertex buffer - THIS WAS THE MISSING PIECE!
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Setup vertex attributes
    glEnableVertexAttribArray(pos_attr as u32);
    glVertexAttribPointer(
        pos_attr as u32,
        2,  // 2 components (x, y)
        GL_FLOAT,
        0,  // not normalized
        5 * std::mem::size_of::<f32>() as i32,  // stride
        std::ptr::null(),
    );
    
    glEnableVertexAttribArray(color_attr as u32);
    glVertexAttribPointer(
        color_attr as u32,
        3,  // 3 components (r, g, b)
        GL_FLOAT,
        0,  // not normalized
        5 * std::mem::size_of::<f32>() as i32,  // stride
        (2 * std::mem::size_of::<f32>()) as *const std::ffi::c_void,  // offset
    );
    
    // Draw the triangle
    glDrawArrays(GL_TRIANGLES, 0, 3);
    
    // Print status every 30 frames to avoid spam
    if frame % 30 == 0 {
        println!("Frame {} - Dashboard triangle rendered! Static triangle with RGB vertices...", frame);
    }
    
    glFlush();
}

// Dashboard test functions for automotive-style gauges and animations

/// Run dashboard gauge test with multiple indicators
pub fn run_dashboard_gauges_test() -> Result<(), String> {
    println!("Starting Multi-Gauge Dashboard Test...");
    
    unsafe {
        // Initialize SDL2 and OpenGL context
        if SDL_Init(SDL_INIT_VIDEO) < 0 {
            return Err("Failed to initialize SDL2".to_string());
        }
        
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 2);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 0);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_ES);
        
        let title = CString::new("Niva Dashboard - Multi-Gauge Test").unwrap();
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            800, 480,
            SDL_WINDOW_OPENGL
        );
        
        if window.is_null() {
            SDL_Quit();
            return Err("Failed to create SDL2 window".to_string());
        }
        
        let gl_context = SDL_GL_CreateContext(window);
        if gl_context.is_null() {
            SDL_DestroyWindow(window);
            SDL_Quit();
            return Err("Failed to create OpenGL context".to_string());
        }
        
        glViewport(0, 0, 800, 480);
        
        println!("Creating dashboard gauges shader program...");
        let shader_program = create_dashboard_shader_program()?;
        
        // Create geometry for multiple gauge types
        let (speedometer_vbo, rpm_vbo, fuel_vbo, temp_vbo) = create_dashboard_geometry()?;
        
        let pos_attr = glGetAttribLocation(shader_program, b"position\0".as_ptr() as *const i8);
        let color_attr = glGetAttribLocation(shader_program, b"color\0".as_ptr() as *const i8);
        let time_uniform = glGetUniformLocation(shader_program, b"time\0".as_ptr() as *const i8);
        
        if pos_attr == -1 || color_attr == -1 {
            return Err("Failed to get dashboard shader attributes".to_string());
        }
        
        println!("Running multi-gauge dashboard animation...");
        
        let mut running = true;
        let mut frame_count = 0;
        
        while running {
            let mut event = SDL_Event { type_: 0, padding: [0; 52] };
            while SDL_PollEvent(&mut event) != 0 {
                if event.type_ == SDL_QUIT {
                    running = false;
                }
            }
            
            render_dashboard_gauges_frame(frame_count, shader_program, speedometer_vbo, rpm_vbo, 
                                        fuel_vbo, temp_vbo, pos_attr, color_attr, time_uniform);
            
            SDL_GL_SwapWindow(window);
            frame_count += 1;
            
            // Run for 480 frames (~8 seconds)
            if frame_count > 480 {
                running = false;
            }
            
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        
        SDL_GL_DeleteContext(gl_context);
        SDL_DestroyWindow(window);
        SDL_Quit();
        
        println!("Dashboard gauges test completed successfully!");
    }
    
    Ok(())
}

/// Simple moving needle test - sweeps from 8 o'clock to 4 o'clock over 4 seconds
pub fn run_moving_needle_test() -> Result<(), String> {
    println!("Starting Simple Moving Needle Test...");
    
    unsafe {
        // Initialize SDL2 and OpenGL context
        if SDL_Init(SDL_INIT_VIDEO) < 0 {
            return Err("Failed to initialize SDL2".to_string());
        }
        
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 2);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 0);
        SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_ES);
        
        let title = CString::new("Niva Dashboard - Moving Needle Test").unwrap();
        let window = SDL_CreateWindow(
            title.as_ptr(),
            SDL_WINDOWPOS_CENTERED,
            SDL_WINDOWPOS_CENTERED,
            800, 480,
            SDL_WINDOW_OPENGL
        );
        
        if window.is_null() {
            SDL_Quit();
            return Err("Failed to create SDL2 window".to_string());
        }
        
        let gl_context = SDL_GL_CreateContext(window);
        if gl_context.is_null() {
            SDL_DestroyWindow(window);
            SDL_Quit();
            return Err("Failed to create OpenGL context".to_string());
        }
        
        glViewport(0, 0, 800, 480);
        
        println!("Creating simple needle shader program...");
        let shader_program = create_simple_needle_shader_program()?;
        
        // Create needle geometry
        let needle_vbo = create_simple_needle_geometry()?;
        
        // Get shader attribute and uniform locations
        let pos_attr = glGetAttribLocation(shader_program, b"position\0".as_ptr() as *const i8);
        let color_attr = glGetAttribLocation(shader_program, b"color\0".as_ptr() as *const i8);
        let angle_uniform = glGetUniformLocation(shader_program, b"angle\0".as_ptr() as *const i8);
        
        if pos_attr == -1 || color_attr == -1 {
            return Err("Failed to get needle shader attributes".to_string());
        }
        
        println!("Running needle animation - 8 o'clock to 4 o'clock sweep...");
        
        let mut running = true;
        let mut frame_count = 0;
        let total_frames = 240; // 4 seconds at 60fps
        
        while running && frame_count < total_frames {
            let mut event = SDL_Event { type_: 0, padding: [0; 52] };
            while SDL_PollEvent(&mut event) != 0 {
                if event.type_ == SDL_QUIT {
                    running = false;
                }
            }
            
            render_simple_needle_frame(frame_count, total_frames, shader_program, needle_vbo, 
                                     pos_attr, color_attr, angle_uniform);
            
            SDL_GL_SwapWindow(window);
            frame_count += 1;
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
        SDL_GL_DeleteContext(gl_context);
        SDL_DestroyWindow(window);
        SDL_Quit();
        
        println!("Simple needle test completed successfully!");
    }
    
    Ok(())
}

unsafe fn create_simple_needle_shader_program() -> Result<u32, String> {
    let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
varying vec3 v_color;
uniform float angle;

void main() {
    // Apply rotation matrix to needle
    float cos_a = cos(angle);
    float sin_a = sin(angle);
    
    vec2 rotated_pos = vec2(
        position.x * cos_a - position.y * sin_a,
        position.x * sin_a + position.y * cos_a
    );
    
    gl_Position = vec4(rotated_pos, 0.0, 1.0);
    v_color = color;
}
\0";
    
    let fragment_shader_source = b"
precision mediump float;
varying vec3 v_color;

void main() {
    gl_FragColor = vec4(v_color, 1.0);
}
\0";
    
    // Create and compile vertex shader
    let vertex_shader = glCreateShader(GL_VERTEX_SHADER);
    if vertex_shader == 0 {
        return Err("Failed to create needle vertex shader".to_string());
    }
    
    let vertex_src_ptr = vertex_shader_source.as_ptr() as *const i8;
    glShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
    glCompileShader(vertex_shader);
    
    // Check vertex shader compilation
    let mut compile_status = 0i32;
    glGetShaderiv(vertex_shader, GL_COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Needle vertex shader compilation failed".to_string());
    }
    
    // Create and compile fragment shader
    let fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
    if fragment_shader == 0 {
        return Err("Failed to create needle fragment shader".to_string());
    }
    
    let fragment_src_ptr = fragment_shader_source.as_ptr() as *const i8;
    glShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
    glCompileShader(fragment_shader);
    
    // Check fragment shader compilation
    let mut compile_status = 0i32;
    glGetShaderiv(fragment_shader, GL_COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Needle fragment shader compilation failed".to_string());
    }
    
    // Create and link shader program
    let program = glCreateProgram();
    if program == 0 {
        return Err("Failed to create needle shader program".to_string());
    }
    
    glAttachShader(program, vertex_shader);
    glAttachShader(program, fragment_shader);
    glLinkProgram(program);
    
    // Check program linking
    let mut link_status = 0i32;
    glGetProgramiv(program, GL_LINK_STATUS, &mut link_status);
    if link_status == 0 {
        return Err("Needle shader program linking failed".to_string());
    }
    
    println!("Simple needle shader program created successfully!");
    Ok(program)
}

unsafe fn create_simple_needle_geometry() -> Result<u32, String> {
    let mut needle_vbo = 0u32;
    glGenBuffers(1, &mut needle_vbo);
    glBindBuffer(GL_ARRAY_BUFFER, needle_vbo);
    
    // Simple needle: line from center to edge, starts pointing up (12 o'clock)
    let needle_vertices: [f32; 10] = [
        // Center point
        0.0, 0.0, 1.0, 1.0, 1.0,     // White center
        // Needle tip pointing up initially (will be rotated)
        0.0, 0.3, 1.0, 0.0, 0.0,     // Red tip
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (needle_vertices.len() * std::mem::size_of::<f32>()) as isize,
                needle_vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(needle_vbo)
}

unsafe fn render_simple_needle_frame(frame: i32, total_frames: i32, shader_program: u32, needle_vbo: u32, 
                                   pos_attr: i32, color_attr: i32, angle_uniform: i32) {
    // Clear screen with dark dashboard background
    glClearColor(0.02, 0.02, 0.08, 1.0);
    glClear(GL_COLOR_BUFFER_BIT);
    
    glUseProgram(shader_program);
    
    // Calculate needle angle: 8 o'clock to 4 o'clock sweep
    let progress = frame as f32 / total_frames as f32; // 0.0 to 1.0
    let cycle_progress = (progress * std::f32::consts::PI).sin().abs(); // Smooth back and forth
    
    // 8 o'clock = -150 degrees, 4 o'clock = -30 degrees (clockwise from 12)
    let start_angle = -150.0 * std::f32::consts::PI / 180.0; // 8 o'clock in radians
    let end_angle = -30.0 * std::f32::consts::PI / 180.0;    // 4 o'clock in radians
    let current_angle = start_angle + (end_angle - start_angle) * cycle_progress;
    
    // Set angle uniform
    glUniform1f(angle_uniform, current_angle);
    
    // Render needle
    glBindBuffer(GL_ARRAY_BUFFER, needle_vbo);
    
    // Setup vertex attributes
    glEnableVertexAttribArray(pos_attr as u32);
    glVertexAttribPointer(
        pos_attr as u32, 2, GL_FLOAT, 0,
        5 * std::mem::size_of::<f32>() as i32,
        std::ptr::null(),
    );
    
    glEnableVertexAttribArray(color_attr as u32);
    glVertexAttribPointer(
        color_attr as u32, 3, GL_FLOAT, 0,
        5 * std::mem::size_of::<f32>() as i32,
        (2 * std::mem::size_of::<f32>()) as *const std::ffi::c_void,
    );
    
    // Draw needle as a line
    glDrawArrays(GL_LINES, 0, 2);
    
    // Print progress every 30 frames
    if frame % 30 == 0 {
        let angle_degrees = current_angle * 180.0 / std::f32::consts::PI;
        println!("Frame {}/{} - Needle at {:.1}° (progress: {:.1}%)", 
                frame, total_frames, angle_degrees, cycle_progress * 100.0);
    }
    
    glFlush();
}

unsafe fn create_dashboard_shader_program() -> Result<u32, String> {
    let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
varying vec3 v_color;
uniform float time;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    v_color = color;
}
\0";
    
    let fragment_shader_source = b"
precision mediump float;
varying vec3 v_color;
uniform float time;

void main() {
    // Pulsing effect for dashboard elements
    float pulse = 0.8 + 0.2 * sin(time * 1.5);
    gl_FragColor = vec4(v_color * pulse, 1.0);
}
\0";
    
    let vertex_shader = glCreateShader(GL_VERTEX_SHADER);
    let vertex_src_ptr = vertex_shader_source.as_ptr() as *const i8;
    glShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
    glCompileShader(vertex_shader);
    
    let fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
    let fragment_src_ptr = fragment_shader_source.as_ptr() as *const i8;
    glShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
    glCompileShader(fragment_shader);
    
    let program = glCreateProgram();
    glAttachShader(program, vertex_shader);
    glAttachShader(program, fragment_shader);
    glLinkProgram(program);
    
    Ok(program)
}

unsafe fn create_dashboard_geometry() -> Result<(u32, u32, u32, u32), String> {
    // Speedometer (top-left)
    let mut speedometer_vbo = 0u32;
    glGenBuffers(1, &mut speedometer_vbo);
    glBindBuffer(GL_ARRAY_BUFFER, speedometer_vbo);
    
    let speedometer_vertices: [f32; 15] = [
        -0.7, 0.3, 0.0, 0.8, 1.0,    // Cyan gauge
        -0.5, 0.5, 0.0, 0.8, 1.0,
        -0.3, 0.3, 0.0, 0.8, 1.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER,
                (speedometer_vertices.len() * std::mem::size_of::<f32>()) as isize,
                speedometer_vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    // RPM gauge (top-right)
    let mut rpm_vbo = 0u32;
    glGenBuffers(1, &mut rpm_vbo);
    glBindBuffer(GL_ARRAY_BUFFER, rpm_vbo);
    
    let rpm_vertices: [f32; 15] = [
        0.3, 0.3, 1.0, 0.3, 0.0,     // Orange RPM gauge
        0.5, 0.5, 1.0, 0.3, 0.0,
        0.7, 0.3, 1.0, 0.3, 0.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER,
                (rpm_vertices.len() * std::mem::size_of::<f32>()) as isize,
                rpm_vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    // Fuel gauge (bottom-left)
    let mut fuel_vbo = 0u32;
    glGenBuffers(1, &mut fuel_vbo);
    glBindBuffer(GL_ARRAY_BUFFER, fuel_vbo);
    
    let fuel_vertices: [f32; 15] = [
        -0.7, -0.3, 0.0, 1.0, 0.0,   // Green fuel gauge
        -0.5, -0.1, 0.0, 1.0, 0.0,
        -0.3, -0.3, 0.0, 1.0, 0.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER,
                (fuel_vertices.len() * std::mem::size_of::<f32>()) as isize,
                fuel_vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    // Temperature gauge (bottom-right)
    let mut temp_vbo = 0u32;
    glGenBuffers(1, &mut temp_vbo);
    glBindBuffer(GL_ARRAY_BUFFER, temp_vbo);
    
    let temp_vertices: [f32; 15] = [
        0.3, -0.3, 1.0, 0.0, 0.0,    // Red temperature gauge
        0.5, -0.1, 1.0, 0.0, 0.0,
        0.7, -0.3, 1.0, 0.0, 0.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER,
                (temp_vertices.len() * std::mem::size_of::<f32>()) as isize,
                temp_vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok((speedometer_vbo, rpm_vbo, fuel_vbo, temp_vbo))
}

unsafe fn render_dashboard_gauges_frame(frame: i32, shader_program: u32, speedometer_vbo: u32, 
                                      rpm_vbo: u32, fuel_vbo: u32, temp_vbo: u32, 
                                      pos_attr: i32, color_attr: i32, time_uniform: i32) {
    glClearColor(0.05, 0.05, 0.1, 1.0);   // Dark dashboard background
    glClear(GL_COLOR_BUFFER_BIT);
    
    glUseProgram(shader_program);
    
    let time = frame as f32 * 0.03;
    glUniform1f(time_uniform, time);
    
    // Render speedometer (top-left)
    glBindBuffer(GL_ARRAY_BUFFER, speedometer_vbo);
    setup_vertex_attributes(pos_attr, color_attr);
    glDrawArrays(GL_TRIANGLES, 0, 3);
    
    // Render RPM gauge (top-right)  
    glBindBuffer(GL_ARRAY_BUFFER, rpm_vbo);
    setup_vertex_attributes(pos_attr, color_attr);
    glDrawArrays(GL_TRIANGLES, 0, 3);
    
    // Render fuel gauge (bottom-left)
    glBindBuffer(GL_ARRAY_BUFFER, fuel_vbo);
    setup_vertex_attributes(pos_attr, color_attr);
    glDrawArrays(GL_TRIANGLES, 0, 3);
    
    // Render temperature gauge (bottom-right)
    glBindBuffer(GL_ARRAY_BUFFER, temp_vbo);
    setup_vertex_attributes(pos_attr, color_attr);
    glDrawArrays(GL_TRIANGLES, 0, 3);
    
    if frame % 40 == 0 {
        println!("Frame {} - Dashboard: all gauges rendered with pulsing effect", frame);
    }
    
    glFlush();
}

unsafe fn setup_vertex_attributes(pos_attr: i32, color_attr: i32) {
    glEnableVertexAttribArray(pos_attr as u32);
    glVertexAttribPointer(
        pos_attr as u32, 2, GL_FLOAT, 0,
        5 * std::mem::size_of::<f32>() as i32,
        std::ptr::null(),
    );
    
    glEnableVertexAttribArray(color_attr as u32);
    glVertexAttribPointer(
        color_attr as u32, 3, GL_FLOAT, 0,
        5 * std::mem::size_of::<f32>() as i32,
        (2 * std::mem::size_of::<f32>()) as *const std::ffi::c_void,
    );
}
