use crate::graphics::context::GraphicsContext;
use gl::types::*;
use freetype_sys as ft;
use std::collections::HashMap;

#[derive(Clone)]
struct CachedGlyph {
    texture_id: u32,
    width: f32,
    height: f32,
    bearing_x: f32,
    bearing_y: f32,
    advance: f32,
}

/// Run basic geometry rendering test with triangle, rectangle, hexagon, and circle
pub fn run_basic_geometry_test(context: &mut GraphicsContext) -> Result<(), String> {
    println!("Starting Basic Geometry Test with gl crate...");
    println!("Rendering: Triangle, Rectangle, Hexagon, and Circle");
    
    unsafe {
        // Load OpenGL function pointers
        gl::load_with(|name| {
            let c_str = std::ffi::CString::new(name).unwrap();
            context.get_proc_address(c_str.as_ptr()) as *const _
        });
        
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for smooth rendering
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Create shader program
        let shader_program = create_basic_shader_program()?;
        
        // Create geometry for all shapes
        let triangle_vbo = create_triangle_geometry()?;
        let rectangle_vbo = create_rectangle_geometry()?;
        let hexagon_vbo = create_hexagon_geometry()?;
        let circle_vbo = create_circle_geometry()?;
        
        // Get shader attribute locations
        let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
        let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());
        
        if pos_attr == -1 || color_attr == -1 {
            return Err("Failed to get shader attribute locations".to_string());
        }
        
        println!("Running geometry rendering animation...");
        
        let mut frame_count = 0;
        let total_frames = 300; // 5 seconds at 60fps
        
        while frame_count < total_frames {
            if context.should_quit() {
                break;
            }
            
            render_geometry_frame(
                frame_count,
                shader_program,
                triangle_vbo,
                rectangle_vbo,
                hexagon_vbo,
                circle_vbo,
                pos_attr,
                color_attr,
            );
            
            context.swap_buffers();
            frame_count += 1;
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
        // Cleanup
        gl::DeleteBuffers(1, &triangle_vbo);
        gl::DeleteBuffers(1, &rectangle_vbo);
        gl::DeleteBuffers(1, &hexagon_vbo);
        gl::DeleteBuffers(1, &circle_vbo);
        gl::DeleteProgram(shader_program);
        
        println!("Basic geometry test completed successfully!");
    }
    
    Ok(())
}

unsafe fn create_basic_shader_program() -> Result<u32, String> {
    let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
varying vec3 v_color;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
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
    let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
    if vertex_shader == 0 {
        return Err("Failed to create vertex shader".to_string());
    }
    
    let vertex_src_ptr = vertex_shader_source.as_ptr();
    gl::ShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
    gl::CompileShader(vertex_shader);
    
    // Check vertex shader compilation
    let mut compile_status = 0i32;
    gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Vertex shader compilation failed".to_string());
    }
    
    // Create and compile fragment shader
    let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
    if fragment_shader == 0 {
        return Err("Failed to create fragment shader".to_string());
    }
    
    let fragment_src_ptr = fragment_shader_source.as_ptr();
    gl::ShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
    gl::CompileShader(fragment_shader);
    
    // Check fragment shader compilation
    let mut compile_status = 0i32;
    gl::GetShaderiv(fragment_shader, gl::COMPILE_STATUS, &mut compile_status);
    if compile_status == 0 {
        return Err("Fragment shader compilation failed".to_string());
    }
    
    // Create and link shader program
    let program = gl::CreateProgram();
    if program == 0 {
        return Err("Failed to create shader program".to_string());
    }
    
    gl::AttachShader(program, vertex_shader);
    gl::AttachShader(program, fragment_shader);
    gl::LinkProgram(program);
    
    // Check program linking
    let mut link_status = 0i32;
    gl::GetProgramiv(program, gl::LINK_STATUS, &mut link_status);
    if link_status == 0 {
        return Err("Shader program linking failed".to_string());
    }
    
    // Cleanup shaders (they're now linked into the program)
    gl::DeleteShader(vertex_shader);
    gl::DeleteShader(fragment_shader);
    
    println!("Basic shader program created successfully!");
    Ok(program)
}

