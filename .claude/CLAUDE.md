# Niva Dashboard - Raspberry Pi Car Dashboard

## Response constraints
- Ask before generating demos or examples.
- Write brief, meaningful, descriptive comments only where the WHY isn't obvious from the code. Do not restate the code in comments. Do not reference design decisions or internal project docs.

## Project Overview
A software dashboard for automotive use, written in Rust, running on Raspberry Pi 4. Mimics a multi-functional display (MFD) as found in aircraft: central screen with configurable button rows on the left and right sides. On-screen text is in Russian, using military-style abbreviations and shortened words where applicable.

## Hardware Platform
- **Target Device**: Raspberry Pi 4
- **ADC Module**: STM32F103C8T6 (via USB Serial, exposed as `/dev/niva_adc`)
- **Graphics**: Raspberry Pi OpenGL ES/KMS/DRM
- **Input**: Physical buttons (2 rows, left and right sides) connected to ADC module input pins, state transferred via USB serial; Master warning indicator/button connected to GPIO
- **Display**: 800x480 central screen

### Power supply
```
Car 12V → TPS40057 (DC-DC, 9-35V in, 5V 5A out)
        → UPS HAT (battery-backed 5V supply)
        → Raspberry Pi 4
              ├── GPIO 5V header → Display (power only, video via HDMI)
              ├── GPIO 5V header → Cooling fan (optional)
              ├── USB port → STM32 ADC module
              └── USB port → UM982 GNSS Receiver
```
- Display/fan are powered via Pi GPIO 5V header (not USB) so they stay on the same power domain as the Pi (powered on UPS battery when ignition/TPS40057 is off) and to avoid the Pi's fixed USB current limit.
- **Pi 4's GPIO 5V header has no polyfuse** — direct unprotected connection to the input rail. Use an inline fuse on this tap; spread the combined draw across both 5V/GND pin pairs.
- UPS HAT's battery-boost converter (not the TPS40057) must cover the combined Pi+GPIO+USB load when running on battery with ignition off — verify its rated output covers this before relying on it.

## Software Architecture

### Core Components
1. **Page Manager System** — central UI framework managing pages (Main, Diagnostics, Oscilloscope); page-specific button labels, dynamic button function assignment, navigation/state, event-driven inter-page communication.
2. **Hardware Interface Layer** — sensor management via three-stage chains: Hardware Provider → Signal Processors → Logical Sensor.
   - Hardware Providers: GPIO, I2C, Test providers.
   - Digital signal processing: debouncing, edge detection, state smoothing.
   - Analog signal processing: moving averages, dampening, low-pass filtering.
   - `SensorDigitalInputChain` / `SensorAnalogInputChain`, managed by a central Sensor Manager.
3. **Visualization System** — bar indicators, digital segmented (7-segment) displays, gauge displays, text displays, alert/warning indicators.

### Technology Stack
- **Language**: Rust
- **Graphics**: OpenGL ES/KMS/DRM, freetype for text rendering
- **Hardware**: `rppal` (0.22.1) for GPIO
- Other deps: `drm` (0.11), `gl` (0.14), `freetype-sys` (0.13), `gbm-rs` (0.2), `crossbeam-channel` (0.5), `serde` (1.0)
- Build: Cargo with custom `build.rs` for native library linking

