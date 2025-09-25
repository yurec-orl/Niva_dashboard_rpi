use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::needle_indicator::{NeedleIndicator, NeedleGaugeMarksDecorator};
use crate::indicators::decorator::ArcDecorator;
use crate::graphics::ui_style::*;
use std::f32::consts::PI;

/// Build a temperature gauge with customizable center point, radius and styling
/// 
/// # Parameters
/// - `center_x`: X coordinate of the gauge center
/// - `center_y`: Y coordinate of the gauge center  
/// - `radius`: Radius of the gauge
/// - `ui_style`: UI styling configuration
///
/// # Returns
/// A boxed temperature gauge indicator ready for rendering
pub fn build_temperature_gauge(
    center_x: f32,
    center_y: f32,
    radius: f32,
    ui_style: &UIStyle,
) -> (Box<dyn Indicator>, IndicatorBounds) {
    // Temperature gauge configuration
    let start_angle = -225.0f32.to_radians(); // Start at 7 o'clock position
    let end_angle = 45.0f32.to_radians();     // End at 1 o'clock position
    let needle_length = radius * ui_style.get_float(GAUGE_NEEDLE_LENGTH, 0.8);
    let needle_base_width = ui_style.get_float(GAUGE_NEEDLE_WIDTH, 8.0);
    let needle_tip_width = ui_style.get_float(GAUGE_NEEDLE_TIP_WIDTH, 1.0);
    let needle_color = ui_style.get_color(GAUGE_NEEDLE_COLOR, (1.0, 0.0, 0.0));

    // Border arc parameters
    let arc_color = ui_style.get_color(GAUGE_BORDER_COLOR, (1.0, 1.0, 1.0));
    let inactive_arc_color = ui_style.get_color(GAUGE_INACTIVE_ZONE_COLOR, (0.2, 0.2, 0.2));
    let arc_width = ui_style.get_float(GAUGE_INACTIVE_ZONE_WIDTH, 4.0);

    // Style parameters from UI configuration
    let major_marks_color = ui_style.get_color(GAUGE_MAJOR_MARK_COLOR, (1.0, 1.0, 1.0));
    let minor_marks_color = ui_style.get_color(GAUGE_MINOR_MARK_COLOR, (1.0, 1.0, 1.0));

    let gauge_minor_mark_length = ui_style.get_float(GAUGE_MINOR_MARK_LENGTH, 6.0);
    let gauge_minor_mark_thickness = ui_style.get_float(GAUGE_MINOR_MARK_WIDTH, 2.0);
    let gauge_major_mark_length = ui_style.get_float(GAUGE_MAJOR_MARK_LENGTH, 12.0);
    let gauge_major_mark_thickness = ui_style.get_float(GAUGE_MAJOR_MARK_WIDTH, 4.0);

    let temperature_gauge = NeedleIndicator::new(
        start_angle,
        end_angle,
        needle_length,
        needle_base_width,
        needle_tip_width,
        needle_color,
    ).with_decorators(vec![
        // Fine marks for temperature readings (50-120°C)
        Box::new(NeedleGaugeMarksDecorator::new(
            7, // 7 marks for temperature range
            gauge_minor_mark_length,
            gauge_minor_mark_thickness,
            minor_marks_color,
            radius,
            start_angle,
            end_angle,
        )),
        // Major marks for main temperature levels (Cold, Normal, Hot)
        Box::new(NeedleGaugeMarksDecorator::new(
            3, // 3 major marks (Cold, Normal, Hot)
            gauge_major_mark_length,
            gauge_major_mark_thickness,
            major_marks_color,
            radius,
            start_angle,
            end_angle,
        )),
        // Active arc (white) covering the valid range
        Box::new(ArcDecorator::new(
            radius,
            arc_width,
            arc_color,
            start_angle,
            end_angle,
        )),
        // Inactive arc (dark grey) for the remaining circle
        Box::new(ArcDecorator::new(
            radius,
            arc_width, // Arc thickness
            inactive_arc_color,
            end_angle,
            start_angle + 2.0 * PI, // Complete the circle
        )),
    ]);

    let bounds = IndicatorBounds::new(
        center_x - radius,
        center_y - radius,
        radius * 2.0,
        radius * 2.0,
    );

    (Box::new(temperature_gauge), bounds)
}
