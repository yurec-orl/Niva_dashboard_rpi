use rppal::i2c::I2c;

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// I2C bus wired to the UPS HAT (D) pogo-pin header — physical pins 3/5 on a Pi 4.
const I2C_BUS: u8 = 1;

/// INA219 voltage/current monitor on the UPS HAT, in series with the battery.
const INA219_ADDR: u16 = 0x43;
/// The UPS HAT's own MCU, which owns the "boot when power applied" latch.
const MCU_ADDR: u16 = 0x2d;

// INA219 registers (see INA219 datasheet / Waveshare's INA219.py demo at
// /home/user/UPS_HAT_D/INA219.py).
const REG_CONFIG: u8 = 0x00;
const REG_CURRENT: u8 = 0x04;
const REG_CALIBRATION: u8 = 0x05;

/// MCU register that arms the "boot when power applied" latch (see PROJECT_CONTEXT.md /
/// Waveshare UPS HAT (D) wiki, "Boot When Power Applied"). Writing MCU_ARM_VALUE here makes
/// the MCU start watching its charging port 30s later, and pull the Pi's GPIO3/SCL pin low
/// to boot it once power is detected. The wiki notes the Pi must power off promptly after
/// this write or the arm doesn't take, so it's issued immediately before `sudo poweroff`.
const MCU_REG_ARM: u8 = 0x01;
const MCU_ARM_VALUE: u8 = 0x55;

/// Calibration/config values ported from Waveshare's INA219.py `set_calibration_16V_5A`,
/// which assumes a 0.01 ohm shunt and measures up to 16V / 5A (counter overflow at 16A).
/// CURRENT_LSB_MA is the resulting mA-per-bit scale for the raw current register.
const CAL_VALUE: u16 = 26868;
const CURRENT_LSB_MA: f64 = 0.1524;
/// BRNG=16V(0)<<13 | PGA=/2,80mV(1)<<11 | BADC=12bit,32samp(0x0D)<<7 |
/// SADC=12bit,32samp(0x0D)<<3 | MODE=shunt+bus continuous(0x07)
const CONFIG_VALUE: u16 = (0x00 << 13) | (0x01 << 11) | (0x0D << 7) | (0x0D << 3) | 0x07;

/// Current draw below this (mA) counts as "discharging" (running on battery, mains
/// absent or insufficient) — see UpsMonitor doc comment. Comfortably under a Pi 4's idle
/// draw so routine float-charging near 0 mA is never misread as on-battery.
const ON_BATTERY_CURRENT_THRESHOLD_MA: f64 = -50.0;

/// How long the battery-discharge condition must hold continuously before a shutdown is
/// triggered.
const ON_BATTERY_SHUTDOWN_DELAY: Duration = Duration::from_secs(60);

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// A reading is shown as unavailable ("–") once it's older than this — i.e. the background
/// thread has stopped producing fresh values (I2C errors, or the thread died) — rather than
/// silently displaying a frozen last-known number.
const READING_MAX_AGE: Duration = Duration::from_secs(5);

/// Cloneable, thread-safe handle to the UPS monitor's most recent current reading, for
/// display elsewhere (e.g. the dashboard's status line). Cheap to clone (Arc clone).
/// Mirrors the ADCFrame handle pattern used by ADCDataProvider.
#[derive(Clone)]
pub struct UpsReading {
    current_ma: Arc<Mutex<f64>>,
    last_update: Arc<Mutex<Instant>>,
}

impl UpsReading {
    fn new() -> Self {
        UpsReading {
            current_ma: Arc::new(Mutex::new(0.0)),
            last_update: Arc::new(Mutex::new(Instant::now() - READING_MAX_AGE)),
        }
    }

