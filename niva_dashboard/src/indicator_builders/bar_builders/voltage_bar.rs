use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::vertical_bar_indicator::{VerticalBarIndicator, VerticalBarScaleDecorator};
use crate::indicators::decorator::{LabelDecorator, DecoratorAlignmentH, DecoratorAlignmentV};
use crate::graphics::ui_style::*;

/// Build a voltage vertical bar indicator with customizable position and styling
/// 
/// # Parameters
/// - `x`: X coordinate of the indicator position
/// - `y`: Y coordinate of the indicator position  
/// - `width`: Width of the indicator
/// - `height`: Height of the indicator
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed voltage bar indicator ready for rendering
pub fn build_voltage_bar(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Bar configuration from UI style
    let segment_count = ui_style.get_integer(StyleKey::BarSegmentCount) as usize;
    let segment_gap = ui_style.get_float(StyleKey::BarSegmentGap);

    // Text styling from UI configuration
    let font_path = ui_style.get_string(StyleKey::TextSecondaryFont);
    let title_font_size = ui_style.get_integer(StyleKey::TextPrimaryFontSize) as u32;
    let unit_font_size = ui_style.get_integer(StyleKey::TextSecondaryFontSize) as u32;
    let scale_font_size = ui_style.get_integer(StyleKey::TextSecondaryFontSize) as u32;
    
    // Scale marks styling
    let marks_width = ui_style.get_float(StyleKey::BarMarksWidth);
    let marks_thickness = ui_style.get_float(StyleKey::BarMarksThickness);

    let voltage_bar = VerticalBarIndicator::new(segment_count)
        .with_segment_gap(segment_gap)
        .with_decorators(vec![
            // Title label
            Box::new(LabelDecorator::new(
                "СЕТЬ".into(),
                font_path.clone(),
                title_font_size,
                StyleKey::BarMarkLabelsColor,
                DecoratorAlignmentH::Center,
                DecoratorAlignmentV::Top,
            )),
            // Unit label
            Box::new(LabelDecorator::new(
                "В".into(),
                font_path.clone(),
                unit_font_size,
                StyleKey::BarMarkLabelsColor,
                DecoratorAlignmentH::Center,
                DecoratorAlignmentV::Bottom,
            )),
            // Scale with marks
            Box::new(VerticalBarScaleDecorator::new(
                vec!["16-".into(), "12-".into(), "8-".into()],
                font_path,
                scale_font_size,
                StyleKey::BarMarkLabelsColor,
                DecoratorAlignmentH::Left,
            ).with_scale_marks(StyleKey::BarMarksColor, marks_width, marks_thickness)),
        ]);

    let bounds = IndicatorBounds::new(x, y, width, height);
    (Box::new(voltage_bar), bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_voltage_bar_bounds_and_type() {
        let ui_style = UIStyle::from_file(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json"))
            .expect("repo ui_style.json should load and validate");
        let (indicator, bounds) = build_voltage_bar(2.0, 4.0, 70.0, 260.0, &ui_style);

        assert_eq!(bounds.x, 2.0);
        assert_eq!(bounds.y, 4.0);
        assert_eq!(bounds.width, 70.0);
        assert_eq!(bounds.height, 260.0);
        assert_eq!(indicator.indicator_type(), "VerticalBarIndicator");
    }
}
