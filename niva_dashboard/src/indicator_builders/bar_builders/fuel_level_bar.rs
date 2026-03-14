use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::vertical_bar_indicator::{VerticalBarIndicator, VerticalBarScaleDecorator};
use crate::indicators::decorator::{LabelDecorator, DecoratorAlignmentH, DecoratorAlignmentV};
use crate::graphics::ui_style::*;

/// Build a fuel level vertical bar indicator with customizable position and styling
/// 
/// # Parameters
/// - `x`: X coordinate of the indicator position
/// - `y`: Y coordinate of the indicator position  
/// - `width`: Width of the indicator
/// - `height`: Height of the indicator
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed fuel level bar indicator ready for rendering
pub fn build_fuel_level_bar(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Bar configuration from UI style
    let segment_count = ui_style.get_integer(BAR_SEGMENT_COUNT, 10) as usize;
    let segment_gap = ui_style.get_float(BAR_SEGMENT_GAP, 4.0);

    // Text styling from UI configuration
    let font_path = ui_style.get_string(TEXT_SECONDARY_FONT, DEFAULT_GLOBAL_FONT_PATH);
    let title_font_size = ui_style.get_integer(TEXT_SECONDARY_FONT_SIZE, 14) as u32;
    let unit_font_size = ui_style.get_integer(TEXT_SECONDARY_FONT_SIZE, 10) as u32;
    let scale_font_size = ui_style.get_integer(TEXT_SECONDARY_FONT_SIZE, 10) as u32;
    let text_color = ui_style.get_color(BAR_MARK_LABELS_COLOR, (0.45, 0.45, 0.45));
    
    // Scale marks styling
    let marks_color = ui_style.get_color(BAR_MARKS_COLOR, (1.0, 0.5, 0.0));
    let marks_width = ui_style.get_float(BAR_MARKS_WIDTH, 10.0);
    let marks_thickness = ui_style.get_float(BAR_MARKS_THICKNESS, 4.0);

    let fuel_level_bar = VerticalBarIndicator::new(segment_count)
        .with_segment_gap(segment_gap)
        .with_decorators(vec![
            // Title label
            Box::new(LabelDecorator::new(
                "ТОПЛ".into(),
                font_path.clone(),
                title_font_size,
                text_color,
                DecoratorAlignmentH::Center,
                DecoratorAlignmentV::Top,
            )),
            // Unit label
            Box::new(LabelDecorator::new(
                "%".into(),
                font_path.clone(),
                unit_font_size,
                text_color,
                DecoratorAlignmentH::Center,
                DecoratorAlignmentV::Bottom,
            )),
            // Scale with marks
            Box::new(VerticalBarScaleDecorator::new(
                vec!["1".into(), "1/2".into(), "0".into()],
                font_path,
                scale_font_size,
                text_color,
                DecoratorAlignmentH::Left,
            ).with_scale_marks(marks_color, marks_width, marks_thickness)),
        ]);

    let bounds = IndicatorBounds::new(x, y, width, height);
    (Box::new(fuel_level_bar), bounds)
}
