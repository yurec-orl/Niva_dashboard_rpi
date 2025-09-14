pub mod indicator;
pub mod text_indicator;
pub mod gauge_indicator;

// Re-export main types for convenience
pub use indicator::{
    Indicator, 
    IndicatorBounds
};

pub use text_indicator::{TextIndicator, TextAlignment};
pub use gauge_indicator::GaugeIndicator;
