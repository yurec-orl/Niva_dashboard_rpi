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
pub struct OpenGLTextRenderer {
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
    // Cached uniform and attribute locations for performance
    projection_uniform: i32,
    color_uniform: i32,
    texture_uniform: i32,
    vertex_attr: i32,
}

impl OpenGLTextRenderer {
    pub unsafe fn new(font_path: &str, font_size: u32) -> Result<Self, String> {
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
        
        // Cache uniform and attribute locations for performance
        let projection_uniform = gl::GetUniformLocation(shader_program, b"projection\0".as_ptr());
        let color_uniform = gl::GetUniformLocation(shader_program, b"text_color\0".as_ptr());
        let texture_uniform = gl::GetUniformLocation(shader_program, b"text_texture\0".as_ptr());
        let vertex_attr = gl::GetAttribLocation(shader_program, b"vertex\0".as_ptr());
        
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
            projection_uniform,
            color_uniform,
            texture_uniform,
            vertex_attr,
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
    
    pub unsafe fn render_text(&mut self, text: &str, x: f32, y: f32, scale: f32, color: (f32, f32, f32), width: f32, height: f32) -> Result<(), String> {
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
            
            // Upload to GPU using cached uniform location
            gl::UniformMatrix4fv(self.projection_uniform, 1, 0, self.projection_matrix.as_ptr());
        }
        
        // Set text color using cached uniform location
        gl::Uniform3f(self.color_uniform, color.0, color.1, color.2);
        
        // Set up texture uniform using cached location
        gl::Uniform1i(self.texture_uniform, 0);
        
        // Set up vertex attributes using cached location
        gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
        gl::EnableVertexAttribArray(self.vertex_attr as u32);
        gl::VertexAttribPointer(self.vertex_attr as u32, 4, gl::FLOAT, 0, 0, std::ptr::null());
        
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
    
    /// Calculate the total width of a text string with the current font and scale
    unsafe fn calculate_text_width(&mut self, text: &str, scale: f32) -> Result<f32, String> {
        let mut total_width = 0.0;
        
        for ch in text.chars() {
            let glyph = self.get_or_cache_glyph(ch)?;
            total_width += glyph.advance * scale;
        }
        
        Ok(total_width)
    }
    
    /// Calculate the maximum height of a text string with the current font and scale
    unsafe fn calculate_text_height(&mut self, text: &str, scale: f32) -> Result<f32, String> {
        let mut max_height = 0.0;
        let mut max_descent = 0.0;
        
        for ch in text.chars() {
            let glyph = self.get_or_cache_glyph(ch)?;
            let char_height = glyph.bearing_y * scale;
            let char_descent = (glyph.height - glyph.bearing_y) * scale;
            
            if char_height > max_height {
                max_height = char_height;
            }
            if char_descent > max_descent {
                max_descent = char_descent;
            }
        }
        
        Ok(max_height + max_descent)
    }
    
    /// Calculate both width and height of a text string (convenience function)
    unsafe fn calculate_text_dimensions(&mut self, text: &str, scale: f32) -> Result<(f32, f32), String> {
        let width = self.calculate_text_width(text, scale)?;
        let height = self.calculate_text_height(text, scale)?;
        Ok((width, height))
    }
    
    /// Get the line height for the current font (useful for multi-line text)
    fn get_line_height(&self, scale: f32) -> f32 {
        unsafe {
            let face_ref = &*self.ft_face;
            (face_ref.size as *const ft::FT_SizeRec).as_ref().unwrap().metrics.height as f32 / 64.0 * scale
        }
    }
    
    /// Get the baseline-to-baseline distance for the current font
    fn get_line_spacing(&self, scale: f32) -> f32 {
        // Use line height as default line spacing
        self.get_line_height(scale)
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

/// Gauge data structure for dashboard performance test
#[derive(Clone)]
struct Gauge {
    name: String,
    x: f32,
    y: f32,
    radius: f32,
    min_value: f32,
    max_value: f32,
    current_value: f32,
    unit: String,
    color: (f32, f32, f32),
    animation_speed: f32,
    target_value: f32,
}

/// Complex dashboard performance test with 9 animated gauges
pub fn run_dashboard_performance_test(context: &mut GraphicsContext) -> Result<(), String> {
    println!("=== Dashboard Performance Test ===");
    println!("9 animated gauges with scale marks and numeric labels");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for smooth text rendering
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }
    
    // Initialize text renderer
    let mut text_renderer = unsafe {
        OpenGLTextRenderer::new(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            16
        )?
    };
    
    // Create 9 different gauges arranged in a 3x3 grid
    let mut gauges = vec![
        // Top row
        Gauge {
            name: "Engine RPM".to_string(),
            x: 130.0, y: 120.0, radius: 80.0,
            min_value: 0.0, max_value: 8000.0, current_value: 1500.0,
            unit: "RPM".to_string(),
            color: (1.0, 0.2, 0.2), // Red
            animation_speed: 150.0, target_value: 4500.0,
        },
        Gauge {
            name: "Speed".to_string(),
            x: 400.0, y: 120.0, radius: 80.0,
            min_value: 0.0, max_value: 200.0, current_value: 45.0,
            unit: "km/h".to_string(),
            color: (0.2, 0.8, 0.2), // Green
            animation_speed: 25.0, target_value: 120.0,
        },
        Gauge {
            name: "Fuel".to_string(),
            x: 670.0, y: 120.0, radius: 80.0,
            min_value: 0.0, max_value: 100.0, current_value: 85.0,
            unit: "%".to_string(),
            color: (0.2, 0.5, 1.0), // Blue
            animation_speed: 8.0, target_value: 15.0,
        },
        // Middle row
        Gauge {
            name: "Oil Temp".to_string(),
            x: 130.0, y: 280.0, radius: 80.0,
            min_value: 60.0, max_value: 120.0, current_value: 85.0,
            unit: "°C".to_string(),
            color: (1.0, 0.6, 0.0), // Orange
            animation_speed: 12.0, target_value: 95.0,
        },
        Gauge {
            name: "Boost".to_string(),
            x: 400.0, y: 280.0, radius: 80.0,
            min_value: -1.0, max_value: 2.0, current_value: 0.2,
            unit: "bar".to_string(),
            color: (0.8, 0.2, 0.8), // Purple
            animation_speed: 0.4, target_value: 1.5,
        },
        Gauge {
            name: "Voltage".to_string(),
            x: 670.0, y: 280.0, radius: 80.0,
            min_value: 11.0, max_value: 15.0, current_value: 12.6,
            unit: "V".to_string(),
            color: (0.0, 0.8, 0.8), // Cyan
            animation_speed: 0.15, target_value: 14.2,
        },
        // Bottom row
        Gauge {
            name: "Coolant".to_string(),
            x: 130.0, y: 440.0, radius: 80.0,
            min_value: 70.0, max_value: 110.0, current_value: 88.0,
            unit: "°C".to_string(),
            color: (0.2, 0.9, 0.9), // Light Blue
            animation_speed: 8.0, target_value: 102.0,
        },
        Gauge {
            name: "Oil Press".to_string(),
            x: 400.0, y: 440.0, radius: 80.0,
            min_value: 0.0, max_value: 8.0, current_value: 3.2,
            unit: "bar".to_string(),
            color: (0.9, 0.9, 0.2), // Yellow
            animation_speed: 0.8, target_value: 6.5,
        },
        Gauge {
            name: "AFR".to_string(),
            x: 670.0, y: 440.0, radius: 80.0,
            min_value: 10.0, max_value: 18.0, current_value: 14.7,
            unit: ":1".to_string(),
            color: (0.9, 0.4, 0.6), // Pink
            animation_speed: 1.2, target_value: 12.8,
        },
    ];
    
    let mut frame_count = 0;
    let start_time = std::time::Instant::now();
    
    unsafe {
        println!("Starting dashboard performance test...");
        
        loop {
            frame_count += 1;
            let elapsed = start_time.elapsed().as_secs_f32();
            
            // Exit after 30 seconds or on any input
            if elapsed > 30.0 {
                break;
            }
            
            // Clear screen with dark background
            gl::ClearColor(0.05, 0.05, 0.15, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            
            // Update and render each gauge
            for gauge in &mut gauges {
                // Animate gauge values
                let direction = if gauge.current_value < gauge.target_value { 1.0 } else { -1.0 };
                gauge.current_value += direction * gauge.animation_speed * 0.016; // 16ms frame time
                
                // Clamp to min/max
                gauge.current_value = gauge.current_value.clamp(gauge.min_value, gauge.max_value);
                
                // Reverse direction when target is reached
                if (gauge.current_value - gauge.target_value).abs() < gauge.animation_speed * 0.032 {
                    gauge.target_value = if gauge.target_value > (gauge.min_value + gauge.max_value) / 2.0 {
                        gauge.min_value + (gauge.max_value - gauge.min_value) * 0.1
                    } else {
                        gauge.min_value + (gauge.max_value - gauge.min_value) * 0.9
                    };
                }
                
                // Render gauge using simple text rendering for now
                render_gauge_simple(&mut text_renderer, gauge, context.width as f32, context.height as f32)?;
            }
            
            // Render performance info with glyph cache stats
            let fps = frame_count as f32 / elapsed;
            let cache_size = text_renderer.glyph_cache.len();
            let perf_text = format!("Frame: {} FPS: {:.1} Glyphs: {}", frame_count, fps, cache_size);
            text_renderer.render_text(&perf_text, 10.0, 30.0, 0.7, (0.9, 0.9, 0.9), context.width as f32, context.height as f32)?;
            
            // Update display
            context.swap_buffers();
            
            // Print progress every 60 frames with detailed stats
            if frame_count % 60 == 0 {
                println!("Frame {} - FPS: {:.1} - Glyph cache size: {} - {} gauges", frame_count, fps, cache_size, gauges.len());
            }
            
            // 60fps timing
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        
        let final_fps = frame_count as f32 / start_time.elapsed().as_secs_f32();
        println!("Dashboard performance test completed!");
        println!("Final statistics:");
        println!("  Frames rendered: {}", frame_count);
        println!("  Average FPS: {:.1}", final_fps);
        println!("  Gauges: {}", gauges.len());
        println!("  Performance: {} gauge renders/second", (gauges.len() as f32 * final_fps) as i32);
    }
    
    Ok(())
}

/// Optimized gauge rendering with reduced text calls and pre-computed strings
unsafe fn render_gauge_simple(
    text_renderer: &mut OpenGLTextRenderer,
    gauge: &Gauge,
    width: f32,
    height: f32
) -> Result<(), String> {
    // Combine multiple text elements into fewer render calls for better performance
    
    // Render gauge name and unit in one call
    let name_unit = format!("{} ({})", gauge.name, gauge.unit);
    text_renderer.render_text(&name_unit, gauge.x - 40.0, gauge.y - 30.0, 0.7, (0.8, 0.8, 0.8), width, height)?;
    
    // Render current value with large text
    let value_text = format!("{:.1}", gauge.current_value);
    text_renderer.render_text(&value_text, gauge.x - 25.0, gauge.y - 5.0, 1.2, gauge.color, width, height)?;
    
    // Render range info compactly
    let range_text = format!("{:.0}-{:.0}", gauge.min_value, gauge.max_value);
    text_renderer.render_text(&range_text, gauge.x - 30.0, gauge.y + 30.0, 0.4, (0.5, 0.5, 0.5), width, height)?;
    
    // Simplified progress indicator using fewer characters for better performance
    let progress = ((gauge.current_value - gauge.min_value) / (gauge.max_value - gauge.min_value)).clamp(0.0, 1.0);
    let bar_length = 10; // Reduced from 20 for better performance
    let filled_chars = (progress * bar_length as f32) as usize;
    
    // Pre-allocate string with known capacity
    let mut bar = String::with_capacity(bar_length);
    for i in 0..bar_length {
        bar.push(if i < filled_chars { '█' } else { '░' });
    }
    
    text_renderer.render_text(&bar, gauge.x - 35.0, gauge.y + 50.0, 0.6, gauge.color, width, height)?;
    
    Ok(())
}

/// Render a circle outline using triangles (since we're in OpenGL ES 2.0)
unsafe fn render_circle_outline(x: f32, y: f32, radius: f32, width: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32) {
    // For now, use a simplified line-based approach with existing triangle renderer
    let segments = 32;
    let pi = std::f32::consts::PI;
    
    for i in 0..segments {
        let angle1 = 2.0 * pi * i as f32 / segments as f32;
        let angle2 = 2.0 * pi * (i + 1) as f32 / segments as f32;
        
        let x1 = x + radius * angle1.cos();
        let y1 = y + radius * angle1.sin();
        let x2 = x + radius * angle2.cos();
        let y2 = y + radius * angle2.sin();
        
        render_line(x1, y1, x2, y2, width, color, screen_w, screen_h);
    }
}

/// Render a filled circle using triangles
unsafe fn render_circle_filled(x: f32, y: f32, radius: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32) {
    let segments = 16;
    let pi = std::f32::consts::PI;
    
    // Use our existing triangle renderer to create a circle
    for i in 0..segments {
        let angle1 = 2.0 * pi * i as f32 / segments as f32;
        let angle2 = 2.0 * pi * (i + 1) as f32 / segments as f32;
        
        let x1 = x + radius * angle1.cos();
        let y1 = y + radius * angle1.sin();
        let x2 = x + radius * angle2.cos();
        let y2 = y + radius * angle2.sin();
        
        // Create triangle from center to edge
        render_triangle(x, y, x1, y1, x2, y2, color, screen_w, screen_h);
    }
}

/// Render a line using a thin rectangle
unsafe fn render_line(x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32) {
    // Calculate line direction and perpendicular
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();
    
    if length < 0.001 {
        return; // Avoid division by zero
    }
    
    // Normalized perpendicular vector
    let px = -dy / length * width * 0.5;
    let py = dx / length * width * 0.5;
    
    // Create rectangle vertices
    let v1x = x1 + px;
    let v1y = y1 + py;
    let v2x = x1 - px;
    let v2y = y1 - py;
    let v3x = x2 - px;
    let v3y = y2 - py;
    let v4x = x2 + px;
    let v4y = y2 + py;
    
    // Render as two triangles
    render_triangle(v1x, v1y, v2x, v2y, v3x, v3y, color, screen_w, screen_h);
    render_triangle(v1x, v1y, v3x, v3y, v4x, v4y, color, screen_w, screen_h);
}

/// Render a single triangle using our basic geometry renderer
unsafe fn render_triangle(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32) {
    // Convert to OpenGL coordinates
    let vertices = [
        (x1 / screen_w) * 2.0 - 1.0, 1.0 - (y1 / screen_h) * 2.0, // Vertex 1
        (x2 / screen_w) * 2.0 - 1.0, 1.0 - (y2 / screen_h) * 2.0, // Vertex 2
        (x3 / screen_w) * 2.0 - 1.0, 1.0 - (y3 / screen_h) * 2.0, // Vertex 3
    ];
    
    // Use a simple colored triangle shader (we'll need to create this)
    static mut TRIANGLE_SHADER: u32 = 0;
    static mut TRIANGLE_VBO: u32 = 0;
    static mut TRIANGLE_VAO: u32 = 0;
    
    // Initialize shader if needed
    if TRIANGLE_SHADER == 0 {
        TRIANGLE_SHADER = create_simple_color_shader();
        
        gl::GenVertexArrays(1, &raw mut TRIANGLE_VAO);
        gl::GenBuffers(1, &raw mut TRIANGLE_VBO);
        
        gl::BindVertexArray(TRIANGLE_VAO);
        gl::BindBuffer(gl::ARRAY_BUFFER, TRIANGLE_VBO);
        
        let pos_attrib = gl::GetAttribLocation(TRIANGLE_SHADER, b"position\0".as_ptr());
        gl::EnableVertexAttribArray(pos_attrib as u32);
        gl::VertexAttribPointer(pos_attrib as u32, 2, gl::FLOAT, 0, 0, std::ptr::null());
    }
    
    // Use shader and upload triangle data
    gl::UseProgram(TRIANGLE_SHADER);
    gl::BindVertexArray(TRIANGLE_VAO);
    gl::BindBuffer(gl::ARRAY_BUFFER, TRIANGLE_VBO);
    
    // Upload vertices
    gl::BufferData(
        gl::ARRAY_BUFFER,
        (vertices.len() * std::mem::size_of::<f32>()) as isize,
        vertices.as_ptr() as *const std::ffi::c_void,
        gl::STATIC_DRAW,
    );
    
    // Set color
    let color_uniform = gl::GetUniformLocation(TRIANGLE_SHADER, b"color\0".as_ptr());
    gl::Uniform3f(color_uniform, color.0, color.1, color.2);
    
    // Draw triangle
    gl::DrawArrays(gl::TRIANGLES, 0, 3);
}

/// Create a simple color shader for basic shapes
unsafe fn create_simple_color_shader() -> u32 {
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

    // Create vertex shader
    let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
    let vertex_src_ptr = vertex_shader_source.as_ptr();
    gl::ShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
    gl::CompileShader(vertex_shader);

    // Create fragment shader
    let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
    let fragment_src_ptr = fragment_shader_source.as_ptr();
    gl::ShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
    gl::CompileShader(fragment_shader);

    // Create program
    let program = gl::CreateProgram();
    gl::AttachShader(program, vertex_shader);
    gl::AttachShader(program, fragment_shader);
    gl::LinkProgram(program);

    // Clean up shaders
    gl::DeleteShader(vertex_shader);
    gl::DeleteShader(fragment_shader);

    program
}

/// Run rotating needle gauge test with circular border, numbered marks, and triangular needle
pub fn run_rotating_needle_gauge_test(context: &mut GraphicsContext) -> Result<(), String> {
    println!("=== Rotating Needle Gauge Test ===");
    println!("Circular gauge with numbered marks and animated triangular needle");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for smooth rendering
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }
    
    // Initialize text renderer for numbers
    let mut text_renderer = unsafe {
        OpenGLTextRenderer::new(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            20
        )?
    };
    
    // Gauge parameters
    let center_x = 400.0;
    let center_y = 240.0;
    let outer_radius = 180.0;
    let inner_radius = 170.0;
    let needle_length = 150.0;
    let min_value = 0.0;
    let max_value = 100.0;
    let start_angle = -225.0f32.to_radians(); // Start at bottom-left
    let end_angle = 45.0f32.to_radians();     // End at bottom-right (270 degrees total)
    
    let mut frame_count = 0;
    let start_time = std::time::Instant::now();
    
    unsafe {
        // Create shader program for shapes
        let shader_program = create_simple_color_shader();
        
        println!("Starting rotating needle gauge animation...");
        context.swap_buffers();
        
        loop {
            let elapsed = start_time.elapsed().as_secs_f32();
            
            // Animate needle value (sine wave pattern)
            let mut current_value = 50.0 + 40.0 * (elapsed * 0.8).sin();
            
            // Clear screen
            gl::ClearColor(0.05, 0.05, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            
            // Render gauge components
            render_gauge_circle_border(center_x, center_y, outer_radius, inner_radius, (0.8, 0.8, 0.9), context.width as f32, context.height as f32, shader_program);
            render_gauge_marks(center_x, center_y, inner_radius - 20.0, start_angle, end_angle, 11, (0.9, 0.9, 1.0), context.width as f32, context.height as f32, shader_program);
            render_gauge_numbers(&mut text_renderer, center_x, center_y, inner_radius - 40.0, start_angle, end_angle, min_value, max_value, 11, (1.0, 1.0, 1.0), context.width as f32, context.height as f32)?;
            render_triangular_needle(center_x, center_y, needle_length, start_angle, end_angle, min_value, max_value, current_value, (1.0, 0.1, 0.0), context.width as f32, context.height as f32, shader_program);
            
            // Render center circle
            render_gauge_center_circle(center_x, center_y, 12.0, (0.4, 0.4, 0.5), context.width as f32, context.height as f32, shader_program);
            
            // Render current value text (centered using text measurement)
            let value_text = format!("{:.1}", current_value);
            let scale = 1.5;
            let (text_width, text_height) = text_renderer.calculate_text_dimensions(&value_text, scale)?;
            let text_x = center_x - text_width / 2.0;  // Center horizontally
            let text_y = center_y + 60.0;  // Position below gauge
            text_renderer.render_text(&value_text, text_x, text_y, scale, (1.0, 1.0, 0.3), context.width as f32, context.height as f32)?;
            
            // Print text dimensions on first frame for demonstration
            if frame_count == 1 {
                println!("Text '{}' dimensions: {:.1}x{:.1} pixels at scale {:.1}", value_text, text_width, text_height, scale);
                println!("Line height: {:.1} pixels", text_renderer.get_line_height(scale));
            }
            
            context.swap_buffers();
            frame_count += 1;
            
            // Print FPS every 60 frames
            if frame_count % 60 == 0 {
                let fps = frame_count as f32 / elapsed;
                println!("Frame {} - FPS: {:.1} - Needle value: {:.1}", frame_count, fps, current_value);
            }
            
            // Exit after 10 seconds
            if elapsed > 10.0 {
                break;
            }
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
        }
    }
    
    println!("Rotating needle gauge test completed!");
    Ok(())
}

// Helper function to render circular border
unsafe fn render_gauge_circle_border(center_x: f32, center_y: f32, outer_radius: f32, inner_radius: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
    gl::UseProgram(shader_program);
    
    let segments = 64;
    let mut vertices = Vec::new();
    
    // Create ring geometry using triangle strip
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        
        // Outer vertex
        let outer_x = (center_x + cos_a * outer_radius) / screen_w * 2.0 - 1.0;
        let outer_y = 1.0 - (center_y + sin_a * outer_radius) / screen_h * 2.0;
        vertices.extend_from_slice(&[outer_x, outer_y, color.0, color.1, color.2]);
        
        // Inner vertex
        let inner_x = (center_x + cos_a * inner_radius) / screen_w * 2.0 - 1.0;
        let inner_y = 1.0 - (center_y + sin_a * inner_radius) / screen_h * 2.0;
        vertices.extend_from_slice(&[inner_x, inner_y, color.0, color.1, color.2]);
    }
    
    let mut vbo = 0;
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    gl::BufferData(gl::ARRAY_BUFFER, (vertices.len() * 4) as isize, vertices.as_ptr() as *const _, gl::STATIC_DRAW);
    
    let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
    let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());
    
    gl::EnableVertexAttribArray(pos_attr as u32);
    gl::VertexAttribPointer(pos_attr as u32, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
    gl::EnableVertexAttribArray(color_attr as u32);
    gl::VertexAttribPointer(color_attr as u32, 3, gl::FLOAT, gl::FALSE, 20, (8) as *const _);
    
    gl::DrawArrays(gl::TRIANGLE_STRIP, 0, vertices.len() as i32 / 5);
    
    gl::DeleteBuffers(1, &vbo);
}

// Helper function to render gauge marks
unsafe fn render_gauge_marks(center_x: f32, center_y: f32, radius: f32, start_angle: f32, end_angle: f32, num_marks: i32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
    gl::UseProgram(shader_program);
    
    let angle_range = end_angle - start_angle;
    let mark_length = 15.0;
    
    for i in 0..num_marks {
        let t = i as f32 / (num_marks - 1) as f32;
        let angle = start_angle + t * angle_range;
        
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        
        // Mark line from radius to radius + mark_length
        let x1 = center_x + cos_a * radius;
        let y1 = center_y + sin_a * radius;
        let x2 = center_x + cos_a * (radius + mark_length);
        let y2 = center_y + sin_a * (radius + mark_length);
        
        // Convert to normalized coordinates
        let nx1 = x1 / screen_w * 2.0 - 1.0;
        let ny1 = 1.0 - y1 / screen_h * 2.0;
        let nx2 = x2 / screen_w * 2.0 - 1.0;
        let ny2 = 1.0 - y2 / screen_h * 2.0;
        
        let vertices = [
            nx1, ny1, color.0, color.1, color.2,
            nx2, ny2, color.0, color.1, color.2,
        ];
        
        let mut vbo = 0;
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(gl::ARRAY_BUFFER, (vertices.len() * 4) as isize, vertices.as_ptr() as *const _, gl::STATIC_DRAW);
        
        let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
        let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());
        
        gl::EnableVertexAttribArray(pos_attr as u32);
        gl::VertexAttribPointer(pos_attr as u32, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
        gl::EnableVertexAttribArray(color_attr as u32);
        gl::VertexAttribPointer(color_attr as u32, 3, gl::FLOAT, gl::FALSE, 20, (8) as *const _);
        
        gl::LineWidth(3.0);
        gl::DrawArrays(gl::LINES, 0, 2);
        
        gl::DeleteBuffers(1, &vbo);
    }
}

// Helper function to render gauge numbers
fn render_gauge_numbers(text_renderer: &mut OpenGLTextRenderer, center_x: f32, center_y: f32, radius: f32, start_angle: f32, end_angle: f32, min_value: f32, max_value: f32, num_marks: i32, color: (f32, f32, f32), screen_w: f32, screen_h: f32) -> Result<(), String> {
    let angle_range = end_angle - start_angle;
    let value_range = max_value - min_value;
    
    for i in 0..num_marks {
        let t = i as f32 / (num_marks - 1) as f32;
        let angle = start_angle + t * angle_range;
        let value = min_value + t * value_range;
        
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let text = format!("{:.0}", value);
        unsafe {
            let (text_width, text_height) = text_renderer.calculate_text_dimensions(&text, 1.0)?;
        
            let text_x = center_x + cos_a * radius - text_width / 2.0;
            let text_y = center_y + sin_a * radius + text_height / 2.0;

            text_renderer.render_text(&text, text_x, text_y, 1.0, color, screen_w, screen_h)?;
        }
    }
    
    Ok(())
}

// Helper function to render triangular needle
// Helper function to render triangular needle with glowing effect
unsafe fn render_triangular_needle(center_x: f32, center_y: f32, length: f32, start_angle: f32, end_angle: f32, min_value: f32, max_value: f32, current_value: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
    gl::UseProgram(shader_program);
    
    // Calculate needle angle based on value
    let value_ratio = (current_value - min_value) / (max_value - min_value);
    let needle_angle = start_angle + value_ratio * (end_angle - start_angle);
    
    let cos_a = needle_angle.cos();
    let sin_a = needle_angle.sin();
    
    // Base needle parameters
    let base_needle_width = 16.0;
    let tip_needle_width = 6.0;  // Separate tip width for tapered shape
    let tip_x = center_x + cos_a * length;
    let tip_y = center_y + sin_a * length;
    
    // Render glow layers (from largest/faintest to smallest/brightest)
    let glow_layers = [
        (3.0, 0.15), // Outermost glow: 2.5x size, 15% opacity
        (2.0, 0.25), // Middle glow: 2.0x size, 25% opacity  
        (1.5, 0.40), // Inner glow: 1.5x size, 40% opacity
        (0.75, 1.00), // Core needle: 15% narrower, full opacity
    ];
    
    for (size_multiplier, opacity) in glow_layers.iter() {
        let base_width = base_needle_width * size_multiplier;
        let tip_width = tip_needle_width * size_multiplier;
        
        // Base vertices (perpendicular to needle direction)
        let base_perp_cos = (-sin_a) * base_width * 0.5;
        let base_perp_sin = cos_a * base_width * 0.5;
        
        let base1_x = center_x + base_perp_cos;
        let base1_y = center_y + base_perp_sin;
        let base2_x = center_x - base_perp_cos;
        let base2_y = center_y - base_perp_sin;
        
        // Tip vertices (perpendicular to needle direction at tip)
        let tip_perp_cos = (-sin_a) * tip_width * 0.5;
        let tip_perp_sin = cos_a * tip_width * 0.5;
        
        let tip1_x = tip_x + tip_perp_cos;
        let tip1_y = tip_y + tip_perp_sin;
        let tip2_x = tip_x - tip_perp_cos;
        let tip2_y = tip_y - tip_perp_sin;
        
        // Convert to normalized coordinates
        let base1_nx = base1_x / screen_w * 2.0 - 1.0;
        let base1_ny = 1.0 - base1_y / screen_h * 2.0;
        let base2_nx = base2_x / screen_w * 2.0 - 1.0;
        let base2_ny = 1.0 - base2_y / screen_h * 2.0;
        let tip1_nx = tip1_x / screen_w * 2.0 - 1.0;
        let tip1_ny = 1.0 - tip1_y / screen_h * 2.0;
        let tip2_nx = tip2_x / screen_w * 2.0 - 1.0;
        let tip2_ny = 1.0 - tip2_y / screen_h * 2.0;
        
        // Apply progressive color brightness and temperature to match automotive red glow
        let glow_color = match *size_multiplier {
            s if s >= 2.5 => {
                // Outermost: deep red glow
                let brightness = 0.5;
                (
                    (color.0 * brightness * 1.0).min(1.0) * opacity,
                    (color.1 * brightness * 0.3).min(1.0) * opacity,
                    (color.2 * brightness * 0.1).min(1.0) * opacity,
                )
            },
            s if s >= 2.0 => {
                // Middle: bright red-orange
                let brightness = 0.7;
                (
                    (color.0 * brightness * 1.0).min(1.0) * opacity,
                    (color.1 * brightness * 0.5).min(1.0) * opacity,
                    (color.2 * brightness * 0.2).min(1.0) * opacity,
                )
            },
            s if s >= 1.5 => {
                // Inner: intense red-white
                let brightness = 1.0;
                (
                    (color.0 * brightness * 1.0).min(1.0) * opacity,
                    (color.1 * brightness * 0.8).min(1.0) * opacity,
                    (color.2 * brightness * 0.4).min(1.0) * opacity,
                )
            },
            _ => {
                // Core: brilliant white-hot center - override base color for true white
                (
                    1.0 * opacity,  // Pure white core
                    1.0 * opacity,
                    1.0 * opacity,
                )
            }
        };
        
        let vertices = [
            // First triangle: base1 -> base2 -> tip1
            base1_nx, base1_ny, glow_color.0, glow_color.1, glow_color.2,
            base2_nx, base2_ny, glow_color.0, glow_color.1, glow_color.2,
            tip1_nx, tip1_ny, glow_color.0, glow_color.1, glow_color.2,
            // Second triangle: base2 -> tip2 -> tip1
            base2_nx, base2_ny, glow_color.0, glow_color.1, glow_color.2,
            tip2_nx, tip2_ny, glow_color.0, glow_color.1, glow_color.2,
            tip1_nx, tip1_ny, glow_color.0, glow_color.1, glow_color.2,
        ];
        
        let mut vbo = 0;
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(gl::ARRAY_BUFFER, (vertices.len() * 4) as isize, vertices.as_ptr() as *const _, gl::STATIC_DRAW);
        
        let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
        let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());
        
        gl::EnableVertexAttribArray(pos_attr as u32);
        gl::VertexAttribPointer(pos_attr as u32, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
        gl::EnableVertexAttribArray(color_attr as u32);
        gl::VertexAttribPointer(color_attr as u32, 3, gl::FLOAT, gl::FALSE, 20, (8) as *const _);
        
        // Enable additive blending for glow effect
        if *size_multiplier > 1.0 {
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE); // Additive blending for glow
        } else {
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA); // Normal blending for core
        }
        
        gl::DrawArrays(gl::TRIANGLES, 0, 6);
        
        gl::DeleteBuffers(1, &vbo);
    }
    
    // Restore normal blending mode
    gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
}

// Helper function to render center circle
unsafe fn render_gauge_center_circle(center_x: f32, center_y: f32, radius: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
    gl::UseProgram(shader_program);
    
    let segments = 32;
    let mut vertices = Vec::new();
    
    // Center vertex
    let center_nx = center_x / screen_w * 2.0 - 1.0;
    let center_ny = 1.0 - center_y / screen_h * 2.0;
    vertices.extend_from_slice(&[center_nx, center_ny, color.0, color.1, color.2]);
    
    // Circle vertices
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
        let x = center_x + angle.cos() * radius;
        let y = center_y + angle.sin() * radius;
        
        let nx = x / screen_w * 2.0 - 1.0;
        let ny = 1.0 - y / screen_h * 2.0;
        vertices.extend_from_slice(&[nx, ny, color.0, color.1, color.2]);
    }
    
    let mut vbo = 0;
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
    gl::BufferData(gl::ARRAY_BUFFER, (vertices.len() * 4) as isize, vertices.as_ptr() as *const _, gl::STATIC_DRAW);
    
    let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
    let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());
    
    gl::EnableVertexAttribArray(pos_attr as u32);
    gl::VertexAttribPointer(pos_attr as u32, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
    gl::EnableVertexAttribArray(color_attr as u32);
    gl::VertexAttribPointer(color_attr as u32, 3, gl::FLOAT, gl::FALSE, 20, (8) as *const _);
    
    gl::DrawArrays(gl::TRIANGLE_FAN, 0, vertices.len() as i32 / 5);
    
    gl::DeleteBuffers(1, &vbo);
}