unsafe fn create_triangle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    
    // Triangle vertices: x, y, r, g, b
    let vertices: [f32; 15] = [
        // Top vertex - red
         0.0,  0.8, 1.0, 0.0, 0.0,
        // Bottom left - green
        -0.7, -0.2, 0.0, 1.0, 0.0,
        // Bottom right - blue
         0.7, -0.2, 0.0, 0.0, 1.0,
    ];
    
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (vertices.len() * std::mem::size_of::<f32>()) as isize,
        vertices.as_ptr() as *const std::ffi::c_void,
        gl::STATIC_DRAW,
    );
    
    Ok(vbo)
}

unsafe fn create_rectangle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    
    // Rectangle vertices (two triangles): x, y, r, g, b
    let vertices: [f32; 30] = [
        // First triangle (top-left, bottom-left, top-right)
        -0.6,  0.6, 1.0, 1.0, 0.0, // Top-left - yellow
        -0.6, -0.6, 1.0, 0.5, 0.0, // Bottom-left - orange
         0.6,  0.6, 1.0, 1.0, 0.0, // Top-right - yellow
        
        // Second triangle (bottom-left, bottom-right, top-right)
        -0.6, -0.6, 1.0, 0.5, 0.0, // Bottom-left - orange
         0.6, -0.6, 1.0, 0.0, 0.0, // Bottom-right - red
         0.6,  0.6, 1.0, 1.0, 0.0, // Top-right - yellow
    ];
    
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (vertices.len() * std::mem::size_of::<f32>()) as isize,
        vertices.as_ptr() as *const std::ffi::c_void,
        gl::STATIC_DRAW,
    );
    
    Ok(vbo)
}

unsafe fn create_hexagon_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    
    // Hexagon as triangle fan - center vertex + 6 outer vertices + repeat first outer vertex
    let mut vertices = Vec::new();
    let radius = 0.5f32;
    let sides = 6;
    
    // Center vertex - white
    vertices.extend_from_slice(&[0.0, 0.0, 1.0, 1.0, 1.0]);
    
    // Generate outer vertices
    for i in 0..=sides {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (sides as f32);
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        
        // Color varies around the hexagon
        let hue = (i as f32) / (sides as f32);
        let r = (hue * 2.0 * std::f32::consts::PI).cos() * 0.5 + 0.5;
        let g = (hue * 2.0 * std::f32::consts::PI + 2.0).cos() * 0.5 + 0.5;
        let b = (hue * 2.0 * std::f32::consts::PI + 4.0).cos() * 0.5 + 0.5;
        
        vertices.extend_from_slice(&[x, y, r, g, b]);
    }
    
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (vertices.len() * std::mem::size_of::<f32>()) as isize,
        vertices.as_ptr() as *const std::ffi::c_void,
        gl::STATIC_DRAW,
    );
    
    Ok(vbo)
}

unsafe fn create_circle_geometry() -> Result<u32, String> {
    let mut vbo = 0u32;
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    
    // Circle as triangle fan - center vertex + outer vertices
    let mut vertices = Vec::new();
    let radius = 0.4f32;
    let segments = 32; // More segments for smoother circle
    
    // Center vertex - cyan
    vertices.extend_from_slice(&[0.0, 0.0, 0.0, 1.0, 1.0]);
    
    // Generate outer vertices
    for i in 0..=segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        
        // Gradient from cyan center to magenta edge
        let t = i as f32 / segments as f32;
        let r = t;
        let g = 1.0 - t * 0.5;
        let b = 1.0;
        
        vertices.extend_from_slice(&[x, y, r, g, b]);
    }
    
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (vertices.len() * std::mem::size_of::<f32>()) as isize,
        vertices.as_ptr() as *const std::ffi::c_void,
        gl::STATIC_DRAW,
    );
    
    Ok(vbo)
}

