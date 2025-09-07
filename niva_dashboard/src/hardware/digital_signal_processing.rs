use rppal::gpio::Level;
use std::time::{Duration, Instant};

use crate::hardware::hw_providers::{HWDigitalInput, HWDigitalProvider};

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
            self.stable_count += 1;
            
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