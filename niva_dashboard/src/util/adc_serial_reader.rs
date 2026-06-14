use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use serialport::SerialPort;

pub trait SerialReader {
    fn read_line(&mut self) -> Option<String>;
}

pub struct ADCSerialReader {
    reader: BufReader<Box<dyn SerialPort>>,
}

impl ADCSerialReader {
    pub fn new(port: &str, baud: u32) -> Self {
        let port = serialport::new(port, baud)
            .timeout(Duration::from_millis(100))
            .open()
            .expect("Failed to open serial port");

        ADCSerialReader { reader: BufReader::new(port) }
    }
}

impl SerialReader for ADCSerialReader {
    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Some(String::new()),          // timeout — no data yet
            Ok(_) => {
                return Some(line.trim().to_string());
            }
            Err(e) => {
                print!("Read error: {}", e);
                return None;
            }
        }
    }
}