unsafe fn render_geometry_frame(
    frame: i32,
    shader_program: u32,
    triangle_vbo: u32,
    rectangle_vbo: u32,
    hexagon_vbo: u32,
    circle_vbo: u32,
    pos_attr: i32,
    color_attr: i32,
) {
    // Clear screen with dark background
    gl::ClearColor(0.1, 0.1, 0.15, 1.0);
    gl::Clear(gl::COLOR_BUFFER_BIT);
    
    gl::UseProgram(shader_program);
    
    // Calculate which shape to show based on frame
    let cycle_length = 75; // Show each shape for 75 frames
    let current_shape = (frame / cycle_length) % 4;
    
    match current_shape {
        0 => {
            // Render triangle (top-left quadrant)
            gl::Viewport(0, 240, 400, 240); // Top-left
            render_shape(triangle_vbo, 3, gl::TRIANGLES, pos_attr, color_attr);
            
            if frame % 30 == 0 {
                println!("Frame {} - Rendering Triangle", frame);
            }
        }
        1 => {
            // Render rectangle (top-right quadrant)
            gl::Viewport(400, 240, 400, 240); // Top-right
            render_shape(rectangle_vbo, 6, gl::TRIANGLES, pos_attr, color_attr);
            
            if frame % 30 == 0 {
                println!("Frame {} - Rendering Rectangle", frame);
            }
        }
        2 => {
            // Render hexagon (bottom-left quadrant)
            gl::Viewport(0, 0, 400, 240); // Bottom-left
            render_shape(hexagon_vbo, 8, gl::TRIANGLE_FAN, pos_attr, color_attr);
            
            if frame % 30 == 0 {
                println!("Frame {} - Rendering Hexagon", frame);
            }
        }
        3 => {
            // Render circle (bottom-right quadrant)
            gl::Viewport(400, 0, 400, 240); // Bottom-right
            render_shape(circle_vbo, 34, gl::TRIANGLE_FAN, pos_attr, color_attr);
            
            if frame % 30 == 0 {
                println!("Frame {} - Rendering Circle", frame);
            }
        }
        _ => unreachable!(),
    }
    
    // Reset viewport for next frame
    gl::Viewport(0, 0, 800, 480);
    
    gl::Flush();
}

unsafe fn render_shape(
    vbo: u32,
    vertex_count: i32,
    draw_mode: GLenum,
    pos_attr: i32,
    color_attr: i32,
) {
    // Bind the vertex buffer
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    
    // Setup vertex attributes
    gl::EnableVertexAttribArray(pos_attr as u32);
    gl::VertexAttribPointer(
        pos_attr as u32,
        2, // 2 components (x, y)
        gl::FLOAT,
        gl::FALSE,
        5 * std::mem::size_of::<f32>() as i32, // stride
        std::ptr::null(),
    );
    
    gl::EnableVertexAttribArray(color_attr as u32);
    gl::VertexAttribPointer(
        color_attr as u32,
        3, // 3 components (r, g, b)
        gl::FLOAT,
        gl::FALSE,
        5 * std::mem::size_of::<f32>() as i32, // stride
        (2 * std::mem::size_of::<f32>()) as *const std::ffi::c_void, // offset
    );
    
    // Draw the shape
    gl::DrawArrays(draw_mode, 0, vertex_count);
    
    // Disable vertex attributes
    gl::DisableVertexAttribArray(pos_attr as u32);
    gl::DisableVertexAttribArray(color_attr as u32);
}

// Text rendering system using FreeType with glyph caching
struct OpenGLTextRenderer {
    ft_library: ft::FT_Library,
    ft_face: ft::FT_Face,
    shader_program: u32,
    vao: u32,
    vbo: u32,
    font_size: u32,
    glyph_cache: HashMap<char, CachedGlyph>,
    projection_width: f32,
    projection_height: f32,
    projection_matrix: [f32; 16],
}

