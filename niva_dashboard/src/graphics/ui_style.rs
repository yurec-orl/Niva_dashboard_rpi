#![allow(dead_code)]

//! UI Style Configuration System
//!
//! Every style value lives in one checked-in JSON file (see `default_path()`), keyed by
//! the `StyleKey` enum below. There are no compiled-in style defaults: a key missing from
//! the file, or present with the wrong kind of value, fails validation in `from_file`/
//! `from_json` -- the caller (see `main.rs::setup_ui_style`) treats that like any other
//! bad config file and shows the config-error screen rather than falling back silently.
//!
//! Example JSON format (bare literals, no type tags -- the expected type comes from each
//! key's declared `ValueKind`, not from how the JSON value happens to be shaped):
//! ```json
//! {
//!   "gauge_needle_color": "#FF0000",
//!   "gauge_background_color": "#000000",
//!   "gauge_mark_font_size": 14,
//!   "gauge_major_mark_width": 2.0,
//!   "bar_fill_color": "#00FF00",
//!   "global_contrast": 1.0
//! }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// =============================================================================
// BOOTSTRAP FONT CONSTANTS
// =============================================================================
// These stay as plain Rust constants, outside the StyleKey/JSON system: they're used
// where a font is needed independently of whether the style file loaded successfully --
// main.rs's config_error_fallback_loop (which renders the error screen when e.g. THIS
// file fails to load) and IndicatorBase's own struct Default (text_indicator.rs).

pub const DEFAULT_GLOBAL_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/OpenGostTypeB.ttf";  // Use monospace for more digital look
pub const DEFAULT_GLOBAL_FONT_SIZE: u32 = 18;

// Digital Display Fonts
pub const DIGITAL_DISPLAY_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG7ClassicMini-Regular.ttf";
pub const DIGITAL_DISPLAY_FONT_ITALIC_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG7ClassicMini-Italic.ttf";
pub const DIGITAL_DISPLAY_14SEG_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG14ClassicMini-Regular.ttf";
pub const DIGITAL_DISPLAY_14SEG_ITALIC_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG14ClassicMini-Italic.ttf";
pub const DIGITAL_DISPLAY_MONO_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/OpenGostTypeB.ttf";

// Terminal-style monospace font, for scrolling text boxes (log/ADC diagnostic output)
pub const TERMINAL_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DejaVuSansMono.ttf";

// =============================================================================
// STYLE KEYS
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Color,
    Float,
    Integer,
    Boolean,
    String,
}

/// Declares the `StyleKey` enum from one list of `Variant: Kind = "json_name"` entries,
/// plus `StyleKey::ALL` (every variant, for exhaustive load-time validation),
/// `StyleKey::json_name()`, and `StyleKey::kind()`. One list to edit when adding a key --
/// nothing to keep in sync by hand.
macro_rules! define_style_keys {
    ($($variant:ident : $kind:ident = $json_name:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum StyleKey {
            $($variant),+
        }

        impl StyleKey {
            pub const ALL: &'static [StyleKey] = &[$(StyleKey::$variant),+];

            pub fn json_name(&self) -> &'static str {
                match self {
                    $(StyleKey::$variant => $json_name),+
                }
            }

            pub fn kind(&self) -> ValueKind {
                match self {
                    $(StyleKey::$variant => ValueKind::$kind),+
                }
            }
        }
    };
}

