use crate::indicators::indicator::{Indicator, IndicatorBounds, IndicatorBase};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::indicators::decorator::Decorator;
use std::f32::consts::PI;
use std::sync::Once;
use gl;

// Cached shader programs - created once and reused
static mut NEEDLE_SHADER_PROGRAM: u32 = 0;
static mut MARK_SHADER_PROGRAM: u32 = 0;
static NEEDLE_SHADER_INIT: Once = Once::new();
static MARK_SHADER_INIT: Once = Once::new();

/// Needle indicator that displays sensor values as a rotating needle
/// The needle rotates between start_angle and end_angle based on normalized sensor value
pub struct NeedleIndicator {
    /// Starting angle in radians (0 = right, PI/2 = up, PI = left, 3*PI/2 = down)
    start_angle: f32,
    /// Ending angle in radians
    end_angle: f32,
    /// Length of the needle from center to tip
    needle_length: f32,
    /// Width of the needle at the base (near center)
    needle_base_width: f32,
    /// Width of the needle at the tip
    needle_tip_width: f32,
    /// Color of the needle (R, G, B)
    needle_color: (f32, f32, f32),
    /// Base indicator functionality
    base: IndicatorBase,
}

impl NeedleIndicator {
    /// Create a new needle indicator with specified parameters
    ///
    /// # Parameters
    /// - `start_angle`: Starting angle in radians
    /// - `end_angle`: Ending angle in radians  
    /// - `needle_length`: Length of needle as fraction of available radius (0.0-1.0)
    /// - `needle_base_width`: Width at base as fraction of available radius
    /// - `needle_tip_width`: Width at tip as fraction of available radius
    /// - `needle_color`: RGB color tuple (each component 0.0-1.0)
    pub fn new(
        start_angle: f32,
        end_angle: f32,
        needle_length: f32,
        needle_base_width: f32,
        needle_tip_width: f32,
        needle_color: (f32, f32, f32),
    ) -> Self {
        Self {
            start_angle,
            end_angle,
            needle_length,
            needle_base_width,
            needle_tip_width,
            needle_color,
            base: IndicatorBase {
                decorators: Vec::new(),
            },
        }
    }

    unsafe fn get_needle_shader() -> u32 {
        NEEDLE_SHADER_INIT.call_once(|| {
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

            NEEDLE_SHADER_PROGRAM = program;
        });
        NEEDLE_SHADER_PROGRAM
    }

    /// Calculate the current needle angle based on normalized value (0.0-1.0)
    fn calculate_needle_angle(&self, normalized_value: f32) -> f32 {
        let clamped_value = normalized_value.clamp(0.0, 1.0);
        
        // Handle angle wrapping for cases where end_angle < start_angle
        let angle_range = if self.end_angle < self.start_angle {
            (self.end_angle + 2.0 * PI) - self.start_angle
        } else {
            self.end_angle - self.start_angle
        };
        
        let angle_offset = clamped_value * angle_range;
        let result_angle = self.start_angle + angle_offset;
        
        // Normalize angle to 0-2π range
        result_angle % (2.0 * PI)
    }

    unsafe fn render_needle(&self, center_x: f32, center_y: f32, length: f32,
                            needle_angle: f32, color: (f32, f32, f32),
                            screen_w: f32, screen_h: f32, shader_program: u32) {
        gl::UseProgram(shader_program);
        
        let cos_a = needle_angle.cos();
        let sin_a = needle_angle.sin();
        
        // Base needle parameters
        let tip_x = center_x + cos_a * length;
        let tip_y = center_y + sin_a * length;

        let base_width = self.needle_base_width;
        let tip_width = self.needle_tip_width;

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
        
        let vertices = [
            // First triangle: base1 -> base2 -> tip1
            base1_nx, base1_ny, color.0, color.1, color.2,
            base2_nx, base2_ny, color.0, color.1, color.2,
            tip1_nx, tip1_ny, color.0, color.1, color.2,
            // Second triangle: base2 -> tip2 -> tip1
            base2_nx, base2_ny, color.0, color.1, color.2,
            tip2_nx, tip2_ny, color.0, color.1, color.2,
            tip1_nx, tip1_ny, color.0, color.1, color.2,
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
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        gl::DrawArrays(gl::TRIANGLES, 0, 6);
        
        gl::DeleteBuffers(1, &vbo);
        
        // Restore normal blending mode
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }
}

impl Indicator for NeedleIndicator {
    fn with_decorators(mut self, decorators: Vec<Box<dyn Decorator>>) -> Self where Self: Sized {
        self.base.decorators = decorators;
        self
    }