impl OpenGLTextRenderer {
    unsafe fn new(font_path: &str, font_size: u32) -> Result<Self, String> {
        // Initialize FreeType
        let mut ft_library: ft::FT_Library = std::ptr::null_mut();
        if ft::FT_Init_FreeType(&mut ft_library) != 0 {
            return Err("Failed to initialize FreeType library".to_string());
        }
        
        // Load font face
        let mut ft_face: ft::FT_Face = std::ptr::null_mut();
        let font_path_cstr = std::ffi::CString::new(font_path).map_err(|_| "Invalid font path")?;
        
        if ft::FT_New_Face(ft_library, font_path_cstr.as_ptr(), 0, &mut ft_face) != 0 {
            ft::FT_Done_FreeType(ft_library);
            return Err(format!("Failed to load font: {}", font_path));
        }
        
        // Set font size
        if ft::FT_Set_Pixel_Sizes(ft_face, 0, font_size) != 0 {
            ft::FT_Done_Face(ft_face);
            ft::FT_Done_FreeType(ft_library);
            return Err("Failed to set font size".to_string());
        }
        
        // Create text rendering shader
        let shader_program = Self::create_text_shader_program()?;
        
        // Create VAO and VBO for text quads
        let mut vao = 0u32;
        let mut vbo = 0u32;
        gl::GenBuffers(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        
        println!("OpenGL text renderer initialized with FreeType + glyph caching");
        println!("Font: {}, Size: {}px", font_path, font_size);
        
        Ok(OpenGLTextRenderer {
            ft_library,
            ft_face,
            shader_program,
            vao,
            vbo,
            font_size,
            glyph_cache: HashMap::new(),
            projection_width: 0.0,
            projection_height: 0.0,
            projection_matrix: [0.0; 16],
        })
    }
    
    unsafe fn create_text_shader_program() -> Result<u32, String> {
        let vertex_shader_source = b"
attribute vec4 vertex; // <vec2 pos, vec2 tex>
varying vec2 tex_coords;
uniform mat4 projection;

void main() {
    gl_Position = projection * vec4(vertex.xy, 0.0, 1.0);
    tex_coords = vertex.zw;
}
\0";
        
        let fragment_shader_source = b"
precision mediump float;
varying vec2 tex_coords;
uniform sampler2D text_texture;
uniform vec3 text_color;

void main() {
    vec4 sampled = vec4(1.0, 1.0, 1.0, texture2D(text_texture, tex_coords).r);
    gl_FragColor = vec4(text_color, 1.0) * sampled;
}
\0";
        
        // Create and compile vertex shader
        let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
        if vertex_shader == 0 {
            return Err("Failed to create text vertex shader".to_string());
        }
        
        let vertex_src_ptr = vertex_shader_source.as_ptr();
        gl::ShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
        gl::CompileShader(vertex_shader);
        
        let mut compile_status = 0i32;
        gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut compile_status);
        if compile_status == 0 {
            return Err("Text vertex shader compilation failed".to_string());
        }
        
        // Create and compile fragment shader
        let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        if fragment_shader == 0 {
            return Err("Failed to create text fragment shader".to_string());
        }
        
        let fragment_src_ptr = fragment_shader_source.as_ptr();
        gl::ShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
        gl::CompileShader(fragment_shader);
        
        let mut compile_status = 0i32;
        gl::GetShaderiv(fragment_shader, gl::COMPILE_STATUS, &mut compile_status);
        if compile_status == 0 {
            return Err("Text fragment shader compilation failed".to_string());
        }
        
        // Create and link shader program
        let program = gl::CreateProgram();
        if program == 0 {
            return Err("Failed to create text shader program".to_string());
        }
        
        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);
        
