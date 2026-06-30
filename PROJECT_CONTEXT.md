# Niva Dashboard - Raspberry Pi Car Dashboard

## General constraints
When answering the question, ask before generating demos or examples first.
Write meaningful, descriptive comments where appropriate. Avoid obvious comments which repeat the code, like:
// Add new page
self.add_new_page()

## Project Overview
A software dashboard for automotive use, written in Rust and designed to run on Raspberry Pi 4. The system mimics a multi-functional display (MFD) commonly found in aircraft, featuring a central screen with one row of configurable buttons on the left and right sides.
On-screen text is in Russian, with a military-style abbreviations and shortened words where applicable.

## Hardware Platform
- **Target Device**: Raspberry Pi 4
- **ADC Module**: STM32F103C8T6 (via USB Serial)
- **Graphics**: Raspberry Pi OpenGL library
- **Input**: GPIO-connected physical buttons (2 rows, left and right sides)
- **Display**: Central screen for data visualization, 800x480 pixels.

## Physical Layout
```
[BTN]           [BTN]
[BTN]  SCREEN   [BTN]
[BTN]           [BTN]
[BTN]           [BTN]
```

## Power Supply Layout
```
Car 12V
  └── XWST XW-0945-5-40W-ISO (DC-DC, 9-45V in, 5V 8A out, isolated)
        └── UPS HAT (battery-backed 5V supply)
              └── Raspberry Pi 4
                    ├── USB port → Display (power only, video via HDMI)
                    └── USB port → STM32 ADC module
```
- **XWST** is the primary power source, handles automotive voltage transients and spikes (up to 45V)
- **UPS HAT** provides battery backup and clean 5V to the Pi
- **Display** is powered via Pi USB port (software-controlled via `uhubctl`)
- **STM32 ADC module** is powered via Pi USB port

## Software Architecture

### Core Components

#### 1. Page Manager System
- **Purpose**: Central UI framework managing different functional pages (Main, Diagnostics, Oscilloscope)
- **Features**:
  - Page-specific button label configuration
  - Dynamic button function assignment
  - Page navigation and state management
  - Context-sensitive UI layouts
  - Event-driven communication between pages

#### 2. Hardware Interface Layer
- **Sensor Management System**:
  - **Three-stage processing chains**: Hardware Provider → Signal Processors → Logical Sensor
  - **Hardware Providers**: Abstract hardware interface layer (GPIO, I2C, Test providers)
  - **Digital Signal Processing**: Debouncing, edge detection, state smoothing with configurable processors
  - **Analog Signal Processing**: Moving averages, dampening, low-pass filtering, noise reduction
  - **Sensor Chains**: `SensorDigitalInputChain` and `SensorAnalogInputChain` for different data types
  - **Sensor Manager**: Central management of sensor chains with routing and processing pipeline execution
- **GPIO Button Handler**:
  - Physical button press detection
  - Advanced debouncing algorithms
  - Button state management

#### 3. Visualization System
- **Data Presentation Classes**:
  - Bar indicators (progress bars, level meters)
  - Digital segmented displays (7-segment style)
  - Gauge displays (analog-style meters)
  - Text displays
  - Alert/warning indicators

### Technology Stack
- **Language**: Rust
- **Graphics**: Raspberry Pi OpenGL ES/KMS/DRM, freetype library for text rendering
- **Hardware Interface**: rppal GPIO libraries 
- **Build System**: Cargo with custom build script for native library linking
- **Key Dependencies**: 
  - `rppal` (0.22.1) - Raspberry Pi peripheral access
  - `drm` (0.11) - Direct Rendering Manager
  - `gl` (0.14) - OpenGL bindings
  - `freetype-sys` (0.13) - Font rendering
  - `gbm-rs` (0.2) - Graphics Buffer Manager
  - `crossbeam-channel` (0.5) - Multi-producer, multi-consumer channels
  - `serde` (1.0) - Serialization framework