    fn render(&self, 
              value: &SensorValue, 
              bounds: IndicatorBounds, 
              style: &UIStyle, 
              context: &mut GraphicsContext) -> Result<(), String> {
        
        // Get normalized value (0.0 to 1.0)
        let normalized_value = value.as_normalized();
        
        // Calculate center and radius from bounds
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;
        
        // Render decorators
        self.base.render_decorators(bounds, style, context)?;
        
        unsafe {
            // Enable blending for smooth rendering
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            
            // Get cached shader program
            let shader_program = Self::get_needle_shader();

            // Calculate needle angle
            let needle_angle = self.calculate_needle_angle(normalized_value);
        
            // Render the needle
            self.render_needle(center_x, center_y, self.needle_length, 
                               needle_angle, self.needle_color,
                               context.width as f32, context.height as f32,
                               shader_program);
        }
        
        Ok(())
    }

    fn indicator_type(&self) -> &'static str {
        "NeedleIndicator"
    }

    fn supports_value_type(&self, value: &ValueData) -> bool {
        matches!(value, ValueData::Analog(_) | ValueData::Integer(_) | ValueData::Percentage(_))
    }
}

// Needle gauge marks decorator
pub struct NeedleGaugeMarksDecorator {
    num_marks: u32,
    mark_length: f32,
    mark_width: f32,
    color: (f32, f32, f32),
    radius: f32,
    start_angle: f32,
    end_angle: f32,
}

impl NeedleGaugeMarksDecorator {
    pub fn new(
        num_marks: u32,
        mark_length: f32,
        mark_width: f32,
        color: (f32, f32, f32),
        radius: f32,
        start_angle: f32,
        end_angle: f32,
    ) -> Self {
        Self {
            num_marks,
            mark_length,
            mark_width,
            color,
            radius,
            start_angle,
            end_angle,
        }
    }

    unsafe fn get_mark_shader() -> u32 {
        MARK_SHADER_INIT.call_once(|| {
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

            MARK_SHADER_PROGRAM = program;
        });
        MARK_SHADER_PROGRAM
    }

    /// Calculate vertices for a single mark (returns 30 floats: 6 vertices × 5 components each)
    fn calculate_mark_vertices(&self, center_x: f32, center_y: f32, radius: f32, angle: f32,
                               screen_w: f32, screen_h: f32) -> [f32; 30] {
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Calculate inner and outer points of the mark
        let inner_radius = radius - self.mark_length;
        let outer_radius = radius;

        let inner_x = center_x + cos_a * inner_radius;
        let inner_y = center_y + sin_a * inner_radius;
        let outer_x = center_x + cos_a * outer_radius;
        let outer_y = center_y + sin_a * outer_radius;

        // Calculate perpendicular direction for width
        let perp_cos = -sin_a * self.mark_width * 0.5;
        let perp_sin = cos_a * self.mark_width * 0.5;

        // Four corners of the rectangular mark
        let inner1_x = inner_x + perp_cos;
        let inner1_y = inner_y + perp_sin;
        let inner2_x = inner_x - perp_cos;
        let inner2_y = inner_y - perp_sin;
        let outer1_x = outer_x + perp_cos;
        let outer1_y = outer_y + perp_sin;
        let outer2_x = outer_x - perp_cos;
        let outer2_y = outer_y - perp_sin;

        // Convert to normalized coordinates (-1 to 1)
        let inner1_nx = inner1_x / screen_w * 2.0 - 1.0;
        let inner1_ny = 1.0 - inner1_y / screen_h * 2.0;
        let inner2_nx = inner2_x / screen_w * 2.0 - 1.0;
        let inner2_ny = 1.0 - inner2_y / screen_h * 2.0;
        let outer1_nx = outer1_x / screen_w * 2.0 - 1.0;
        let outer1_ny = 1.0 - outer1_y / screen_h * 2.0;
        let outer2_nx = outer2_x / screen_w * 2.0 - 1.0;
        let outer2_ny = 1.0 - outer2_y / screen_h * 2.0;

        // Return vertices for two triangles forming a rectangle
        [
            // First triangle: inner1 -> inner2 -> outer1
            inner1_nx, inner1_ny, self.color.0, self.color.1, self.color.2,
            inner2_nx, inner2_ny, self.color.0, self.color.1, self.color.2,
            outer1_nx, outer1_ny, self.color.0, self.color.1, self.color.2,
            // Second triangle: inner2 -> outer2 -> outer1
            inner2_nx, inner2_ny, self.color.0, self.color.1, self.color.2,
            outer2_nx, outer2_ny, self.color.0, self.color.1, self.color.2,
            outer1_nx, outer1_ny, self.color.0, self.color.1, self.color.2,
        ]
    }

    /// Render all marks in a single batched draw call
    unsafe fn render_batched_marks(&self, vertices: &[f32], shader_program: u32) {
        // Create and bind VBO for all marks
        let mut vbo = 0;
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER, 
            (vertices.len() * std::mem::size_of::<f32>()) as isize, 
            vertices.as_ptr() as *const _, 
            gl::STATIC_DRAW
        );

