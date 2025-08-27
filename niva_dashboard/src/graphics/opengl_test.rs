#![allow(unused)]
use std::ffi::CString;
use crate::graphics::context::GraphicsContext;

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
    
    // Additional OpenGL functions for antialiasing and blending
    fn glEnable(cap: u32);
    fn glDisable(cap: u32);
    fn glBlendFunc(sfactor: u32, dfactor: u32);
    fn glLineWidth(width: f32);
    fn glUniformMatrix4fv(location: i32, count: i32, transpose: u8, value: *const f32);
    fn glUniform2f(location: i32, v0: f32, v1: f32);
    fn glUniform4f(location: i32, v0: f32, v1: f32, v2: f32, v3: f32);
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

// Additional constants for antialiasing and blending
const GL_BLEND: u32 = 0x0BE2;
const GL_SRC_ALPHA: u32 = 0x0302;
const GL_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
const GL_LINE_SMOOTH: u32 = 0x0B20;
const GL_POLYGON_SMOOTH: u32 = 0x0B41;
const GL_TRIANGLE_STRIP: u32 = 0x0005;
const GL_TRIANGLE_FAN: u32 = 0x0006;

// SDL Event structure (simplified)
#[repr(C)]
#[allow(non_camel_case_types)]
struct SDL_Event {
    type_: u32,
    padding: [u8; 52], // Simplified event structure
}

const SDL_QUIT: u32 = 0x100;

