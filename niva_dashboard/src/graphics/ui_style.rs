
//! UI Style Configuration System
//!
//! Simple flat key-value style system for dashboard elements.
//! Uses string constants for style element names and supports JSON serialization.
//!
//! Example JSON format:
//! ```json
//! {
//!   "GAUGE_NEEDLE_COLOR": "#FF0000",
//!   "gauge_background_color": "#000000",
//!   "gauge_mark_font": "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
//!   "gauge_mark_font_size": 14,
//!   "gauge_major_mark_width": 2.0,
//!   "gauge_minor_mark_width": 1.0,
//!   "bar_fill_color": "#00FF00",
//!   "global_brightness": 1.0
//! }
//! ```

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// =============================================================================
// STYLE ELEMENT NAME CONSTANTS
// =============================================================================

// Default values
pub const DEFAULT_GLOBAL_FONT_PATH: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf";  // Use monospace for more digital look
pub const DEFAULT_GLOBAL_FONT_SIZE: u32 = 14;

// Digital Display Fonts
pub const DIGITAL_DISPLAY_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG7ClassicMini-Regular.ttf";
pub const DIGITAL_DISPLAY_FONT_ITALIC_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG7ClassicMini-Italic.ttf";
pub const DIGITAL_DISPLAY_14SEG_FONT_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG14ClassicMini-Regular.ttf";
pub const DIGITAL_DISPLAY_14SEG_ITALIC_PATH: &str = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG14ClassicMini-Italic.ttf";
pub const DIGITAL_DISPLAY_MONO_FONT_PATH: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf";

// Global Style Elements
pub const GLOBAL_BRIGHTNESS: &str = "global_brightness";
pub const GLOBAL_CONTRAST: &str = "global_contrast";
pub const GLOBAL_BACKGROUND_COLOR: &str = "global_background_color";
pub const GLOBAL_FONT_PATH: &str = "global_font_path";
pub const GLOBAL_FONT_SIZE: &str = "global_font_size";

// Page manager style elements
pub const PAGE_BUTTON_LABEL_FONT: &str = "page_button_label_font";
pub const PAGE_BUTTON_LABEL_FONT_SIZE: &str = "page_button_label_font_size";
pub const PAGE_BUTTON_LABEL_ORIENTATION: &str = "page_button_label_orientation"; // "horizontal" or "vertical"
pub const PAGE_BUTTON_LABEL_COLOR: &str = "page_button_label_color";
pub const PAGE_STATUS_FONT: &str = "page_status_font";
pub const PAGE_STATUS_FONT_SIZE: &str = "page_status_font_size";
pub const PAGE_STATUS_COLOR: &str = "page_status_color";

// Gauge Style Elements
pub const GAUGE_BACKGROUND_COLOR: &str = "gauge_background_color";
pub const GAUGE_BORDER_COLOR: &str = "gauge_border_color";
pub const GAUGE_BORDER_WIDTH: &str = "gauge_border_width";
pub const GAUGE_RADIUS: &str = "gauge_radius";

// Gauge Needle
pub const GAUGE_NEEDLE_COLOR: &str = "GAUGE_NEEDLE_COLOR";
pub const GAUGE_NEEDLE_WIDTH: &str = "GAUGE_NEEDLE_WIDTH";
pub const GAUGE_NEEDLE_LENGTH: &str = "GAUGE_NEEDLE_LENGTH";
pub const GAUGE_NEEDLE_TIP_WIDTH: &str = "GAUGE_NEEDLE_TIP_WIDTH";
pub const GAUGE_NEEDLE_CENTER_COLOR: &str = "GAUGE_NEEDLE_CENTER_COLOR";
pub const GAUGE_NEEDLE_CENTER_RADIUS: &str = "GAUGE_NEEDLE_CENTER_RADIUS";
pub const GAUGE_NEEDLE_SHADOW_ENABLED: &str = "GAUGE_NEEDLE_SHADOW_ENABLED";
pub const GAUGE_NEEDLE_SHADOW_COLOR: &str = "GAUGE_NEEDLE_SHADOW_COLOR";
pub const GAUGE_NEEDLE_GLOW_ENABLED: &str = "GAUGE_NEEDLE_GLOW_ENABLED";

// Gauge Marks
pub const GAUGE_MAJOR_MARK_COLOR: &str = "gauge_major_mark_color";
pub const GAUGE_MAJOR_MARK_WIDTH: &str = "gauge_major_mark_width";
pub const GAUGE_MAJOR_MARK_LENGTH: &str = "gauge_major_mark_length";
pub const GAUGE_MAJOR_MARK_OFFSET: &str = "gauge_major_mark_offset";
pub const GAUGE_MAJOR_MARK_ENABLED: &str = "gauge_major_mark_enabled";
pub const GAUGE_MAJOR_MARK_COUNT: &str = "gauge_major_mark_count";

pub const GAUGE_MINOR_MARK_COLOR: &str = "gauge_minor_mark_color";
pub const GAUGE_MINOR_MARK_WIDTH: &str = "gauge_minor_mark_width";
pub const GAUGE_MINOR_MARK_LENGTH: &str = "gauge_minor_mark_length";
pub const GAUGE_MINOR_MARK_OFFSET: &str = "gauge_minor_mark_offset";
pub const GAUGE_MINOR_MARK_ENABLED: &str = "gauge_minor_mark_enabled";
pub const GAUGE_MINOR_MARK_COUNT: &str = "gauge_minor_mark_count";

