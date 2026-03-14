use crate::indicators::indicator::{Indicator, IndicatorBounds, IndicatorBase};
use crate::indicators::decorator::{Decorator, DecoratorAlignmentH};
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use std::sync::Once;
use gl;

// Cached shader programs
static mut VERTICAL_BAR_SHADER_PROGRAM: u32 = 0;
static VERTICAL_BAR_SHADER_INIT: Once = Once::new();

/// Vertical bar indicator that fills from bottom to top
pub struct VerticalBarIndicator {
    base: IndicatorBase,
    /// Number of segments in the bar
    segments: usize,
    /// Gap between segments (in pixels)
    segment_gap: f32,
}

impl VerticalBarIndicator {
    /// Create a new vertical bar indicator
    pub fn new(segments: usize) -> Self {
        Self {
            base: IndicatorBase::new(),
            segments,
            segment_gap: 2.0, // Default 2px gap between segments
        }
    }
    
    /// Set the gap between segments
    pub fn with_segment_gap(mut self, gap: f32) -> Self {
        self.segment_gap = gap;
        self
    }
    
    /// Calculate which segments should be filled based on normalized value (0.0 to 1.0)
    fn calculate_filled_segments(&self, normalized_value: f32) -> usize {
        let clamped_value = normalized_value.clamp(0.0, 1.0);
        (clamped_value * self.segments as f32).round() as usize
    }
    
    /// Get segment color based on normalized position and value constraints
    fn get_segment_color(&self, segment_index: usize, normalized_value: f32, value: &SensorValue, style: &UIStyle) -> (f32, f32, f32) {
        let segment_position = (segment_index + 1) as f32 / self.segments as f32;
        
        // Check if we're in warning or critical range based on constraints
        if let Some(critical_high) = value.constraints.critical_high {
            let normalized_critical = (critical_high - value.constraints.min_value) / (value.constraints.max_value - value.constraints.min_value);
            if segment_position <= normalized_critical && normalized_value >= normalized_critical {
                return style.get_color("bar_critical_color", (1.0, 0.0, 0.0)); // Red for critical
            }
        }
        
        if let Some(warning_high) = value.constraints.warning_high {
            let normalized_warning = (warning_high - value.constraints.min_value) / (value.constraints.max_value - value.constraints.min_value);
            if segment_position <= normalized_warning && normalized_value >= normalized_warning {
                return style.get_color("bar_warning_color", (1.0, 0.65, 0.0)); // Orange for warning
            }
        }
        
        // Default normal color
        style.get_color("bar_normal_color", (0.0, 1.0, 0.0)) // Green for normal
    }

    /// Get cached shader program for batch rendering
    unsafe fn get_vertical_bar_shader() -> u32 {
        VERTICAL_BAR_SHADER_INIT.call_once(|| {
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

            VERTICAL_BAR_SHADER_PROGRAM = program;
        });
        VERTICAL_BAR_SHADER_PROGRAM
    }

    /// Calculate vertices for a single rectangle segment (returns 30 floats: 6 vertices × 5 components each)
    fn calculate_segment_vertices(&self, x: f32, y: f32, width: f32, height: f32, 
                                 color: (f32, f32, f32), screen_w: f32, screen_h: f32) -> [f32; 30] {
        // Convert screen coordinates to normalized coordinates (-1 to 1)
        let x1_norm = x / screen_w * 2.0 - 1.0;
        let y1_norm = 1.0 - y / screen_h * 2.0;
        let x2_norm = (x + width) / screen_w * 2.0 - 1.0;
        let y2_norm = 1.0 - (y + height) / screen_h * 2.0;

        // Return vertices for two triangles forming a rectangle
        [
            // First triangle: top-left -> top-right -> bottom-left
            x1_norm, y1_norm, color.0, color.1, color.2,
            x2_norm, y1_norm, color.0, color.1, color.2,
            x1_norm, y2_norm, color.0, color.1, color.2,
            // Second triangle: top-right -> bottom-right -> bottom-left  
            x2_norm, y1_norm, color.0, color.1, color.2,
            x2_norm, y2_norm, color.0, color.1, color.2,
            x1_norm, y2_norm, color.0, color.1, color.2,
        ]
    }

    /// Render all segments in a single batched draw call for optimal performance
    unsafe fn render_batched_segments(&self, vertices: &[f32], shader_program: u32) {
        // Create and bind VBO for all segments
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

        // Single draw call for all segments
        let vertex_count = (vertices.len() / 5) as i32; // 5 floats per vertex
        gl::DrawArrays(gl::TRIANGLES, 0, vertex_count);

        // Clean up
        gl::DeleteBuffers(1, &vbo);
    }
}