define_style_keys! {
    // Global
    GlobalContrast: Float = "global_contrast",
    GlobalBackgroundColor: Color = "global_background_color",
    GlobalFontPath: String = "global_font_path",
    GlobalFontSize: Integer = "global_font_size",

    // Page manager
    PageButtonLabelFont: String = "page_button_label_font",
    PageButtonLabelFontSize: Integer = "page_button_label_font_size",
    PageButtonLabelOrientation: String = "page_button_label_orientation", // "horizontal" or "vertical"
    PageButtonLabelColor: Color = "page_button_label_color",
    PageButtonPressedFrameColor: Color = "page_button_pressed_frame_color",
    PageButtonPressedFrameWidth: Float = "page_button_pressed_frame_width",
    PageButtonPressedFramePadding: Float = "page_button_pressed_frame_padding",
    PageStatusFont: String = "page_status_font",
    PageStatusFontSize: Integer = "page_status_font_size",
    PageStatusColor: Color = "page_status_color",

    // Gauge
    GaugeBackgroundColor: Color = "gauge_background_color",
    GaugeBorderColor: Color = "gauge_border_color",
    GaugeBorderWidth: Float = "gauge_border_width",
    GaugeRadius: Float = "gauge_radius",

    // Gauge needle
    GaugeNeedleColor: Color = "gauge_needle_color",
    GaugeNeedleWidth: Float = "gauge_needle_width",
    GaugeNeedleLength: Float = "gauge_needle_length",
    GaugeNeedleTipWidth: Float = "gauge_needle_tip_width",
    GaugeNeedleCenterColor: Color = "gauge_needle_center_color",
    GaugeNeedleCenterRadius: Float = "gauge_needle_center_radius",
    GaugeNeedleShadowEnabled: Boolean = "gauge_needle_shadow_enabled",
    GaugeNeedleShadowColor: Color = "gauge_needle_shadow_color",
    GaugeNeedleGlowEnabled: Boolean = "gauge_needle_glow_enabled",

    // Gauge marks
    GaugeMajorMarkColor: Color = "gauge_major_mark_color",
    GaugeMajorMarkWidth: Float = "gauge_major_mark_width",
    GaugeMajorMarkLength: Float = "gauge_major_mark_length",
    GaugeMajorMarkOffset: Float = "gauge_major_mark_offset",
    GaugeMajorMarkEnabled: Boolean = "gauge_major_mark_enabled",
    GaugeMajorMarkCount: Integer = "gauge_major_mark_count",
    GaugeMinorMarkColor: Color = "gauge_minor_mark_color",
    GaugeMinorMarkWidth: Float = "gauge_minor_mark_width",
    GaugeMinorMarkLength: Float = "gauge_minor_mark_length",
    GaugeMinorMarkOffset: Float = "gauge_minor_mark_offset",
    GaugeMinorMarkEnabled: Boolean = "gauge_minor_mark_enabled",
    GaugeMinorMarkCount: Integer = "gauge_minor_mark_count",

    // Gauge labels
    GaugeLabelColor: Color = "gauge_label_color",
    GaugeLabelFont: String = "gauge_label_font",
    GaugeLabelFontSize: Integer = "gauge_label_font_size",
    GaugeLabelOffset: Float = "gauge_label_offset",
    GaugeLabelEnabled: Boolean = "gauge_label_enabled",
    GaugeTitleColor: Color = "gauge_title_color",
    GaugeTitleFont: String = "gauge_title_font",
    GaugeTitleFontSize: Integer = "gauge_title_font_size",
    GaugeTitleOffsetH: Float = "gauge_title_offset_h",
    GaugeTitleOffsetV: Float = "gauge_title_offset_v",
    GaugeTitleEnabled: Boolean = "gauge_title_enabled",
    GaugeUnitColor: Color = "gauge_unit_color",
    GaugeUnitFont: String = "gauge_unit_font",
    GaugeUnitFontSize: Integer = "gauge_unit_font_size",
    GaugeUnitOffsetH: Float = "gauge_unit_offset_h",
    GaugeUnitOffsetV: Float = "gauge_unit_offset_v",
    GaugeUnitEnabled: Boolean = "gauge_unit_enabled",

    // Gauge zones
    GaugeNormalZoneColor: Color = "gauge_normal_zone_color",
    GaugeNormalZoneWidth: Float = "gauge_normal_zone_width",
    GaugeNormalZoneEnabled: Boolean = "gauge_normal_zone_enabled",
    GaugeWarningZoneColor: Color = "gauge_warning_zone_color",
    GaugeWarningZoneWidth: Float = "gauge_warning_zone_width",
    GaugeWarningZoneEnabled: Boolean = "gauge_warning_zone_enabled",
    GaugeCriticalZoneColor: Color = "gauge_critical_zone_color",
    GaugeCriticalZoneWidth: Float = "gauge_critical_zone_width",
    GaugeCriticalZoneEnabled: Boolean = "gauge_critical_zone_enabled",
    GaugeInactiveZoneColor: Color = "gauge_inactive_zone_color",
    GaugeInactiveZoneWidth: Float = "gauge_inactive_zone_width",
    GaugeInactiveZoneEnabled: Boolean = "gauge_inactive_zone_enabled",

    // Bar indicator
    BarBackgroundColor: Color = "bar_background_color",
    BarBackgroundEnabled: Boolean = "bar_background_enabled",
    BarBorderColor: Color = "bar_border_color",
    BarBorderEnabled: Boolean = "bar_border_enabled",
    BarBorderWidth: Float = "bar_border_width",
    BarCornerRadius: Float = "bar_corner_radius",
    BarEmptyColor: Color = "bar_empty_color",
    BarNormalColor: Color = "bar_normal_color",
    BarWarningColor: Color = "bar_warning_color",
    BarCriticalColor: Color = "bar_critical_color",
    BarMarksColor: Color = "bar_marks_color",
    BarMarksWidth: Float = "bar_marks_width",
    BarMarksThickness: Float = "bar_marks_thickness",
    BarMarkLabelsColor: Color = "bar_mark_labels_color",
    BarSegmentCount: Integer = "bar_segment_count",
    BarSegmentGap: Float = "bar_segment_gap",

    // Compass
    CompassMajorMarkColor: Color = "compass_major_mark_color",
    CompassMinorMarkColor: Color = "compass_minor_mark_color",
    CompassLabelColor: Color = "compass_label_color",
    CompassLabelFont: String = "compass_label_font",
    CompassLabelFontSize: Integer = "compass_label_font_size",
    CompassArrowColor: Color = "compass_arrow_color",
    CompassCenterLineColor: Color = "compass_center_line_color",
    CompassHeadingColor: Color = "compass_heading_color",
    CompassHdopExcellentColor: Color = "compass_hdop_excellent_color",
    CompassHdopGoodColor: Color = "compass_hdop_good_color",
    CompassHdopModerateColor: Color = "compass_hdop_moderate_color",
    CompassHdopPoorColor: Color = "compass_hdop_poor_color",

    // Pitch (artificial horizon)
    PitchSkyColor: Color = "pitch_sky_color",
    PitchGroundColor: Color = "pitch_ground_color",
    PitchAboveHorizonLabelColor: Color = "pitch_above_horizon_label_color",
    PitchBelowHorizonLabelColor: Color = "pitch_below_horizon_label_color",
    PitchBorderColor: Color = "pitch_border_color",
    PitchBorderWidth: Float = "pitch_border_width",
    PitchLabelFont: String = "pitch_label_font",
    PitchLabelFontSize: Integer = "pitch_label_font_size",
    RollIndicatorColor: Color = "roll_indicator_color",
    RollScaleColor: Color = "roll_scale_color",
    RollScaleLabelFont: String = "roll_scale_label_font",
    RollScaleLabelFontSize: Integer = "roll_scale_label_font_size",

    // Text
    TextPrimaryColor: Color = "text_primary_color",
    TextSecondaryColor: Color = "text_secondary_color",
    TextAccentColor: Color = "text_accent_color",
    TextWarningColor: Color = "text_warning_color",
    TextErrorColor: Color = "text_error_color",
    TextPrimaryFont: String = "text_primary_font",
    TextPrimaryFontSize: Integer = "text_primary_font_size",
    TextSecondaryFont: String = "text_secondary_font",
    TextSecondaryFontSize: Integer = "text_secondary_font_size",
    TextMonospaceFont: String = "text_monospace_font",
    TextMonospaceFontSize: Integer = "text_monospace_font_size",
    TextSmallFont: String = "text_small_font",
    TextSmallFontSize: Integer = "text_small_font_size",
    TextLineSpacing: Float = "text_line_spacing",
    TextLetterSpacing: Float = "text_letter_spacing",

    // Terminal / scrolling text box
    TerminalBackgroundColor: Color = "terminal_background_color",
    TerminalBackgroundEnabled: Boolean = "terminal_background_enabled",
    TerminalBorderColor: Color = "terminal_border_color",
    TerminalBorderEnabled: Boolean = "terminal_border_enabled",
    TerminalBorderWidth: Float = "terminal_border_width",
    TerminalTextColor: Color = "terminal_text_color",
    TerminalPadding: Float = "terminal_padding",

    // Digital display (7-segment style)
    DigitalDisplayFont: String = "digital_display_font",
    DigitalDisplayFontSize: Integer = "digital_display_font_size",
    DigitalDisplayScale: Float = "digital_display_scale",
    DigitalDisplayActiveColor: Color = "digital_display_active_color",
    DigitalDisplayInactiveColor: Color = "digital_display_inactive_color",
    DigitalDisplayInactiveColorBlending: Float = "digital_display_inactive_color_blending",
    DigitalDisplayBackgroundColor: Color = "digital_display_background_color",
    DigitalDisplayBackgroundEnabled: Boolean = "digital_display_background_enabled",
    DigitalDisplayBorderEnabled: Boolean = "digital_display_border_enabled",
    DigitalDisplayBorderColor: Color = "digital_display_border_color",
    DigitalDisplayBorderWidth: Float = "digital_display_border_width",
    DigitalDisplayBorderRadius: Float = "digital_display_border_radius",
    DigitalDisplayFontItalic: String = "digital_display_font_italic",
    DigitalDisplay14SegFont: String = "digital_display_14seg_font",
    DigitalDisplay14SegItalic: String = "digital_display_14seg_italic",

    // Warning indicator
    IndicatorNormalColor: Color = "indicator_normal_color",
    IndicatorWarningColor: Color = "indicator_warning_color",
    IndicatorCriticalColor: Color = "indicator_critical_color",
    IndicatorOffColor: Color = "indicator_off_color",
    IndicatorBlinkSpeed: Float = "indicator_blink_speed",
    IndicatorGlowEnabled: Boolean = "indicator_glow_enabled",
    IndicatorGlowRadius: Float = "indicator_glow_radius",
    IndicatorSize: Float = "indicator_size",

    // Animation
    AnimationNeedleSpeed: Float = "animation_needle_speed",
    AnimationBarSpeed: Float = "animation_bar_speed",
    AnimationSmoothEnabled: Boolean = "animation_smooth_enabled",

    // Alerts
    AlertFontPath: String = "alert_font_path",
    AlertFontSize: Integer = "alert_font_size",
    AlertWarningColor: Color = "alert_warning_color",
    AlertCriticalColor: Color = "alert_critical_color",
    AlertBackgroundColor: Color = "alert_background_color",
    AlertBorderColor: Color = "alert_border_color",
    AlertBorderWidth: Float = "alert_border_width",
    AlertMargin: Float = "alert_margin",
    AlertCornerRadius: Float = "alert_corner_radius",
    AlertSoundPath: String = "alert_sound_path",
}

