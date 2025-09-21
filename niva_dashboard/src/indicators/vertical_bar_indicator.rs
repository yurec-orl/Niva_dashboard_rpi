use crate::indicators::indicator::{Indicator, IndicatorBounds, IndicatorBase};
use crate::indicators::decorator::Decorator;
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::{SensorValue, ValueData};

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
        
        // Render each segment from bottom to top
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
            
            // Render the segment as a filled rectangle
            context.fill_rect(segments_start_x, segment_y, segment_width, segment_height, color)?;
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