impl Default for VerticalBarIndicator {
    fn default() -> Self {
        Self::new(10) // Default to 10 segments
    }
}

impl Indicator for VerticalBarIndicator {
    fn with_decorators(mut self, decorators: Vec<Box<dyn Decorator>>) -> Self {
        self.base.decorators = decorators;
        self
    }

    fn render(
        &self,
        value: &SensorValue,
        bounds: IndicatorBounds,
        style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        // Extract numeric value
        let numeric_value = match &value.value {
            ValueData::Analog(v) => *v,
            ValueData::Integer(i) => *i as f32,
            ValueData::Percentage(p) => *p,
            _ => return Ok(()), // Skip non-numeric values
        };

        // Render decorators first, then the display itself over the decorators
        self.base.render_decorators(bounds, style, context)?;
        
        let background_enabled = style.get_bool(BAR_BACKGROUND_ENABLED, true);
        let border_enabled = style.get_bool(BAR_BORDER_ENABLED, true);
        let border_width = style.get_float(BAR_BORDER_WIDTH, 4.0);

        if background_enabled {
            let bg_color = style.get_color(BAR_BACKGROUND_COLOR, (1.0, 0.65, 0.0)); // Default amber
            context.render_rectangle(bounds.x, bounds.y, bounds.width, bounds.height,
                bg_color, true, 1.0,
                style.get_float(BAR_CORNER_RADIUS, 8.0))?;
        } else if border_enabled {
            let border_color = style.get_color(BAR_BORDER_COLOR, (1.0, 0.65, 0.0)); // Default amber
            context.render_rectangle(bounds.x, bounds.y, bounds.width, bounds.height,
                border_color, false, border_width,
                style.get_float(BAR_CORNER_RADIUS, 8.0))?;
        }

        // Normalize the value to 0.0-1.0 range
        let normalized_value = ((numeric_value - value.constraints.min_value) / 
                               (value.constraints.max_value - value.constraints.min_value)).clamp(0.0, 1.0);
        
        // Calculate how many segments should be filled
        let filled_segments = self.calculate_filled_segments(normalized_value);
        
        // Calculate margins based on background and border settings
        let margin = if background_enabled || border_enabled {
            let base_margin = self.segment_gap;
            if border_enabled {
                base_margin + border_width  // Add border width to prevent overlap
            } else {
                base_margin  // Just the gap for background-only
            }
        } else {
            0.0  // No background/border: no margins
        };
        
        // Calculate available area for segments (accounting for margins)
        let available_width = bounds.width - (2.0 * margin);
        let available_height = bounds.height - (2.0 * margin);
        let segments_start_x = bounds.x + margin;
        let segments_start_y = bounds.y + margin;
        
        // Calculate segment dimensions within the available area
        let total_gaps = (self.segments - 1) as f32 * self.segment_gap;
        let segment_height = (available_height - total_gaps) / self.segments as f32;
        let segment_width = available_width;
        
        // Get background color for empty segments
        let empty_color = style.get_color("bar_empty_color", (0.2, 0.2, 0.2)); // Dark gray for empty
        
        unsafe {
            // Enable blending for smooth rendering
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

            // Get cached shader program for batch rendering
            let shader_program = Self::get_vertical_bar_shader();
            gl::UseProgram(shader_program);

            // Build all vertices in a single buffer for batch rendering
            let mut all_vertices = Vec::with_capacity(self.segments * 6 * 5); // 6 vertices per segment, 5 floats per vertex

            // Generate vertices for each segment from bottom to top
            for i in 0..self.segments {
                let segment_index_from_bottom = self.segments - 1 - i; // Bottom segment = 0, top segment = segments-1
                
                // Calculate segment position (from top of available area)
                let segment_y = segments_start_y + (i as f32 * (segment_height + self.segment_gap));
                
                // Determine if this segment should be filled
                let is_filled = segment_index_from_bottom < filled_segments;
                
                // Get appropriate color
                let color = if is_filled {
                    self.get_segment_color(segment_index_from_bottom, normalized_value, value, style)
                } else {
                    empty_color
                };
                
                // Calculate segment vertices and append to batch buffer
                let segment_vertices = self.calculate_segment_vertices(
                    segments_start_x, segment_y, segment_width, segment_height, 
                    color, context.width as f32, context.height as f32
                );
                
                all_vertices.extend_from_slice(&segment_vertices);
            }

            // Single batched draw call for all segments
            self.render_batched_segments(&all_vertices, shader_program);
        }
        
        Ok(())
    }
    