### Project Structure
```
niva_dashboard/
├── src/
│   ├── main.rs                     # Entry point with test modes
│   ├── page_framework/             # Page management system
│   │   ├── page_manager.rs         # Central page management
│   │   ├── main_page.rs            # Main dashboard page
│   │   ├── diag_page.rs            # Diagnostics page
│   │   ├── gnss_page.rs            # GNSS/nav page
│   │   ├── terminal_page.rs        # Log/ADC terminal page
│   │   ├── osc_page.rs             # Oscilloscope page (ADC capture waveform view)
│   │   ├── horz_page.rs            # Artificial horizon page (pitch/roll from BNO085)
│   │   ├── events.rs               # Event handling / message passing
│   │   └── input.rs                # Input processing / button handling
│   ├── hardware/
│   │   ├── hw_providers.rs         # HW abstraction (GPIO, I2C, Test providers)
│   │   ├── gpio_input.rs           # rppal-based GPIO wrapper (not wired to hw_providers traits, see TODO)
│   │   ├── sensor_manager.rs       # Sensor chain management
│   │   ├── sensor_value.rs
│   │   ├── digital_signal_processing.rs
│   │   ├── analog_signal_processing.rs
│   │   ├── sensors.rs              # Logical Sensor stage (live code, not legacy)
│   │   └── heading_fusion_sensor.rs  # BNO085 IMU + GNSS heading fusion
│   ├── indicators/                 # Indicator widgets
│   │   ├── indicator.rs            # Shared Indicator trait / IndicatorBase
│   │   ├── decorator.rs
│   │   ├── needle_indicator.rs
│   │   ├── needle_shape.rs
│   │   ├── gauge_indicator.rs
│   │   ├── vertical_bar_indicator.rs
│   │   ├── digital_segmented_indicator.rs
│   │   ├── compass_indicator.rs
│   │   └── text_indicator.rs
│   ├── indicator_builders/         # Per-signal indicator factory functions
│   │   ├── bar_builders/           # fuel level, oil pressure, temperature, voltage
│   │   ├── gauge_builders/         # fuel level, oil pressure, speedometer, temperature, voltage
│   │   └── digital_builders/       # speed
│   ├── alerts/
│   │   ├── alert.rs
│   │   ├── alert_manager.rs
│   │   └── watchdog.rs
│   ├── graphics/
│   │   ├── context.rs              # OpenGL context and text rendering
│   │   ├── ui_style.rs
│   │   ├── text_box.rs
│   │   ├── default_style.json      # dead, see TODO
│   │   └── opengl_test.rs
│   ├── util/
│   │   ├── adc_data_provider.rs    # STM32 ADC serial protocol
│   │   ├── bno085_data_provider.rs # BNO085 IMU serial protocol
│   │   ├── bno085_protocol.rs
│   │   ├── gnss_data_provider.rs   # UM982 GNSS serial protocol
│   │   ├── nmea.rs                 # NMEA sentence parsing
│   │   ├── serial_reader.rs
│   │   ├── ups_i2c_provider.rs
│   │   ├── ups_monitor.rs          # UPS HAT auto power on/off
│   │   ├── shutdown.rs
│   │   ├── diagnostics.rs
│   │   └── logging.rs
│   └── test/
│       └── run_test.rs             # Test execution framework
├── build.rs
├── run.sh
├── install-service.sh
└── splash.png
```

## Text Rendering Coordinate System
`render_text_with_font(x, y, text, font_size, color)` interprets `y` as the **top edge** of the text line (not baseline). For vertical centering: `center_y - (text_height / 2)`; text height ≈ `font_size * 1.2` if font metrics aren't available. Applies to all text rendering in the graphics context.

## Render Loop Performance
No manual frame timing/sleep/target-FPS constant — frame pacing is delegated entirely to KMS/DRM:
- `eglSwapInterval` has no effect (KMS/DRM path bypasses it).
- Timing is governed by `drmModePageFlip` with `DRM_MODE_PAGE_FLIP_EVENT`, consumed via `drmHandleEvent()` at the start of the next frame (`select()` with 50ms timeout).
- Steady 60 FPS matches the display's 60Hz refresh.
- For uncapped/benchmark rendering, `DRM_MODE_PAGE_FLIP_ASYNC` allows 120+ FPS but causes tearing.
- FPS < 60 means a frame missed a vblank (16.67ms), or `drmModePageFlip` returned `-EBUSY` from an undrained pending flip — both already handled.

**Rule:** Never call `glGenBuffers`/`glDeleteBuffers` inside a per-frame render function on the RPi V3D driver — `glDeleteBuffers` is deferred until the GPU finishes reading the buffer, and doing this every frame causes an unbounded growth in queued deletions (memory + CPU cost climb over time). Pre-allocate VBOs once at init (see `NEEDLE_VBO`/`MARKS_VBO` pattern using `Once` guards) and stream data via `glBufferData` with `GL_DYNAMIC_DRAW`.

