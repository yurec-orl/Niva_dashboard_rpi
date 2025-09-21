pub mod indicator;
pub mod text_indicator;
pub mod gauge_indicator;
pub mod digital_segmented_indicator;
pub mod vertical_bar_indicator;

// Re-export main types for convenience
pub use indicator::{
    Indicator, 
    IndicatorBounds
};
pub use digital_segmented_indicator::DigitalSegmentedIndicator;
pub use vertical_bar_indicator::VerticalBarIndicator;