pub fn run_opengl_test(context: &GraphicsContext) -> Result<(), String> {
    println!("Starting OpenGL ES test for Raspberry Pi Dashboard...");
    
    unsafe {
        // Set viewport
        glViewport(0, 0, context.width, context.height);
        
        println!("OpenGL ES context created successfully!");
        println!("Setting up simple triangle rendering...");
        
        // Create and compile shaders
        println!("Creating shader program...");
        let shader_program = create_shader_program()?;
        println!("Shader program created with ID: {}", shader_program);
        if shader_program == 0 {
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
            return Err("Failed to get shader attribute locations".to_string());
        }
        
        println!("Running dashboard visualization test...");
        
        // Main render loop
        let mut frame_count = 0;
        
        while frame_count < 300 {
            // Check for quit events
            if context.should_quit() {
                break;
            }
            
            // Render test graphics
            render_dashboard_frame(frame_count, shader_program, vbo, pos_attr, color_attr);
            
            // Swap buffers
            context.swap_buffers();
            
            frame_count += 1;
            
            // Simple frame rate control
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
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
pub fn run_dashboard_gauges_test(context: &GraphicsContext) -> Result<(), String> {
    println!("Starting Multi-Gauge Dashboard Test...");
    
    unsafe {
        glViewport(0, 0, context.width, context.height);
        
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
        
        let mut frame_count = 0;
        
        while frame_count < 480 {
            // Check for quit events
            if context.should_quit() {
                break;
            }
            
            render_dashboard_gauges_frame(frame_count, shader_program, speedometer_vbo, rpm_vbo, 
                                        fuel_vbo, temp_vbo, pos_attr, color_attr, time_uniform);
            
            context.swap_buffers();
            frame_count += 1;
            
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        
        println!("Dashboard gauges test completed successfully!");
    }
    
    Ok(())
}

/// Simple moving needle test - sweeps from 8 o'clock to 4 o'clock over 4 seconds
pub fn run_moving_needle_test(context: &GraphicsContext) -> Result<(), String> {
    println!("Starting Simple Moving Needle Test...");
    
    unsafe {
        glViewport(0, 0, context.width, context.height);
        
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
        
        let mut frame_count = 0;
        let total_frames = 240; // 4 seconds at 60fps
        
        while frame_count < total_frames {
            // Check for quit events
            if context.should_quit() {
                break;
            }
            
            render_simple_needle_frame(frame_count, total_frames, shader_program, needle_vbo, 
                                     pos_attr, color_attr, angle_uniform);
            
            context.swap_buffers();
            frame_count += 1;
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
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

// Text rendering test using SDL2 TTF
pub fn run_text_rendering_test(context: &GraphicsContext) -> Result<(), String> {
    // Import SDL2 TTF functionality
    use sdl2::pixels::Color;
    use sdl2::rect::Rect;
    use sdl2::render::TextureQuery;
    use sdl2::ttf::FontStyle;
    
    println!("Starting Text Rendering Test with SDL2 TTF...");
    println!("Note: This test creates its own SDL2 canvas for TTF rendering");
    
    // Initialize SDL2 and TTF (separate from the OpenGL context)
    let sdl_context = sdl2::init().map_err(|e| e.to_string())?;
    let video_subsystem = sdl_context.video().map_err(|e| e.to_string())?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;
    
    println!("SDL2 TTF version: {}", sdl2::ttf::get_linked_version());
    
    // Create window for text rendering (separate from OpenGL window)
    let window = video_subsystem
        .window("Niva Dashboard - Text Rendering Test", 800, 480)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;
    
    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    
    // Load built-in fonts or use system fonts
    // For this demo, we'll try to load common system fonts
    let font_paths = vec![
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/System/Library/Fonts/Arial.ttf", // macOS
        "/System/Library/Fonts/Courier.ttc", // macOS
        "C:/Windows/Fonts/arial.ttf", // Windows
        "C:/Windows/Fonts/cour.ttf", // Windows
    ];
    
    // Find available fonts
    let mut fonts = Vec::new();
    for path in &font_paths {
        if std::path::Path::new(path).exists() {
            match ttf_context.load_font(path, 24) {
                Ok(font) => {
                    fonts.push((font, path.to_string()));
                    if fonts.len() >= 2 { break; } // We only need 2 fonts
                }
                Err(_) => continue,
            }
        }
    }
    
    // If no system fonts found, create embedded font data (fallback)
    if fonts.is_empty() {
        println!("No system fonts found. Using fallback text rendering...");
        return run_fallback_text_test();
    }
    
    println!("Found {} fonts to test with", fonts.len());
    
    // Create different sized versions of the fonts
    let font_sizes = vec![16, 24, 32, 48, 64];
    let mut all_fonts = Vec::new();
    
    for (_, font_path) in &fonts {
        for &size in &font_sizes {
            match ttf_context.load_font(font_path, size) {
                Ok(mut font) => {
                    // Set different styles for variety
                    match size {
                        48 => font.set_style(FontStyle::BOLD),
                        64 => font.set_style(FontStyle::BOLD | FontStyle::ITALIC),
                        _ => font.set_style(FontStyle::NORMAL),
                    }
                    all_fonts.push((font, size, font_path.clone()));
                }
                Err(e) => println!("Failed to load font {} at size {}: {}", font_path, size, e),
            }
        }
    }
    
    // Test texts for dashboard
    let test_texts = vec![
        ("НИВА ПАНЕЛЬ", Color::RGB(255, 255, 255)),
        ("Speed: 85 km/h", Color::RGB(0, 255, 0)),
        ("RPM: 3500", Color::RGB(255, 165, 0)),
        ("Fuel: 75%", Color::RGB(255, 255, 0)),
        ("Temp: 89°C", Color::RGB(255, 100, 100)),
        ("ENGINE OK", Color::RGB(0, 255, 0)),
        ("12:34 PM", Color::RGB(100, 200, 255)),
        ("GPS: ACTIVE", Color::RGB(150, 255, 150)),
    ];
    
    // Main rendering loop
    let mut event_pump = sdl_context.event_pump().map_err(|e| e.to_string())?;
    let mut frame_count = 0;
    let total_frames = 300; // 5 seconds at 60fps
    
    'running: loop {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } |
                sdl2::event::Event::KeyDown { keycode: Some(sdl2::keyboard::Keycode::Escape), .. } => {
                    break 'running;
                }
                _ => {}
            }
        }
        
        // Clear screen with dark dashboard background
        canvas.set_draw_color(Color::RGB(5, 5, 15));
        canvas.clear();
        
        let mut y_offset = 10;
        let mut font_index = 0;
        
        // Render each test text with different fonts and sizes
        for (text, color) in &test_texts {
            if font_index < all_fonts.len() {
                let (ref font, size, ref font_name) = all_fonts[font_index];
                
                // Render text to surface
                match font.render(text).blended(*color) {
                    Ok(surface) => {
                        match texture_creator.create_texture_from_surface(&surface) {
                            Ok(texture) => {
                                let TextureQuery { width, height, .. } = texture.query();
                                
                                // Position text
                                let dest_rect = Rect::new(20, y_offset, width, height);
                                
                                // Draw the text
                                if let Err(e) = canvas.copy(&texture, None, Some(dest_rect)) {
                                    println!("Failed to copy texture: {}", e);
                                }
                                
                                // Show font info occasionally
                                if frame_count % 60 == 0 {
                                    println!("Rendered '{}' with {} (size {})", text, font_name, size);
                                }
                                
                                y_offset += height as i32 + 5;
                            }
                            Err(e) => println!("Failed to create texture: {}", e),
                        }
                    }
                    Err(e) => println!("Failed to render text '{}': {}", text, e),
                }
                
                font_index = (font_index + 1) % all_fonts.len();
            }
        }
        
        // Add animated element - cycling through font sizes
        let anim_font_idx = (frame_count / 30) % all_fonts.len();
        if anim_font_idx < all_fonts.len() {
            let (ref anim_font, anim_size, _) = all_fonts[anim_font_idx];
            let animation_text = format!("Font Size: {}", anim_size);
            
            match anim_font.render(&animation_text).blended(Color::RGB(255, 255, 255)) {
                Ok(surface) => {
                    if let Ok(texture) = texture_creator.create_texture_from_surface(&surface) {
                        let TextureQuery { width, height, .. } = texture.query();
                        let dest_rect = Rect::new(400, 50, width, height);
                        let _ = canvas.copy(&texture, None, Some(dest_rect));
                    }
                }
                Err(_) => {}
            }
        }
        
        canvas.present();
        frame_count += 1;
        
        if frame_count >= total_frames {
            break 'running;
        }
        
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
    }
    
    println!("Text rendering test completed successfully!");
    Ok(())
}

// Fallback text rendering for when no system fonts are available
fn run_fallback_text_test() -> Result<(), String> {
    println!("Running fallback text rendering test...");
    
    // Initialize basic SDL2 for drawing rectangles as "text"
    let sdl_context = sdl2::init().map_err(|e| e.to_string())?;
    let video_subsystem = sdl_context.video().map_err(|e| e.to_string())?;
    
    let window = video_subsystem
        .window("Niva Dashboard - Fallback Text Test", 800, 480)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;
    
    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let mut event_pump = sdl_context.event_pump().map_err(|e| e.to_string())?;
    
    // Simple bitmap-style "text" using rectangles
    let mut frame_count = 0;
    
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } |
                sdl2::event::Event::KeyDown { keycode: Some(sdl2::keyboard::Keycode::Escape), .. } => {
                    break 'running;
                }
                _ => {}
            }
        }
        
        canvas.set_draw_color(sdl2::pixels::Color::RGB(5, 5, 15));
        canvas.clear();
        
        // Draw simple rectangles to represent different "font sizes"
        let sizes = vec![10, 15, 20, 25, 30];
        let colors = vec![
            sdl2::pixels::Color::RGB(255, 255, 255),
            sdl2::pixels::Color::RGB(0, 255, 0),
            sdl2::pixels::Color::RGB(255, 165, 0),
            sdl2::pixels::Color::RGB(255, 255, 0),
            sdl2::pixels::Color::RGB(255, 100, 100),
        ];
        
        for (i, (&size, &color)) in sizes.iter().zip(colors.iter()).enumerate() {
            canvas.set_draw_color(color);
            let y = 50 + i as i32 * 60;
            // Draw a series of rectangles to represent text
            for j in 0..10 {
                let rect = sdl2::rect::Rect::new(50 + j * (size + 5), y, size as u32, size as u32);
                let _ = canvas.fill_rect(rect);
            }
        }
        
        canvas.present();
        frame_count += 1;
        
        if frame_count >= 180 { // 3 seconds
            break 'running;
        }
        
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    
    println!("Fallback text test completed!");
    Ok(())
}

/// Advanced OpenGL rotating needles demo with antialiasing and variable thickness
pub fn run_opengl_rotating_needles_demo(context: &GraphicsContext) -> Result<(), String> {
    println!("Starting OpenGL Rotating Needles Demo with Antialiasing...");
    println!("Resolution: 800x480 - Multiple needles with different sizes and thickness");
    
    unsafe {
        glViewport(0, 0, context.width, context.height);
        
        // Enable antialiasing and blending for smooth needles
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        glEnable(GL_LINE_SMOOTH);
        
        println!("Creating advanced needle shader program...");
        let shader_program = create_advanced_needle_shader_program()?;
        
        // Create different needle geometries with varying thickness
        let needle_configs = create_multiple_needle_geometries()?;
        
        // Get shader locations
        let pos_attr = glGetAttribLocation(shader_program, b"position\0".as_ptr() as *const i8);
        let color_attr = glGetAttribLocation(shader_program, b"color\0".as_ptr() as *const i8);
        let thickness_attr = glGetAttribLocation(shader_program, b"thickness\0".as_ptr() as *const i8);
        let time_uniform = glGetUniformLocation(shader_program, b"time\0".as_ptr() as *const i8);
        let resolution_uniform = glGetUniformLocation(shader_program, b"resolution\0".as_ptr() as *const i8);
        let rotation_uniform = glGetUniformLocation(shader_program, b"rotation\0".as_ptr() as *const i8);
        let center_uniform = glGetUniformLocation(shader_program, b"center\0".as_ptr() as *const i8);
        let scale_uniform = glGetUniformLocation(shader_program, b"scale\0".as_ptr() as *const i8);
        
        if pos_attr == -1 || color_attr == -1 {
            return Err("Failed to get needle shader attributes".to_string());
        }
        
        println!("Running advanced rotating needles animation...");
        println!("Features: 6 needles, variable thickness, antialiasing, smooth rotation");
        
        let mut frame_count = 0;
        let total_frames = 720; // 12 seconds at 60fps
        
        while frame_count < total_frames {
            if context.should_quit() {
                break;
            }
            
            render_rotating_needles_frame(
                frame_count, 
                shader_program, 
                &needle_configs,
                pos_attr, 
                color_attr, 
                thickness_attr,
                time_uniform, 
                resolution_uniform,
                rotation_uniform,
                center_uniform,
                scale_uniform
            );
            
            context.swap_buffers();
            frame_count += 1;
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
        println!("OpenGL rotating needles demo completed successfully!");
    }
    
    Ok(())
}

#[derive(Clone)]
struct NeedleConfig {
    vbo: u32,
    vertex_count: i32,
    position: (f32, f32),    // Center position on screen
    scale: f32,              // Size multiplier
    color: (f32, f32, f32),  // RGB color
    speed: f32,              // Rotation speed multiplier
    thickness_base: f32,     // Base thickness
    draw_mode: u32,          // GL_TRIANGLES, GL_TRIANGLE_STRIP, etc.
}

unsafe fn create_advanced_needle_shader_program() -> Result<u32, String> {
    let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
attribute float thickness;
varying vec3 v_color;
varying float v_thickness;
uniform float time;
uniform vec2 resolution;
uniform float rotation;
uniform vec2 center;
uniform float scale;

void main() {
    // Apply rotation matrix
    float cos_r = cos(rotation);
    float sin_r = sin(rotation);
    
    vec2 rotated_pos = vec2(
        position.x * cos_r - position.y * sin_r,
        position.x * sin_r + position.y * cos_r
    );
    
    // Scale and translate
    vec2 scaled_pos = rotated_pos * scale + center;
    
    // Convert to normalized device coordinates
    vec2 ndc = (scaled_pos / resolution) * 2.0 - 1.0;
    ndc.y = -ndc.y; // Flip Y for screen coordinates
    
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_color = color;
    v_thickness = thickness;
}
\0";
    
    let fragment_shader_source = b"
precision mediump float;
varying vec3 v_color;
varying float v_thickness;
uniform float time;

void main() {
    // Antialiasing effect based on thickness
    float alpha = 1.0;
    
    // Add slight pulsing effect
    float pulse = 0.9 + 0.1 * sin(time * 2.0);
    
    // Color modulation with thickness
    vec3 final_color = v_color * pulse;
    
    // Smooth antialiasing
    alpha = smoothstep(0.0, v_thickness * 0.1, v_thickness);
    
    gl_FragColor = vec4(final_color, alpha);
}
\0";
    
    // Create and compile vertex shader
    let vertex_shader = glCreateShader(GL_VERTEX_SHADER);
    if vertex_shader == 0 {
        return Err("Failed to create advanced needle vertex shader".to_string());
    }
    
    let vertex_src_ptr = vertex_shader_source.as_ptr() as *const i8;
    glShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
    glCompileShader(vertex_shader);
    
    let mut compile_status = 0i32;
    glGetShaderiv(vertex_shader, GL_COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Advanced needle vertex shader compilation failed".to_string());
    }
    
    // Create and compile fragment shader
    let fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
    if fragment_shader == 0 {
        return Err("Failed to create advanced needle fragment shader".to_string());
    }
    
    let fragment_src_ptr = fragment_shader_source.as_ptr() as *const i8;
    glShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
    glCompileShader(fragment_shader);
    
    let mut compile_status = 0i32;
    glGetShaderiv(fragment_shader, GL_COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Advanced needle fragment shader compilation failed".to_string());
    }
    
    // Create and link shader program
    let program = glCreateProgram();
    if program == 0 {
        return Err("Failed to create advanced needle shader program".to_string());
    }
    
    glAttachShader(program, vertex_shader);
    glAttachShader(program, fragment_shader);
    glLinkProgram(program);
    
    let mut link_status = 0i32;
    glGetProgramiv(program, GL_LINK_STATUS, &mut link_status);
    if link_status == 0 {
        return Err("Advanced needle shader program linking failed".to_string());
    }
    
    println!("Advanced needle shader program created successfully!");
    Ok(program)
}

unsafe fn create_multiple_needle_geometries() -> Result<Vec<NeedleConfig>, String> {
    let mut configs = Vec::new();
    
    // Needle 1: Thin speedometer needle (top-left)
    let thin_needle_vbo = create_thin_needle_geometry()?;
    configs.push(NeedleConfig {
        vbo: thin_needle_vbo,
        vertex_count: 6, // Triangle strip for thin needle
        position: (200.0, 120.0),
        scale: 80.0,
        color: (1.0, 1.0, 1.0), // White
        speed: 1.0,
        thickness_base: 1.0,
        draw_mode: GL_TRIANGLE_STRIP,
    });
    
    // Needle 2: Medium RPM needle (top-right)
    let medium_needle_vbo = create_medium_needle_geometry()?;
    configs.push(NeedleConfig {
        vbo: medium_needle_vbo,
        vertex_count: 8, // Triangle strip for medium needle
        position: (600.0, 120.0),
        scale: 90.0,
        color: (1.0, 0.3, 0.0), // Orange
        speed: 1.5,
        thickness_base: 2.0,
        draw_mode: GL_TRIANGLE_STRIP,
    });
    
    // Needle 3: Thick fuel needle (middle-left)
    let thick_needle_vbo = create_thick_needle_geometry()?;
    configs.push(NeedleConfig {
        vbo: thick_needle_vbo,
        vertex_count: 10, // Triangle strip for thick needle
        position: (200.0, 240.0),
        scale: 70.0,
        color: (0.0, 1.0, 0.0), // Green
        speed: 0.8,
        thickness_base: 3.0,
        draw_mode: GL_TRIANGLE_STRIP,
    });
    
    // Needle 4: Temperature needle (middle-right)
    let temp_needle_vbo = create_tapered_needle_geometry()?;
    configs.push(NeedleConfig {
        vbo: temp_needle_vbo,
        vertex_count: 12, // Triangle strip for tapered needle
        position: (600.0, 240.0),
        scale: 85.0,
        color: (1.0, 0.0, 0.0), // Red
        speed: 0.6,
        thickness_base: 2.5,
        draw_mode: GL_TRIANGLE_STRIP,
    });
    
    // Needle 5: Oil pressure needle (bottom-left)
    let oil_needle_vbo = create_wide_needle_geometry()?;
    configs.push(NeedleConfig {
        vbo: oil_needle_vbo,
        vertex_count: 14, // Triangle strip for wide needle
        position: (200.0, 360.0),
        scale: 60.0,
        color: (0.0, 0.0, 1.0), // Blue
        speed: 1.2,
        thickness_base: 4.0,
        draw_mode: GL_TRIANGLE_STRIP,
    });
    
    // Needle 6: Voltage needle (bottom-right)
    let voltage_needle_vbo = create_precision_needle_geometry()?;
    configs.push(NeedleConfig {
        vbo: voltage_needle_vbo,
        vertex_count: 16, // Triangle strip for precision needle
        position: (600.0, 360.0),
        scale: 75.0,
        color: (1.0, 1.0, 0.0), // Yellow
        speed: 2.0,
        thickness_base: 1.5,
        draw_mode: GL_TRIANGLE_STRIP,
    });
    
    println!("Created {} needle configurations with varying thickness and styles", configs.len());
    Ok(configs)
}

// Create different needle geometries with variable thickness

unsafe fn create_thin_needle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    glGenBuffers(1, &mut vbo);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Thin needle: x, y, r, g, b, thickness
    let vertices: [f32; 36] = [
        // Base (wide)
        -0.02,  0.0, 1.0, 1.0, 1.0, 2.0,
         0.02,  0.0, 1.0, 1.0, 1.0, 2.0,
        // Mid point
        -0.01,  0.5, 1.0, 1.0, 1.0, 1.5,
         0.01,  0.5, 1.0, 1.0, 1.0, 1.5,
        // Tip (narrow)
        -0.005, 0.9, 1.0, 1.0, 1.0, 1.0,
         0.005, 0.9, 1.0, 1.0, 1.0, 1.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(vbo)
}

unsafe fn create_medium_needle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    glGenBuffers(1, &mut vbo);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Medium needle with more segments
    let vertices: [f32; 48] = [
        // Base
        -0.03,  0.0, 1.0, 0.3, 0.0, 3.0,
         0.03,  0.0, 1.0, 0.3, 0.0, 3.0,
        // First segment
        -0.025, 0.3, 1.0, 0.3, 0.0, 2.5,
         0.025, 0.3, 1.0, 0.3, 0.0, 2.5,
        // Second segment
        -0.02,  0.6, 1.0, 0.3, 0.0, 2.0,
         0.02,  0.6, 1.0, 0.3, 0.0, 2.0,
        // Tip
        -0.01,  0.85, 1.0, 0.3, 0.0, 1.5,
         0.01,  0.85, 1.0, 0.3, 0.0, 1.5,
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(vbo)
}

unsafe fn create_thick_needle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    glGenBuffers(1, &mut vbo);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Thick needle
    let vertices: [f32; 60] = [
        // Base (very wide)
        -0.05,  0.0, 0.0, 1.0, 0.0, 5.0,
         0.05,  0.0, 0.0, 1.0, 0.0, 5.0,
        // Segment 1
        -0.04,  0.2, 0.0, 1.0, 0.0, 4.0,
         0.04,  0.2, 0.0, 1.0, 0.0, 4.0,
        // Segment 2
        -0.035, 0.4, 0.0, 1.0, 0.0, 3.5,
         0.035, 0.4, 0.0, 1.0, 0.0, 3.5,
        // Segment 3
        -0.025, 0.6, 0.0, 1.0, 0.0, 3.0,
         0.025, 0.6, 0.0, 1.0, 0.0, 3.0,
        // Tip
        -0.015, 0.8, 0.0, 1.0, 0.0, 2.0,
         0.015, 0.8, 0.0, 1.0, 0.0, 2.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(vbo)
}

unsafe fn create_tapered_needle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    glGenBuffers(1, &mut vbo);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Tapered needle with smooth transition
    let vertices: [f32; 72] = [
        // Base
        -0.04,  0.0, 1.0, 0.0, 0.0, 4.0,
         0.04,  0.0, 1.0, 0.0, 0.0, 4.0,
        // Smooth taper
        -0.035, 0.15, 1.0, 0.0, 0.0, 3.5,
         0.035, 0.15, 1.0, 0.0, 0.0, 3.5,
        -0.03,  0.3, 1.0, 0.0, 0.0, 3.0,
         0.03,  0.3, 1.0, 0.0, 0.0, 3.0,
        -0.025, 0.45, 1.0, 0.0, 0.0, 2.5,
         0.025, 0.45, 1.0, 0.0, 0.0, 2.5,
        -0.02,  0.6, 1.0, 0.0, 0.0, 2.0,
         0.02,  0.6, 1.0, 0.0, 0.0, 2.0,
        -0.01,  0.75, 1.0, 0.0, 0.0, 1.5,
         0.01,  0.75, 1.0, 0.0, 0.0, 1.5,
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(vbo)
}

unsafe fn create_wide_needle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    glGenBuffers(1, &mut vbo);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Wide needle - maintains width longer
    let vertices: [f32; 84] = [
        // Base (extra wide)
        -0.06,  0.0, 0.0, 0.0, 1.0, 6.0,
         0.06,  0.0, 0.0, 0.0, 1.0, 6.0,
        // Keep wide for longer
        -0.055, 0.1, 0.0, 0.0, 1.0, 5.5,
         0.055, 0.1, 0.0, 0.0, 1.0, 5.5,
        -0.05,  0.2, 0.0, 0.0, 1.0, 5.0,
         0.05,  0.2, 0.0, 0.0, 1.0, 5.0,
        -0.045, 0.3, 0.0, 0.0, 1.0, 4.5,
         0.045, 0.3, 0.0, 0.0, 1.0, 4.5,
        -0.04,  0.4, 0.0, 0.0, 1.0, 4.0,
         0.04,  0.4, 0.0, 0.0, 1.0, 4.0,
        -0.03,  0.5, 0.0, 0.0, 1.0, 3.0,
         0.03,  0.5, 0.0, 0.0, 1.0, 3.0,
        // Finally taper
        -0.02,  0.7, 0.0, 0.0, 1.0, 2.0,
         0.02,  0.7, 0.0, 0.0, 1.0, 2.0,
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(vbo)
}

unsafe fn create_precision_needle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    glGenBuffers(1, &mut vbo);
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    
    // Precision needle with many segments for smooth antialiasing
    let vertices: [f32; 96] = [
        // Base
        -0.025, 0.0, 1.0, 1.0, 0.0, 2.5,
         0.025, 0.0, 1.0, 1.0, 0.0, 2.5,
        // Many small segments for smooth rendering
        -0.024, 0.1, 1.0, 1.0, 0.0, 2.4,
         0.024, 0.1, 1.0, 1.0, 0.0, 2.4,
        -0.022, 0.2, 1.0, 1.0, 0.0, 2.2,
         0.022, 0.2, 1.0, 1.0, 0.0, 2.2,
        -0.02,  0.3, 1.0, 1.0, 0.0, 2.0,
         0.02,  0.3, 1.0, 1.0, 0.0, 2.0,
        -0.018, 0.4, 1.0, 1.0, 0.0, 1.8,
         0.018, 0.4, 1.0, 1.0, 0.0, 1.8,
        -0.015, 0.5, 1.0, 1.0, 0.0, 1.5,
         0.015, 0.5, 1.0, 1.0, 0.0, 1.5,
        -0.012, 0.6, 1.0, 1.0, 0.0, 1.2,
         0.012, 0.6, 1.0, 1.0, 0.0, 1.2,
        -0.008, 0.75, 1.0, 1.0, 0.0, 0.8,
         0.008, 0.75, 1.0, 1.0, 0.0, 0.8,
    ];
    
    glBufferData(GL_ARRAY_BUFFER, 
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                GL_STATIC_DRAW);
    
    Ok(vbo)
}

unsafe fn render_rotating_needles_frame(
    frame: i32,
    shader_program: u32,
    needle_configs: &[NeedleConfig],
    pos_attr: i32,
    color_attr: i32,
    thickness_attr: i32,
    time_uniform: i32,
    resolution_uniform: i32,
    rotation_uniform: i32,
    center_uniform: i32,
    scale_uniform: i32,
) {
    // Clear with dark dashboard background
    glClearColor(0.02, 0.02, 0.08, 1.0);
    glClear(GL_COLOR_BUFFER_BIT);
    
    glUseProgram(shader_program);
    
    let time = frame as f32 * 0.016; // ~60fps timing
    glUniform1f(time_uniform, time);
    glUniform2f(resolution_uniform, 800.0, 480.0);
    
    // Render each needle with its specific configuration
    for (i, config) in needle_configs.iter().enumerate() {
        // Calculate individual needle rotation
        let rotation_angle = time * config.speed * 0.5; // Slow, smooth rotation
        let needle_angle = rotation_angle + (i as f32 * 0.3); // Offset each needle
        
        // Set uniforms for this needle
        glUniform1f(rotation_uniform, needle_angle);
        glUniform2f(center_uniform, config.position.0, config.position.1);
        glUniform1f(scale_uniform, config.scale);
        
        // Bind needle geometry
        glBindBuffer(GL_ARRAY_BUFFER, config.vbo);
        
        // Setup vertex attributes with 6 floats per vertex (x, y, r, g, b, thickness)
        let stride = 6 * std::mem::size_of::<f32>() as i32;
        
        glEnableVertexAttribArray(pos_attr as u32);
        glVertexAttribPointer(
            pos_attr as u32, 2, GL_FLOAT, 0,
            stride, std::ptr::null(),
        );
        
        glEnableVertexAttribArray(color_attr as u32);
        glVertexAttribPointer(
            color_attr as u32, 3, GL_FLOAT, 0,
            stride, (2 * std::mem::size_of::<f32>()) as *const std::ffi::c_void,
        );
        
        glEnableVertexAttribArray(thickness_attr as u32);
        glVertexAttribPointer(
            thickness_attr as u32, 1, GL_FLOAT, 0,
            stride, (5 * std::mem::size_of::<f32>()) as *const std::ffi::c_void,
        );
        
        // Draw the needle
        glDrawArrays(config.draw_mode, 0, config.vertex_count);
    }
    
    // Print status every 60 frames
    if frame % 60 == 0 {
        let rotation_degrees = (time * 0.5 * 180.0 / std::f32::consts::PI) % 360.0;
        println!(
            "Frame {} - {} needles rotating at {:.1}° with antialiasing", 
            frame, needle_configs.len(), rotation_degrees
        );
    }
    
    glFlush();
}