        let mut link_status = 0i32;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut link_status);
        if link_status == 0 {
            return Err("Text shader program linking failed".to_string());
        }
        
        println!("Text rendering shader program created successfully!");
        Ok(program)
    }
    
    unsafe fn render_text(&mut self, text: &str, x: f32, y: f32, scale: f32, color: (f32, f32, f32), width: f32, height: f32) -> Result<(), String> {
        // Use cached program state
        gl::UseProgram(self.shader_program);
        
        // Only update projection matrix if dimensions changed
        if self.projection_width != width || self.projection_height != height {
            self.projection_width = width;
            self.projection_height = height;
            
            // Calculate projection matrix once
            self.projection_matrix = [
                2.0/width, 0.0,         0.0, 0.0,
                0.0,       -2.0/height, 0.0, 0.0,  // Negative Y scaling to flip coordinate system
                0.0,       0.0,         -1.0, 0.0,
                -1.0,      1.0,         0.0, 1.0,  // Y translation adjusted for flipped coordinates
            ];
            
            // Upload to GPU
            let projection_uniform = gl::GetUniformLocation(self.shader_program, b"projection\0".as_ptr());
            gl::UniformMatrix4fv(projection_uniform, 1, 0, self.projection_matrix.as_ptr());
        }
        
        // Set text color (this changes per text string)
        let color_uniform = gl::GetUniformLocation(self.shader_program, b"text_color\0".as_ptr());
        gl::Uniform3f(color_uniform, color.0, color.1, color.2);
        
        // Set up texture uniform (will be updated per character)
        let texture_uniform = gl::GetUniformLocation(self.shader_program, b"text_texture\0".as_ptr());
        gl::Uniform1i(texture_uniform, 0);
        
        // Set up vertex attributes (cached)
        let vertex_attr = gl::GetAttribLocation(self.shader_program, b"vertex\0".as_ptr());
        gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
        gl::EnableVertexAttribArray(vertex_attr as u32);
        gl::VertexAttribPointer(vertex_attr as u32, 4, gl::FLOAT, 0, 0, std::ptr::null());
        
        // Render each character using cached glyphs
        let mut cursor_x = x;
        for ch in text.chars() {
            cursor_x += self.render_cached_character(ch, cursor_x, y, scale)?;
        }
        
        Ok(())
    }
    
    unsafe fn get_or_cache_glyph(&mut self, ch: char) -> Result<CachedGlyph, String> {
        // Check if glyph is already cached
        if let Some(cached_glyph) = self.glyph_cache.get(&ch) {
            return Ok(cached_glyph.clone());
        }
        
        // Load character glyph
        if ft::FT_Load_Char(self.ft_face, ch as u64, ft::FT_LOAD_RENDER as i32) != 0 {
            return Err(format!("Failed to load character: {}", ch));
        }
        
        // Get glyph slot
        let glyph = (*self.ft_face).glyph;
        
        // Create a dedicated texture for this glyph
        let mut texture_id = 0u32;
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_2D, texture_id);
        
        // Set pixel alignment to 1 byte to handle FreeType's bitmap format
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RED as i32,
            (*glyph).bitmap.width as i32,
            (*glyph).bitmap.rows as i32,
            0,
            gl::RED,
            gl::UNSIGNED_BYTE,
            (*glyph).bitmap.buffer as *const std::ffi::c_void,
        );
        
        // Reset pixel alignment to default
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);
        
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        
        // Cache the glyph data
        let cached_glyph = CachedGlyph {
            texture_id,
            width: (*glyph).bitmap.width as f32,
            height: (*glyph).bitmap.rows as f32,
            bearing_x: (*glyph).bitmap_left as f32,
            bearing_y: (*glyph).bitmap_top as f32,
            advance: ((*glyph).advance.x >> 6) as f32,
        };
        
        self.glyph_cache.insert(ch, cached_glyph.clone());
        Ok(cached_glyph)
    }
    
    unsafe fn render_cached_character(&mut self, ch: char, x: f32, y: f32, scale: f32) -> Result<f32, String> {
        // Get cached glyph (or create if not cached)
        let glyph = self.get_or_cache_glyph(ch)?;
        
        // Bind the glyph's texture
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, glyph.texture_id);
        
        // Calculate quad vertices
        let w = glyph.width * scale;
        let h = glyph.height * scale;
        let xrel = x + glyph.bearing_x * scale;
        let yrel = y - glyph.bearing_y * scale;
        
        // Create quad vertices (x, y, tex_x, tex_y)
        let vertices: [f32; 24] = [
            xrel,     yrel + h, 0.0, 1.0,  // Top-left corner, tex coords (0,1) - flipped V
            xrel,     yrel,     0.0, 0.0,  // Bottom-left corner, tex coords (0,0) - flipped V
            xrel + w, yrel,     1.0, 0.0,  // Bottom-right corner, tex coords (1,0) - flipped V
            
            xrel,     yrel + h, 0.0, 1.0,  // Top-left corner, tex coords (0,1) - flipped V
            xrel + w, yrel,     1.0, 0.0,  // Bottom-right corner, tex coords (1,0) - flipped V
            xrel + w, yrel + h, 1.0, 1.0,  // Top-right corner, tex coords (1,1) - flipped V
        ];
        
        // Upload vertex data
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const std::ffi::c_void,
            gl::STATIC_DRAW,
        );
        
        // Render quad
        gl::DrawArrays(gl::TRIANGLES, 0, 6);
        
        // Return advance for next character
        Ok(glyph.advance * scale)
    }
}