// =============================================================================
// VALUE COERCION
// =============================================================================
// Operates on raw serde_json::Value scalars rather than a hand-rolled tagged enum, so the
// checked-in JSON can use plain literals ("gauge_needle_color": "#FF0000") -- the expected
// type comes from the StyleKey's declared ValueKind, not from the JSON value's shape.

fn value_as_color(value: &serde_json::Value) -> Result<(f32, f32, f32), String> {
    match value.as_str() {
        Some(s) => parse_color(s),
        None => Err(format!("expected a color string, got {value}")),
    }
}

fn value_as_float(value: &serde_json::Value) -> Result<f32, String> {
    value.as_f64().map(|f| f as f32).ok_or_else(|| format!("expected a number, got {value}"))
}

fn value_as_integer(value: &serde_json::Value) -> Result<u32, String> {
    if let Some(u) = value.as_u64() {
        Ok(u as u32)
    } else if let Some(f) = value.as_f64() {
        Ok(f as u32)
    } else {
        Err(format!("expected an integer, got {value}"))
    }
}

fn value_as_bool(value: &serde_json::Value) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| format!("expected a boolean, got {value}"))
}

fn value_as_string(value: &serde_json::Value) -> Result<String, String> {
    value.as_str().map(|s| s.to_string()).ok_or_else(|| format!("expected a string, got {value}"))
}

