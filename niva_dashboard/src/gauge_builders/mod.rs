pub mod speedometer_gauge;
pub mod fuel_level_gauge;
pub mod oil_pressure_gauge;
pub mod temperature_gauge;
pub mod voltage_gauge;

pub use speedometer_gauge::build_speedometer_gauge;
pub use fuel_level_gauge::build_fuel_level_gauge;
pub use oil_pressure_gauge::build_oil_pressure_gauge;
pub use temperature_gauge::build_temperature_gauge;
pub use voltage_gauge::build_voltage_gauge;