impl Drop for OpenGLTextRenderer {
    fn drop(&mut self) {
        unsafe {
            if !self.ft_face.is_null() {
                ft::FT_Done_Face(self.ft_face);
            }
            if !self.ft_library.is_null() {
                ft::FT_Done_FreeType(self.ft_library);
            }
            
            // Clean up cached glyph textures
            for cached_glyph in self.glyph_cache.values() {
                gl::DeleteTextures(1, &cached_glyph.texture_id);
            }
            // Note: VAO/VBO cleanup would need proper OpenGL context
        }
    }
}

/// OpenGL text rendering test using FreeType
pub fn run_opengl_text_rendering_test(context: &mut GraphicsContext) -> Result<(), String> {
    println!("Starting OpenGL Text Rendering Test with FreeType...");
    println!("Rendering high-quality text directly in OpenGL context");
    
    unsafe {
        // Load OpenGL function pointers
        gl::load_with(|name| {
            let c_str = std::ffi::CString::new(name).unwrap();
            context.get_proc_address(c_str.as_ptr()) as *const _
        });
        
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for text transparency
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Try to find a suitable font
        let font_paths = vec![
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/System/Library/Fonts/Arial.ttf", // macOS
            "C:/Windows/Fonts/arial.ttf", // Windows
        ];
        
        let mut font_path = None;
        for path in &font_paths {
            if std::path::Path::new(path).exists() {
                font_path = Some(*path);
                break;
            }
        }
        
        let font_path = font_path.ok_or("No suitable font found for OpenGL text rendering")?;
        
        // Create text renderer
        let mut text_renderer = OpenGLTextRenderer::new(font_path, 24)?;
        
        println!("Running OpenGL text rendering demonstration...");
        
        // Dashboard text samples
        let dashboard_texts = vec![
            ("НИВА МФИ ТЕСТ", 50.0, 50.0, 1.5, (1.0, 1.0, 1.0)),
            ("Speed: 85 km/h", 50.0, 100.0, 1.0, (0.0, 1.0, 0.0)),
            ("RPM: 3500", 50.0, 140.0, 1.0, (1.0, 0.6, 0.0)),
            ("Fuel: 75%", 50.0, 180.0, 1.0, (1.0, 1.0, 0.0)),
            ("Temp: 89C", 50.0, 220.0, 1.0, (1.0, 0.4, 0.4)),
            ("ENGINE OK", 400.0, 100.0, 1.2, (0.0, 1.0, 0.0)),
            ("GPS: ACTIVE", 400.0, 140.0, 1.0, (0.6, 1.0, 0.6)),
            ("12:34 PM", 400.0, 180.0, 1.0, (0.4, 0.8, 1.0)),
        ];
        
        let mut frame_count = 0;
        let total_frames = 300; // 5 seconds at 60fps
        
        while frame_count < total_frames {
            if context.should_quit() {
                break;
            }
            
            // Clear with dark dashboard background
            gl::ClearColor(0.02, 0.02, 0.08, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            
            // Render all dashboard text efficiently (glyph caching handles texture binding)
            for (text, x, y, scale, color) in &dashboard_texts {
                text_renderer.render_text(text, *x, *y, *scale, *color, context.width as f32, context.height as f32)?;
            }
            
            // Add animated text
            let time = frame_count as f32 * 0.016;
            let pulse_scale = 1.0 + 0.2 * (time * 2.0).sin();
            let animated_text = format!("Frame: {}", frame_count);
            text_renderer.render_text(&animated_text, 50.0, 350.0, pulse_scale, (1.0, 0.5, 1.0), context.width as f32, context.height as f32)?;
            
            // Add FPS counter
            let fps_text = format!("FPS: {:.1}", 1.0 / 0.016);
            text_renderer.render_text(&fps_text, 600.0, 50.0, 0.8, (0.8, 0.8, 0.8), context.width as f32, context.height as f32)?;
            
            // Clean up OpenGL state
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
            
            context.swap_buffers();
            frame_count += 1;
            
            // Print status every 60 frames
            if frame_count % 60 == 0 {
                println!("Frame {} - OpenGL text rendering with FreeType", frame_count);
            }
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
        }
        
        println!("OpenGL text rendering test completed successfully!");
    }
    
    Ok(())
}
