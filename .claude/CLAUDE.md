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
│   │   ├── sensor_config.rs        # Data-driven ADC chain loader (see sensor_config.json, TODO)
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

## OpenGL FFI char signedness
Raw `gl::` calls (`GetUniformLocation`, `GetAttribLocation`, `ShaderSource`, etc., mainly in `context.rs` and `opengl_test.rs`) pass byte-string pointers — `b"...\0".as_ptr()`, always `*const u8` — into parameters typed `*const i8` (`GLchar` aliases `c_char`). This only compiles because `c_char` is **unsigned** on ARM (the Pi target), so `u8`/`c_char` coincide there; on x86_64 `c_char` is signed and every one of these ~80 call sites fails with `E0308: mismatched types`. Not yet fixed at the source level — see CI below and TODO.

## CI
`.github/workflows/ci.yml` runs `cargo test` via GitHub Actions on every push to `master`.
- Runner is `ubuntu-24.04-arm`, not `ubuntu-latest` — ARM's ABI makes `c_char` unsigned like the Pi (see OpenGL FFI char signedness above), so the existing code compiles as-is; x86_64 runners fail on ~80 call sites. This also means CI tests the same architecture family the dashboard actually ships on.
- System deps installed via apt before build: `libegl1-mesa-dev`, `libgles2-mesa-dev`, `libdrm-dev`, `libgbm-dev`, `libfreetype6-dev`, `libudev-dev` (the last for `serialport`'s Linux backend, `libudev-sys`), `pkg-config`.
- No custom email step — failure notification relies on GitHub's own per-account setting (Settings → Notifications → Actions → "failed workflows only"), not repo config.
- `build.rs` reads `../.git/HEAD` relative to the crate dir, matching `actions/checkout`'s default layout (`.git` one level above `niva_dashboard/`) — no special checkout config needed.

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
Tracked in [GitHub Issues](https://github.com/yurec-orl/Niva_dashboard_rpi/issues).

## PiOS login
`user` / `@Niva21#`; `root` password is standard password with a single numeric character.
