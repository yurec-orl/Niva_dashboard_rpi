use gl;
use crate::graphics::context::GraphicsContext;

/// Configuration for gauge rendering
#[derive(Clone)]
pub struct GaugeConfig {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub min_value: f32,
    pub max_value: f32,
    pub current_value: f32,
    pub needle_angle_degrees: f32,
    pub marks_count: u32,
    pub border_color: (f32, f32, f32),
    pub needle_color: (f32, f32, f32),
    pub needle_glow_color: (f32, f32, f32),
    pub marks_color: (f32, f32, f32),
    pub text_color: (f32, f32, f32),
    pub center_circle_color: (f32, f32, f32),
    pub border_width: f32,
    pub needle_length: f32,
    pub needle_width: f32,
    pub marks_length: f32,
    pub marks_width: f32,
    pub center_circle_radius: f32,
    pub show_numbers: bool,
    pub show_marks: bool,
    pub show_center_circle: bool,
}

impl Default for GaugeConfig {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            radius: 0.8,
            min_value: 0.0,
            max_value: 100.0,
            current_value: 50.0,
            needle_angle_degrees: 0.0,
            marks_count: 10,
            border_color: (1.0, 1.0, 1.0),
            needle_color: (1.0, 0.2, 0.2),
            needle_glow_color: (1.0, 0.5, 0.5),
            marks_color: (0.8, 0.8, 0.8),
            text_color: (1.0, 1.0, 1.0),
            center_circle_color: (0.3, 0.3, 0.3),
            border_width: 0.02,
            needle_length: 0.6,
            needle_width: 0.03,
            marks_length: 0.1,
            marks_width: 0.01,
            center_circle_radius: 0.1,
            show_numbers: true,
            show_marks: true,
            show_center_circle: true,
        }
    }
}

impl GaugeConfig {
    /// Create a speedometer configuration (0-180 km/h)
    pub fn speedometer() -> Self {
        Self {
            min_value: 0.0,
            max_value: 180.0,
            marks_count: 18,
            needle_color: (1.0, 0.3, 0.0), // Orange needle
            needle_glow_color: (1.0, 0.6, 0.3),
            text_color: (0.9, 0.9, 1.0), // Light blue text
            ..Default::default()
        }
    }
    
    /// Create a tachometer configuration (0-8000 RPM)
    pub fn tachometer() -> Self {
        Self {
            min_value: 0.0,
            max_value: 8000.0,
            marks_count: 16,
            needle_color: (0.2, 1.0, 0.2), // Green needle
            needle_glow_color: (0.5, 1.0, 0.5),
            text_color: (0.9, 1.0, 0.9), // Light green text
            ..Default::default()
        }
    }
    
    /// Create a fuel gauge configuration (0-100%)
    pub fn fuel_gauge() -> Self {
        Self {
            center_x: 0.6,
            center_y: -0.6,
            radius: 0.3,
            min_value: 0.0,
            max_value: 100.0,
            marks_count: 5,
            needle_color: (0.2, 0.2, 1.0), // Blue needle
            needle_glow_color: (0.5, 0.5, 1.0),
            text_color: (0.9, 0.9, 1.0),
            needle_length: 0.2,
            center_circle_radius: 0.05,
            ..Default::default()
        }
    }
}

/// Gauge renderer with parameterized configuration
pub struct GaugeRenderer {
    config: GaugeConfig,
    shader_program: u32,
    circle_vao: u32,
    circle_vbo: u32,
    line_vao: u32,
    line_vbo: u32,
    triangle_vao: u32,
    triangle_vbo: u32,
}

impl GaugeRenderer {
    pub fn new(config: GaugeConfig) -> Self {
        let mut gauge = Self {
            config,
            shader_program: 0,
            circle_vao: 0,
            circle_vbo: 0,
            line_vao: 0,
            line_vbo: 0,
            triangle_vao: 0,
            triangle_vbo: 0,
        };
        
        unsafe {
            gauge.init_opengl();
        }
        
        gauge
    }
    
