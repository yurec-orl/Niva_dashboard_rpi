use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::sensors::UPS_ON_BATTERY_CURRENT_THRESHOLD_MA;

use rppal::i2c::I2c;

use std::process::Command;
use std::time::{Duration, Instant};

/// I2C bus wired to the UPS HAT (D) pogo-pin header — physical pins 3/5 on a Pi 4. Same
/// bus as UpsI2CDataProvider's INA219 polling, but the MCU is a distinct I2C address and
/// this connection is opened fresh only for the rare arm/shutdown sequence, not held open
/// continuously — see arm_boot_on_power.
const I2C_BUS: u8 = 1;
/// The UPS HAT's own MCU, which owns the "boot when power applied" latch.
const MCU_ADDR: u16 = 0x2d;
/// MCU register that arms the "boot when power applied" latch (see PROJECT_CONTEXT.md /
/// Waveshare UPS HAT (D) wiki, "Boot When Power Applied"). Writing MCU_ARM_VALUE here makes
/// the MCU start watching its charging port 30s later, and pull the Pi's GPIO3/SCL pin low
/// to boot it once power is detected. The wiki notes the Pi must power off promptly after
/// this write or the arm doesn't take, so it's issued immediately before `sudo poweroff`.
const MCU_REG_ARM: u8 = 0x01;
const MCU_ARM_VALUE: u8 = 0x55;

/// How long the battery-discharge condition must hold continuously before a shutdown is
/// triggered.
const ON_BATTERY_SHUTDOWN_DELAY: Duration = Duration::from_secs(10);

/// How long the battery-discharge condition must hold continuously before a message is
/// written to log.
const ON_BATTERY_LOG_DELAY: Duration = Duration::from_secs(1);

/// Monitors the UPS current sensor chain (HwUPSCurrent, converted by
/// hardware::sensors::UpsCurrentSensor) and shuts the system down after it has read
/// "on battery" continuously for ON_BATTERY_SHUTDOWN_DELAY, arming the HAT's own "boot when
/// power applied" latch first so the Pi restarts on its own once mains power returns (see
/// PROJECT_CONTEXT.md, "UPS module connectivity").
///
/// `check` is driven from PageManager's event loop tick, the same place watchdogs are
/// checked — this struct owns no thread of its own. Detection is based on the sensor
/// chain's converted current reading rather than a voltage heuristic: current sign directly
/// distinguishes "battery discharging" (mains lost) from "battery charging"/float (mains
/// present), regardless of how full the battery currently is.
///
/// Set NIVA_UPS_DRY_RUN (any value) to log the shutdown decision instead of calling
/// `poweroff`, for validating detection/timing on the bench before trusting the real
/// power-off path.
pub struct UpsMonitor {
    dry_run: bool,
    on_battery_since: Option<Instant>,
    shutdown_triggered: bool,
    battery_power_logged: bool,
}

impl UpsMonitor {
    pub fn new() -> Self {
        let dry_run = std::env::var("NIVA_UPS_DRY_RUN").is_ok();
        if dry_run {
            log::warn!("UPS monitor: NIVA_UPS_DRY_RUN set — shutdown will be logged, not executed");
        }
        UpsMonitor {
            dry_run,
            on_battery_since: None,
            shutdown_triggered: false,
            battery_power_logged: false,
        }
    }

    /// Checks the latest HwUPSCurrent sensor value and advances the on-battery timer,
    /// triggering a shutdown once it has held continuously for ON_BATTERY_SHUTDOWN_DELAY.
    /// Called once per event loop tick; a missing/stale reading is treated as "unknown",
    /// not "on battery" — a transient sensor error shouldn't be able to trigger a shutdown.
    pub fn check(&mut self, sensor_manager: &SensorManager) {
        let Some(current_ma) = sensor_manager.get_sensor_value(&HWInput::HwUPSCurrent).map(|v| v.as_f32()) else {
            return;
        };

        if current_ma < UPS_ON_BATTERY_CURRENT_THRESHOLD_MA {
            let since = *self.on_battery_since.get_or_insert_with(Instant::now);
            let elapsed = since.elapsed();

            if !self.shutdown_triggered && elapsed >= ON_BATTERY_SHUTDOWN_DELAY {
                log::warn!(
                    "UPS monitor: on battery power for {:?} (current {:.1} mA), shutting down",
                    elapsed, current_ma
                );
                self.trigger_shutdown();
                self.shutdown_triggered = true;
            } else if !self.battery_power_logged && elapsed >= ON_BATTERY_LOG_DELAY {
                log::warn!("UPS monitor: on battery power (current {:.1} mA)", current_ma);
                self.battery_power_logged = true;
            }
        } else {
            if self.on_battery_since.is_some() {
                log::info!("UPS monitor: mains power restored ({:.1} mA)", current_ma);
            }
            self.on_battery_since = None;
            self.battery_power_logged = false;
        }
    }

    /// Arms the MCU's boot-on-power latch, then powers the system off (or logs the intent,
    /// in dry-run mode).
    fn trigger_shutdown(&self) {
        if let Err(e) = Self::arm_boot_on_power() {
            // Still proceed to shut down — losing mains power is the more urgent risk than
            // losing the auto-boot convenience for this one outage.
            log::error!("UPS monitor: failed to arm boot-on-power latch: {}", e);
        }

        if self.dry_run {
            log::warn!("UPS monitor: dry run, skipping actual poweroff");
            return;
        }

        match Command::new("sudo").arg("poweroff").status() {
            Ok(status) if status.success() => log::info!("UPS monitor: poweroff issued"),
            Ok(status) => log::error!("UPS monitor: poweroff exited with {}", status),
            Err(e) => log::error!("UPS monitor: failed to spawn poweroff: {}", e),
        }
    }

    /// Opens a short-lived I2C connection to arm the latch — this is a one-shot, end-of-life
    /// write, so it doesn't need to share UpsI2CDataProvider's continuously-held connection.
    fn arm_boot_on_power() -> rppal::i2c::Result<()> {
        let mut i2c = I2c::with_bus(I2C_BUS)?;
        i2c.set_slave_address(MCU_ADDR)?;
        i2c.smbus_write_byte(MCU_REG_ARM, MCU_ARM_VALUE)
    }
}
