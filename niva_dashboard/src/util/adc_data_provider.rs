use crate::util::adc_serial_reader::{ADCSerialReader, SerialReader};
use crate::hardware::hw_providers::*;

use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_DATA: usize = 32;           // Headroom if consumer lags

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

    pub fn run(&mut self) {
        let mut adc_reader = self.adc_reader.take().expect("already started");
        let should_stop = Arc::clone(&self.should_stop);
        let data = Arc::clone(&self.data);

        self.thread = Some(std::thread::spawn(move || {
            while !should_stop.load(Ordering::Relaxed) {
                let sample = adc_reader.read_line();

                let mut _data = data.lock().unwrap();

                if _data.len() < MAX_DATA {
                    if let Some(value) = sample {
                        print!("DEBUG: {}\r\n", value);
                        _data.push(value);
                    }
                } else {
                    print!("WARNING: ADC data buffer full\r\n");
                    thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }));
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
        self.thread.take().unwrap().join().expect("ADC data provider thread panicked");
    }
}