fn validate_kind(value: &serde_json::Value, kind: ValueKind) -> Result<(), String> {
    match kind {
        ValueKind::Color => value_as_color(value).map(|_| ()),
        ValueKind::Float => value_as_float(value).map(|_| ()),
        ValueKind::Integer => value_as_integer(value).map(|_| ()),
        ValueKind::Boolean => value_as_bool(value).map(|_| ()),
        ValueKind::String => value_as_string(value).map(|_| ()),
    }
}

// =============================================================================
// UI STYLE
// =============================================================================

#[derive(Debug, Clone)]
pub struct UIStyle {
    values: HashMap<StyleKey, serde_json::Value>,
}

impl UIStyle {
    /// Where the checked-in style file lives -- alongside Cargo.toml in the niva_dashboard
    /// crate dir, the same HOME-based construction as sensor_config::default_path() /
    /// sensor_calibration::default_path().
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/niva_dashboard/ui_style.json"))
    }

    /// Loads and validates `path`. Fail-fast: every `StyleKey` must be present with a
    /// value that coerces to its declared `ValueKind`, or this returns `Err` listing
    /// *every* problem found (not just the first), so a bad file can be fixed in one pass.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let json_str = std::fs::read_to_string(path)
            .map_err(|e| format!("ui style: failed to read {path:?}: {e}"))?;
        Self::from_json(&json_str)
    }

    /// Loads and validates a style from a JSON string. See [`Self::from_file`].
    pub fn from_json(json_str: &str) -> Result<Self, String> {
        let raw: HashMap<String, serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| format!("ui style: failed to parse JSON: {e}"))?;

        let mut values = HashMap::with_capacity(StyleKey::ALL.len());
        let mut problems = Vec::new();

        for key in StyleKey::ALL {
            match raw.get(key.json_name()) {
                Some(value) => match validate_kind(value, key.kind()) {
                    Ok(()) => {
                        values.insert(*key, value.clone());
                    }
                    Err(reason) => problems.push(format!("'{}': {}", key.json_name(), reason)),
                },
                None => problems.push(format!("'{}': missing", key.json_name())),
            }
        }

        if !problems.is_empty() {
            return Err(format!(
                "ui style validation failed ({} problem(s)):\n  {}",
                problems.len(),
                problems.join("\n  ")
            ));
        }

        Ok(UIStyle { values })
    }

    // Every accessor below panics if `key` is somehow absent -- validation in
    // from_file/from_json already guarantees every StyleKey is present and coerces to its
    // declared kind, so reaching the "missing"/coercion-failure branch here means a
    // programming error (a key read via the wrong accessor for its kind), not a bad file.

    pub fn get_color(&self, key: StyleKey) -> (f32, f32, f32) {
        let value = self.values.get(&key).unwrap_or_else(|| panic!("StyleKey::{key:?} missing after validation"));
        value_as_color(value).unwrap_or_else(|e| panic!("StyleKey::{key:?}: {e}"))
    }

    pub fn get_float(&self, key: StyleKey) -> f32 {
        let value = self.values.get(&key).unwrap_or_else(|| panic!("StyleKey::{key:?} missing after validation"));
        value_as_float(value).unwrap_or_else(|e| panic!("StyleKey::{key:?}: {e}"))
    }

    pub fn get_integer(&self, key: StyleKey) -> u32 {
        let value = self.values.get(&key).unwrap_or_else(|| panic!("StyleKey::{key:?} missing after validation"));
        value_as_integer(value).unwrap_or_else(|e| panic!("StyleKey::{key:?}: {e}"))
    }

    pub fn get_bool(&self, key: StyleKey) -> bool {
        let value = self.values.get(&key).unwrap_or_else(|| panic!("StyleKey::{key:?} missing after validation"));
        value_as_bool(value).unwrap_or_else(|e| panic!("StyleKey::{key:?}: {e}"))
    }

    pub fn get_string(&self, key: StyleKey) -> String {
        let value = self.values.get(&key).unwrap_or_else(|| panic!("StyleKey::{key:?} missing after validation"));
        value_as_string(value).unwrap_or_else(|e| panic!("StyleKey::{key:?}: {e}"))
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Parse color string to RGB values (0.0-1.0)
fn parse_color(color_str: &str) -> Result<(f32, f32, f32), String> {
    if let Some(hex) = color_str.strip_prefix('#') {
        // Hex color: #RRGGBB or #RGB
        match hex.len() {
            3 => {
                // #RGB -> #RRGGBB
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)
                    .map_err(|_| format!("Invalid hex color: {}", color_str))?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)
                    .map_err(|_| format!("Invalid hex color: {}", color_str))?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)
                    .map_err(|_| format!("Invalid hex color: {}", color_str))?;
                Ok((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
            },
            6 => {
                // #RRGGBB
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("Invalid hex color: {}", color_str))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("Invalid hex color: {}", color_str))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("Invalid hex color: {}", color_str))?;
                Ok((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
            },
            _ => Err(format!("Invalid hex color format: {}", color_str)),
        }
    } else {
        // Named color
        match color_str.to_lowercase().as_str() {
            "black" => Ok((0.0, 0.0, 0.0)),
            "white" => Ok((1.0, 1.0, 1.0)),
            "red" => Ok((1.0, 0.0, 0.0)),
            "green" => Ok((0.0, 1.0, 0.0)),
            "blue" => Ok((0.0, 0.0, 1.0)),
            "yellow" => Ok((1.0, 1.0, 0.0)),
            "cyan" => Ok((0.0, 1.0, 1.0)),
            "magenta" => Ok((1.0, 0.0, 1.0)),
            "gray" | "grey" => Ok((0.5, 0.5, 0.5)),
            "orange" => Ok((1.0, 0.5, 0.0)),
            _ => Err(format!("Unknown color name: {}", color_str)),
        }
    }
}