// Gauge Labels
pub const GAUGE_LABEL_COLOR: &str = "gauge_label_color";
pub const GAUGE_LABEL_FONT: &str = "gauge_label_font";
pub const GAUGE_LABEL_FONT_SIZE: &str = "gauge_label_font_size";
pub const GAUGE_LABEL_OFFSET: &str = "gauge_label_offset";
pub const GAUGE_LABEL_ENABLED: &str = "gauge_label_enabled";

pub const GAUGE_TITLE_COLOR: &str = "gauge_title_color";
pub const GAUGE_TITLE_FONT: &str = "gauge_title_font";
pub const GAUGE_TITLE_FONT_SIZE: &str = "gauge_title_font_size";
pub const GAUGE_TITLE_OFFSET_H: &str = "gauge_title_offset_h";
pub const GAUGE_TITLE_OFFSET_V: &str = "gauge_title_offset_v";
pub const GAUGE_TITLE_ENABLED: &str = "gauge_title_enabled";

pub const GAUGE_UNIT_COLOR: &str = "gauge_unit_color";
pub const GAUGE_UNIT_FONT: &str = "gauge_unit_font";
pub const GAUGE_UNIT_FONT_SIZE: &str = "gauge_unit_font_size";
pub const GAUGE_UNIT_OFFSET_H: &str = "gauge_unit_offset_h";
pub const GAUGE_UNIT_OFFSET_V: &str = "gauge_unit_offset_v";
pub const GAUGE_UNIT_ENABLED: &str = "gauge_unit_enabled";

// Gauge Zones
pub const GAUGE_WARNING_ZONE_COLOR: &str = "gauge_warning_zone_color";
pub const GAUGE_WARNING_ZONE_WIDTH: &str = "gauge_warning_zone_width";
pub const GAUGE_WARNING_ZONE_ENABLED: &str = "gauge_warning_zone_enabled";

pub const GAUGE_CRITICAL_ZONE_COLOR: &str = "gauge_critical_zone_color";
pub const GAUGE_CRITICAL_ZONE_WIDTH: &str = "gauge_critical_zone_width";
pub const GAUGE_CRITICAL_ZONE_ENABLED: &str = "gauge_critical_zone_enabled";

pub const GAUGE_INACTIVE_ZONE_COLOR: &str = "gauge_inactive_zone_color";
pub const GAUGE_INACTIVE_ZONE_WIDTH: &str = "gauge_inactive_zone_width";
pub const GAUGE_INACTIVE_ZONE_ENABLED: &str = "gauge_inactive_zone_enabled";

// Bar Indicator Style Elements
pub const BAR_BACKGROUND_COLOR: &str = "bar_background_color";
pub const BAR_BACKGROUND_ENABLED: &str = "bar_background_enabled";
pub const BAR_BORDER_COLOR: &str = "bar_border_color";
pub const BAR_BORDER_ENABLED: &str = "bar_border_enabled";
pub const BAR_BORDER_WIDTH: &str = "bar_border_width";
pub const BAR_CORNER_RADIUS: &str = "bar_corner_radius";

pub const BAR_EMPTY_COLOR: &str = "bar_empty_color";
pub const BAR_NORMAL_COLOR: &str = "bar_normal_color";
pub const BAR_WARNING_COLOR: &str = "bar_warning_color";
pub const BAR_CRITICAL_COLOR: &str = "bar_critical_color";

pub const BAR_MARKS_COLOR: &str = "bar_marks_color";
pub const BAR_MARKS_WIDTH: &str = "bar_marks_width";
pub const BAR_MARKS_THICKNESS: &str = "bar_marks_thickness";

pub const BAR_MARK_LABELS_COLOR: &str = "bar_mark_labels_color";

pub const BAR_SEGMENT_COUNT: &str = "bar_segment_count";
pub const BAR_SEGMENT_GAP: &str = "bar_segment_gap";

// Text Style Elements
pub const TEXT_PRIMARY_COLOR: &str = "text_primary_color";
pub const TEXT_SECONDARY_COLOR: &str = "text_secondary_color";
pub const TEXT_ACCENT_COLOR: &str = "text_accent_color";
pub const TEXT_WARNING_COLOR: &str = "text_warning_color";
pub const TEXT_ERROR_COLOR: &str = "text_error_color";

pub const TEXT_PRIMARY_FONT: &str = "text_primary_font";
pub const TEXT_PRIMARY_FONT_SIZE: &str = "text_primary_font_size";
pub const TEXT_SECONDARY_FONT: &str = "text_secondary_font";
pub const TEXT_SECONDARY_FONT_SIZE: &str = "text_secondary_font_size";
pub const TEXT_MONOSPACE_FONT: &str = "text_monospace_font";
pub const TEXT_MONOSPACE_FONT_SIZE: &str = "text_monospace_font_size";
pub const TEXT_SMALL_FONT: &str = "text_small_font";
pub const TEXT_SMALL_FONT_SIZE: &str = "text_small_font_size";

