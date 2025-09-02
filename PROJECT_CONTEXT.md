# Niva Dashboard - Raspberry Pi Car Dashboard

## General constraints
When answering the question, ask before generating demos or examples first.
Write meaningful, descriptive comments where appropriate. Avoid obvious comments which repeat the code, like:
// Add new page
self.add_new_page()

## Project Overview
A software dashboard for automotive use, written in Rust and designed to run on Raspberry Pi 4. The system mimics a multi-functional display (MFD) commonly found in aircraft, featuring a central screen with one row of configurable buttons on the left and right sides.

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
- **Purpose**: Central UI framework managing different functional pages
- **Features**:
  - Page-specific button label configuration
  - Dynamic button function assignment
  - Page navigation and state management
  - Context-sensitive UI layouts

#### 2. Hardware Interface Layer
- **Sensor Reading Framework**:
  - Hardware provider abstraction (GPIO, I2C, Test providers)
  - Digital signal processing (debouncing, pulse counting, frequency calculation)
  - Analog signal processing (moving average filtering)
  - Automotive sensor support (speed, temperature, pressure, etc.)
- **GPIO Button Handler**:
  - Physical button press detection
  - Debouncing algorithms
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
- **Graphics**: Raspberry Pi OpenGL/KMS/DRM, freetype library for text rendering
- **Hardware Interface**: GPIO libraries
- **Build System**: Cargo

## Project Structure
```
niva_dashboard/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── page_manager/           # Page management system
│   ├── hardware/               # GPIO and sensor interfaces
│   │   ├── hw_providers.rs     # Hardware abstraction layer (GPIO, I2C, Test providers)
│   │   ├── digital_signal_processing.rs  # Digital signal processing (debouncing, pulse counting)
│   │   ├── gpio_input.rs       # GPIO button handling and debouncing
│   │   └── sensors.rs          # Legacy sensor definitions (being refactored)
│   ├── graphics/               # Graphics rendering system
│   │   ├── context.rs          # OpenGL graphics context and text rendering
│   │   ├── colors.rs           # Color definitions and utilities
│   │   └── opengl_test.rs      # OpenGL testing utilities
│   ├── page_framework/         # Page management framework
│   │   ├── page_manager.rs     # Central page management system
│   │   ├── main_page.rs        # Main dashboard page implementation
│   │   ├── events.rs           # Event handling system
│   │   └── input.rs            # Input processing
│   └── test/                   # Testing utilities
│       └── run_test.rs         # Test execution framework
```

## Development Goals
1. Create a robust page management system for different dashboard views
2. Implement reliable GPIO button handling with proper debouncing
3. Develop flexible visualization components for various data types
4. Ensure smooth real-time performance on Raspberry Pi 4
5. Design intuitive navigation similar to aircraft MFD systems
6. Enhance text renderer to support multiple fonts

## Recent Progress (August 29, 2025)
- **Text Rendering Architecture**: Successfully migrated OpenGLTextRenderer from opengl_test.rs to context.rs, integrating it as a core GraphicsContext capability
- **Resource Management**: Fixed critical bus error by implementing proper cleanup order - text renderer resources are now cleaned up before OpenGL context destruction
- **Page Framework**: Completed page manager implementation with 60 FPS event loop, status line rendering, and FPS tracking
- **Testing**: Verified clean shutdown sequence and bus error elimination through proper resource cleanup timing

## Coding Session (August 30, 2025)
- **Text Rendering Fix**: Modified OpenGL text rendering to treat y-coordinate as top of text line instead of baseline, using font ascender metrics for proper positioning
- **Page Management Architecture**: Transitioned from copying page structures to using shared references with Rc<RefCell<dyn Page>> pattern for efficient memory management
- **Button System Implementation**: 
  - Created comprehensive button label rendering system with left/right alignment

## Coding Session (September 2, 2025)
- **Sensor Reading Framework**: Developed comprehensive hardware abstraction layer for automotive sensors
  - **Hardware Provider Traits**: Created HWAnalogProvider and HWDigitalProvider traits for hardware abstraction
  - **Multiple Implementations**: 
    - GPIOProvider: Direct Raspberry Pi GPIO digital input reading
    - I2CProvider: External ADC/controller interface via I2C protocol
    - TestDataProvider: Time-based test data generation with realistic patterns
    - TestPulseDataProvider: Simulates speed sensor pulses (0-83.3 Hz representing 0-100 km/h)
  - **Signal Processing Layer**:
    - DigitalSignalDebouncer: Configurable debouncing with stable count and time requirements
    - DigitalSignalProcessorPulseCounter: Counts signal transitions for frequency measurement
    - DigitalSignalProcessorPulsePerSecond: Calculates pulses per second with smart update intervals
    - AnalogSignalProcessorMovingAverage: Smooths analog signals using configurable window size
  - **Automotive Calculations**: Speed sensor analysis showing 6 pulses/revolution produces 17-83 pulses/second at 20-100 km/h
  - **Architecture**: Clean separation between hardware providers, signal processing, and logical sensor conversion

## Target Use Cases
- Engine monitoring (RPM, temperature, pressure)
- Vehicle diagnostics and alerts
- Navigation and trip information
- System configuration and settings
- Real-time sensor data visualization

## Notes
- Focus on reliability and real-time performance
- Design for automotive environment (vibration, temperature)
- Ensure intuitive operation with physical buttons only
- Plan for extensibility and easy addition of new pages/features

---
*Created: August 26, 2025*
*Last Updated: September 2, 2025*
