
//! UI Style Configuration System
//!
//! Simple flat key-value style system for dashboard elements.
//! Uses string constants for style element names and supports JSON serialization.
//!
//! Example JSON format:
//! ```json
//! {
//!   "needle_color": "#FF0000",
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

// Global Style Elements
pub const GLOBAL_BRIGHTNESS: &str = "global_brightness";
pub const GLOBAL_CONTRAST: &str = "global_contrast";
pub const GLOBAL_BACKGROUND_COLOR: &str = "global_background_color";
pub const GLOBAL_BRAND_PRIMARY_COLOR: &str = "global_brand_primary_color";
pub const GLOBAL_BRAND_SECONDARY_COLOR: &str = "global_brand_secondary_color";
pub const GLOBAL_BRAND_ACCENT_COLOR: &str = "global_brand_accent_color";
pub const GLOBAL_FONT_PATH: &str = "global_font_path";
pub const GLOBAL_FONT_SIZE: &str = "global_font_size";

// Gauge Style Elements
pub const GAUGE_BACKGROUND_COLOR: &str = "gauge_background_color";
pub const GAUGE_BORDER_COLOR: &str = "gauge_border_color";
pub const GAUGE_BORDER_WIDTH: &str = "gauge_border_width";
pub const GAUGE_RADIUS: &str = "gauge_radius";

// Gauge Needle
pub const NEEDLE_COLOR: &str = "needle_color";
pub const NEEDLE_WIDTH: &str = "needle_width";
pub const NEEDLE_LENGTH: &str = "needle_length";
pub const NEEDLE_TIP_WIDTH: &str = "needle_tip_width";
pub const NEEDLE_CENTER_COLOR: &str = "needle_center_color";
pub const NEEDLE_CENTER_RADIUS: &str = "needle_center_radius";
pub const NEEDLE_SHADOW_ENABLED: &str = "needle_shadow_enabled";
pub const NEEDLE_SHADOW_COLOR: &str = "needle_shadow_color";

// Gauge Marks
pub const GAUGE_MAJOR_MARK_COLOR: &str = "gauge_major_mark_color";
pub const GAUGE_MAJOR_MARK_WIDTH: &str = "gauge_major_mark_width";
pub const GAUGE_MAJOR_MARK_LENGTH: &str = "gauge_major_mark_length";
pub const GAUGE_MAJOR_MARK_OFFSET: &str = "gauge_major_mark_offset";
pub const GAUGE_MAJOR_MARK_ENABLED: &str = "gauge_major_mark_enabled";

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
pub const GAUGE_TITLE_OFFSET: &str = "gauge_title_offset";
pub const GAUGE_TITLE_ENABLED: &str = "gauge_title_enabled";

pub const GAUGE_UNIT_COLOR: &str = "gauge_unit_color";
pub const GAUGE_UNIT_FONT: &str = "gauge_unit_font";
pub const GAUGE_UNIT_FONT_SIZE: &str = "gauge_unit_font_size";
pub const GAUGE_UNIT_OFFSET: &str = "gauge_unit_offset";
pub const GAUGE_UNIT_ENABLED: &str = "gauge_unit_enabled";

// Gauge Zones
pub const GAUGE_WARNING_ZONE_COLOR: &str = "gauge_warning_zone_color";
pub const GAUGE_WARNING_ZONE_START: &str = "gauge_warning_zone_start";
pub const GAUGE_WARNING_ZONE_END: &str = "gauge_warning_zone_end";
pub const GAUGE_WARNING_ZONE_WIDTH: &str = "gauge_warning_zone_width";
pub const GAUGE_WARNING_ZONE_ENABLED: &str = "gauge_warning_zone_enabled";

pub const GAUGE_CRITICAL_ZONE_COLOR: &str = "gauge_critical_zone_color";
pub const GAUGE_CRITICAL_ZONE_START: &str = "gauge_critical_zone_start";
pub const GAUGE_CRITICAL_ZONE_END: &str = "gauge_critical_zone_end";
pub const GAUGE_CRITICAL_ZONE_WIDTH: &str = "gauge_critical_zone_width";
pub const GAUGE_CRITICAL_ZONE_ENABLED: &str = "gauge_critical_zone_enabled";

