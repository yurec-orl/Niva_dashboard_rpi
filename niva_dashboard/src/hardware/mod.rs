//! Hardware interface module for Raspberry Pi GPIO and sensors

pub mod gpio_input;
pub mod hw_providers;
pub mod digital_signal_processing;
pub mod analog_signal_processing;
pub mod sensors;
pub mod sensor_manager;

pub use gpio_input::GpioInput;
