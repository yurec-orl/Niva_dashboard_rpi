pub mod indicator;
pub mod text_indicator;

// Re-export main types for convenience
pub use indicator::{
    Indicator, 
    SensorValue, 
    ValueData, 
    ValueConstraints, 
    ValueMetadata, 
    IndicatorBounds
};

pub use text_indicator::{TextIndicator, TextAlignment};