pub const TEXT_LINE_SPACING: &str = "text_line_spacing";
pub const TEXT_LETTER_SPACING: &str = "text_letter_spacing";

// Digital Display Style Elements (7-segment style)
pub const DIGITAL_DISPLAY_FONT: &str = "digital_display_font";
pub const DIGITAL_DISPLAY_FONT_SIZE: &str = "digital_display_font_size";
pub const DIGITAL_DISPLAY_SCALE: &str = "digital_display_scale";
pub const DIGITAL_DISPLAY_ACTIVE_COLOR: &str = "digital_display_active_color";
pub const DIGITAL_DISPLAY_INACTIVE_COLOR: &str = "digital_display_inactive_color";
pub const DIGITAL_DISPLAY_INACTIVE_COLOR_BLENDING: &str = "digital_display_inactive_color_blending";
pub const DIGITAL_DISPLAY_BACKGROUND_COLOR: &str = "digital_display_background_color";
pub const DIGITAL_DISPLAY_BACKGROUND_ENABLED: &str = "digital_display_background_enabled";
pub const DIGITAL_DISPLAY_BORDER_ENABLED: &str = "digital_display_border_enabled";
pub const DIGITAL_DISPLAY_BORDER_COLOR: &str = "digital_display_border_color";
pub const DIGITAL_DISPLAY_BORDER_WIDTH: &str = "digital_display_border_width";
pub const DIGITAL_DISPLAY_BORDER_RADIUS: &str = "digital_display_border_radius";

// Extended Digital Display Fonts (additional variants)
pub const DIGITAL_DISPLAY_FONT_ITALIC: &str = "digital_display_font_italic";
pub const DIGITAL_DISPLAY_14SEG_FONT: &str = "digital_display_14seg_font";
pub const DIGITAL_DISPLAY_14SEG_ITALIC: &str = "digital_display_14seg_italic";

// Warning Indicator Style Elements
pub const INDICATOR_NORMAL_COLOR: &str = "indicator_normal_color";
pub const INDICATOR_WARNING_COLOR: &str = "indicator_warning_color";
pub const INDICATOR_CRITICAL_COLOR: &str = "indicator_critical_color";
pub const INDICATOR_OFF_COLOR: &str = "indicator_off_color";
pub const INDICATOR_BLINK_SPEED: &str = "indicator_blink_speed";
pub const INDICATOR_GLOW_ENABLED: &str = "indicator_glow_enabled";
pub const INDICATOR_GLOW_RADIUS: &str = "indicator_glow_radius";
pub const INDICATOR_SIZE: &str = "indicator_size";

// Animation Settings
pub const ANIMATION_NEEDLE_SPEED: &str = "animation_needle_speed";
pub const ANIMATION_BAR_SPEED: &str = "animation_bar_speed";
pub const ANIMATION_SMOOTH_ENABLED: &str = "animation_smooth_enabled";

// Alerts settings
pub const ALERT_FONT_PATH: &str = "alert_font_path";
pub const ALERT_FONT_SIZE: &str = "alert_font_size";
pub const ALERT_WARNING_COLOR: &str = "alert_warning_color";
pub const ALERT_CRITICAL_COLOR: &str = "alert_critical_color";
pub const ALERT_BACKGROUND_COLOR: &str = "alert_background_color";
pub const ALERT_BORDER_COLOR: &str = "alert_border_color";
pub const ALERT_BORDER_WIDTH: &str = "alert_border_width";
pub const ALERT_MARGIN: &str = "alert_border_margin";
pub const ALERT_CORNER_RADIUS: &str = "alert_corner_radius";
pub const ALERT_SOUND_PATH: &str = "alert_sound_path";

// =============================================================================
// STYLE VALUE TYPES
// =============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UIStyleValue {
    Color(String),      // Hex color: "#FF0000" or named: "red"
    Float(f32),         // Numeric values: width, size, etc.
    Integer(u32),       // Integer values: count, size
    Boolean(bool),      // Enable/disable flags
    String(String),     // Font paths, text values
}

impl UIStyleValue {
    /// Convert to color tuple (r, g, b) with values 0.0-1.0
    pub fn as_color(&self) -> Result<(f32, f32, f32), String> {
        match self {
            UIStyleValue::Color(color_str) => parse_color(color_str),
            _ => Err("Value is not a color".to_string()),
        }
    }
    
    /// Convert to color tuple with alpha (r, g, b, a) with values 0.0-1.0
    pub fn as_color_rgba(&self) -> Result<(f32, f32, f32, f32), String> {
        let (r, g, b) = self.as_color()?;
        Ok((r, g, b, 1.0))
    }
    
    pub fn as_float(&self) -> Result<f32, String> {
        match self {
            UIStyleValue::Float(f) => Ok(*f),
            UIStyleValue::Integer(i) => Ok(*i as f32),
            _ => Err("Value is not a float".to_string()),
        }
    }
    
    pub fn as_integer(&self) -> Result<u32, String> {
        match self {
            UIStyleValue::Integer(i) => Ok(*i),
            UIStyleValue::Float(f) => Ok(*f as u32),
            _ => Err("Value is not an integer".to_string()),
        }
    }
    
    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            UIStyleValue::Boolean(b) => Ok(*b),
            _ => Err("Value is not a boolean".to_string()),
        }
    }
    
    pub fn as_string(&self) -> Result<&str, String> {
        match self {
            UIStyleValue::String(s) => Ok(s),
            _ => Err("Value is not a string".to_string()),
        }
    }
}