## ADC Module Connectivity
- udev rule (`/etc/udev/rules.d/99-niva-adc.rules`) creates `/dev/niva_adc` symlink for the STM32 (vendor `0483`, product `5740`).
- **Never read a freshly-created serial/USB-CDC node with `cat`** before forcing raw mode — cooked-mode echo reflects received bytes back down the full-duplex link, and firmware that doesn't drain its RX buffer (like this STM32 firmware) can lock up. Force raw mode first: `stty -F /dev/niva_adc raw -echo -ixon -ixoff 115200`. The Rust app itself is unaffected — it opens the port via the `serialport` crate, which sets raw mode on open.
- If the STM32 firmware hangs, the dashboard recovers by power-cycling the whole USB hub (`2109:3431`, location `1-1`) via `uhubctl -l 1-1 -a 2` — cycling only the device's individual port was tested and found unreliable. Requires a narrow passwordless sudoers entry (`/etc/sudoers.d/niva-uhubctl`) for that exact command; any change to the invoked args must be mirrored there.

## GNSS Receiver Connectivity
- udev rule (`/etc/udev/rules.d/99-niva-gps.rules`) creates `/dev/niva_gps` symlink for the UM982's USB-serial adapter. It enumerates as a generic CH340 chip (vendor `1a86`, product `7523`), not as the UM982 itself — the rule matches on that. Unlike the STM32, the CH340 doesn't report a USB serial number, so the rule can't disambiguate by serial; this is fine only because it's the sole CH340 device on this fixed wiring (see the power supply diagram above).

## Logging
`src/util/logging.rs` uses `flexi_logger`, writing to `~/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs` and duplicating to stdout. Size-based rotation (5 MB, keep last 10). Each process start forces rotation so every run gets a fresh log file — requires one throwaway log write before `trigger_rotation()` since flexi_logger opens files lazily. Side effect: the startup marker line lands at the end of the *previous* run's rotated file, not the new one.

## Boot Time
Boot reduced from ~16.8s to ~5.1s by disabling unused systemd services (`NetworkManager-wait-online`, `e2scrub_reap`, `ModemManager`, `rpi-eeprom-update`, `bluetooth`, `hciuart` — see `/home/user/boot-optimizations.md`). `avahi-daemon` stays enabled for `.local` SSH access. These are OS-level `systemctl disable` calls, not part of this repo — a fresh SD flash needs them reapplied. Remaining ~18s gap is pre-kernel firmware/bootloader stage, invisible to OS tools; further profiling would need a `BOOT_UART=1` serial capture.

## TODO
- Data-driven sensor creation: JSON describing hardware inputs, sensor chains, logical sensor parameters.
- Improve sensor->watchdog->alert construction: currently, it is a multi-step process involving a lot of parameters, and it's easy to mismatch one of the parameters which can cause the alert to never trigger. Also benefits from data-driven
  sensor creation (see item above).
- [Done] UPS HAT integration (automatic startup/shutdown).
- [Rejected] Display power control (USB port shutdown during boot, re-enable when dashboard ready) - Pi 4 does
  not have individual USB port control, would require shutting down all USB devices.