// Bar Indicator Style Elements
pub const BAR_BACKGROUND_COLOR: &str = "bar_background_color";
pub const BAR_BORDER_COLOR: &str = "bar_border_color";
pub const BAR_BORDER_WIDTH: &str = "bar_border_width";
pub const BAR_CORNER_RADIUS: &str = "bar_corner_radius";
pub const BAR_WIDTH: &str = "bar_width";
pub const BAR_HEIGHT: &str = "bar_height";

pub const BAR_FILL_COLOR: &str = "bar_fill_color";
pub const BAR_FILL_GRADIENT_ENABLED: &str = "bar_fill_gradient_enabled";
pub const BAR_FILL_GRADIENT_START: &str = "bar_fill_gradient_start";
pub const BAR_FILL_GRADIENT_END: &str = "bar_fill_gradient_end";

pub const BAR_SEGMENTS_ENABLED: &str = "bar_segments_enabled";
pub const BAR_SEGMENT_COUNT: &str = "bar_segment_count";
pub const BAR_SEGMENT_SPACING: &str = "bar_segment_spacing";
pub const BAR_SEGMENT_NORMAL_COLOR: &str = "bar_segment_normal_color";
pub const BAR_SEGMENT_WARNING_COLOR: &str = "bar_segment_warning_color";
pub const BAR_SEGMENT_CRITICAL_COLOR: &str = "bar_segment_critical_color";

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
    values: HashMap<String, UIStyleValue>,
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
    pub fn from_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let values: HashMap<String, UIStyleValue> = serde_json::from_str(json_str)?;
        let mut style = UIStyle::new(); // Start with defaults
        
        // Override defaults with loaded values
        for (key, value) in values {
            style.values.insert(key, value);
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
    
    /// Get a style value
    pub fn get(&self, key: &str) -> Option<&UIStyleValue> {
        self.values.get(key)
    }
    
    /// Set a style value
    pub fn set(&mut self, key: &str, value: UIStyleValue) {
        self.values.insert(key.to_string(), value);
    }
    
    /// Get color value with brightness applied
    pub fn get_color(&self, key: &str, default: (f32, f32, f32)) -> (f32, f32, f32) {
        match self.get(key) {
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
        let (r, g, b) = self.get_color(key, (default.0, default.1, default.2));
        (r, g, b, default.3)
    }
    
    /// Get float value with fallback
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        match self.get(key) {
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
        match self.get(key) {
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
        match self.get(key) {
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
        match self.get(key) {
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
        // Global defaults
        self.set(GLOBAL_BRIGHTNESS, UIStyleValue::Float(1.0));
        self.set(GLOBAL_CONTRAST, UIStyleValue::Float(1.0));
        self.set(GLOBAL_BACKGROUND_COLOR, UIStyleValue::Color("#000000".to_string()));
        self.set(GLOBAL_BRAND_PRIMARY_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GLOBAL_BRAND_SECONDARY_COLOR, UIStyleValue::Color("#808080".to_string()));
        self.set(GLOBAL_BRAND_ACCENT_COLOR, UIStyleValue::Color("#0080FF".to_string()));
        self.set(GLOBAL_FONT_PATH, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(GLOBAL_FONT_SIZE, UIStyleValue::Integer(16));
        
        // Gauge defaults
        self.set(GAUGE_BACKGROUND_COLOR, UIStyleValue::Color("#000000".to_string()));
        self.set(GAUGE_BORDER_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_BORDER_WIDTH, UIStyleValue::Float(2.0));
        self.set(GAUGE_RADIUS, UIStyleValue::Float(80.0));
        
        // Needle defaults
        self.set(NEEDLE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        self.set(NEEDLE_WIDTH, UIStyleValue::Float(4.0));
        self.set(NEEDLE_LENGTH, UIStyleValue::Float(0.8));
        self.set(NEEDLE_TIP_WIDTH, UIStyleValue::Float(1.0));
        self.set(NEEDLE_CENTER_COLOR, UIStyleValue::Color("#404040".to_string()));
        self.set(NEEDLE_CENTER_RADIUS, UIStyleValue::Float(8.0));
        self.set(NEEDLE_SHADOW_ENABLED, UIStyleValue::Boolean(false));
        self.set(NEEDLE_SHADOW_COLOR, UIStyleValue::Color("#000000".to_string()));
        
        // Gauge marks defaults
        self.set(GAUGE_MAJOR_MARK_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_MAJOR_MARK_WIDTH, UIStyleValue::Float(2.0));
        self.set(GAUGE_MAJOR_MARK_LENGTH, UIStyleValue::Float(15.0));
        self.set(GAUGE_MAJOR_MARK_OFFSET, UIStyleValue::Float(5.0));
        self.set(GAUGE_MAJOR_MARK_ENABLED, UIStyleValue::Boolean(true));
        
        self.set(GAUGE_MINOR_MARK_COLOR, UIStyleValue::Color("#808080".to_string()));
        self.set(GAUGE_MINOR_MARK_WIDTH, UIStyleValue::Float(1.0));
        self.set(GAUGE_MINOR_MARK_LENGTH, UIStyleValue::Float(8.0));
        self.set(GAUGE_MINOR_MARK_OFFSET, UIStyleValue::Float(5.0));
        self.set(GAUGE_MINOR_MARK_ENABLED, UIStyleValue::Boolean(true));
        self.set(GAUGE_MINOR_MARK_COUNT, UIStyleValue::Integer(4));
        
        // Label defaults
        self.set(GAUGE_LABEL_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_LABEL_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(GAUGE_LABEL_FONT_SIZE, UIStyleValue::Integer(14));
        self.set(GAUGE_LABEL_OFFSET, UIStyleValue::Float(25.0));
        self.set(GAUGE_LABEL_ENABLED, UIStyleValue::Boolean(true));
        
        self.set(GAUGE_TITLE_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(GAUGE_TITLE_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".to_string()));
        self.set(GAUGE_TITLE_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(GAUGE_TITLE_OFFSET, UIStyleValue::Float(30.0));
        self.set(GAUGE_TITLE_ENABLED, UIStyleValue::Boolean(true));
        
        self.set(GAUGE_UNIT_COLOR, UIStyleValue::Color("#C0C0C0".to_string()));
        self.set(GAUGE_UNIT_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(GAUGE_UNIT_FONT_SIZE, UIStyleValue::Integer(12));
        self.set(GAUGE_UNIT_OFFSET, UIStyleValue::Float(15.0));
        self.set(GAUGE_UNIT_ENABLED, UIStyleValue::Boolean(true));
        
        // Zone defaults
        self.set(GAUGE_WARNING_ZONE_COLOR, UIStyleValue::Color("#FFAA00".to_string()));
        self.set(GAUGE_WARNING_ZONE_START, UIStyleValue::Float(75.0));
        self.set(GAUGE_WARNING_ZONE_END, UIStyleValue::Float(90.0));
        self.set(GAUGE_WARNING_ZONE_WIDTH, UIStyleValue::Float(5.0));
        self.set(GAUGE_WARNING_ZONE_ENABLED, UIStyleValue::Boolean(false));
        
        self.set(GAUGE_CRITICAL_ZONE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        self.set(GAUGE_CRITICAL_ZONE_START, UIStyleValue::Float(90.0));
        self.set(GAUGE_CRITICAL_ZONE_END, UIStyleValue::Float(100.0));
        self.set(GAUGE_CRITICAL_ZONE_WIDTH, UIStyleValue::Float(5.0));
        self.set(GAUGE_CRITICAL_ZONE_ENABLED, UIStyleValue::Boolean(false));
        
        // Bar defaults
        self.set(BAR_BACKGROUND_COLOR, UIStyleValue::Color("#404040".to_string()));
        self.set(BAR_BORDER_COLOR, UIStyleValue::Color("#808080".to_string()));
        self.set(BAR_BORDER_WIDTH, UIStyleValue::Float(1.0));
        self.set(BAR_CORNER_RADIUS, UIStyleValue::Float(4.0));
        self.set(BAR_WIDTH, UIStyleValue::Float(200.0));
        self.set(BAR_HEIGHT, UIStyleValue::Float(20.0));
        self.set(BAR_FILL_COLOR, UIStyleValue::Color("#00FF00".to_string()));
        self.set(BAR_FILL_GRADIENT_ENABLED, UIStyleValue::Boolean(true));
        self.set(BAR_FILL_GRADIENT_START, UIStyleValue::Color("#00FF00".to_string()));
        self.set(BAR_FILL_GRADIENT_END, UIStyleValue::Color("#FFFF00".to_string()));
        
        self.set(BAR_SEGMENTS_ENABLED, UIStyleValue::Boolean(false));
        self.set(BAR_SEGMENT_COUNT, UIStyleValue::Integer(10));
        self.set(BAR_SEGMENT_SPACING, UIStyleValue::Float(2.0));
        self.set(BAR_SEGMENT_NORMAL_COLOR, UIStyleValue::Color("#00FF00".to_string()));
        self.set(BAR_SEGMENT_WARNING_COLOR, UIStyleValue::Color("#FFAA00".to_string()));
        self.set(BAR_SEGMENT_CRITICAL_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        
        // Text defaults
        self.set(TEXT_PRIMARY_COLOR, UIStyleValue::Color("#FFFFFF".to_string()));
        self.set(TEXT_SECONDARY_COLOR, UIStyleValue::Color("#C0C0C0".to_string()));
        self.set(TEXT_ACCENT_COLOR, UIStyleValue::Color("#0080FF".to_string()));
        self.set(TEXT_WARNING_COLOR, UIStyleValue::Color("#FFAA00".to_string()));
        self.set(TEXT_ERROR_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        
        self.set(TEXT_PRIMARY_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(TEXT_PRIMARY_FONT_SIZE, UIStyleValue::Integer(16));
        self.set(TEXT_SECONDARY_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(TEXT_SECONDARY_FONT_SIZE, UIStyleValue::Integer(14));
        self.set(TEXT_MONOSPACE_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".to_string()));
        self.set(TEXT_MONOSPACE_FONT_SIZE, UIStyleValue::Integer(14));
        self.set(TEXT_SMALL_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string()));
        self.set(TEXT_SMALL_FONT_SIZE, UIStyleValue::Integer(10));

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
        
        // Animation defaults
        self.set(ANIMATION_NEEDLE_SPEED, UIStyleValue::Float(1.0));
        self.set(ANIMATION_BAR_SPEED, UIStyleValue::Float(1.0));
        self.set(ANIMATION_SMOOTH_ENABLED, UIStyleValue::Boolean(true));
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
        style.set(NEEDLE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        style.set(GAUGE_BORDER_WIDTH, UIStyleValue::Float(2.5));
        style.set(GAUGE_LABEL_ENABLED, UIStyleValue::Boolean(true));
        
        let json = style.to_json().unwrap();
        let loaded_style = UIStyle::from_json(&json).unwrap();
        
        assert_eq!(loaded_style.get_color(NEEDLE_COLOR, (0.0, 0.0, 0.0)), (1.0, 0.0, 0.0));
        assert_eq!(loaded_style.get_float(GAUGE_BORDER_WIDTH, 0.0), 2.5);
        assert_eq!(loaded_style.get_bool(GAUGE_LABEL_ENABLED, false), true);
    }
    
    #[test]
    fn test_brightness_application() {
        let mut style = UIStyle::new();
        style.set(GLOBAL_BRIGHTNESS, UIStyleValue::Float(0.5));
        style.set(NEEDLE_COLOR, UIStyleValue::Color("#FF0000".to_string()));
        
        let color = style.get_color(NEEDLE_COLOR, (0.0, 0.0, 0.0));
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