## Project Structure
```
niva_dashboard/
├── src/
│   ├── main.rs                 # Application entry point with test modes
│   ├── page_framework/         # Page management system
│   │   ├── page_manager.rs     # Central page management system
│   │   ├── main_page.rs        # Main dashboard page implementation
│   │   ├── diag_page.rs        # Diagnostics page
│   │   ├── osc_page.rs         # Oscilloscope page for signal visualization
│   │   ├── events.rs           # Event handling system with message passing
│   │   └── input.rs            # Input processing and button handling
│   ├── hardware/               # Hardware interface and sensor management
│   │   ├── hw_providers.rs     # Hardware abstraction layer (GPIO, I2C, Test providers)
│   │   ├── sensor_manager.rs   # Sensor chain management system
│   │   ├── digital_signal_processing.rs  # Digital signal processing (debouncing, pulse counting)
│   │   ├── analog_signal_processing.rs   # Analog signal processing (moving averages, dampening)
│   │   ├── gpio_input.rs       # GPIO button handling and debouncing
│   │   └── sensors.rs          # Legacy sensor definitions (being refactored)
│   ├── graphics/               # Graphics rendering system
│   │   ├── context.rs          # OpenGL graphics context and text rendering
│   │   ├── ui_style.rs         # UI styling and color definitions
│   │   ├── default_style.json  # Default UI style configuration
│   │   └── opengl_test.rs      # OpenGL testing utilities
│   └── test/                   # Testing utilities
│       └── run_test.rs         # Test execution framework with multiple test modes
├── build.rs                    # Build script for native library linking
├── run.sh                      # Execution script
└── splash.png                  # Dashboard splash screen
```

## Development Goals
1. Create a robust page management system for different dashboard views
2. Implement reliable GPIO button handling with proper debouncing
3. Develop flexible visualization components for various data types
4. Ensure smooth real-time performance on Raspberry Pi 4
5. Design intuitive navigation similar to aircraft MFD systems
6. Enhance text renderer to support multiple fonts
7. Implement comprehensive sensor management with configurable signal processing chains
8. Add oscilloscope functionality for real-time signal analysis
9. Build a modular event-driven architecture for page communication

## Target Use Cases
- Engine monitoring (RPM, temperature, pressure)
- Vehicle diagnostics and alerts  
- Navigation and trip information
- System configuration and settings
- Real-time sensor data visualization
- Signal analysis and oscilloscope functionality
- Multi-page dashboard navigation with physical buttons

## Sensor Specifications

### Analog Sensors
- **Engine Temperature Sensor**: 
  - Operational range: 5-100°C (engine temp sensor operating range)
  - Dashboard range: 0-120°C (extended range for diagnostics)
  - Purpose: Engine coolant temperature monitoring
  
- **12V System Voltage**:
  - Normal range: 12-14.4V (healthy electrical system)
  - Dashboard range: 0-20V (full diagnostic capability)
  - Purpose: Electrical system health monitoring, can detect battery drain (0V) or regulator failure (>16V)
  
- **Oil Pressure Sensor**:
  - Range: 0-8 kgf/cm² (kilogram-force per square centimeter)
  - Purpose: Engine lubrication system monitoring
  - Critical threshold: <1 kgf/cm² at idle indicates potential engine damage
  
- **Fuel Level Sensor**:
  - Range: 0-100% (percentage of tank capacity)
  - Purpose: Fuel quantity monitoring

### Digital Sensors
- **Speed and Tachometer**: Pulse-based sensors (active high)
- **Warning Indicators**: Active-low sensors for brake fluid, oil pressure, fuel level warnings
- **Status Indicators**: Active-low sensors for lights, charging system, parking brake, differential lock

## Current Test Modes
The application supports multiple test modes for development and validation:
1. Basic OpenGL triangle test
2. OpenGL text rendering test with FreeType
3. Dashboard performance test (9 animated gauges)
4. Rotating needle gauge test (circular gauge with numbers)
5. GPIO input test
6. Sensor manager test

## Text Rendering Coordinate System
The FreeType text rendering system uses a specific coordinate convention that's critical for proper text positioning:
- `render_text_with_font(x, y, text, font_size, color)` interprets the `y` parameter as the **top edge** of the text line
- This is different from typical typography baseline positioning
- For vertical centering, calculate: `center_y - (text_height / 2)`
- The text height can be obtained from font metrics or estimated as `font_size * 1.2` for most fonts
- This coordinate system applies to all text rendering operations in the graphics context