    /// Initialize OpenGL resources
    unsafe fn init_opengl(&mut self) {
        self.shader_program = create_simple_color_shader();
        
        // Create VAOs and VBOs for different shapes
        gl::GenVertexArrays(1, &mut self.circle_vao);
        gl::GenBuffers(1, &mut self.circle_vbo);
        
        gl::GenVertexArrays(1, &mut self.line_vao);
        gl::GenBuffers(1, &mut self.line_vbo);
        
        gl::GenVertexArrays(1, &mut self.triangle_vao);
        gl::GenBuffers(1, &mut self.triangle_vbo);
    }
    
    /// Update the gauge configuration
    pub fn set_config(&mut self, config: GaugeConfig) {
        self.config = config;
    }
    
    /// Update just the needle value and angle
    pub fn set_value(&mut self, value: f32, angle_degrees: f32) {
        self.config.current_value = value;
        self.config.needle_angle_degrees = angle_degrees;
    }
    
    /// Render the complete gauge
    pub fn render(&self, context: &mut GraphicsContext) {
        unsafe {
            gl::UseProgram(self.shader_program);
            
            // Render gauge border
            if self.config.border_width > 0.0 {
                self.render_gauge_border();
            }
            
            // Render marks
            if self.config.show_marks {
                self.render_gauge_marks();
            }
            
            // Render numbers
            if self.config.show_numbers {
                self.render_gauge_numbers(context);
            }
            
            // Render needle with glow
            self.render_needle_with_glow();
            
            // Render center circle
            if self.config.show_center_circle {
                self.render_center_circle();
            }
        }
    }
    
    /// Render the gauge circular border
    unsafe fn render_gauge_border(&self) {
        let segments = 64;
        let mut vertices = Vec::new();
        
        // Create triangle strip for circle border
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            
            // Outer vertex
            vertices.push(self.config.center_x + cos_a * self.config.radius);
            vertices.push(self.config.center_y + sin_a * self.config.radius);
            vertices.push(self.config.border_color.0);
            vertices.push(self.config.border_color.1);
            vertices.push(self.config.border_color.2);
            
            // Inner vertex
            vertices.push(self.config.center_x + cos_a * (self.config.radius - self.config.border_width));
            vertices.push(self.config.center_y + sin_a * (self.config.radius - self.config.border_width));
            vertices.push(self.config.border_color.0);
            vertices.push(self.config.border_color.1);
            vertices.push(self.config.border_color.2);
        }
        
        gl::BindVertexArray(self.circle_vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, self.circle_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const _,
            gl::DYNAMIC_DRAW,
        );
        
        // Set up vertex attributes
        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
        gl::EnableVertexAttribArray(1);
        gl::VertexAttribPointer(1, 3, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, (2 * std::mem::size_of::<f32>()) as *const _);
        
