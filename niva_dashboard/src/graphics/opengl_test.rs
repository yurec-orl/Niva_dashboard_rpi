#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
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
        
        log::info!("OpenGL text renderer initialized with FreeType + glyph caching");
        log::info!("Font: {}, Size: {}px", font_path, font_size);
        
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
        
        log::info!("Text rendering shader program created successfully!");
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
    log::info!("=== Rotating Needle Gauge Test ===");
    log::info!("Circular gauge with numbered marks and animated triangular needle");
    
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
        
        log::info!("Starting rotating needle gauge animation...");
        context.swap_buffers();
        
        loop {
            let elapsed = start_time.elapsed().as_secs_f32();
            
            // Animate needle value (sine wave pattern)
            let current_value = 50.0 + 40.0 * (elapsed * 0.8).sin();
            
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
                log::info!("Text '{}' dimensions: {:.1}x{:.1} pixels at scale {:.1}", value_text, text_width, text_height, scale);
                log::info!("Line height: {:.1} pixels", text_renderer.get_line_height(scale));
            }
            
            context.swap_buffers();
            frame_count += 1;
            
            // Print FPS every 60 frames
            if frame_count % 60 == 0 {
                let fps = frame_count as f32 / elapsed;
                log::info!("Frame {} - FPS: {:.1} - Needle value: {:.1}", frame_count, fps, current_value);
            }
            
            // Exit after 10 seconds
            if elapsed > 10.0 {
                break;
            }
            
            std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
        }
    }
    
    log::info!("Rotating needle gauge test completed!");
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