// =============================================================================
// UI STYLE MAIN STRUCT
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIStyle {
    values: HashMap<String, HashMap<String, UIStyleValue>>,
}

impl UIStyle {
    pub fn new() -> Self {
        let mut style = UIStyle {
            values: HashMap::new(),
        };
        style.load_defaults();
        style
    }
    
    /// Load style from JSON string
    /// Supports both old flat format and new grouped format
    pub fn from_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Try to parse as new grouped format first
        if let Ok(grouped_values) = serde_json::from_str::<HashMap<String, HashMap<String, UIStyleValue>>>(json_str) {
            let mut style = UIStyle { values: grouped_values };
            // Ensure we have a default group
            if !style.values.contains_key("default") {
                style.values.insert("default".to_string(), HashMap::new());
                style.load_defaults();
            }
            return Ok(style);
        }
        
        // Fall back to old flat format for backward compatibility
        let flat_values: HashMap<String, UIStyleValue> = serde_json::from_str(json_str)?;
        let mut style = UIStyle::new(); // Start with defaults
        
        // Put flat values into "default" group
        let default_group = style.values.get_mut("default").unwrap();
        for (key, value) in flat_values {
            default_group.insert(key, value);
        }
        
        Ok(style)
    }
    
    /// Save style to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.values)
    }
    
    /// Load style from JSON file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let json_str = std::fs::read_to_string(path)?;
        Self::from_json(&json_str)
    }
    
    /// Save style to JSON file
    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json_str = self.to_json()?;
        std::fs::write(path, json_str)?;
        Ok(())
    }
    
    /// Get a style value from specific group, with fallback to "default" group
    pub fn get(&self, key: &str) -> Option<&UIStyleValue> {
        self.get_with_group(key, None)
    }
    
    /// Get a style value with optional group parameter
    pub fn get_with_group(&self, key: &str, group: Option<&str>) -> Option<&UIStyleValue> {
        // Try specific group first if provided
        if let Some(group_name) = group {
            if let Some(group_values) = self.values.get(group_name) {
                if let Some(value) = group_values.get(key) {
                    return Some(value);
                }
            }
        }
        
        // Fall back to default group
        self.values.get("default")?.get(key)
    }
    
    /// Set a style value in specific group (defaults to "default" group)
    pub fn set(&mut self, key: &str, value: UIStyleValue) {
        self.set_with_group(key, value, None);
    }
    
    /// Set a style value with optional group parameter
    pub fn set_with_group(&mut self, key: &str, value: UIStyleValue, group: Option<&str>) {
        let group_name = group.unwrap_or("default");
        
        // Ensure group exists
        if !self.values.contains_key(group_name) {
            self.values.insert(group_name.to_string(), HashMap::new());
        }
        
        self.values.get_mut(group_name).unwrap().insert(key.to_string(), value);
    }
    
    /// Get color value with brightness applied
    pub fn get_color(&self, key: &str, default: (f32, f32, f32)) -> (f32, f32, f32) {
        self.get_color_with_group(key, default, None)
    }
    
    /// Get color value with optional group parameter and brightness applied
    pub fn get_color_with_group(&self, key: &str, default: (f32, f32, f32), group: Option<&str>) -> (f32, f32, f32) {
        match self.get_with_group(key, group) {
            Some(value) => match value.as_color() {
                Ok((r, g, b)) => {
                    // Apply global brightness
                    let brightness = self.get(GLOBAL_BRIGHTNESS)
                        .and_then(|v| v.as_float().ok())
                        .unwrap_or(1.0);
                    (r * brightness, g * brightness, b * brightness)
                },
                Err(_) => {
                    print!("Warning: Style key '{}' exists but cannot be converted to color, using default: ({}, {}, {})\r\n", key, default.0, default.1, default.2);
                    default
                }
            },
            None => {
                print!("Warning: Style key '{}' not found, using default color: ({}, {}, {})\r\n", key, default.0, default.1, default.2);
                default
            }
        }
    }
    
    /// Get color value with alpha and brightness applied
    pub fn get_color_rgba(&self, key: &str, default: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        self.get_color_rgba_with_group(key, default, None)
    }
    
    /// Get color value with alpha, optional group parameter, and brightness applied
    pub fn get_color_rgba_with_group(&self, key: &str, default: (f32, f32, f32, f32), group: Option<&str>) -> (f32, f32, f32, f32) {
        let (r, g, b) = self.get_color_with_group(key, (default.0, default.1, default.2), group);
        (r, g, b, default.3)
    }
    
    /// Get float value with fallback
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.get_float_with_group(key, default, None)
    }
    
    /// Get float value with optional group parameter and fallback
    pub fn get_float_with_group(&self, key: &str, default: f32, group: Option<&str>) -> f32 {
        match self.get_with_group(key, group) {
            Some(value) => match value.as_float() {
                Ok(val) => val,
                Err(_) => {
                    print!("Warning: Style key '{}' exists but cannot be converted to float, using default: {}\r\n", key, default);
                    default
                }
            },
            None => {
                print!("Warning: Style key '{}' not found, using default float: {}\r\n", key, default);
                default
            }
        }
    }
    
    /// Get integer value with fallback
    pub fn get_integer(&self, key: &str, default: u32) -> u32 {
        self.get_integer_with_group(key, default, None)
    }
    
    /// Get integer value with optional group parameter and fallback
    pub fn get_integer_with_group(&self, key: &str, default: u32, group: Option<&str>) -> u32 {
        match self.get_with_group(key, group) {
            Some(value) => match value.as_integer() {
                Ok(val) => val,
                Err(_) => {
                    print!("Warning: Style key '{}' exists but cannot be converted to integer, using default: {}\r\n", key, default);
                    default
                }
            },
            None => {
                print!("Warning: Style key '{}' not found, using default integer: {}\r\n", key, default);
                default
            }
        }
    }
    
    /// Get boolean value with fallback
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.get_bool_with_group(key, default, None)
    }
    
    /// Get boolean value with optional group parameter and fallback
    pub fn get_bool_with_group(&self, key: &str, default: bool, group: Option<&str>) -> bool {
        match self.get_with_group(key, group) {
            Some(value) => match value.as_bool() {
                Ok(val) => val,
                Err(_) => {
                    print!("Warning: Style key '{}' exists but cannot be converted to boolean, using default: {}\r\n", key, default);
                    default
                }
            },
            None => {
                print!("Warning: Style key '{}' not found, using default boolean: {}\r\n", key, default);
                default
            }
        }
    }
    
    /// Get string value with fallback
    pub fn get_string(&self, key: &str, default: &str) -> String {
        self.get_string_with_group(key, default, None)
    }
    
    /// Get string value with optional group parameter and fallback
    pub fn get_string_with_group(&self, key: &str, default: &str, group: Option<&str>) -> String {
        match self.get_with_group(key, group) {
            Some(value) => match value.as_string() {
                Ok(val) => val.to_string(),
                Err(_) => {
                    print!("Warning: Style key '{}' exists but cannot be converted to string, using default: '{}'\r\n", key, default);
                    default.to_string()
                }
            },
            None => {
                print!("Warning: Style key '{}' not found, using default string: '{}'\r\n", key, default);
                default.to_string()
            }
        }
    }
    
    // =============================================================================
    // BRIGHTNESS MANAGEMENT
    // =============================================================================
    
    /// Set global brightness (0.0 to 1.0)
    pub fn set_brightness(&mut self, brightness: f32) {
        let clamped_brightness = brightness.clamp(0.0, 1.0);
        self.set(GLOBAL_BRIGHTNESS, UIStyleValue::Float(clamped_brightness));
    }
    
    /// Get current global brightness
    pub fn get_brightness(&self) -> f32 {
        self.get_float(GLOBAL_BRIGHTNESS, 1.0)
    }
    
    /// Increase brightness by a step
    pub fn increase_brightness(&mut self, step: f32) {
        let current = self.get_brightness();
        let new_brightness = (current + step).clamp(0.0, 1.0);
        self.set_brightness(new_brightness);
    }
    
    /// Decrease brightness by a step
    pub fn decrease_brightness(&mut self, step: f32) {
        let current = self.get_brightness();
        let new_brightness = (current - step).clamp(0.0, 1.0);
        self.set_brightness(new_brightness);
    }
    
    /// Apply brightness to a color tuple
    pub fn apply_brightness(&self, color: (f32, f32, f32)) -> (f32, f32, f32) {
        let brightness = self.get_brightness();
        (color.0 * brightness, color.1 * brightness, color.2 * brightness)
    }
    
    /// Load default style values
    fn load_defaults(&mut self) {
        // Ensure default group exists
        self.values.insert("default".to_string(), HashMap::new());
        
        // Global defaults
        self.set(GLOBAL_BRIGHTNESS, UIStyleValue::Float(1.0));
        self.set(GLOBAL_CONTRAST, UIStyleValue::Float(1.0));
        self.set(GLOBAL_BACKGROUND_COLOR, UIStyleValue::Color("#000000".to_string()));
        self.set(GLOBAL_FONT_PATH, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(GLOBAL_FONT_SIZE, UIStyleValue::Integer(16));
        
        // Page manager defaults
        self.set(PAGE_BUTTON_LABEL_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(PAGE_BUTTON_LABEL_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(PAGE_BUTTON_LABEL_ORIENTATION, UIStyleValue::String("vertical".to_string()));
        self.set(PAGE_BUTTON_LABEL_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(PAGE_STATUS_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(PAGE_STATUS_FONT_SIZE, UIStyleValue::Integer(14));
        self.set(PAGE_STATUS_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));

        // Gauge defaults
        self.set(GAUGE_BACKGROUND_COLOR, UIStyleValue::Color("#000000".to_string()));
        self.set(GAUGE_BORDER_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_BORDER_WIDTH, UIStyleValue::Float(2.0));
        self.set(GAUGE_RADIUS, UIStyleValue::Float(80.0));
        
        // Needle defaults
        self.set(GAUGE_NEEDLE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        self.set(GAUGE_NEEDLE_WIDTH, UIStyleValue::Float(8.0));
        self.set(GAUGE_NEEDLE_LENGTH, UIStyleValue::Float(0.8));
        self.set(GAUGE_NEEDLE_TIP_WIDTH, UIStyleValue::Float(2.0));
        self.set(GAUGE_NEEDLE_CENTER_COLOR, UIStyleValue::Color("#404040".to_string()));
        self.set(GAUGE_NEEDLE_CENTER_RADIUS, UIStyleValue::Float(8.0));
        self.set(GAUGE_NEEDLE_SHADOW_ENABLED, UIStyleValue::Boolean(false));
        self.set(GAUGE_NEEDLE_SHADOW_COLOR, UIStyleValue::Color("#000000".to_string()));
        self.set(GAUGE_NEEDLE_GLOW_ENABLED, UIStyleValue::Boolean(false));

        // Gauge marks defaults
        self.set(GAUGE_MAJOR_MARK_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_MAJOR_MARK_WIDTH, UIStyleValue::Float(2.0));
        self.set(GAUGE_MAJOR_MARK_LENGTH, UIStyleValue::Float(16.0));
        self.set(GAUGE_MAJOR_MARK_OFFSET, UIStyleValue::Float(0.0));
        self.set(GAUGE_MAJOR_MARK_ENABLED, UIStyleValue::Boolean(true));
        self.set(GAUGE_MAJOR_MARK_COUNT, UIStyleValue::Integer(10));

        self.set(GAUGE_MINOR_MARK_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_MINOR_MARK_WIDTH, UIStyleValue::Float(2.0));
        self.set(GAUGE_MINOR_MARK_LENGTH, UIStyleValue::Float(10.0));
        self.set(GAUGE_MINOR_MARK_OFFSET, UIStyleValue::Float(0.0));
        self.set(GAUGE_MINOR_MARK_ENABLED, UIStyleValue::Boolean(true));
        self.set(GAUGE_MINOR_MARK_COUNT, UIStyleValue::Integer(37));
        
        // Label defaults
        self.set(GAUGE_LABEL_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_LABEL_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(GAUGE_LABEL_FONT_SIZE, UIStyleValue::Integer(14));
        self.set(GAUGE_LABEL_OFFSET, UIStyleValue::Float(-35.0));   // Negative to move inside the gauge
        self.set(GAUGE_LABEL_ENABLED, UIStyleValue::Boolean(true));
        
        self.set(GAUGE_TITLE_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_TITLE_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".to_string()));
        self.set(GAUGE_TITLE_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(GAUGE_TITLE_OFFSET_H, UIStyleValue::Float(0.0));
        self.set(GAUGE_TITLE_OFFSET_V, UIStyleValue::Float(-20.0));
        self.set(GAUGE_TITLE_ENABLED, UIStyleValue::Boolean(true));
        
        self.set(GAUGE_UNIT_COLOR, UIStyleValue::Color("#727272ff".to_string()));
        self.set(GAUGE_UNIT_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(GAUGE_UNIT_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(GAUGE_UNIT_OFFSET_H, UIStyleValue::Float(0.0));
        self.set(GAUGE_UNIT_OFFSET_V, UIStyleValue::Float(50.0));
        self.set(GAUGE_UNIT_ENABLED, UIStyleValue::Boolean(true));
        
        // Zone defaults
        self.set(GAUGE_WARNING_ZONE_COLOR, UIStyleValue::Color("#FFAA00".to_string()));
        self.set(GAUGE_WARNING_ZONE_WIDTH, UIStyleValue::Float(4.0));
        self.set(GAUGE_WARNING_ZONE_ENABLED, UIStyleValue::Boolean(false));
        
        self.set(GAUGE_CRITICAL_ZONE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        self.set(GAUGE_CRITICAL_ZONE_WIDTH, UIStyleValue::Float(4.0));
        self.set(GAUGE_CRITICAL_ZONE_ENABLED, UIStyleValue::Boolean(false));
        
        self.set(GAUGE_INACTIVE_ZONE_COLOR, UIStyleValue::Color("#202020".to_string()));
        self.set(GAUGE_INACTIVE_ZONE_WIDTH, UIStyleValue::Float(4.0));
        self.set(GAUGE_INACTIVE_ZONE_ENABLED, UIStyleValue::Boolean(true));

        // Bar defaults
        self.set(BAR_BACKGROUND_COLOR, UIStyleValue::Color("#404040".to_string()));
        self.set(BAR_BACKGROUND_ENABLED, UIStyleValue::Boolean(false));
        self.set(BAR_BORDER_COLOR, UIStyleValue::Color("#FFA500".to_string()));
        self.set(BAR_BORDER_ENABLED, UIStyleValue::Boolean(true));
        self.set(BAR_BORDER_WIDTH, UIStyleValue::Float(4.0));
        self.set(BAR_CORNER_RADIUS, UIStyleValue::Float(8.0));

        self.set(BAR_EMPTY_COLOR, UIStyleValue::Color("#202020".to_string()));
        self.set(BAR_NORMAL_COLOR, UIStyleValue::Color("#FF7D00".to_string()));
        self.set(BAR_WARNING_COLOR, UIStyleValue::Color("#FFFF00".to_string()));
        self.set(BAR_CRITICAL_COLOR, UIStyleValue::Color("#FF0000".to_string()));

        self.set(BAR_MARKS_COLOR, UIStyleValue::Color("#FF7D00".to_string()));
        self.set(BAR_MARKS_WIDTH, UIStyleValue::Float(12.0));
        self.set(BAR_MARKS_THICKNESS, UIStyleValue::Float(4.0));

        self.set(BAR_MARK_LABELS_COLOR, UIStyleValue::Color("#FF7D00".to_string()));

        self.set(BAR_SEGMENT_COUNT, UIStyleValue::Integer(10));
        self.set(BAR_SEGMENT_GAP, UIStyleValue::Float(2.0));

        // Text defaults
        self.set(TEXT_PRIMARY_COLOR, UIStyleValue::Color("#FF7D00".to_string()));
        self.set(TEXT_SECONDARY_COLOR, UIStyleValue::Color("#b77700".to_string()));
        self.set(TEXT_ACCENT_COLOR, UIStyleValue::Color("#0080FF".to_string()));
        self.set(TEXT_WARNING_COLOR, UIStyleValue::Color("#FFFF00".to_string()));
        self.set(TEXT_ERROR_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        
        self.set(TEXT_PRIMARY_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(TEXT_PRIMARY_FONT_SIZE, UIStyleValue::Integer(24));
        self.set(TEXT_SECONDARY_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(TEXT_SECONDARY_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(TEXT_MONOSPACE_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".to_string()));
        self.set(TEXT_MONOSPACE_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(TEXT_SMALL_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(TEXT_SMALL_FONT_SIZE, UIStyleValue::Integer(12));

        self.set(TEXT_LINE_SPACING, UIStyleValue::Float(1.2));
        self.set(TEXT_LETTER_SPACING, UIStyleValue::Float(0.0));
        
        // Indicator defaults
        self.set(INDICATOR_NORMAL_COLOR, UIStyleValue::Color("#00FF00".to_string()));
        self.set(INDICATOR_WARNING_COLOR, UIStyleValue::Color("#FFAA00".to_string()));
        self.set(INDICATOR_CRITICAL_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        self.set(INDICATOR_OFF_COLOR, UIStyleValue::Color("#404040".to_string()));
        self.set(INDICATOR_BLINK_SPEED, UIStyleValue::Float(2.0));
        self.set(INDICATOR_GLOW_ENABLED, UIStyleValue::Boolean(false));
        self.set(INDICATOR_GLOW_RADIUS, UIStyleValue::Float(5.0));
        self.set(INDICATOR_SIZE, UIStyleValue::Float(24.0));
        
        // Digital display defaults (amber theme like classic LCD displays)
        self.set(DIGITAL_DISPLAY_FONT, UIStyleValue::String(DIGITAL_DISPLAY_FONT_ITALIC_PATH.to_string()));
        self.set(DIGITAL_DISPLAY_FONT_SIZE, UIStyleValue::Integer(32));
        self.set(DIGITAL_DISPLAY_SCALE, UIStyleValue::Float(2.0));
        self.set(DIGITAL_DISPLAY_ACTIVE_COLOR, UIStyleValue::Color("#FFA500".to_string())); // Amber active segments
        self.set(DIGITAL_DISPLAY_INACTIVE_COLOR, UIStyleValue::Color("#996600".to_string())); // Dark amber inactive segments
        self.set(DIGITAL_DISPLAY_INACTIVE_COLOR_BLENDING, UIStyleValue::Float(0.4));
        self.set(DIGITAL_DISPLAY_BACKGROUND_COLOR, UIStyleValue::Color("#000000".to_string())); // Amber background
        self.set(DIGITAL_DISPLAY_BACKGROUND_ENABLED, UIStyleValue::Boolean(false));
        self.set(DIGITAL_DISPLAY_BORDER_ENABLED, UIStyleValue::Boolean(true));
        self.set(DIGITAL_DISPLAY_BORDER_COLOR, UIStyleValue::Color("#FFA500".to_string()));
        self.set(DIGITAL_DISPLAY_BORDER_WIDTH, UIStyleValue::Float(4.0));
        self.set(DIGITAL_DISPLAY_BORDER_RADIUS, UIStyleValue::Float(10.0));

        // Extended digital display font defaults
        self.set(DIGITAL_DISPLAY_FONT_ITALIC, UIStyleValue::String(DIGITAL_DISPLAY_FONT_ITALIC_PATH.to_string()));
        self.set(DIGITAL_DISPLAY_14SEG_FONT, UIStyleValue::String(DIGITAL_DISPLAY_14SEG_FONT_PATH.to_string()));
        self.set(DIGITAL_DISPLAY_14SEG_ITALIC, UIStyleValue::String(DIGITAL_DISPLAY_14SEG_ITALIC_PATH.to_string()));
        
        // Animation defaults
        self.set(ANIMATION_NEEDLE_SPEED, UIStyleValue::Float(1.0));
        self.set(ANIMATION_BAR_SPEED, UIStyleValue::Float(1.0));
        self.set(ANIMATION_SMOOTH_ENABLED, UIStyleValue::Boolean(true));

        // Alerts defaults
        self.set(ALERT_FONT_PATH, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".to_string()));
        self.set(ALERT_FONT_SIZE, UIStyleValue::Integer(32));
        self.set(ALERT_WARNING_COLOR, UIStyleValue::Color("#FFFF00".to_string()));
        self.set(ALERT_CRITICAL_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        self.set(ALERT_BACKGROUND_COLOR, UIStyleValue::Color("#000000".to_string()));
        self.set(ALERT_BORDER_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(ALERT_BORDER_WIDTH, UIStyleValue::Float(4.0));
        self.set(ALERT_MARGIN, UIStyleValue::Float(8.0));
        self.set(ALERT_CORNER_RADIUS, UIStyleValue::Float(8.0));
        self.set(ALERT_SOUND_PATH, UIStyleValue::String("".to_string())); // No sound by default
    }
}

impl Default for UIStyle {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Parse color string to RGB values (0.0-1.0)
fn parse_color(color_str: &str) -> Result<(f32, f32, f32), String> {
    if color_str.starts_with('#') {
        // Hex color: #RRGGBB or #RGB
        let hex = &color_str[1..];
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

/// Check if string is a named color
fn is_named_color(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), 
        "black" | "white" | "red" | "green" | "blue" | "yellow" | 
        "cyan" | "magenta" | "gray" | "grey" | "orange")
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
    
    #[test]
    fn test_style_value_conversion() {
        let color_val = UIStyleValue::Color("#FF0000".to_string());
        assert_eq!(color_val.as_color().unwrap(), (1.0, 0.0, 0.0));
        
        let float_val = UIStyleValue::Float(2.5);
        assert_eq!(float_val.as_float().unwrap(), 2.5);
        
        let bool_val = UIStyleValue::Boolean(true);
        assert_eq!(bool_val.as_bool().unwrap(), true);
    }
    
    #[test]
    fn test_json_serialization() {
        let mut style = UIStyle::new();
        style.set(GAUGE_NEEDLE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        style.set(GAUGE_BORDER_WIDTH, UIStyleValue::Float(2.5));
        style.set(GAUGE_LABEL_ENABLED, UIStyleValue::Boolean(true));
        
        let json = style.to_json().unwrap();
        let loaded_style = UIStyle::from_json(&json).unwrap();
        
        assert_eq!(loaded_style.get_color(GAUGE_NEEDLE_COLOR, (0.0, 0.0, 0.0)), (1.0, 0.0, 0.0));
        assert_eq!(loaded_style.get_float(GAUGE_BORDER_WIDTH, 0.0), 2.5);
        assert_eq!(loaded_style.get_bool(GAUGE_LABEL_ENABLED, false), true);
    }
    
    #[test]
    fn test_brightness_application() {
        let mut style = UIStyle::new();
        style.set(GLOBAL_BRIGHTNESS, UIStyleValue::Float(0.5));
        style.set(GAUGE_NEEDLE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        
        let color = style.get_color(GAUGE_NEEDLE_COLOR, (0.0, 0.0, 0.0));
        assert_eq!(color, (0.5, 0.0, 0.0)); // Should be dimmed
    }

    #[test]
    fn test_brightness_management() {
        let mut style = UIStyle::new();
        
        // Test default brightness
        assert_eq!(style.get_brightness(), 1.0);
        
        // Test setting brightness
        style.set_brightness(0.7);
        assert_eq!(style.get_brightness(), 0.7);
        
        // Test increasing brightness
        style.increase_brightness(0.2);
        assert_eq!(style.get_brightness(), 0.9);
        
        // Test decreasing brightness (use epsilon for floating point comparison)
        style.decrease_brightness(0.3);
        let brightness = style.get_brightness();
        assert!((brightness - 0.6).abs() < 0.001, "Expected ~0.6, got {}", brightness);
        
        // Test clamping to bounds
        style.set_brightness(1.5); // Should be clamped to 1.0
        assert_eq!(style.get_brightness(), 1.0);
        
        style.set_brightness(-0.5); // Should be clamped to 0.0
        assert_eq!(style.get_brightness(), 0.0);
        
        // Test apply_brightness function
        let color = (1.0, 0.5, 0.2);
        style.set_brightness(0.8);
        let adjusted = style.apply_brightness(color);
        
        // Use epsilon comparison for floating point values
        assert!((adjusted.0 - 0.8).abs() < 0.001);
        assert!((adjusted.1 - 0.4).abs() < 0.001);
        assert!((adjusted.2 - 0.16).abs() < 0.001);
    }

    #[test]
    fn test_warning_messages() {
        let style = UIStyle::new();
        
        // Test color warning
        let color = style.get_color("non_existent_color", (0.5, 0.5, 0.5));
        assert_eq!(color, (0.5, 0.5, 0.5));
        
        // Test float warning
        let float_val = style.get_float("non_existent_float", 3.14);
        assert_eq!(float_val, 3.14);
        
        // Test integer warning
        let int_val = style.get_integer("non_existent_int", 42);
        assert_eq!(int_val, 42);
        
        // Test boolean warning
        let bool_val = style.get_bool("non_existent_bool", true);
        assert_eq!(bool_val, true);
        
        // Test string warning
        let string_val = style.get_string("non_existent_string", "default");
        assert_eq!(string_val, "default");
    }
}