        gl::DrawArrays(gl::TRIANGLE_STRIP, 0, vertices.len() as i32 / 5);
    }
    
    /// Render gauge marks
    unsafe fn render_gauge_marks(&self) {
        for i in 0..=self.config.marks_count {
            let angle = (i as f32 / self.config.marks_count as f32) * std::f32::consts::PI + std::f32::consts::PI;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            
            let inner_radius = self.config.radius - self.config.border_width * 2.0;
            let outer_radius = inner_radius - self.config.marks_length;
            
            let vertices = [
                // Start point (outer)
                self.config.center_x + cos_a * inner_radius,
                self.config.center_y + sin_a * inner_radius,
                self.config.marks_color.0, self.config.marks_color.1, self.config.marks_color.2,
                // End point (inner)
                self.config.center_x + cos_a * outer_radius,
                self.config.center_y + sin_a * outer_radius,
                self.config.marks_color.0, self.config.marks_color.1, self.config.marks_color.2,
            ];
            
            gl::BindVertexArray(self.line_vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.line_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );
            
            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
            gl::EnableVertexAttribArray(1);
            gl::VertexAttribPointer(1, 3, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, (2 * std::mem::size_of::<f32>()) as *const _);
            
            gl::LineWidth(self.config.marks_width * 100.0); // Scale for visibility
            gl::DrawArrays(gl::LINES, 0, 2);
        }
    }
    
    /// Render gauge numbers
    unsafe fn render_gauge_numbers(&self, context: &mut GraphicsContext) {
        for i in 0..=self.config.marks_count {
            let angle = (i as f32 / self.config.marks_count as f32) * std::f32::consts::PI + std::f32::consts::PI;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            
            let text_radius = self.config.radius - self.config.border_width * 2.0 - self.config.marks_length - 0.1;
            let text_x = self.config.center_x + cos_a * text_radius;
            let text_y = self.config.center_y + sin_a * text_radius;
            
            let value = self.config.min_value + (i as f32 / self.config.marks_count as f32) * (self.config.max_value - self.config.min_value);
            let text = format!("{:.0}", value);
            
            // Convert screen coordinates for text rendering
            let screen_x = (text_x + 1.0) * 400.0; // Assuming 800px width
            let screen_y = (1.0 - text_y) * 300.0; // Assuming 600px height
            
            let _ = context.render_text(&text, screen_x, screen_y, 24.0, self.config.text_color);
        }
    }
    
    /// Render triangular needle with glow effect
    unsafe fn render_needle_with_glow(&self) {
        let angle_rad = self.config.needle_angle_degrees.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();
        
        // Needle tip
        let tip_x = self.config.center_x + cos_a * self.config.needle_length;
        let tip_y = self.config.center_y + sin_a * self.config.needle_length;
        
        // Needle base (perpendicular to needle direction)
        let perp_x = -sin_a * self.config.needle_width;
        let perp_y = cos_a * self.config.needle_width;
        
        // First render glow (slightly larger, dimmer)
        let glow_scale = 1.5;
        let glow_tip_x = self.config.center_x + cos_a * self.config.needle_length * glow_scale;
        let glow_tip_y = self.config.center_y + sin_a * self.config.needle_length * glow_scale;
        let glow_perp_x = perp_x * glow_scale;
        let glow_perp_y = perp_y * glow_scale;
        
        let glow_vertices = [
            // Tip
            glow_tip_x, glow_tip_y,
            self.config.needle_glow_color.0, self.config.needle_glow_color.1, self.config.needle_glow_color.2,
            // Base left
            self.config.center_x - glow_perp_x, self.config.center_y - glow_perp_y,
            self.config.needle_glow_color.0, self.config.needle_glow_color.1, self.config.needle_glow_color.2,
            // Base right
            self.config.center_x + glow_perp_x, self.config.center_y + glow_perp_y,
            self.config.needle_glow_color.0, self.config.needle_glow_color.1, self.config.needle_glow_color.2,
        ];
        
        gl::BindVertexArray(self.triangle_vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, self.triangle_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (glow_vertices.len() * std::mem::size_of::<f32>()) as isize,
            glow_vertices.as_ptr() as *const _,
            gl::DYNAMIC_DRAW,
        );
        
        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
        gl::EnableVertexAttribArray(1);
        gl::VertexAttribPointer(1, 3, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, (2 * std::mem::size_of::<f32>()) as *const _);
        
        gl::DrawArrays(gl::TRIANGLES, 0, 3);
        
        // Then render main needle
        let needle_vertices = [
            // Tip
            tip_x, tip_y,
            self.config.needle_color.0, self.config.needle_color.1, self.config.needle_color.2,
            // Base left
            self.config.center_x - perp_x, self.config.center_y - perp_y,
            self.config.needle_color.0, self.config.needle_color.1, self.config.needle_color.2,
            // Base right
            self.config.center_x + perp_x, self.config.center_y + perp_y,
            self.config.needle_color.0, self.config.needle_color.1, self.config.needle_color.2,
        ];
        
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (needle_vertices.len() * std::mem::size_of::<f32>()) as isize,
            needle_vertices.as_ptr() as *const _,
            gl::DYNAMIC_DRAW,
        );
        
        gl::DrawArrays(gl::TRIANGLES, 0, 3);
    }
    
    /// Render center circle
    unsafe fn render_center_circle(&self) {
        let segments = 32;
        let mut vertices = Vec::new();
        
        // Center vertex
        vertices.push(self.config.center_x);
        vertices.push(self.config.center_y);
        vertices.push(self.config.center_circle_color.0);
        vertices.push(self.config.center_circle_color.1);
        vertices.push(self.config.center_circle_color.2);
        
        // Perimeter vertices
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
            let x = self.config.center_x + angle.cos() * self.config.center_circle_radius;
            let y = self.config.center_y + angle.sin() * self.config.center_circle_radius;
            
            vertices.push(x);
            vertices.push(y);
            vertices.push(self.config.center_circle_color.0);
            vertices.push(self.config.center_circle_color.1);
            vertices.push(self.config.center_circle_color.2);
        }
        
        gl::BindVertexArray(self.circle_vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, self.circle_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const _,
            gl::DYNAMIC_DRAW,
        );
        
        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
        gl::EnableVertexAttribArray(1);
        gl::VertexAttribPointer(1, 3, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, (2 * std::mem::size_of::<f32>()) as *const _);
        
        gl::DrawArrays(gl::TRIANGLE_FAN, 0, vertices.len() as i32 / 5);
    }
}

