# Niva Dashboard - Raspberry Pi Car Dashboard

## Project Overview
A software dashboard for automotive use, written in Rust and designed to run on Raspberry Pi 4. The system mimics a multi-functional display (MFD) commonly found in aircraft, featuring a central screen with one row of configurable buttons on the left and right sides.

## Hardware Platform
- **Target Device**: Raspberry Pi 4
- **Graphics**: Raspberry Pi OpenGL library
- **Input**: GPIO-connected physical buttons (2 rows, left and right sides)
- **Display**: Central screen for data visualization

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
- **GPIO Button Handler**:
  - Physical button press detection
  - Debouncing algorithms
  - Button state management
- **Sensor Data Reader**:
  - Car sensor data acquisition
  - Signal smoothing and filtering
  - Real-time data processing

#### 3. Visualization System
- **Data Presentation Classes**:
  - Bar indicators (progress bars, level meters)
  - Digital segmented displays (7-segment style)
  - Gauge displays (analog-style meters)
  - Text displays
  - Alert/warning indicators

### Technology Stack
- **Language**: Rust
- **Graphics**: Raspberry Pi OpenGL
- **Hardware Interface**: GPIO libraries
- **Build System**: Cargo

## Project Structure
```
niva_dashboard/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── page_manager/           # Page management system
│   ├── hardware/               # GPIO and sensor interfaces
│   │   ├── buttons.rs          # Button handling and debouncing
│   │   └── sensors.rs          # Car sensor data reading
│   ├── visualization/          # Data display components
│   │   ├── bar_indicator.rs    # Bar-style displays
│   │   ├── digital_display.rs  # Segmented digital displays
│   │   └── gauge.rs            # Analog-style gauges
│   └── utils/                  # Signal processing utilities
│       ├── smoothing.rs        # Signal smoothing algorithms
│       └── debounce.rs         # Debouncing utilities
```

## Development Goals
1. Create a robust page management system for different dashboard views
2. Implement reliable GPIO button handling with proper debouncing
3. Develop flexible visualization components for various data types
4. Ensure smooth real-time performance on Raspberry Pi 4
5. Design intuitive navigation similar to aircraft MFD systems

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
*Last Updated: August 26, 2025*
