use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::time::Duration;
use serialport::SerialPort;

pub trait SerialReader {
    fn read_line(&mut self) -> Option<String>;
}

pub struct LineSerialReader {
    reader: BufReader<Box<dyn SerialPort>>,
}

impl LineSerialReader {
    pub fn try_new(port: &str, baud: u32) -> Result<Self, String> {
        let opened = match serialport::new(port, baud)
            .timeout(Duration::from_millis(100))
            .open() {
                Ok(p) => p,
                Err(e) => {
                    let msg = format!("Error opening serial port '{}': {}", port, e);
                    // Logged at debug: callers that retry (e.g. ADCDataProvider's reconnect
                    // loop) own user-facing, throttled logging so a missing/unplugged port
                    // doesn't spam the log every retry.
                    log::debug!("{}", msg);
                    return Err(msg);
                }
            };

        log::info!("Opened serial port '{}' at {} baud", port, baud);
        Ok(LineSerialReader { reader: BufReader::new(opened) })
    }

    /// Writes raw bytes to the port (e.g. an `$OSCCAP\n` command line). The reader wraps the
    /// port in a `BufReader`, which only buffers reads — writes go straight through.
    pub fn write_line(&mut self, s: &str) -> Result<(), String> {
        self.reader.get_mut().write_all(s.as_bytes())
            .map_err(|e| format!("serial write failed: {}", e))
    }
}

impl SerialReader for LineSerialReader {
    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Some(String::new()),          // true EOF (rare for a serial port)
            Ok(_) => {
                return Some(line.trim().to_string());
            }
            // A read timeout is routine (no data within the port's configured timeout) —
            // it is NOT an error condition and must not be treated as a fatal read failure.
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Some(String::new()),
            Err(e) => {
                log::error!("Serial read error: {} (kind: {:?})", e, e.kind());
                return None;
            }
        }
    }
}
