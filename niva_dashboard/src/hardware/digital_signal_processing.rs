use rppal::gpio::Level;
use std::time::{Duration, Instant};


// Raw digital data processors

pub trait DigitalSignalProcessor {
    fn read(&mut self, input: Level) -> Result<Level, String>;
}

pub struct DigitalSignalDebouncer {
    required_stable_count: u8,
    required_stable_delay: Duration,
    last_stable_state: Level,
    last_confirmed_state: Level,
    stable_count: u8,
    timer: Instant,
}

impl DigitalSignalDebouncer {
    pub fn new(required_stable_count: u8, required_stable_delay: Duration) -> Self {
        DigitalSignalDebouncer {
            required_stable_count,
            required_stable_delay,
            last_stable_state: Level::Low,
            last_confirmed_state: Level::Low,
            stable_count: 0,
            timer: Instant::now(),
        }
    }
}

impl DigitalSignalProcessor for DigitalSignalDebouncer {
    fn read(&mut self, input: Level) -> Result<Level, String> {
        let current_state = input;

        if current_state == self.last_stable_state {
            // State is same as what we're tracking
            if self.stable_count < u8::MAX {
                self.stable_count += 1;
            }
            
            // If state has been stable for required duration, confirm it
            if self.stable_count >= self.required_stable_count 
               && self.timer.elapsed() >= self.required_stable_delay {
                self.last_confirmed_state = self.last_stable_state;
            }
        } else {
            // State changed, reset counter and start tracking new state
            self.stable_count = 1; // Start counting the new state
            self.last_stable_state = current_state;
            self.timer = Instant::now();
        }
        
        // Always return the last confirmed stable state
        Ok(self.last_confirmed_state)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_digital_signal_debouncer_creation() {
        let debouncer = DigitalSignalDebouncer::new(3, Duration::from_millis(50));
        
        // Initial state should be Low
        assert_eq!(debouncer.last_stable_state, Level::Low);
        assert_eq!(debouncer.last_confirmed_state, Level::Low);
        assert_eq!(debouncer.stable_count, 0);
        assert_eq!(debouncer.required_stable_count, 3);
        assert_eq!(debouncer.required_stable_delay, Duration::from_millis(50));
    }

    #[test]
    fn test_digital_signal_debouncer_basic_operation() {
        let mut debouncer = DigitalSignalDebouncer::new(2, Duration::from_millis(10));
        
        // Initial reading should be Low
        let result = debouncer.read(Level::Low).unwrap();
        assert_eq!(result, Level::Low);
        
        // First High reading - not stable yet
        let result = debouncer.read(Level::High).unwrap();
        assert_eq!(result, Level::Low); // Still returns last confirmed state
        
        // Second High reading - still need time delay
        let result = debouncer.read(Level::High).unwrap();
        assert_eq!(result, Level::Low); // Still returns last confirmed state
        
        // Wait for required delay
        thread::sleep(Duration::from_millis(15));
        
        // Third High reading after delay - should confirm High
        let result = debouncer.read(Level::High).unwrap();
        assert_eq!(result, Level::High); // Now confirmed
    }

    #[test]
    fn test_digital_signal_debouncer_state_changes() {
        let mut debouncer = DigitalSignalDebouncer::new(1, Duration::from_millis(5));
        
        // Start with Low
        debouncer.read(Level::Low).unwrap();
        thread::sleep(Duration::from_millis(10));
        
        // Confirm Low state
        let result = debouncer.read(Level::Low).unwrap();
        assert_eq!(result, Level::Low);
        
        // Change to High
        debouncer.read(Level::High).unwrap();
        thread::sleep(Duration::from_millis(10));
        
        // Confirm High state
        let result = debouncer.read(Level::High).unwrap();
        assert_eq!(result, Level::High);
        
        // Change back to Low
        debouncer.read(Level::Low).unwrap();
        thread::sleep(Duration::from_millis(10));
        
        // Confirm Low state again
        let result = debouncer.read(Level::Low).unwrap();
        assert_eq!(result, Level::Low);
    }

    #[test]
    fn test_digital_signal_debouncer_bouncing_signals() {
        let mut debouncer = DigitalSignalDebouncer::new(3, Duration::from_millis(20));
        
        // Initial state
        debouncer.read(Level::Low).unwrap();
        thread::sleep(Duration::from_millis(25));
        assert_eq!(debouncer.read(Level::Low).unwrap(), Level::Low);
        
        // Simulate bouncing: High-Low-High-Low-High
        assert_eq!(debouncer.read(Level::High).unwrap(), Level::Low);
        assert_eq!(debouncer.read(Level::Low).unwrap(), Level::Low);
        assert_eq!(debouncer.read(Level::High).unwrap(), Level::Low);
        assert_eq!(debouncer.read(Level::Low).unwrap(), Level::Low);
        assert_eq!(debouncer.read(Level::High).unwrap(), Level::Low);
        
        // Should still be Low because signals were not stable
        thread::sleep(Duration::from_millis(25));
        assert_eq!(debouncer.read(Level::High).unwrap(), Level::Low);
    }

    #[test]
    fn test_digital_signal_processor_trait_implementations() {
        // Test that debouncer implements the trait correctly
        let mut debouncer: Box<dyn DigitalSignalProcessor> = Box::new(
            DigitalSignalDebouncer::new(1, Duration::from_millis(1))
        );

        assert!(debouncer.read(Level::High).is_ok());
        assert!(debouncer.read(Level::Low).is_ok());
    }
}