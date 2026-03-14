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


pub struct DigitalSignalProcessorPulseCounter {
    pulse_count: u32,
    last_level: Level,
}

impl DigitalSignalProcessorPulseCounter {
    pub fn new() -> Self {
        DigitalSignalProcessorPulseCounter {
            pulse_count: 0,
            last_level: Level::Low,
        }
    }

    pub fn count(&self) -> u32 {
        self.pulse_count
    }

    pub fn reset(&mut self) {
        self.pulse_count = 0;
    }
}

impl DigitalSignalProcessor for DigitalSignalProcessorPulseCounter {
    fn read(&mut self, input: Level) -> Result<Level, String> {
        if input != self.last_level {
            self.pulse_count += 1;
            self.last_level = input;
        }
        Ok(input)
    }
}

pub struct DigitalSignalProcessorPulsePerSecond {
    counter: DigitalSignalProcessorPulseCounter,
    last_update: Instant,
    current_pps: f32,
    update_interval: Duration,
}

impl DigitalSignalProcessorPulsePerSecond {
    pub fn new() -> Self {
        Self::with_update_interval(Duration::from_millis(1000))
    }
    
    pub fn with_update_interval(update_interval: Duration) -> Self {
        DigitalSignalProcessorPulsePerSecond {
            counter: DigitalSignalProcessorPulseCounter::new(),
            last_update: Instant::now(),
            current_pps: 0.0,
            update_interval,
        }
    }

    pub fn pulses_per_second(&mut self) -> f32 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        
        // Only update the rate if enough time has passed
        if elapsed >= self.update_interval {
            let elapsed_secs = elapsed.as_secs_f32();
            if elapsed_secs > 0.0 {
                //let new_pps = self.counter.count() as f32 / elapsed_secs;
                //println!("PPS Debug: Elapsed: {:.3}s, Count: {}, Old PPS: {:.2}, New PPS: {:.2}", 
                //         elapsed_secs, self.counter.count(), self.current_pps, new_pps);
                //self.current_pps = new_pps;
                self.current_pps = self.counter.count() as f32 / elapsed_secs;
            }
            self.counter.reset();
            self.last_update = now;
        }
        
        // Always return the current calculated rate
        self.current_pps
    }
}

