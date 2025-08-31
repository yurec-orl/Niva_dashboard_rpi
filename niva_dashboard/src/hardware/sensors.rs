// Framework to read and process data from hardware pins.
// This module provides abstractions for various sensors and their data.
// Intended architecture for data flow:
//   HWDigReader -> digital signal processing (debouncing, smoothing) ->
//   -> DigSensor(convert raw data to logical values) -> UI Rendering
//   HWAnalogReader -> analog signal processing (filtering, smoothing) ->
//   -> AnalogSensor(convert raw data to logical values) -> UI Rendering
// Considerations: digital signals could be read from gpio directly.
// Analog signals require ADC (Analog-to-Digital Converter) for processing,
// which is not available on Raspi. For analog signals,
// interfacing with external ADC or controller is required, most likely using
// I2C interface.