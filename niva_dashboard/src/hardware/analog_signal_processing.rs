use rppal::gpio::Level;
use crate::hardware::hw_providers::{HWAnalogInput, HWAnalogProvider};
 
// Raw analog data processors

pub trait AnalogSignalProcessor {
    fn read(&mut self, input: u16) -> Result<u16, String>;
}

struct AnalogSignalProcessorMovingAverage {
    window_size: usize,
    values: Vec<u16>,
}

impl AnalogSignalProcessorMovingAverage {
    pub fn new(window_size: usize) -> Self {
        AnalogSignalProcessorMovingAverage {
            window_size,
            values: Vec::with_capacity(window_size),
        }
    }
}

impl AnalogSignalProcessor for AnalogSignalProcessorMovingAverage {
    fn read(&mut self, input: u16) -> Result<u16, String> {
        // Add new value to the window
        self.values.push(input);
        
        // Remove oldest value if we exceed window size
        if self.values.len() > self.window_size {
            self.values.remove(0);
        }
        
        // Calculate moving average
        let sum: u32 = self.values.iter().map(|&x| x as u32).sum();
        let average = sum / self.values.len() as u32;
        
        Ok(average as u16)
    }
}

struct AnalogSignalProcessorDampener {
    last_value: u16,
    alpha: f32, // Smoothing factor between 0.0 and 1.0
}

impl AnalogSignalProcessorDampener {
    pub fn new(alpha: f32) -> Self {
        AnalogSignalProcessorDampener {
            last_value: 0,
            alpha,
        }
    }
}

impl AnalogSignalProcessor for AnalogSignalProcessorDampener {
    fn read(&mut self, input: u16) -> Result<u16, String> {
        self.last_value = (self.alpha * input as f32 + (1.0 - self.alpha) * self.last_value as f32) as u16;
        Ok(self.last_value)
    }
}