    fn indicator_type(&self) -> &'static str {
        "VerticalBarIndicator"
    }
    
    fn supports_value_type(&self, value: &ValueData) -> bool {
        matches!(value, ValueData::Analog(_) | ValueData::Integer(_) | ValueData::Percentage(_))
    }
}

/// Decorator for rendering scale marks and labels vertically alongside a vertical bar indicator
/// Labels are ordered from top to bottom
/// Scale marks are optional and can be enabled during construction
pub struct VerticalBarScaleDecorator {
    labels: Vec<String>,    // Labels for each scale mark - no labels if empty
    font_path: String,
    font_size: u32,
    color: (f32, f32, f32),
    scale_marks: bool,      // Whether to draw scale marks
    marks_color: (f32, f32, f32),
    marks_width: f32,
    marks_thickness: f32,
    alignment_h: DecoratorAlignmentH,
}

impl VerticalBarScaleDecorator {
    /// Create a new vertical bar scale decorator
    /// Labels are ordered from top to bottom
    pub fn new(
        labels: Vec<String>,
        font_path: String,
        font_size: u32,
        color: (f32, f32, f32),
        alignment_h: DecoratorAlignmentH,
    ) -> Self {
        Self {
            labels,
            scale_marks: false,
            font_path,
            font_size,
            color,
            marks_color: (1.0, 1.0, 1.0),
            marks_thickness: 1.0,
            marks_width: 5.0,
            alignment_h,
        }
    }

    /// Enable scale marks with specified color, width and thickness
    pub fn with_scale_marks(mut self, color: (f32, f32, f32), width: f32, thickness: f32) -> Self {
        self.scale_marks = true;
        self.marks_color = color;
        self.marks_width = width;
        self.marks_thickness = thickness;
        self
    }
}

impl Decorator for VerticalBarScaleDecorator {
    fn render(
        &self,
        bounds: IndicatorBounds,
        _style: &UIStyle,
        context: &mut GraphicsContext,
    ) -> Result<(), String> {
        let segment_count = self.labels.len();
        if segment_count == 0 {
            return Ok(()); // Nothing to render
        }
        
        let mut base_x_pos = match self.alignment_h {
            DecoratorAlignmentH::Left => bounds.x - self.marks_thickness, // 5px margin
            DecoratorAlignmentH::Right => bounds.x + bounds.width + self.marks_thickness,
            DecoratorAlignmentH::Center => Err("Center alignment not supported".to_string())?,
        };
        let segment_height = bounds.height / segment_count as f32;

        // Draw scale marks if enabled
        if self.scale_marks {
            base_x_pos += match self.alignment_h {
                DecoratorAlignmentH::Left => -(self.marks_width + self.marks_thickness),
                DecoratorAlignmentH::Right => (self.marks_width + self.marks_thickness),
                DecoratorAlignmentH::Center => 0.0, // Not applicable
            };

            for i in 0..segment_count {
                let y = bounds.y + i as f32 * segment_height + segment_height / 2.0;
                context.render_rectangle(base_x_pos, y - self.marks_thickness / 2.0,
                                         self.marks_width, self.marks_thickness,
                                         self.marks_color, true, 1.0, 0.0)?;
            }
            // context.render_rectangle(base_x_pos, bounds.y + segment_height / 2.0,
            //                          self.marks_thickness,
            //                          (segment_count - 1) as f32 * segment_height,
            //                          self.marks_color, true, 1.0, 0.0)?; // Vertical line
        }

        for (i, label) in self.labels.iter().enumerate() {
            let y = bounds.y + i as f32 * segment_height + (segment_height - self.font_size as f32) / 2.0;
            let x = match self.alignment_h {
                DecoratorAlignmentH::Left => base_x_pos - 5.0 - context.calculate_text_width_with_font(label, 1.0, &self.font_path, self.font_size)?,
                DecoratorAlignmentH::Right => base_x_pos + bounds.width + 5.0,
                DecoratorAlignmentH::Center => Err("Center alignment not supported".to_string())?,
            };
            
            context.render_text_with_font(
                label,
                x,
                y,
                1.0, // scale
                self.color,
                &self.font_path,
                self.font_size,
            )?;
        }
        
        Ok(())
    }
}