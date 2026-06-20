use crate::util::adc_serial_reader::ADCSerialReader;

use std::fmt;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

const MAX_DATA: usize = 32;           // Headroom if consumer lags

/// Errors that can occur when starting the ADC data provider.
#[derive(Debug)]
pub enum AdcDataProviderError {
    /// The provider was already started (cannot be run twice).
    AlreadyStarted,
}

impl fmt::Display for AdcDataProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "ADC data provider already started"),
        }
    }
}

impl std::error::Error for AdcDataProviderError {}

// Reads data from ADC module via USB serial in separate thread
pub struct ADCDataProvider {
    adc_reader: Option<ADCSerialReader>,
    should_stop: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<String>>>,    // Expected to hold 0-1 entries
    thread: Option<thread::JoinHandle<()>>,
}

impl ADCDataProvider {
    pub fn new(adc_reader: ADCSerialReader) -> Self {
        ADCDataProvider {
            adc_reader: Some(adc_reader),
            should_stop: Arc::new(AtomicBool::new(false)),
            data: Arc::new(Mutex::new(Vec::new())),
            thread: None,
        }
    }

    /// Starts the background thread to read ADC data.
    ///
    /// Returns `Err` if the provider has already been started.
    pub fn run(&mut self) -> Result<(), AdcDataProviderError> {
        if self.adc_reader.is_none() {
            return Err(AdcDataProviderError::AlreadyStarted);
        }

        let mut adc_reader = self.adc_reader.take().expect("checked above");
        let should_stop = Arc::clone(&self.should_stop);
        let data = Arc::clone(&self.data);

        self.thread = Some(std::thread::Builder::new()
            .name("adc-data-provider".into())
            .spawn(move || {
                while !should_stop.load(Ordering::Relaxed) {
                    let sample = adc_reader.read_line();

                    let mut _data = data.lock().unwrap();

                    if _data.len() < MAX_DATA {
                        if let Some(value) = sample {
                            print!("DEBUG: {}\r\n", value);
                            _data.push(value);
                        }
                    } else {
                        drop(_data); // Release lock before sleeping to avoid blocking consumer
                        print!("WARNING: ADC data buffer full\r\n");
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            })?)
        ;

        Ok(())
    }    

    pub fn stop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }

    pub fn get_data(&self) -> Vec<String> {
        std::mem::take(&mut self.data.lock().unwrap())
    }
}

impl Drop for ADCDataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join(); // Ignore panic for clean shutdown
        }
    }
}