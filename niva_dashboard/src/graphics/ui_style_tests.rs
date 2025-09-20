#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_grouped_style_basic() {
        let mut style = UIStyle::new();
        
        // Test setting values in different groups
        style.set_with_group("needle_color", UIStyleValue::Color("#FF0000".to_string()), Some("default"));
        style.set_with_group("needle_color", UIStyleValue::Color("#00FF00".to_string()), Some("fuel_level"));
        style.set_with_group("needle_color", UIStyleValue::Color("#0000FF".to_string()), Some("engine_temp"));
        
        // Test getting with fallback
        assert_eq!(style.get_with_group("needle_color", Some("fuel_level")).unwrap().as_color().unwrap(), (0.0, 1.0, 0.0));
        assert_eq!(style.get_with_group("needle_color", Some("engine_temp")).unwrap().as_color().unwrap(), (0.0, 0.0, 1.0));
        assert_eq!(style.get_with_group("needle_color", Some("nonexistent")).unwrap().as_color().unwrap(), (1.0, 0.0, 0.0)); // fallback to default
        
        // Test default getter
        assert_eq!(style.get("needle_color").unwrap().as_color().unwrap(), (1.0, 0.0, 0.0));
    }
    
    #[test]
    fn test_grouped_json_serialization() {
        let json_str = r#"
        {
            "default": {
                "needle_color": "#FF0000",
                "gauge_radius": 80.0
            },
            "fuel_level": {
                "needle_color": "#00FF00"
            }
        }
        "#;
        
        let style = UIStyle::from_json(json_str).unwrap();
        
        // Test that groups are loaded correctly
        assert_eq!(style.get_with_group("needle_color", Some("default")).unwrap().as_color().unwrap(), (1.0, 0.0, 0.0));
        assert_eq!(style.get_with_group("needle_color", Some("fuel_level")).unwrap().as_color().unwrap(), (0.0, 1.0, 0.0));
        assert_eq!(style.get_with_group("gauge_radius", Some("fuel_level")).unwrap().as_float().unwrap(), 80.0); // fallback to default
    }
    
    #[test]
    fn test_backward_compatibility_flat_json() {
        let json_str = r#"
        {
            "needle_color": "#FF0000",
            "gauge_radius": 80.0
        }
        "#;
        
        let style = UIStyle::from_json(json_str).unwrap();
        
        // Old flat format should work and values should go into "default" group
        assert_eq!(style.get("needle_color").unwrap().as_color().unwrap(), (1.0, 0.0, 0.0));
        assert_eq!(style.get("gauge_radius").unwrap().as_float().unwrap(), 80.0);
    }
    
    #[test]
    fn test_convenience_methods_with_groups() {
        let mut style = UIStyle::new();
        
        style.set_with_group("needle_color", UIStyleValue::Color("#FF6600".to_string()), Some("engine_temp"));
        style.set_with_group("needle_width", UIStyleValue::Float(5.0), Some("engine_temp"));
        
        // Test convenience methods with groups
        let color = style.get_color_with_group("needle_color", (1.0, 0.0, 0.0), Some("engine_temp"));
        assert_eq!(color.0, 1.0); // Red component should be 1.0 from #FF6600
        assert!(color.1 > 0.3 && color.1 < 0.5); // Green component should be around 0.4
        
        let width = style.get_float_with_group("needle_width", 2.0, Some("engine_temp"));
        assert_eq!(width, 5.0);
        
        // Test fallback when group doesn't have the value
        let fallback_width = style.get_float_with_group("nonexistent_key", 2.0, Some("engine_temp"));
        assert_eq!(fallback_width, 2.0);
    }
}
