use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::hardware::sensor_value::{SensorValue, ValueData};

/// A circular gauge indicator with a rotating needle, similar to automotive gauges
/// Features:
/// - Circular border with tick marks
/// - Numbered scale marks
/// - Animated triangular needle with glow effect
/// - Value display text
/// - Color coding based on warning/critical thresholds
pub struct GaugeIndicator;

impl GaugeIndicator {
    pub fn new() -> Self {
        Self {}
    }
}

impl Indicator for GaugeIndicator {
    fn with_decorators(self, _decorators: Vec<Box<dyn crate::indicators::decorator::Decorator>>) -> Self {
        // Simple implementation - decorators not yet integrated
        self
    }

    fn render(&self, 
              value: &SensorValue, 
              bounds: IndicatorBounds, 
              style: &UIStyle, 
              context: &mut GraphicsContext) -> Result<(), String> {
        
        // Calculate gauge dimensions from bounds
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;
        let radius = f32::min(bounds.width, bounds.height) / 2.0;
        
        let outer_radius = radius;
        let inner_radius = radius - 5.0;
        let needle_length = radius * 0.8;
        let mark_radius = inner_radius - 15.0;
        let number_radius = mark_radius - 5.0;
        
        // Get numeric value and constraints
        let current_value = value.as_f32();
        let min_value = value.constraints.min_value;
        let max_value = value.constraints.max_value;
        
        // Get colors from UIStyle using constants
        let needle_color = style.get_color_rgba(NEEDLE_COLOR, (1.0, 0.0, 0.0, 1.0));
        let border_color = style.get_color_rgba(GAUGE_BORDER_COLOR, (0.4, 0.4, 0.5, 1.0));
        let mark_color = style.get_color_rgba(GAUGE_MAJOR_MARK_COLOR, (0.9, 0.9, 1.0, 1.0));
        let text_color = style.get_color_rgba(GAUGE_LABEL_COLOR, (1.0, 1.0, 1.0, 1.0));
        // Only use RGB for rendering
        let needle_color = (needle_color.0, needle_color.1, needle_color.2);
        let border_color = (border_color.0, border_color.1, border_color.2);
        let mark_color = (mark_color.0, mark_color.1, mark_color.2);
        let text_color = (text_color.0, text_color.1, text_color.2);

        let needle_glow = style.get_bool(NEEDLE_GLOW_ENABLED, false);

        let start_angle = -225.0f32.to_radians(); // Start at bottom-left
        let end_angle = 45.0f32.to_radians();     // End at bottom-right (270 degrees total)

        let num_marks = 6; // Number of tick marks
        
        unsafe {
            // Enable blending for smooth rendering
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            
            // Create shader program for shapes
            let shader_program = Self::create_simple_color_shader();
            
            // Render gauge components
            self.render_gauge_circle_border(center_x, center_y, outer_radius, inner_radius, 
                                          border_color, context.width as f32, context.height as f32, shader_program);
            
            self.render_gauge_marks(center_x, center_y, mark_radius, start_angle, end_angle, 
                                  num_marks, mark_color, context.width as f32, context.height as f32, shader_program);
            
            self.render_gauge_numbers(context, center_x, center_y, number_radius, 
                                      start_angle, end_angle, min_value, max_value, 
                                      num_marks, text_color, style)?;
            
            self.render_triangular_needle(center_x, center_y, needle_length, 
                                        start_angle, end_angle, min_value, max_value, 
                                        current_value, needle_color, needle_glow,
                                        context.width as f32, context.height as f32,
                                        shader_program);
            
            // Render center circle
            self.render_gauge_center_circle(center_x, center_y, 8.0, (0.4, 0.4, 0.5), 
                                          context.width as f32, context.height as f32, shader_program);
            
            // Clean up shader
            gl::DeleteProgram(shader_program);
        }
        
        Ok(())
    }
    
    fn indicator_type(&self) -> &'static str {
        "gauge"
    }
    
    fn supports_value_type(&self, value: &ValueData) -> bool {
        // Gauges work well with analog and percentage values
        matches!(value, ValueData::Analog(_) | ValueData::Percentage(_))
    }
}

impl GaugeIndicator {
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
    