/// Calculate the average of two RGB colors
/// Returns a color that is the blend of color1 and color2 with equal weight (0.5 each)
pub fn average_colors(color1: (f32, f32, f32), color2: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        (color1.0 + color2.0) * 0.5,
        (color1.1 + color2.1) * 0.5,
        (color1.2 + color2.2) * 0.5,
    )
}

/// Calculate the weighted average of two RGB colors
/// weight: 0.0 = fully color1, 1.0 = fully color2, 0.5 = equal blend
pub fn blend_colors(color1: (f32, f32, f32), color2: (f32, f32, f32), weight: f32) -> (f32, f32, f32) {
    let w = weight.clamp(0.0, 1.0);
    let inv_w = 1.0 - w;
    (
        color1.0 * inv_w + color2.0 * w,
        color1.1 * inv_w + color2.1 * w,
        color1.2 * inv_w + color2.2 * w,
    )
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_parsing() {
        assert_eq!(parse_color("#FF0000"), Ok((1.0, 0.0, 0.0)));
        assert_eq!(parse_color("#F00"), Ok((1.0, 0.0, 0.0)));
        assert_eq!(parse_color("red"), Ok((1.0, 0.0, 0.0)));
        assert_eq!(parse_color("white"), Ok((1.0, 1.0, 1.0)));
        assert!(parse_color("invalid").is_err());
    }

    fn minimal_json_for_all_keys() -> String {
        let mut map = serde_json::Map::new();
        for key in StyleKey::ALL {
            let value = match key.kind() {
                ValueKind::Color => serde_json::json!("#FF0000"),
                ValueKind::Float => serde_json::json!(1.0),
                ValueKind::Integer => serde_json::json!(1),
                ValueKind::Boolean => serde_json::json!(true),
                ValueKind::String => serde_json::json!("value"),
            };
            map.insert(key.json_name().to_string(), value);
        }
        serde_json::Value::Object(map).to_string()
    }

    #[test]
    fn loads_when_every_key_present_and_well_typed() {
        let style = UIStyle::from_json(&minimal_json_for_all_keys()).expect("should validate");
        assert_eq!(style.get_color(StyleKey::GaugeNeedleColor), (1.0, 0.0, 0.0));
        assert_eq!(style.get_integer(StyleKey::GaugeMinorMarkCount), 1);
        assert_eq!(style.get_bool(StyleKey::GaugeLabelEnabled), true);
        assert_eq!(style.get_string(StyleKey::GaugeLabelFont), "value");
    }

    #[test]
    fn integer_key_coerces_from_a_float_shaped_json_number() {
        // ALERT_FONT_SIZE-style case: an Integer-kind key stored as e.g. `48.0`.
        let mut map = serde_json::Map::new();
        for key in StyleKey::ALL {
            let value = if *key == StyleKey::AlertFontSize {
                serde_json::json!(48.0)
            } else {
                match key.kind() {
                    ValueKind::Color => serde_json::json!("#FF0000"),
                    ValueKind::Float => serde_json::json!(1.0),
                    ValueKind::Integer => serde_json::json!(1),
                    ValueKind::Boolean => serde_json::json!(true),
                    ValueKind::String => serde_json::json!("value"),
                }
            };
            map.insert(key.json_name().to_string(), value);
        }
        let json = serde_json::Value::Object(map).to_string();
        let style = UIStyle::from_json(&json).expect("should validate");
        assert_eq!(style.get_integer(StyleKey::AlertFontSize), 48);
        assert_eq!(style.get_float(StyleKey::AlertFontSize), 48.0);
    }

    #[test]
    fn missing_key_is_a_load_error() {
        let mut map = serde_json::Map::new();
        for key in StyleKey::ALL {
            if *key == StyleKey::GaugeNeedleColor {
                continue; // deliberately omitted
            }
            let value = match key.kind() {
                ValueKind::Color => serde_json::json!("#FF0000"),
                ValueKind::Float => serde_json::json!(1.0),
                ValueKind::Integer => serde_json::json!(1),
                ValueKind::Boolean => serde_json::json!(true),
                ValueKind::String => serde_json::json!("value"),
            };
            map.insert(key.json_name().to_string(), value);
        }
        let json = serde_json::Value::Object(map).to_string();
        let err = UIStyle::from_json(&json).expect_err("missing key should fail");
        assert!(err.contains("gauge_needle_color"));
    }

    #[test]
    fn wrong_type_is_a_load_error() {
        let mut map = serde_json::Map::new();
        for key in StyleKey::ALL {
            let value = if *key == StyleKey::GaugeBorderWidth {
                serde_json::json!("not a number")
            } else {
                match key.kind() {
                    ValueKind::Color => serde_json::json!("#FF0000"),
                    ValueKind::Float => serde_json::json!(1.0),
                    ValueKind::Integer => serde_json::json!(1),
                    ValueKind::Boolean => serde_json::json!(true),
                    ValueKind::String => serde_json::json!("value"),
                }
            };
            map.insert(key.json_name().to_string(), value);
        }
        let json = serde_json::Value::Object(map).to_string();
        let err = UIStyle::from_json(&json).expect_err("wrong-typed key should fail");
        assert!(err.contains("gauge_border_width"));
    }

    #[test]
    fn reports_every_problem_at_once() {
        let json = "{}"; // every key missing
        let err = UIStyle::from_json(json).expect_err("empty file should fail");
        assert_eq!(err.matches("missing").count(), StyleKey::ALL.len());
    }

    /// Exercises the repo's actual ui_style.json end to end, so a transcription mistake
    /// fails `cargo test` instead of only surfacing at dashboard startup.
    #[test]
    fn repo_ui_style_json_loads_successfully() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui_style.json");
        UIStyle::from_file(&path).expect("repo ui_style.json should load and validate");
    }
}
