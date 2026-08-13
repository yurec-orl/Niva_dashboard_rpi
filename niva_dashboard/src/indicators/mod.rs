pub mod indicator;
pub mod text_indicator;
pub mod gauge_indicator;
pub mod digital_segmented_indicator;
pub mod vertical_bar_indicator;
pub mod needle_indicator;
pub mod needle_shape;
pub mod compass_indicator;
pub mod pitch_indicator;
pub mod roll_indicator;
pub mod decorator;

// Re-export main types for convenience
pub use indicator::{
    Indicator, 
    IndicatorBounds
};