impl DigitalSignalProcessor for DigitalSignalProcessorPulsePerSecond {
    fn read(&mut self, input: Level) -> Result<Level, String> {
        self.counter.read(input)
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
    fn test_pulse_counter_creation() {
        let counter = DigitalSignalProcessorPulseCounter::new();
        
        assert_eq!(counter.count(), 0);
        assert_eq!(counter.last_level, Level::Low);
    }

    #[test]
    fn test_pulse_counter_basic_counting() {
        let mut counter = DigitalSignalProcessorPulseCounter::new();
        
        // Initial count should be 0
        assert_eq!(counter.count(), 0);
        
        // First transition: Low -> High
        let result = counter.read(Level::High).unwrap();
        assert_eq!(result, Level::High);
        assert_eq!(counter.count(), 1);
        
        // Same level - no increment
        let result = counter.read(Level::High).unwrap();
        assert_eq!(result, Level::High);
        assert_eq!(counter.count(), 1);
        
        // Second transition: High -> Low
        let result = counter.read(Level::Low).unwrap();
        assert_eq!(result, Level::Low);
        assert_eq!(counter.count(), 2);
        
        // Third transition: Low -> High
        let result = counter.read(Level::High).unwrap();
        assert_eq!(result, Level::High);
        assert_eq!(counter.count(), 3);
    }

    #[test]
    fn test_pulse_counter_reset() {
        let mut counter = DigitalSignalProcessorPulseCounter::new();
        
        // Generate some pulses
        counter.read(Level::High).unwrap();
        counter.read(Level::Low).unwrap();
        counter.read(Level::High).unwrap();
        assert_eq!(counter.count(), 3);
        
        // Reset counter
        counter.reset();
        assert_eq!(counter.count(), 0);
        
        // Counter should work normally after reset
        counter.read(Level::Low).unwrap();
        assert_eq!(counter.count(), 1);
    }

    #[test]
    fn test_pulse_counter_multiple_transitions() {
        let mut counter = DigitalSignalProcessorPulseCounter::new();
        
        let transitions = [
            Level::High, Level::Low, Level::High, Level::Low, 
            Level::High, Level::Low, Level::High, Level::High, // Double High should not increment
            Level::Low, Level::High
        ];
        
        for (i, &level) in transitions.iter().enumerate() {
            counter.read(level).unwrap();
            
            // Count expected transitions (excluding the double High)
            let expected_count = if i < 7 { i + 1 } else if i == 7 { 7 } else { i };
            assert_eq!(counter.count(), expected_count as u32);
        }
    }

    #[test]
    fn test_pulse_per_second_creation() {
        let pps = DigitalSignalProcessorPulsePerSecond::new();
        assert_eq!(pps.current_pps, 0.0);
        assert_eq!(pps.update_interval, Duration::from_millis(1000));
        
        let pps_custom = DigitalSignalProcessorPulsePerSecond::with_update_interval(Duration::from_millis(500));
        assert_eq!(pps_custom.current_pps, 0.0);
        assert_eq!(pps_custom.update_interval, Duration::from_millis(500));
    }

    #[test]
    fn test_pulse_per_second_initial_rate() {
        let mut pps = DigitalSignalProcessorPulsePerSecond::new();
        
        // Initial rate should be 0
        assert_eq!(pps.pulses_per_second(), 0.0);
    }

    #[test]
    fn test_pulse_per_second_rate_calculation() {
        let mut pps = DigitalSignalProcessorPulsePerSecond::with_update_interval(Duration::from_millis(100));
        
        // Generate some pulses
        for _ in 0..5 {
            pps.read(Level::High).unwrap();
            pps.read(Level::Low).unwrap();
        }
        
        // Wait for update interval
        thread::sleep(Duration::from_millis(110));
        
        // Check rate calculation (should be around 100 Hz for 10 transitions in 0.1s)
        let rate = pps.pulses_per_second();
        assert!(rate > 90.0 && rate < 110.0, "Rate was {}, expected between 90-110", rate);
    }

    #[test]
    fn test_pulse_per_second_counter_reset() {
        let mut pps = DigitalSignalProcessorPulsePerSecond::with_update_interval(Duration::from_millis(50));
        
        // Generate pulses
        for _ in 0..3 {
            pps.read(Level::High).unwrap();
            pps.read(Level::Low).unwrap();
        }
        
        // Wait for update and get rate
        thread::sleep(Duration::from_millis(60));
        let rate1 = pps.pulses_per_second();
        assert!(rate1 > 0.0);
        
        // Generate more pulses
        for _ in 0..2 {
            pps.read(Level::High).unwrap();
            pps.read(Level::Low).unwrap();
        }
        
        // Wait for another update
        thread::sleep(Duration::from_millis(60));
        let rate2 = pps.pulses_per_second();
        
        // Rates should be different and both positive
        assert!(rate2 > 0.0);
        assert_ne!(rate1, rate2);
    }

    #[test]
    fn test_pulse_per_second_no_pulses() {
        let mut pps = DigitalSignalProcessorPulsePerSecond::with_update_interval(Duration::from_millis(50));
        
        // Wait without generating pulses
        thread::sleep(Duration::from_millis(60));
        
        // Rate should be 0
        let rate = pps.pulses_per_second();
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_pulse_per_second_consistent_signal() {
        let mut pps = DigitalSignalProcessorPulsePerSecond::with_update_interval(Duration::from_millis(100));
        
        // Keep signal at the same level (no transitions)
        for _ in 0..10 {
            pps.read(Level::High).unwrap();
        }
        
        thread::sleep(Duration::from_millis(110));
        let rate = pps.pulses_per_second();
        
        // First reading creates 1 transition from initial Low to High
        // Subsequent readings at High don't create transitions
        // So we expect a small positive rate from the initial transition
        assert!(rate >= 0.0 && rate < 50.0, "Expected small rate from initial transition, got {}", rate);
    }

    #[test]
    fn test_digital_signal_processor_trait_implementations() {
        // Test that all processors implement the trait correctly
        let mut debouncer: Box<dyn DigitalSignalProcessor> = Box::new(
            DigitalSignalDebouncer::new(1, Duration::from_millis(1))
        );
        let mut counter: Box<dyn DigitalSignalProcessor> = Box::new(
            DigitalSignalProcessorPulseCounter::new()
        );
        let mut pps: Box<dyn DigitalSignalProcessor> = Box::new(
            DigitalSignalProcessorPulsePerSecond::new()
        );
        
        // All should handle Level::High input
        assert!(debouncer.read(Level::High).is_ok());
        assert!(counter.read(Level::High).is_ok());
        assert!(pps.read(Level::High).is_ok());
        
        // All should handle Level::Low input  
        assert!(debouncer.read(Level::Low).is_ok());
        assert!(counter.read(Level::Low).is_ok());
        assert!(pps.read(Level::Low).is_ok());
    }
}