impl Drop for GaugeRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.circle_vao);
            gl::DeleteBuffers(1, &self.circle_vbo);
            gl::DeleteVertexArrays(1, &self.line_vao);
            gl::DeleteBuffers(1, &self.line_vbo);
            gl::DeleteVertexArrays(1, &self.triangle_vao);
            gl::DeleteBuffers(1, &self.triangle_vbo);
            gl::DeleteProgram(self.shader_program);
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

/// Example function showing how to use the parameterized gauge
/// This replaces the hardcoded run_rotating_needle_gauge_test function
pub fn run_gauge_example(context: &mut GraphicsContext) {
    static mut GAUGES: Option<(GaugeRenderer, GaugeRenderer, GaugeRenderer)> = None;
    static mut ANGLE: f32 = 0.0;
    
    unsafe {
        // Initialize gauges once
        if GAUGES.is_none() {
            // Create speedometer
            let speedometer_config = GaugeConfig::speedometer();
            let speedometer = GaugeRenderer::new(speedometer_config);

            // Create tachometer
            let mut tacho_config = GaugeConfig::tachometer();
            tacho_config.center_x = -0.6; // Position on left side
            tacho_config.center_y = 0.0;
            let tachometer = GaugeRenderer::new(tacho_config);

            // Create fuel gauge
            let fuel_config = GaugeConfig::fuel_gauge();
            let fuel_gauge = GaugeRenderer::new(fuel_config);

            GAUGES = Some((speedometer, tachometer, fuel_gauge));
        }
        
        if let Some((speedometer, tachometer, fuel_gauge)) = &mut GAUGES {
            // Update needle positions
            ANGLE += 1.0;
            if ANGLE > 360.0 {
                ANGLE = 0.0;
            }
            
            // Calculate values based on angle for demonstration
            let speed_value = (ANGLE.to_radians().sin() * 90.0 + 90.0).max(0.0); // 0-180 km/h
            let rpm_value = (ANGLE.to_radians().cos() * 4000.0 + 4000.0).max(0.0); // 0-8000 RPM
            let fuel_value = ((ANGLE / 4.0).to_radians().sin() * 50.0 + 50.0).max(0.0); // 0-100%
            
            // Update gauge values
            speedometer.set_value(speed_value, ANGLE);
            tachometer.set_value(rpm_value, ANGLE + 45.0); // Offset angle
            fuel_gauge.set_value(fuel_value, ANGLE / 2.0); // Slower movement
            
            // Render all gauges
            speedometer.render(context);
            tachometer.render(context);
            fuel_gauge.render(context);
        }
    }
}