/// Run indicator zero position test with needle and vertical bar gauges
/// Displays indicators at zero position for 5 seconds using TestZeroAnalogDataProvider
pub fn run_indicator_zero_position_test(context: &mut GraphicsContext) -> Result<(), String> {
    use crate::hardware::hw_providers::{TestZeroAnalogDataProvider, HWInput, HWAnalogProvider};
    use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator};
    use crate::indicators::vertical_bar_indicator::VerticalBarIndicator;
    use crate::indicators::indicator::{Indicator, IndicatorBounds};
    use crate::graphics::ui_style::UIStyle;
    use crate::hardware::sensor_value::SensorValue;
    
    log::info!("=== Indicator Zero Position Test ===");
    log::info!("Testing needle and vertical bar indicators at zero position for 5 seconds");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for smooth rendering
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Clear screen with dark background
        gl::ClearColor(0.05, 0.05, 0.1, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    
    // Create UI style
    let ui_style = UIStyle::new();
    
    // Create zero value providers for testing
    let fuel_provider = TestZeroAnalogDataProvider::new(HWInput::HwFuelLvl);
    let oil_provider = TestZeroAnalogDataProvider::new(HWInput::HwOilPress);
    let voltage_provider = TestZeroAnalogDataProvider::new(HWInput::Hw12v);
    let temp_provider = TestZeroAnalogDataProvider::new(HWInput::HwEngineCoolantTemp);
    
    // Read zero values from providers
    let fuel_raw = fuel_provider.read_analog(HWInput::HwFuelLvl).unwrap_or(0);
    let oil_raw = oil_provider.read_analog(HWInput::HwOilPress).unwrap_or(0);
    let voltage_raw = voltage_provider.read_analog(HWInput::Hw12v).unwrap_or(0);
    let temp_raw = temp_provider.read_analog(HWInput::HwEngineCoolantTemp).unwrap_or(0);
    
    // Convert raw values to sensor values (all should be at minimum/zero)
    let fuel_value = SensorValue::analog(
        (fuel_raw as f32 / 1023.0) * 100.0, // Convert to percentage
        0.0, 100.0, "%", "Fuel Level", "fuel_sensor"
    );
    
    let oil_value = SensorValue::analog(
        (oil_raw as f32 / 1023.0) * 8.0, // Convert to kgf/cm²
        0.0, 8.0, "kgf/cm²", "Oil Pressure", "oil_sensor"
    );
    
    let voltage_value = SensorValue::analog(
        (voltage_raw as f32 / 1023.0) * 6.0 + 10.0, // Convert to volts (10-16V range)
        10.0, 16.0, "V", "12V System", "voltage_sensor"
    );
    
    let temp_value = SensorValue::analog(
        (temp_raw as f32 / 1023.0) * 160.0 - 40.0, // Convert to Celsius
        -40.0, 120.0, "°C", "Coolant Temp", "temp_sensor"
    );
    
    // Create indicators with decorators
    let fuel_needle = NeedleIndicator::new(
        -225.0f32.to_radians(), // Start angle (bottom-left)
        45.0f32.to_radians(),   // End angle (bottom-right)
        0.8,                    // Needle length
        0.05,                   // Base width
        0.02,                   // Tip width
        "GAUGE_NEEDLE_COLOR"   // needle color key
    ).with_decorators(vec![
        Box::new(NeedleGaugeMarksDecorator::new(
            6,                                      // Number of marks
            15.0,                                   // Mark length
            2.0,                                    // Mark width
            "gauge_major_mark_color",              // mark color key
            90.0,                                   // Radius for marks
            -225.0f32.to_radians(),                 // Start angle
            45.0f32.to_radians()                    // End angle
        ))
    ]);
    
    let oil_needle = NeedleIndicator::new(
        -225.0f32.to_radians(),
        45.0f32.to_radians(),
        0.8,
        0.05,
        0.02,
        "GAUGE_NEEDLE_COLOR"   // needle color key
    ).with_decorators(vec![
        Box::new(NeedleGaugeMarksDecorator::new(
            6,                                      // Number of marks
            15.0,                                   // Mark length
            2.0,                                    // Mark width
            "gauge_major_mark_color",              // mark color key
            90.0,                                   // Radius for marks
            -225.0f32.to_radians(),                 // Start angle
            45.0f32.to_radians()                    // End angle
        ))
    ]);
    
    let voltage_bar = VerticalBarIndicator::new(10) // 10 segments
        .with_segment_gap(2.0);
    
    let temp_bar = VerticalBarIndicator::new(8)     // 8 segments
        .with_segment_gap(2.0);
    
    // Define bounds for indicators
    let fuel_bounds = IndicatorBounds {
        x: 50.0,
        y: 50.0,
        width: 200.0,
        height: 200.0,
    };
    
    let oil_bounds = IndicatorBounds {
        x: 300.0,
        y: 50.0,
        width: 200.0,
        height: 200.0,
    };
    
    let voltage_bounds = IndicatorBounds {
        x: 550.0,
        y: 50.0,
        width: 80.0,
        height: 200.0,
    };
    
    let temp_bounds = IndicatorBounds {
        x: 650.0,
        y: 50.0,
        width: 80.0,
        height: 200.0,
    };
    
    let start_time = std::time::Instant::now();
    
    log::info!("Rendering indicators at zero position...");
    
    // Render loop for 5 seconds
    loop {
        let elapsed = start_time.elapsed();
        
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        
        // Render all indicators at zero position
        if let Err(e) = fuel_needle.render(&fuel_value, fuel_bounds, &ui_style, context) {
            log::error!("Error rendering fuel needle: {}", e);
        }
        
        if let Err(e) = oil_needle.render(&oil_value, oil_bounds, &ui_style, context) {
            log::error!("Error rendering oil needle: {}", e);
        }
        
        if let Err(e) = voltage_bar.render(&voltage_value, voltage_bounds, &ui_style, context) {
            log::error!("Error rendering voltage bar: {}", e);
        }
        
        if let Err(e) = temp_bar.render(&temp_value, temp_bounds, &ui_style, context) {
            log::error!("Error rendering temperature bar: {}", e);
        }
        
        context.swap_buffers();
        
        // Exit after 5 seconds
        if elapsed.as_secs() >= 5 {
            break;
        }
        
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }
    
    log::info!("Zero position indicator test completed!");
    log::info!("All indicators were displayed at their minimum/zero positions:");
    log::info!("- Fuel Level: {} ({})", fuel_value.as_f32(), fuel_value.metadata.unit);
    log::info!("- Oil Pressure: {} ({})", oil_value.as_f32(), oil_value.metadata.unit);
    log::info!("- Voltage: {} ({})", voltage_value.as_f32(), voltage_value.metadata.unit);
    log::info!("- Temperature: {} ({})", temp_value.as_f32(), temp_value.metadata.unit);
    
    Ok(())
}