## Notes
- Focus on reliability and real-time performance
- Design for automotive environment (vibration, temperature)
- Ensure intuitive operation with physical buttons only
- Plan for extensibility and easy addition of new pages/features

## Render Loop Performance

FPS is hardware-controlled and cannot be increased beyond the display's refresh rate by software means alone.

The render loop does **not** use any manual frame timing, sleep, or target FPS constant. All frame pacing is delegated entirely to the KMS/DRM layer:

- `eglSwapInterval` has no practical effect here — the KMS/DRM path bypasses it.
- Frame timing is governed by `drmModePageFlip` with the `DRM_MODE_PAGE_FLIP_EVENT` flag (0x01), which queues a vsync-aligned page flip. The completion event is consumed via `drmHandleEvent()` at the start of the next frame, using `select()` with a 50ms timeout as the wait mechanism.
- This produces steady **60 FPS**, matching the display's 60Hz refresh rate.
- If uncapped rendering is needed (e.g. for benchmarking), change the flag to `DRM_MODE_PAGE_FLIP_ASYNC` (0x02). This disables vsync alignment and allows 120+ FPS, but may produce visible tearing.
- Any FPS below 60 indicates that a frame took longer than one vblank interval (16.67ms), causing a miss to the next vblank (30 FPS cliff), or that `drmModePageFlip` returned `-EBUSY` due to a pending flip event not being drained — both of which are handled by the current implementation.

## ADC module connectivity
udev rules file: /etc/udev/rules.d/99-niva-adc.rules
udev rule for ADC module: 
```
SUBSYSTEM=="tty", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="5740", ATTRS{serial}=="8D8E416F4957", SYMLINK+="niva_adc", MODE="0666"
```

## TODO list
- Data-driven sensors creation: json describing hardware inputs, sensor chains and logical sensor parameters
- UPS Hat integration (automatic startup/shutdown)
- Display power control (via USB port shutdown during boot, then re-enable when dashboard is ready)

## PiOS login information

Raspberry Pi login:
user
@Niva21#
root - standard password (1 numeric char)

## Known Issues & Fixes

### OpenGL VBO per-frame leak (fixed — June 2026)

**Symptom:** CPU core load and process `VmRSS` grew steadily over time until the loaded core caused FPS to drop. Occasional sharp recovery in frame times was observed mid-run.

**Affected code:** `NeedleIndicator::render_needle` and `NeedleGaugeMarksDecorator::render_batched_marks` in `indicators/needle_indicator.rs`.

**Root cause:** Both functions called `glGenBuffers` + `glDeleteBuffers` on every rendered frame. `glDeleteBuffers` on the RPi V3D driver is **deferred** — the driver cannot release the backing GPU memory until the GPU is actually done reading the buffer, which is always at least one vsync behind the CPU. With 50 needle indicators at 60 fps this produced ~3 000 GPU buffer objects queued for deletion per second. The driver's internal deletion queue accumulated continuously, growing both resident memory and the CPU cost of processing the queue. The occasional sudden frame-time drop was the driver flushing a large batch of accumulated pending deletions at once.

**Confirmed by `run_fuel_level_grid_test` measurements (50 gauges, decorators disabled):**
| Before fix (~2500 s run) | After fix (~2800 s run) |
|---|---|
| VmRSS grew from 135 MB → 160 MB | VmRSS stable at ~72 MB |
| Render avg grew from 11 584 µs → 13 922 µs | Render avg stable at ~1 000–2 200 µs |

**Fix:** Two persistent statics (`NEEDLE_VBO`, `MARKS_VBO`) with `Once` guards — the same pattern already used for the shader programs. Both render functions now call `get_needle_vbo()` / `get_marks_vbo()` once (allocated on first call) and reuse the same GPU buffer object every frame via `glBufferData` with `GL_DYNAMIC_DRAW`. `glGenBuffers` / `glDeleteBuffers` are never called in the hot path.

**General rule:** Never call `glGenBuffers` / `glDeleteBuffers` inside a per-frame render function on the RPi V3D driver. Always pre-allocate VBOs at init time and stream new data with `GL_DYNAMIC_DRAW`.

---
*Created: August 26, 2025*
*Last Updated: June 13, 2026*
