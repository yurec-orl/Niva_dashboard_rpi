use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::digital_segmented_indicator::DigitalSegmentedIndicator;
use crate::indicators::decorator::{LabelDecorator, DecoratorAlignmentH, DecoratorAlignmentV};
use crate::graphics::ui_style::*;

/// Build a digital speed display with customizable position and styling
/// 
/// # Parameters
/// - `x`: X coordinate of the indicator position
/// - `y`: Y coordinate of the indicator position  
/// - `width`: Width of the indicator
/// - `height`: Height of the indicator
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed digital speed indicator ready for rendering
pub fn build_speed_digital(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Digital display configuration from UI style
    let digit_count = 3; // Speed typically shown as 3-digit number (0-999)
    let show_inactive_segments = true;

    // Text styling from UI configuration
    let font_path = ui_style.get_string(TEXT_SECONDARY_FONT, DEFAULT_GLOBAL_FONT_PATH);
    let unit_font_size = ui_style.get_integer(TEXT_SECONDARY_FONT_SIZE, 10) as u32;
    let text_color = ui_style.get_color(TEXT_SECONDARY_COLOR, (0.45, 0.45, 0.45));

    let speed_display = DigitalSegmentedIndicator::integer(digit_count)
        .with_inactive_segments(show_inactive_segments)
        .with_decorators(vec![
            // Unit label
            Box::new(LabelDecorator::new(
                "км/ч".into(),
                font_path,
                unit_font_size,
                text_color,
                DecoratorAlignmentH::Right,
                DecoratorAlignmentV::Bottom,
            )),
        ]);

    let bounds = IndicatorBounds::new(x, y, width, height);
    (Box::new(speed_display), bounds)
}