/// Run indicator middle position test with needle and vertical bar gauges
/// Displays indicators at middle position for 5 seconds using TestMiddleAnalogDataProvider
pub fn run_indicator_middle_position_test(context: &mut GraphicsContext) -> Result<(), String> {
    use crate::hardware::hw_providers::{TestMiddleAnalogDataProvider, HWInput, HWAnalogProvider};
    use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator};
    use crate::indicators::vertical_bar_indicator::VerticalBarIndicator;
    use crate::indicators::indicator::{Indicator, IndicatorBounds};
    use crate::graphics::ui_style::UIStyle;
    use crate::hardware::sensor_value::SensorValue;
    
    log::info!("=== Indicator Middle Position Test ===");
    log::info!("Testing needle and vertical bar indicators at middle position for 5 seconds");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for smooth rendering
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Clear screen with dark background
        gl::ClearColor(0.05, 0.05, 0.1, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    
    // Create UI style
    let ui_style = UIStyle::new();
    
    // Create middle value providers for testing
    let fuel_provider = TestMiddleAnalogDataProvider::new(HWInput::HwFuelLvl);
    let oil_provider = TestMiddleAnalogDataProvider::new(HWInput::HwOilPress);
    let voltage_provider = TestMiddleAnalogDataProvider::new(HWInput::Hw12v);
    let temp_provider = TestMiddleAnalogDataProvider::new(HWInput::HwEngineCoolantTemp);
    
    // Read middle values from providers
    let fuel_raw = fuel_provider.read_analog(HWInput::HwFuelLvl).unwrap_or(512);
    let oil_raw = oil_provider.read_analog(HWInput::HwOilPress).unwrap_or(512);
    let voltage_raw = voltage_provider.read_analog(HWInput::Hw12v).unwrap_or(512);
    let temp_raw = temp_provider.read_analog(HWInput::HwEngineCoolantTemp).unwrap_or(512);
    
    // Convert raw values to sensor values (all should be at middle position)
    let fuel_value = SensorValue::analog(
        (fuel_raw as f32 / 1023.0) * 100.0, // Convert to percentage
        0.0, 100.0, "%", "Fuel Level", "fuel_sensor"
    );
    
    let oil_value = SensorValue::analog(
        (oil_raw as f32 / 1023.0) * 8.0, // Convert to kgf/cm²
        0.0, 8.0, "kgf/cm²", "Oil Pressure", "oil_sensor"
    );
    
    let voltage_value = SensorValue::analog(
        (voltage_raw as f32 / 1023.0) * 6.0 + 10.0, // Convert to volts (10-16V range)
        10.0, 16.0, "V", "12V System", "voltage_sensor"
    );
    
    let temp_value = SensorValue::analog(
        (temp_raw as f32 / 1023.0) * 160.0 - 40.0, // Convert to Celsius
        -40.0, 120.0, "°C", "Coolant Temp", "temp_sensor"
    );
    
    // Create indicators with decorators
    let fuel_needle = NeedleIndicator::new(
        -225.0f32.to_radians(), // Start angle (bottom-left)
        45.0f32.to_radians(),   // End angle (bottom-right)
        0.8,                    // Needle length
        0.05,                   // Base width
        0.02,                   // Tip width
        "GAUGE_NEEDLE_COLOR"    // needle color key
    ).with_decorators(vec![
        Box::new(NeedleGaugeMarksDecorator::new(
            6,                                      // Number of marks
            15.0,                                   // Mark length
            2.0,                                    // Mark width
            "gauge_major_mark_color",              // mark color key
            90.0,                                   // Radius for marks
            -225.0f32.to_radians(),                 // Start angle
            45.0f32.to_radians()                    // End angle
        ))
    ]);
    
    let oil_needle = NeedleIndicator::new(
        -225.0f32.to_radians(),
        45.0f32.to_radians(),
        0.8,
        0.05,
        0.02,
        "GAUGE_NEEDLE_COLOR"    // needle color key
    ).with_decorators(vec![
        Box::new(NeedleGaugeMarksDecorator::new(
            6,                                      // Number of marks
            15.0,                                   // Mark length
            2.0,                                    // Mark width
            "gauge_major_mark_color",              // mark color key
            90.0,                                   // Radius for marks
            -225.0f32.to_radians(),                 // Start angle
            45.0f32.to_radians()                    // End angle
        ))
    ]);
    
    let voltage_bar = VerticalBarIndicator::new(10) // 10 segments
        .with_segment_gap(2.0);
    
    let temp_bar = VerticalBarIndicator::new(10) // 10 segments
        .with_segment_gap(2.0);
    
    log::info!("Rendering indicators at middle position...");

    // Define bounds for indicators
    let fuel_bounds = IndicatorBounds {
        x: 50.0,
        y: 50.0,
        width: 200.0,
        height: 200.0,
    };
    
    let oil_bounds = IndicatorBounds {
        x: 300.0,
        y: 50.0,
        width: 200.0,
        height: 200.0,
    };
    
    let voltage_bounds = IndicatorBounds {
        x: 550.0,
        y: 50.0,
        width: 80.0,
        height: 200.0,
    };
    
    let temp_bounds = IndicatorBounds {
        x: 650.0,
        y: 50.0,
        width: 80.0,
        height: 200.0,
    };
    
    // Display for 5 seconds
    let start_time = std::time::Instant::now();
    let display_duration = std::time::Duration::from_secs(5);
    
    while start_time.elapsed() < display_duration {
        unsafe {
            gl::ClearColor(0.05, 0.05, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        
        // Render all indicators
        fuel_needle.render(&fuel_value, fuel_bounds, &ui_style, context)?;
        oil_needle.render(&oil_value, oil_bounds, &ui_style, context)?;
        voltage_bar.render(&voltage_value, voltage_bounds, &ui_style, context)?;
        temp_bar.render(&temp_value, temp_bounds, &ui_style, context)?;
        
        // Swap buffers
        context.swap_buffers();
        
        // Small delay to prevent excessive CPU usage
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }
    
    log::info!("Middle position indicator test completed!");
    log::info!("All indicators were displayed at their middle positions:");
    log::info!("- Fuel Level: {} ({})", fuel_value.as_f32(), fuel_value.metadata.unit);
    log::info!("- Oil Pressure: {} ({})", oil_value.as_f32(), oil_value.metadata.unit);
    log::info!("- Voltage: {} ({})", voltage_value.as_f32(), voltage_value.metadata.unit);
    log::info!("- Temperature: {} ({})", temp_value.as_f32(), temp_value.metadata.unit);
    
    Ok(())
}

/// Run indicator maximum position test with needle and vertical bar gauges
/// Displays indicators at maximum position for 5 seconds using TestMaxAnalogDataProvider
pub fn run_indicator_max_position_test(context: &mut GraphicsContext) -> Result<(), String> {
    use crate::hardware::hw_providers::{TestMaxAnalogDataProvider, HWInput, HWAnalogProvider};
    use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator};
    use crate::indicators::vertical_bar_indicator::VerticalBarIndicator;
    use crate::indicators::indicator::{Indicator, IndicatorBounds};
    use crate::graphics::ui_style::UIStyle;
    use crate::hardware::sensor_value::SensorValue;
    
    log::info!("=== Indicator Maximum Position Test ===");
    log::info!("Testing needle and vertical bar indicators at maximum position for 5 seconds");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for smooth rendering
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Clear screen with dark background
        gl::ClearColor(0.05, 0.05, 0.1, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    
    // Create UI style
    let ui_style = UIStyle::new();
    
    // Create maximum value providers for testing
    let fuel_provider = TestMaxAnalogDataProvider::new(HWInput::HwFuelLvl);
    let oil_provider = TestMaxAnalogDataProvider::new(HWInput::HwOilPress);
    let voltage_provider = TestMaxAnalogDataProvider::new(HWInput::Hw12v);
    let temp_provider = TestMaxAnalogDataProvider::new(HWInput::HwEngineCoolantTemp);
    
    // Read maximum values from providers
    let fuel_raw = fuel_provider.read_analog(HWInput::HwFuelLvl).unwrap_or(1023);
    let oil_raw = oil_provider.read_analog(HWInput::HwOilPress).unwrap_or(1023);
    let voltage_raw = voltage_provider.read_analog(HWInput::Hw12v).unwrap_or(1023);
    let temp_raw = temp_provider.read_analog(HWInput::HwEngineCoolantTemp).unwrap_or(1023);
    
    // Convert raw values to sensor values (all should be at maximum position)
    let fuel_value = SensorValue::analog(
        (fuel_raw as f32 / 1023.0) * 100.0, // Convert to percentage
        0.0, 100.0, "%", "Fuel Level", "fuel_sensor"
    );
    
    let oil_value = SensorValue::analog(
        (oil_raw as f32 / 1023.0) * 8.0, // Convert to kgf/cm²
        0.0, 8.0, "kgf/cm²", "Oil Pressure", "oil_sensor"
    );
    
    let voltage_value = SensorValue::analog(
        (voltage_raw as f32 / 1023.0) * 6.0 + 10.0, // Convert to volts (10-16V range)
        10.0, 16.0, "V", "12V System", "voltage_sensor"
    );
    
    let temp_value = SensorValue::analog(
        (temp_raw as f32 / 1023.0) * 160.0 - 40.0, // Convert to Celsius
        -40.0, 120.0, "°C", "Coolant Temp", "temp_sensor"
    );
    
    // Create indicators with decorators
    let fuel_needle = NeedleIndicator::new(
        -225.0f32.to_radians(), // Start angle (bottom-left)
        45.0f32.to_radians(),   // End angle (bottom-right)
        0.8,                    // Needle length
        0.05,                   // Base width
        0.02,                   // Tip width
        "GAUGE_NEEDLE_COLOR"   // needle color key
    ).with_decorators(vec![
        Box::new(NeedleGaugeMarksDecorator::new(
            6,                                      // Number of marks
            15.0,                                   // Mark length
            2.0,                                    // Mark width
            "gauge_major_mark_color",              // mark color key
            90.0,                                   // Radius for marks
            -225.0f32.to_radians(),                 // Start angle
            45.0f32.to_radians()                    // End angle
        ))
    ]);
    
    let oil_needle = NeedleIndicator::new(
        -225.0f32.to_radians(),
        45.0f32.to_radians(),
        0.8,
        0.05,
        0.02,
        "GAUGE_NEEDLE_COLOR"    // needle color key
    ).with_decorators(vec![
        Box::new(NeedleGaugeMarksDecorator::new(
            6,                                      // Number of marks
            15.0,                                   // Mark length
            2.0,                                    // Mark width
            "gauge_major_mark_color",              // mark color key
            90.0,                                   // Radius for marks
            -225.0f32.to_radians(),                 // Start angle
            45.0f32.to_radians()                    // End angle
        ))
    ]);
    
    let voltage_bar = VerticalBarIndicator::new(10) // 10 segments
        .with_segment_gap(2.0);
    
    let temp_bar = VerticalBarIndicator::new(10) // 10 segments
        .with_segment_gap(2.0);
    
    log::info!("Rendering indicators at maximum position...");
    
    // Define bounds for indicators
    let fuel_bounds = IndicatorBounds {
        x: 50.0,
        y: 50.0,
        width: 200.0,
        height: 200.0,
    };
    
    let oil_bounds = IndicatorBounds {
        x: 300.0,
        y: 50.0,
        width: 200.0,
        height: 200.0,
    };
    
    let voltage_bounds = IndicatorBounds {
        x: 550.0,
        y: 50.0,
        width: 80.0,
        height: 200.0,
    };
    
    let temp_bounds = IndicatorBounds {
        x: 650.0,
        y: 50.0,
        width: 80.0,
        height: 200.0,
    };

    // Display for 5 seconds
    let start_time = std::time::Instant::now();
    let display_duration = std::time::Duration::from_secs(5);
    
    while start_time.elapsed() < display_duration {
        unsafe {
            gl::ClearColor(0.05, 0.05, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        
        // Render all indicators
        fuel_needle.render(&fuel_value, fuel_bounds, &ui_style, context)?;
        oil_needle.render(&oil_value, oil_bounds, &ui_style, context)?;
        voltage_bar.render(&voltage_value, voltage_bounds, &ui_style, context)?;
        temp_bar.render(&temp_value, temp_bounds, &ui_style, context)?;
        
        // Swap buffers
        context.swap_buffers();
        
        // Small delay to prevent excessive CPU usage
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }
    
    log::info!("Maximum position indicator test completed!");
    log::info!("All indicators were displayed at their maximum positions:");
    log::info!("- Fuel Level: {} ({})", fuel_value.as_f32(), fuel_value.metadata.unit);
    log::info!("- Oil Pressure: {} ({})", oil_value.as_f32(), oil_value.metadata.unit);
    log::info!("- Voltage: {} ({})", voltage_value.as_f32(), voltage_value.metadata.unit);
    log::info!("- Temperature: {} ({})", temp_value.as_f32(), temp_value.metadata.unit);
    
    Ok(())
}

/// Stress test: 10×5 grid of fuel level gauges (50 total) with all decorators enabled.
/// Runs indefinitely (Ctrl+C to stop), printing per-second stats to stdout:
///
///   elapsed | total frames | FPS | render avg/max µs | process VmRSS
///
/// If render avg grows over time → CPU leak is inside the render path.
/// If VmRSS grows → heap memory is leaking.
/// If FPS drops while render avg stays flat → leak is outside render (e.g. swap_buffers / DRM).
pub fn run_fuel_level_grid_test(context: &mut GraphicsContext) -> Result<(), String> {
    use crate::indicator_builders::gauge_builders::fuel_level_gauge::build_fuel_level_gauge;
    use crate::hardware::sensor_value::{SensorValue, ValueData};
    use crate::graphics::ui_style::UIStyle;
    use std::time::Instant;

    log::info!("=== Fuel Level Grid Stress Test ===");
    log::info!("50 fuel level gauges (10×5 grid) with all decorators enabled");
    log::info!("Watching per-frame render time for CPU/memory leak detection");
    log::info!("{:>9} | {:>8} | {:>6} | {:>10} {:>10} | {}", "elapsed", "frames", "fps", "avg µs", "max µs", "VmRSS");
    log::info!("{}", "-".repeat(72));

    let ui_style = UIStyle::new();

    const COLS: usize = 10;
    const ROWS: usize = 5;
    const RADIUS: f32 = 35.0;
    let cell_w = context.width as f32 / COLS as f32;   // 80 px
    let cell_h = context.height as f32 / ROWS as f32;  // 96 px

    // Build all 50 indicators once — no per-frame allocation for setup
    let mut indicators = Vec::with_capacity(COLS * ROWS);
    for row in 0..ROWS {
        for col in 0..COLS {
            let cx = cell_w * col as f32 + cell_w / 2.0;
            let cy = cell_h * row as f32 + cell_h / 2.0;
            let (indicator, bounds) = build_fuel_level_gauge(cx, cy, RADIUS, &ui_style);
            indicators.push((indicator, bounds));
        }
    }

    // Single SensorValue reused across all frames — update value field in-place to
    // avoid per-frame String allocations from SensorValue::analog(...)
    let mut fuel_value = SensorValue::analog(50.0, 0.0, 100.0, "%", "Топливо", "fuel");

    unsafe {
        gl::Viewport(0, 0, context.width, context.height);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let test_start = Instant::now();
    let mut last_report = Instant::now();
    let mut total_frames: u64 = 0;
    // Per-second accumulator for render timing
    let mut render_us_bucket: Vec<u64> = Vec::with_capacity(120);

    loop {
        let elapsed = test_start.elapsed().as_secs_f32();

        // Slow sine sweep: 0→100→0% over 10 seconds, exercises the full needle arc
        fuel_value.value = ValueData::Analog(
            ((elapsed * std::f32::consts::PI / 5.0).sin() * 0.5 + 0.5) * 100.0,
        );

        unsafe {
            gl::ClearColor(0.05, 0.05, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        // Time only the render calls — not swap_buffers — so the measurement is
        // purely about GL submission cost, unaffected by DRM vsync wait time
        let render_start = Instant::now();
        for (indicator, bounds) in &indicators {
            indicator.render(&fuel_value, *bounds, &ui_style, context)?;
        }
        render_us_bucket.push(render_start.elapsed().as_micros() as u64);

        context.swap_buffers();
        total_frames += 1;

        // Print one stats line per second
        let report_secs = last_report.elapsed().as_secs_f32();
        if report_secs >= 1.0 {
            let n = render_us_bucket.len().max(1);
            let fps = n as f32 / report_secs;
            let avg_us = render_us_bucket.iter().sum::<u64>() / n as u64;
            let max_us = render_us_bucket.iter().copied().max().unwrap_or(0);

            // Read resident set size from /proc/self/status (KB)
            let vmrss_kb = std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("VmRSS:"))
                        .and_then(|l| l.split_whitespace().nth(1))
                        .and_then(|v| v.parse::<u64>().ok())
                })
                .unwrap_or(0);

            log::info!(
                "{:>8.1}s | {:>8} | {:>5.1} | {:>9}µs {:>9}µs | {}KB",
                elapsed, total_frames, fps, avg_us, max_us, vmrss_kb
            );

            render_us_bucket.clear();
            last_report = Instant::now();
        }
    }
}

/// Compass indicator test: rotating heading tape + fixed lubber-line overlay, driven by
/// TestGnssDataProvider's synthetic heading sweep (see util/gnss_data_provider.rs) instead of
/// a real GNSS receiver. Runs long enough (COMPASS_TEST_DURATION) to watch several full
/// rotations, including the 359°->0° wraparound.
pub fn run_compass_test(context: &mut GraphicsContext) -> Result<(), String> {
    use crate::graphics::ui_style::*;
    use crate::hardware::sensor_value::SensorValue;
    use crate::indicators::compass_indicator::{CompassHeadingMarkerDecorator, CompassIndicator};
    use crate::indicators::indicator::{Indicator, IndicatorBounds};
    use crate::util::gnss_data_provider::TestGnssDataProvider;
    use std::time::{Duration, Instant};

    log::info!("=== Compass Indicator Test ===");
    log::info!("Rotating heading tape driven by a synthetic GNSS heading sweep");

    const COMPASS_TEST_DURATION: Duration = Duration::from_secs(30);

    unsafe {
        gl::Viewport(0, 0, context.width, context.height);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let ui_style = UIStyle::new();
    let gnss = TestGnssDataProvider::start();
    let frame = gnss.frame();

    // Both objects must agree on visible_half_angle_deg/ring_margin/major_mark_length — see
    // CompassIndicator::geometry's doc comment on why they aren't derived from one shared
    // instance.
    let visible_half_angle_deg = 120.0;
    let ring_margin = 24.0;
    let major_mark_length = 18.0;
    let compass = CompassIndicator::new().with_decorators(vec![
        Box::new(CompassHeadingMarkerDecorator::new(visible_half_angle_deg, ring_margin, major_mark_length)),
    ]);

    let w = context.width as f32;
    let h = context.height as f32;
    let bounds = IndicatorBounds::new(w * 0.2, h * 0.05, w * 0.6, h * 0.85);

    let mut heading_value = SensorValue::analog(0.0, 0.0, 359.999, "\u{00B0}", "Курс", "test_heading");

    let info_font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
    let info_font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);
    let info_color = ui_style.get_color(TEXT_PRIMARY_COLOR, (1.0, 0.5, 0.0));

    let start_time = Instant::now();
    while start_time.elapsed() < COMPASS_TEST_DURATION {
        let fix = frame.fix();
        let heading = fix.heading_deg.unwrap_or(0.0);
        heading_value.value = crate::hardware::sensor_value::ValueData::Analog(heading);

        unsafe {
            gl::ClearColor(0.05, 0.05, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        compass.render(&heading_value, bounds, &ui_style, context)?;

        let info = format!(
            "КУРС: {:>5.1}\u{00B0}   СКОР: {:>5.1} км/ч   СПУТ: {}",
            heading,
            fix.speed_kmh.unwrap_or(0.0),
            fix.satellites.map(|s| s.to_string()).unwrap_or_else(|| "н/д".to_string()),
        );
        context.render_text_with_font(&info, w * 0.1, h * 0.92, 1.0, info_color, &info_font, info_font_size)?;

        context.swap_buffers();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    log::info!("Compass indicator test completed!");
    Ok(())
}
