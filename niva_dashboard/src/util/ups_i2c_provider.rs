//! Reads raw INA219 registers (current, bus voltage) from the Waveshare UPS HAT (D) over
//! I2C in a background thread, keeping the latest raw values available via UpsRawFrame.
//! Mirrors the ADCDataProvider/ADCFrame pattern in adc_data_provider.rs: this module only
//! produces raw hardware data — sign interpretation, mA/V scaling, and SoC estimation are
//! the logical sensor layer's job (see hardware::sensors::UpsCurrentSensor/UpsChargeSensor).

use rppal::i2c::I2c;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// I2C bus wired to the UPS HAT (D) pogo-pin header — physical pins 3/5 on a Pi 4.
const I2C_BUS: u8 = 1;

/// INA219 voltage/current monitor on the UPS HAT, in series with the battery.
const INA219_ADDR: u16 = 0x43;

// INA219 registers (see INA219 datasheet / Waveshare's INA219.py demo at
// /home/user/UPS_HAT_D/INA219.py).
const REG_CONFIG: u8 = 0x00;
const REG_BUSVOLTAGE: u8 = 0x02;
const REG_CURRENT: u8 = 0x04;
const REG_CALIBRATION: u8 = 0x05;

/// Calibration/config values ported from Waveshare's INA219.py `set_calibration_16V_5A`,
/// which assumes a 0.01 ohm shunt and measures up to 16V / 5A (counter overflow at 16A).
const CAL_VALUE: u16 = 26868;
/// BRNG=16V(0)<<13 | PGA=/2,80mV(1)<<11 | BADC=12bit,32samp(0x0D)<<7 |
/// SADC=12bit,32samp(0x0D)<<3 | MODE=shunt+bus continuous(0x07)
const CONFIG_VALUE: u16 = (0x00 << 13) | (0x01 << 11) | (0x0D << 7) | (0x0D << 3) | 0x07;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// A reading is treated as unavailable once it's older than this — i.e. the background
/// thread has stopped producing fresh values (I2C errors, or the thread died) — rather than
/// silently exposing a frozen last-known register value.
const READING_MAX_AGE: Duration = Duration::from_secs(5);

/// Cloneable, thread-safe handle to the UPS HAT's most recent raw INA219 registers.
/// Cheap to clone (Arc clone). Consumed by hw_providers::UPSDataProvider, mirroring how
/// ADCChannelProvider consumes ADCFrame.
#[derive(Clone)]
pub struct UpsRawFrame {
    current_raw: Arc<Mutex<u16>>,
    bus_voltage_raw: Arc<Mutex<u16>>,
    last_update: Arc<Mutex<Instant>>,
}

impl UpsRawFrame {
    fn new() -> Self {
        UpsRawFrame {
            current_raw: Arc::new(Mutex::new(0)),
            bus_voltage_raw: Arc::new(Mutex::new(0)),
            last_update: Arc::new(Mutex::new(Instant::now() - READING_MAX_AGE)),
        }
    }

    fn set(&self, current_raw: u16, bus_voltage_raw: u16) {
        *self.current_raw.lock().unwrap() = current_raw;
        *self.bus_voltage_raw.lock().unwrap() = bus_voltage_raw;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    /// Raw INA219 current register (two's complement, REG_CURRENT LSB units). Sign
    /// reinterpretation and mA scaling happen in UpsCurrentSensor::read.
    pub fn raw_current(&self) -> Result<u16, String> {
        if self.last_update.lock().unwrap().elapsed() > READING_MAX_AGE {
            return Err("no UPS data".to_string());
        }
        Ok(*self.current_raw.lock().unwrap())
    }

    /// Raw INA219 bus voltage register (top 13 bits, 4mV/step, before the >>3 shift).
    /// Voltage/SoC conversion happens in UpsChargeSensor::read.
    pub fn raw_bus_voltage(&self) -> Result<u16, String> {
        if self.last_update.lock().unwrap().elapsed() > READING_MAX_AGE {
            return Err("no UPS data".to_string());
        }
        Ok(*self.bus_voltage_raw.lock().unwrap())
    }
}

/// Errors that can occur when starting the UPS I2C data provider.
#[derive(Debug)]
pub enum UpsI2CProviderError {
    AlreadyStarted,
    SpawnFailed(std::io::Error),
}

impl std::fmt::Display for UpsI2CProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "UPS I2C data provider already started"),
            Self::SpawnFailed(err) => write!(f, "Failed to spawn thread: {}", err),
        }
    }
}