- [Done] `OscPage` (`page_framework/osc_page.rs`) — wired in: declared in `mod.rs`, always registered in `PageManager::setup`, and functional (renders STM32 ADC oscilloscope capture waveforms).
- [Done] `UIEvent::Restart` — implemented as a system reboot (`sudo reboot`, `page_framework/page_manager.rs`), wired to the diag page's top-right button (`ПЕРЕЗАГР`).
- `I2CProvider` (`hardware/hw_providers.rs`) is a dead stub (`read_digital`/`read_analog` unconditionally return `Level::Low`/`0`), never instantiated outside its own unit tests.
- `EngineTemperatureSensor::read` (`hardware/sensors.rs`) uses a placeholder linear conversion (`input as f32 * 0.12`, comment "Example conversion") — needs real ADC-to-temperature calibration.
- Finalize ui style handling: `graphics/default_style.json` is dead: never loaded (only a commented-out call in `main.rs`), and its keys (`needle_color`, `gauge_minor_mark_count: 4`, etc.) don't match the constants actually used in `ui_style.rs` (`GAUGE_NEEDLE_COLOR`, `GAUGE_MINOR_MARK_COUNT: 37`, ...). Delete it or reconcile it with the real style schema. Saving/loading ui style json is not used as well.
- Default font paths hardcoded in `ui_style.rs::load_defaults()` are absolute and dev-machine-specific (`/home/user/Work/Niva_Dashboard_Rpi/...`) — will silently fail (falling back to warning-logged defaults) on any other deployment path.
- `GaugeIndicator::with_decorators` (`indicators/gauge_indicator.rs`) is a stub that ignores its argument ("decorators not yet integrated"), unlike `NeedleIndicator`/`VerticalBarIndicator`/`DigitalSegmentedIndicator` which all wire decorators through `IndicatorBase`.
- Doc/code mismatch: the digital/analog signal processing "edge detection"/"low-pass filtering" terms in Core Components don't correspond to any processor by that name (debounce and the EMA `AnalogSignalProcessorDampener` fill those roles under different names).
- [Done] 'Master warning' button/indicator to the system: non-latching button with a warning light which lights up when an alert is active, and button press clears active alerts. Wired directly to Pi GPIO because STM32 ran out of pins (and to
  have it still function if no link to ADC module). Power considerations: 16 mA draw per pin and <= 50 mA total GPIO draw. 16 mA should be fine for one LED, possibly even less if brightness is enough for a warning light.
  Pins chosen: **GPIO18 for the LED** (output; PWM0-capable, leaves room for hardware-PWM dimming later), **GPIO27 for the button** (pull-up input, active-low). Neither pin is used elsewhere in this codebase (existing GPIO use is I2C0 on GPIO2/3, shared with the UPS HAT, and the BNO085 HINT pin on GPIO17).
- [In progress] GNSS connectivity and indicators
- [In progress] HSI (horizontal situation indicator) styled indicator - heading, speed, altitude, manually set waypoints, etc.
- Nav page map mode - need to decide on which map data to use and how to render
- [Done] BNO085 connectivity and related indicators
- [Done] Out of memory protection: `earlyoom` installed and enabled (OS-level, not part of this repo — a fresh SD flash needs it reinstalled: `apt install earlyoom`). Kills the largest memory consumer before the kernel OOM killer lets the system thrash into unresponsiveness. Config in `/etc/default/earlyoom` avoids killing `sshd` (so remote recovery stays possible) but deliberately does *not* protect the dashboard binary — if it leaks and gets killed, the startup script restarts it fresh, which is the desired behavior.
- [Done] ADC firmware design flaw: STM32 used to report counted pulses since last data frame for HwSpeed/HwTacho, which gave low resolution and visible jitter (or, for HwTacho, couldn't latch "engine running" at all in the normal idle range) since expected counts/tick are rarely integers. Fixed by measuring inter-pulse period instead of counting, on both the STM32 firmware side (10us/unit wire encoding, 100_000 ticks/sec) and the Rust side (`hardware::sensors::SpeedSensor`/`TachoSensor`, `main.rs`'s analog chains, self-test simulation). See `SPEED_TACHO_PULSE_PERIOD_DESIGN.md` for the original analysis (100 km/h worked example, wire-format options, Rust-side conversion design).
- [Done] UPS auto power on/off on ignition: pull UPS power switch pin to GND when ignition is on (UPS ON position). Float UPS switch pin after timeout (~1 m 30 s) when ignition off (UPS OFF position). Timeout is large enough to allow Pi to shut down gracefully. See `UPS_AUTO_POWER_ON_OFF_DESIGN.md`.

## PiOS login
`user` / `@Niva21#`; `root` password is standard password with a single numeric character.
