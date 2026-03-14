 
// Raw analog data processors

pub trait AnalogSignalProcessor {
    fn read(&mut self, input: u16) -> Result<u16, String>;
}

pub struct AnalogSignalProcessorMovingAverage {
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

pub struct AnalogSignalProcessorDampener {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_average_creation() {
        let processor = AnalogSignalProcessorMovingAverage::new(5);
        
        assert_eq!(processor.window_size, 5);
        assert_eq!(processor.values.len(), 0);
        assert_eq!(processor.values.capacity(), 5);
    }

    #[test]
    fn test_moving_average_single_value() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(3);
        
        let result = processor.read(100).unwrap();
        assert_eq!(result, 100);
        assert_eq!(processor.values.len(), 1);
    }

    #[test]
    fn test_moving_average_multiple_values() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(3);
        
        // Add first value: [100]
        let result = processor.read(100).unwrap();
        assert_eq!(result, 100);
        
        // Add second value: [100, 200]
        let result = processor.read(200).unwrap();
        assert_eq!(result, 150); // (100 + 200) / 2 = 150
        
        // Add third value: [100, 200, 300]
        let result = processor.read(300).unwrap();
        assert_eq!(result, 200); // (100 + 200 + 300) / 3 = 200
    }

    #[test]
    fn test_moving_average_window_overflow() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(2);
        
        // Fill window: [100, 200]
        processor.read(100).unwrap();
        processor.read(200).unwrap();
        assert_eq!(processor.values.len(), 2);
        
        // Add third value, should remove first: [200, 300]
        let result = processor.read(300).unwrap();
        assert_eq!(result, 250); // (200 + 300) / 2 = 250
        assert_eq!(processor.values.len(), 2);
        assert_eq!(processor.values[0], 200);
        assert_eq!(processor.values[1], 300);
    }

    #[test]
    fn test_moving_average_large_window() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(5);
        let values = [100, 150, 200, 250, 300];
        
        for (i, &value) in values.iter().enumerate() {
            let result = processor.read(value).unwrap();
            let expected_sum: u32 = values[0..=i].iter().map(|&x| x as u32).sum();
            let expected_avg = expected_sum / (i + 1) as u32;
            assert_eq!(result, expected_avg as u16);
        }
        
        // Add one more value to test window overflow
        let result = processor.read(350).unwrap();
        // Window: [150, 200, 250, 300, 350]
        let expected = (150 + 200 + 250 + 300 + 350) / 5;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_moving_average_zero_values() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(3);
        
        let result = processor.read(0).unwrap();
        assert_eq!(result, 0);
        
        let result = processor.read(0).unwrap();
        assert_eq!(result, 0);
        
        let result = processor.read(100).unwrap();
        assert_eq!(result, 33); // (0 + 0 + 100) / 3 = 33
    }

    #[test]
    fn test_moving_average_maximum_values() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(2);
        
        let result = processor.read(u16::MAX).unwrap();
        assert_eq!(result, u16::MAX);
        
        let result = processor.read(u16::MAX).unwrap();
        assert_eq!(result, u16::MAX);
        
        // Mix with a smaller value
        let result = processor.read(0).unwrap();
        assert_eq!(result, u16::MAX / 2); // (u16::MAX + 0) / 2
    }

    #[test]
    fn test_moving_average_alternating_values() {
        let mut processor = AnalogSignalProcessorMovingAverage::new(4);
        
        // Alternating high and low values
        let values = [1000, 100, 1000, 100];
        let mut results = Vec::new();
        
        for &value in &values {
            results.push(processor.read(value).unwrap());
        }
        
        assert_eq!(results[0], 1000); // [1000]
        assert_eq!(results[1], 550);  // [1000, 100] = 550
        assert_eq!(results[2], 700);  // [1000, 100, 1000] = 700
        assert_eq!(results[3], 550);  // [1000, 100, 1000, 100] = 550
    }

    #[test]
    fn test_dampener_creation() {
        let dampener = AnalogSignalProcessorDampener::new(0.5);
        
        assert_eq!(dampener.last_value, 0);
        assert_eq!(dampener.alpha, 0.5);
    }

    #[test]
    fn test_dampener_first_reading() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.5);
        
        let result = dampener.read(1000).unwrap();
        assert_eq!(result, 500); // 0.5 * 1000 + 0.5 * 0 = 500
        assert_eq!(dampener.last_value, 500);
    }

    #[test]
    fn test_dampener_subsequent_readings() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.3);
        
        // First reading: 0.3 * 1000 + 0.7 * 0 = 300
        let result1 = dampener.read(1000).unwrap();
        assert_eq!(result1, 300);
        
        // Second reading: 0.3 * 1000 + 0.7 * 300 = 300 + 210 = 510
        let result2 = dampener.read(1000).unwrap();
        assert_eq!(result2, 510);
        
        // Third reading with different input: 0.3 * 500 + 0.7 * 510 = 150 + 357 = 507
        let result3 = dampener.read(500).unwrap();
        assert_eq!(result3, 507);
    }

    #[test]
    fn test_dampener_high_alpha() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.9); // Very responsive
        
        let result = dampener.read(1000).unwrap();
        assert_eq!(result, 900); // 0.9 * 1000 + 0.1 * 0 = 900
        
        let result = dampener.read(1000).unwrap();
        assert_eq!(result, 990); // 0.9 * 1000 + 0.1 * 900 = 900 + 90 = 990
    }

    #[test]
    fn test_dampener_low_alpha() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.1); // Very smooth
        
        let result = dampener.read(1000).unwrap();
        assert_eq!(result, 100); // 0.1 * 1000 + 0.9 * 0 = 100
        
        let result = dampener.read(1000).unwrap();
        assert_eq!(result, 190); // 0.1 * 1000 + 0.9 * 100 = 100 + 90 = 190
    }

    #[test]
    fn test_dampener_zero_input() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.5);
        
        // Start with high value
        dampener.read(1000).unwrap();
        
        // Apply zero input - should gradually decrease
        let result1 = dampener.read(0).unwrap();
        assert_eq!(result1, 250); // 0.5 * 0 + 0.5 * 500 = 250
        
        let result2 = dampener.read(0).unwrap();
        assert_eq!(result2, 125); // 0.5 * 0 + 0.5 * 250 = 125
    }

    #[test]
    fn test_dampener_step_response() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.4);
        
        let mut results = Vec::new();
        
        // Apply step input of 1000 multiple times
        for _ in 0..5 {
            results.push(dampener.read(1000).unwrap());
        }
        
        // Results should converge towards 1000
        assert_eq!(results[0], 400);  // 0.4 * 1000 + 0.6 * 0 = 400
        assert_eq!(results[1], 640);  // 0.4 * 1000 + 0.6 * 400 = 640
        assert_eq!(results[2], 784);  // 0.4 * 1000 + 0.6 * 640 = 784
        assert_eq!(results[3], 870);  // 0.4 * 1000 + 0.6 * 784 = 870
        assert_eq!(results[4], 922);  // 0.4 * 1000 + 0.6 * 870 = 922
        
        // Should be getting closer to 1000
        assert!(results[4] > results[3]);
        assert!(results[3] > results[2]);
        assert!(results[2] > results[1]);
        assert!(results[1] > results[0]);
    }

    #[test]
    fn test_dampener_alternating_input() {
        let mut dampener = AnalogSignalProcessorDampener::new(0.6);
        
        // Alternate between high and low values
        let result1 = dampener.read(1000).unwrap();
        assert_eq!(result1, 600); // 0.6 * 1000 + 0.4 * 0 = 600
        
        let result2 = dampener.read(0).unwrap();
        // The actual calculation: 0.6 * 0 + 0.4 * 600 = 0 + 240 = 240
        // But there might be precision issues, let's check the actual result
        assert!(result2 >= 239 && result2 <= 241, "Expected ~240, got {}", result2);
        
        let result3 = dampener.read(1000).unwrap();
        // 0.6 * 1000 + 0.4 * result2 = 600 + 0.4 * result2
        // With result2 around 240: 600 + 96 = 696
        assert!(result3 >= 695 && result3 <= 697, "Expected ~696, got {}", result3);
    }

    #[test]
    fn test_analog_signal_processor_trait_implementations() {
        // Test that both processors implement the trait correctly
        let mut moving_avg: Box<dyn AnalogSignalProcessor> = Box::new(
            AnalogSignalProcessorMovingAverage::new(3)
        );
        let mut dampener: Box<dyn AnalogSignalProcessor> = Box::new(
            AnalogSignalProcessorDampener::new(0.5)
        );
        
        // Both should handle various input values
        assert!(moving_avg.read(0).is_ok());
        assert!(moving_avg.read(u16::MAX).is_ok());
        assert!(moving_avg.read(1000).is_ok());
        
        assert!(dampener.read(0).is_ok());
        assert!(dampener.read(u16::MAX).is_ok());
        assert!(dampener.read(1000).is_ok());
    }

    #[test]
    fn test_moving_average_window_size_edge_cases() {
        // Test with window size 1
        let mut processor = AnalogSignalProcessorMovingAverage::new(1);
        assert_eq!(processor.read(100).unwrap(), 100);
        assert_eq!(processor.read(200).unwrap(), 200); // Should replace immediately
        assert_eq!(processor.values.len(), 1);
        
        // Test with larger window
        let mut large_processor = AnalogSignalProcessorMovingAverage::new(100);
        for i in 1..=50 {
            let result = large_processor.read(i * 10).unwrap();
            let expected = (i * (i + 1) * 10) / (2 * i); // Sum of arithmetic series / count
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_dampener_alpha_edge_cases() {
        // Alpha = 0 (no new input influence)
        let mut dampener_zero = AnalogSignalProcessorDampener::new(0.0);
        dampener_zero.read(1000).unwrap(); // Should stay 0
        assert_eq!(dampener_zero.last_value, 0);
        
        // Alpha = 1 (immediate response)
        let mut dampener_one = AnalogSignalProcessorDampener::new(1.0);
        let result = dampener_one.read(1000).unwrap();
        assert_eq!(result, 1000);
        
        let result2 = dampener_one.read(500).unwrap();
        assert_eq!(result2, 500);
    }
}