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
  - Operational range: 5-100°C (normal engine operation)
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

---
*Created: August 26, 2025*
*Last Updated: September 2, 2025*