        // Set up vertex attributes
        let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr());
        let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr());

        gl::EnableVertexAttribArray(pos_attr as u32);
        gl::VertexAttribPointer(pos_attr as u32, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
        gl::EnableVertexAttribArray(color_attr as u32);
        gl::VertexAttribPointer(color_attr as u32, 3, gl::FLOAT, gl::FALSE, 20, (8) as *const _);

        // Single draw call for all marks
        let vertex_count = (vertices.len() / 5) as i32; // 5 floats per vertex
        gl::DrawArrays(gl::TRIANGLES, 0, vertex_count);

        // Clean up
        gl::DeleteBuffers(1, &vbo);
    }
}

impl Decorator for NeedleGaugeMarksDecorator {
    fn render(
        &self,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        unsafe {
            // Enable blending
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            // Get cached shader program
            let shader_program = Self::get_mark_shader();
            gl::UseProgram(shader_program);

            // Calculate center and use configured radius
            let center_x = bounds.x + bounds.width / 2.0;
            let center_y = bounds.y + bounds.height / 2.0;
            let radius = self.radius;

            // Calculate angle step between marks
            let angle_range = self.end_angle - self.start_angle;
            let angle_step = if self.num_marks > 1 {
                angle_range / (self.num_marks - 1) as f32
            } else {
                0.0
            };

            // Build all vertices in a single buffer for batch rendering
            let mut all_vertices = Vec::with_capacity((self.num_marks * 6 * 5) as usize); // 6 vertices per mark, 5 floats per vertex

            for i in 0..self.num_marks {
                let angle = self.start_angle + (i as f32) * angle_step;
                
                // Properly normalize negative angles to 0-2π range
                let normalized_angle = if angle < 0.0 {
                    angle + 2.0 * PI
                } else {
                    angle % (2.0 * PI)
                };

                // Calculate mark vertices
                let mark_vertices = self.calculate_mark_vertices(
                    center_x, center_y, radius, normalized_angle,
                    context.width as f32, context.height as f32
                );
                
                // Append to batch buffer
                all_vertices.extend_from_slice(&mark_vertices);
            }

            // Single batched draw call for all marks
            self.render_batched_marks(&all_vertices, shader_program);
        }
        Ok(())
    }
}

pub struct NeedleGaugeMarkLabelsDecorator {
    labels: Vec<String>,
    font_path: String,
    font_size: u32,
    color: (f32, f32, f32),
    radius: f32,
    start_angle: f32,
    end_angle: f32,
}

impl NeedleGaugeMarkLabelsDecorator {
    pub fn new(
        labels: Vec<String>,
        font_path: String,
        font_size: u32,
        color: (f32, f32, f32),
        radius: f32,
        start_angle: f32,
        end_angle: f32,
    ) -> Self {
        Self {
            labels,
            font_path,
            font_size,
            color,
            radius,
            start_angle,
            end_angle,
        }
    }

    /// Calculate the position for a label at a specific angle
    fn calculate_label_position(&self, center_x: f32, center_y: f32, angle: f32) -> (f32, f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        
        let x = center_x + cos_a * self.radius;
        let y = center_y + sin_a * self.radius;
        
        (x, y)
    }
}

impl Decorator for NeedleGaugeMarkLabelsDecorator {
    fn render(
        &self,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        if self.labels.is_empty() {
            return Ok(());
        }

        // Calculate center position
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;

        // Calculate angle step between labels
        let angle_range = self.end_angle - self.start_angle;
        let angle_step = if self.labels.len() > 1 {
            angle_range / (self.labels.len() - 1) as f32
        } else {
            0.0
        };

        // Render each label at its calculated position
        for (i, label) in self.labels.iter().enumerate() {
            let angle = self.start_angle + (i as f32) * angle_step;
            
            // Normalize angle to 0-2π range
            let normalized_angle = if angle < 0.0 {
                angle + 2.0 * PI
            } else {
                angle % (2.0 * PI)
            };

            // Calculate label position
            let (label_x, label_y) = self.calculate_label_position(center_x, center_y, normalized_angle);

            // Center the text at the calculated position
            // Estimate text width and height for centering
            let estimated_text_width = label.len() as f32 * self.font_size as f32 * 0.6; // Rough estimate
            let estimated_text_height = self.font_size as f32;
            
            let centered_x = label_x - estimated_text_width / 2.0;
            let centered_y = label_y - estimated_text_height / 2.0; // Adjust for baseline positioning

            // Render the text label using the graphics context
            context.render_text_with_font(
                label,
                centered_x,
                centered_y,
                1.0, // scale
                self.color,
                &self.font_path,
                self.font_size,
            )?;
        }

        Ok(())
    }
}