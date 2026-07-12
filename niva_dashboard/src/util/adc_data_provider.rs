use crate::util::adc_serial_reader::{ADCSerialReader, SerialReader};

use std::fmt;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Errors that can occur when starting the ADC data provider.
#[derive(Debug)]
pub enum AdcDataProviderError {
    AlreadyStarted,
    SpawnFailed(std::io::Error),
}

impl fmt::Display for AdcDataProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "ADC data provider already started"),
            Self::SpawnFailed(err) => write!(f, "Failed to spawn thread: {}", err),
        }
    }
}

impl std::error::Error for AdcDataProviderError {}

/// A cloneable, thread-safe handle to the shared ADC frame.
/// Hardware providers hold this instead of the full ADCDataProvider so that
/// Arc<ADCFrame> does not drag in the non-Sync serial port fields.
#[derive(Clone)]
pub struct ADCFrame {
    data: Arc<Mutex<Vec<u16>>>,
    last_update: Arc<Mutex<Instant>>,
}

impl ADCFrame {
    fn new() -> Self {
        ADCFrame {
            data: Arc::new(Mutex::new(Vec::new())),
            last_update: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn get_channel(&self, index: usize) -> Result<u16, String> {
        self.data.lock().unwrap()
            .get(index)
            .copied()
            .ok_or_else(|| format!("ADC channel {} not in frame", index))
    }

    pub fn get_data(&self) -> Vec<u16> {
        self.data.lock().unwrap().clone()
    }

    /// Time elapsed since the last successfully parsed frame from the STM32 module.
    /// Used to detect a stalled/disconnected ADC link (see AdcLinkStatusProvider).
    pub fn last_update_age(&self) -> Duration {
        self.last_update.lock().unwrap().elapsed()
    }
}

/// Reads comma-separated ADC values from the serial port in a background thread,
/// keeping the latest frame available for reads by hardware providers via ADCFrame.
///
/// The background thread continuously overwrites the frame with each new parsed CSV line —
/// get_data and get_channel always return the most recent sample without consuming it.
pub struct ADCDataProvider {
    adc_reader: Option<ADCSerialReader>,
    should_stop: Arc<AtomicBool>,
    frame: ADCFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl ADCDataProvider {
    pub fn new(adc_reader: ADCSerialReader) -> Self {
        ADCDataProvider {
            adc_reader: Some(adc_reader),
            should_stop: Arc::new(AtomicBool::new(false)),
            frame: ADCFrame::new(),
            thread: None,
        }
    }

    pub fn run(&mut self) -> Result<(), AdcDataProviderError> {
        if self.thread.is_some() {
            return Err(AdcDataProviderError::AlreadyStarted);
        }

        let mut adc_reader = self.adc_reader.take().expect("ADC reader should be available");
        let should_stop = Arc::clone(&self.should_stop);
        let frame = self.frame.clone();

        match std::thread::Builder::new()
            .name("adc-data-provider".into())
            .spawn(move || {
                while !should_stop.load(Ordering::Relaxed) {
                    match adc_reader.read_line() {
                        Some(line) if !line.is_empty() => {
                            // Strip leading '$' frame marker before parsing channel values
                            let values: Vec<u16> = line
                                .trim_start_matches('$')
                                .split(',')
                                .filter_map(|s| s.trim().parse().ok())
                                .collect();
                            if !values.is_empty() {
                                *frame.data.lock().unwrap() = values;
                                *frame.last_update.lock().unwrap() = Instant::now();
                            }
                        }
                        None => break,  // Serial read error — shut down thread
                        _ => {}         // Empty line (timeout) — keep polling
                    }
                }
            }) {
            Ok(handle) => self.thread = Some(handle),
            Err(e) => return Err(AdcDataProviderError::SpawnFailed(e)),
        }

        Ok(())
    }

    pub fn stop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }

    /// Returns a cloneable handle to the shared frame for use by hardware providers.
    pub fn frame(&self) -> ADCFrame {
        self.frame.clone()
    }
}

impl Drop for ADCDataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