    fn set(&self, current_ma: f64) {
        *self.current_ma.lock().unwrap() = current_ma;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    /// Most recent INA219 current reading in mA — positive means charging/mains present,
    /// negative means discharging/on battery (see UpsMonitor doc comment) — or `None` if no
    /// reading has succeeded within READING_MAX_AGE.
    pub fn current_ma(&self) -> Option<f64> {
        if self.last_update.lock().unwrap().elapsed() > READING_MAX_AGE {
            return None;
        }
        Some(*self.current_ma.lock().unwrap())
    }
}

/// Monitors the Waveshare UPS HAT (D) over I2C and shuts the system down after it has run
/// on battery power continuously for ON_BATTERY_SHUTDOWN_DELAY, arming the HAT's own "boot
/// when power applied" latch first so the Pi restarts on its own once mains power returns
/// (see PROJECT_CONTEXT.md, "UPS module connectivity").
///
/// Detection is based on the INA219's current reading rather than the demo script's
/// low-voltage heuristic: current sign directly distinguishes "battery discharging" (mains
/// lost) from "battery charging"/float (mains present), regardless of how full the battery
/// currently is.
///
/// Set NIVA_UPS_DRY_RUN (any value) to log the shutdown decision instead of calling
/// `poweroff`, for validating detection/timing on the bench before trusting the real
/// power-off path.
pub struct UpsMonitor {
    should_stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    dry_run: bool,
    reading: UpsReading,
}

impl UpsMonitor {
    pub fn new() -> Self {
        let dry_run = std::env::var("NIVA_UPS_DRY_RUN").is_ok();
        if dry_run {
            log::warn!("UPS monitor: NIVA_UPS_DRY_RUN set — shutdown will be logged, not executed");
        }
        UpsMonitor {
            should_stop: Arc::new(AtomicBool::new(false)),
            thread: None,
            dry_run,
            reading: UpsReading::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        if self.thread.is_some() {
            return Err("UPS monitor already started".to_string());
        }

        let should_stop = Arc::clone(&self.should_stop);
        let dry_run = self.dry_run;
        let reading = self.reading.clone();
        let handle = thread::Builder::new()
            .name("ups-monitor".into())
            .spawn(move || Self::run_loop(&should_stop, dry_run, &reading))
            .map_err(|e| format!("failed to spawn UPS monitor thread: {}", e))?;
        self.thread = Some(handle);
        Ok(())
    }

    /// Returns a cloneable handle to the most recent current reading, for display elsewhere
    /// (e.g. PageManager's status line). Safe to call before or after `run()`.
    pub fn reading(&self) -> UpsReading {
        self.reading.clone()
    }

    /// Background thread body: calibrates the INA219 once, then polls its current reading
    /// until should_stop is set, triggering a shutdown after ON_BATTERY_SHUTDOWN_DELAY of
    /// continuous discharge.
    fn run_loop(should_stop: &AtomicBool, dry_run: bool, reading: &UpsReading) {
        let mut i2c = match I2c::with_bus(I2C_BUS) {
            Ok(i2c) => i2c,
            Err(e) => {
                log::error!("UPS monitor: failed to open I2C bus {}: {}", I2C_BUS, e);
                return;
            }
        };

        if let Err(e) = Self::calibrate_ina219(&mut i2c) {
            log::error!("UPS monitor: failed to calibrate INA219, giving up: {}", e);
            return;
        }

        let mut on_battery_since: Option<Instant> = None;
        let mut shutdown_triggered = false;

        while !should_stop.load(Ordering::Relaxed) {
            match Self::read_current_ma(&mut i2c) {
                Ok(current_ma) => {
                    reading.set(current_ma);
                    if current_ma < ON_BATTERY_CURRENT_THRESHOLD_MA {
                        let since = *on_battery_since.get_or_insert_with(Instant::now);
                        let elapsed = since.elapsed();
                        if !shutdown_triggered && elapsed >= ON_BATTERY_SHUTDOWN_DELAY {
                            log::warn!(
                                "UPS monitor: on battery power for {:?} (current {:.1} mA), shutting down",
                                elapsed, current_ma
                            );
                            Self::trigger_shutdown(&mut i2c, dry_run);
                            shutdown_triggered = true;
                        }
                    } else {
                        if on_battery_since.is_some() {
                            log::info!("UPS monitor: mains power restored ({:.1} mA)", current_ma);
                        }
                        on_battery_since = None;
                    }
                }
                Err(e) => {
                    // A transient I2C read error shouldn't reset an in-progress on-battery
                    // timer (it isn't evidence power came back), nor should it be able to
                    // trigger a shutdown on its own — so just skip this iteration.
                    log::warn!("UPS monitor: failed to read INA219 current: {}", e);
                }
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

    fn read_current_ma(i2c: &mut I2c) -> rppal::i2c::Result<f64> {
        i2c.set_slave_address(INA219_ADDR)?;
        let raw = i2c.smbus_read_word_swapped(REG_CURRENT)?;
        // Sign conversion matches Waveshare's INA219.py getCurrent_mA exactly.
        let signed = if raw > 32767 { raw as i32 - 65535 } else { raw as i32 };
        // INA219.py negates this raw register reading before interpreting its sign
        // (`current = -ina219.getCurrent_mA()`) — confirmed empirically: on mains power
        // the raw register reads negative (~-1.2A) while the negated, user-facing value
        // reads positive (~+1.2A), consistent with the script's own comment that positive
        // means charging/mains-present and negative means discharging/on-battery. Negate
        // here so ON_BATTERY_CURRENT_THRESHOLD_MA's sign convention matches that.
        Ok(-(signed as f64 * CURRENT_LSB_MA))
    }

    /// Arms the MCU's boot-on-power latch, then powers the system off (or logs the intent,
    /// in dry-run mode).
    fn trigger_shutdown(i2c: &mut I2c, dry_run: bool) {
        if let Err(e) = Self::arm_boot_on_power(i2c) {
            // Still proceed to shut down — losing mains power is the more urgent risk than
            // losing the auto-boot convenience for this one outage.
            log::error!("UPS monitor: failed to arm boot-on-power latch: {}", e);
        }

        if dry_run {
            log::warn!("UPS monitor: dry run, skipping actual poweroff");
            return;
        }

        match Command::new("sudo").arg("poweroff").status() {
            Ok(status) if status.success() => log::info!("UPS monitor: poweroff issued"),
            Ok(status) => log::error!("UPS monitor: poweroff exited with {}", status),
            Err(e) => log::error!("UPS monitor: failed to spawn poweroff: {}", e),
        }
    }

    fn arm_boot_on_power(i2c: &mut I2c) -> rppal::i2c::Result<()> {
        i2c.set_slave_address(MCU_ADDR)?;
        i2c.smbus_write_byte(MCU_REG_ARM, MCU_ARM_VALUE)
    }

    /// Sleeps for `duration`, checking `should_stop` in short increments so a stop request
    /// is picked up promptly instead of blocking for the full interval.
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

impl Drop for UpsMonitor {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