    /// Render circular border for the gauge
    unsafe fn render_gauge_circle_border(&self, center_x: f32, center_y: f32, outer_radius: f32, inner_radius: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
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
    
    /// Render tick marks on the gauge
    unsafe fn render_gauge_marks(&self, center_x: f32, center_y: f32, radius: f32, start_angle: f32, end_angle: f32, num_marks: i32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
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
            
            gl::LineWidth(2.0);
            gl::DrawArrays(gl::LINES, 0, 2);
            
            gl::DeleteBuffers(1, &vbo);
        }
    }
    
    /// Render numbered scale marks
    fn render_gauge_numbers(&self, context: &mut GraphicsContext, center_x: f32, center_y: f32, radius: f32, start_angle: f32, end_angle: f32, min_value: f32, max_value: f32, num_marks: i32, color: (f32, f32, f32), style: &UIStyle) -> Result<(), String> {
        let angle_range = end_angle - start_angle;
        let value_range = max_value - min_value;
        
        // Use style for font path and size if available
        let font_path = style.get_string(GAUGE_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let font_size = style.get_integer(GAUGE_LABEL_FONT_SIZE, DEFAULT_GLOBAL_FONT_SIZE);
        let text_scale = 0.7;
        
        for i in 0..num_marks {
            let t = i as f32 / (num_marks - 1) as f32;
            let angle = start_angle + t * angle_range;
            let value = min_value + t * value_range;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let text = format!("{:.0}", value);
            
            // Calculate the target position on the line from gauge center towards the mark
            let target_x = center_x + cos_a * radius;
            let target_y = center_y + sin_a * radius;
            
            // Calculate text dimensions to find the center offset
            let (text_width, text_height) = context.calculate_text_dimensions_with_font(
                &text, 
                text_scale, 
                &font_path, 
                font_size
            )?;
            
            // Calculate the top-left corner position to center the text at the target position
            let text_x = target_x - text_width / 2.0;
            let text_y = target_y - text_height / 2.0;
            
            context.render_text_with_font(
                &text, 
                text_x, 
                text_y, 
                text_scale, 
                color,
                &font_path,
                font_size
            )?;
        }
        Ok(())
    }
    
    /// Render triangular needle with glow effect
    unsafe fn render_triangular_needle(&self, center_x: f32, center_y: f32, length: f32,
                                       start_angle: f32, end_angle: f32,
                                       min_value: f32, max_value: f32, current_value: f32,
                                       color: (f32, f32, f32), needle_glow: bool,
                                       screen_w: f32, screen_h: f32, shader_program: u32) {
        gl::UseProgram(shader_program);
        
        // Calculate needle angle based on value
        let value_ratio = if max_value == min_value {
            0.0
        } else {
            ((current_value - min_value) / (max_value - min_value)).clamp(0.0, 1.0)
        };
        let needle_angle = start_angle + value_ratio * (end_angle - start_angle);
        
        let cos_a = needle_angle.cos();
        let sin_a = needle_angle.sin();
        
        // Base needle parameters
        let base_needle_width = 16.0;
        let tip_needle_width = 6.0;
        let tip_x = center_x + cos_a * length;
        let tip_y = center_y + sin_a * length;
        
        let core_color = (1.0, 1.0, 1.0);

        // Render glow layers (from largest/faintest to smallest/brightest)
        let mut glow_layers = Vec::new();

        if needle_glow {
            // Glow effect layers
            glow_layers.push((3.0, color, 0.15)); // Outermost glow: 2.5x size, 15% opacity
            glow_layers.push((2.0, color, 0.25)); // Middle glow: 2.0x size, 25% opacity
            glow_layers.push((1.5, color, 0.40)); // Inner glow: 1.5x size, 40% opacity
            glow_layers.push((0.75, blend_colors(color, core_color, 0.7), 1.00)); // Core outer: 25% narrower, full opacity
            glow_layers.push((0.25, core_color, 1.00)); // Core needle: 75% narrower, full opacity
        } else {
            glow_layers.push((1.0, color, 1.0)); // Just the base color, no glow effect
        }

        for (size_multiplier, color, opacity) in glow_layers.iter() {
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
            
            // Apply progressive color brightness for glow effect
            let glow_color = 
                (
                    (color.0 * opacity).min(1.0),
                    (color.1 * opacity).min(1.0),
                    (color.2 * opacity).min(1.0),
                );
            
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
    
    /// Render center circle
    unsafe fn render_gauge_center_circle(&self, center_x: f32, center_y: f32, radius: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32, shader_program: u32) {
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
}