impl std::error::Error for UpsI2CProviderError {}

/// Owns the UPS HAT's INA219 I2C connection lifecycle in a background thread, polling raw
/// current/bus-voltage registers into a shared UpsRawFrame at POLL_INTERVAL.
pub struct UpsI2CDataProvider {
    should_stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    frame: UpsRawFrame,
}

impl UpsI2CDataProvider {
    pub fn new() -> Self {
        UpsI2CDataProvider {
            should_stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            frame: UpsRawFrame::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), UpsI2CProviderError> {
        if self.thread.is_some() {
            return Err(UpsI2CProviderError::AlreadyStarted);
        }

        let should_stop = Arc::clone(&self.should_stop);
        let frame = self.frame.clone();
        let handle = thread::Builder::new()
            .name("ups-i2c-provider".into())
            .spawn(move || Self::run_loop(&should_stop, &frame))
            .map_err(UpsI2CProviderError::SpawnFailed)?;
        self.thread = Some(handle);
        Ok(())
    }

    /// Returns a cloneable handle to the shared raw frame for use by hardware providers.
    pub fn frame(&self) -> UpsRawFrame {
        self.frame.clone()
    }

    fn run_loop(should_stop: &AtomicBool, frame: &UpsRawFrame) {
        let mut i2c = match I2c::with_bus(I2C_BUS) {
            Ok(i2c) => i2c,
            Err(e) => {
                log::error!("UPS I2C provider: failed to open I2C bus {}: {}", I2C_BUS, e);
                return;
            }
        };

        if let Err(e) = Self::calibrate_ina219(&mut i2c) {
            log::error!("UPS I2C provider: failed to calibrate INA219, giving up: {}", e);
            return;
        }

        while !should_stop.load(Ordering::Relaxed) {
            match Self::read_raw_registers(&mut i2c) {
                Ok((current_raw, bus_voltage_raw)) => frame.set(current_raw, bus_voltage_raw),
                Err(e) => log::warn!("UPS I2C provider: failed to read INA219 registers: {}", e),
            }

            Self::sleep_while_running(should_stop, POLL_INTERVAL);
        }
    }

    fn calibrate_ina219(i2c: &mut I2c) -> rppal::i2c::Result<()> {
        i2c.set_slave_address(INA219_ADDR)?;
        i2c.smbus_write_word_swapped(REG_CALIBRATION, CAL_VALUE)?;
        i2c.smbus_write_word_swapped(REG_CONFIG, CONFIG_VALUE)?;
        Ok(())
    }

    fn read_raw_registers(i2c: &mut I2c) -> rppal::i2c::Result<(u16, u16)> {
        i2c.set_slave_address(INA219_ADDR)?;
        let current_raw = i2c.smbus_read_word_swapped(REG_CURRENT)?;
        let bus_voltage_raw = i2c.smbus_read_word_swapped(REG_BUSVOLTAGE)?;
        Ok((current_raw, bus_voltage_raw))
    }

    fn sleep_while_running(should_stop: &AtomicBool, duration: Duration) {
        const POLL_INTERVAL: Duration = Duration::from_millis(100);
        let mut remaining = duration;
        while remaining > Duration::ZERO && !should_stop.load(Ordering::Relaxed) {
            let step = remaining.min(POLL_INTERVAL);
            thread::sleep(step);
            remaining -= step;
        }
    }

    pub fn stop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for UpsI2CDataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
