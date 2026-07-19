#![allow(dead_code)]
use std::time::Duration;
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use crossterm::event::{self, Event, KeyCode};
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::hw_providers::HWInput;

// Page manager input is very simple: user can press one of the physical buttons
// on the MFI, which selects a new page or delegated to the page-specific input handler.
// Only one button press is supported (i.e. no combinations).
// Handler supports different sources, i.e. physical MFI buttons, keyboard input.

pub struct InputHandler {
    input_sources: Vec<Box<dyn InputSource>>,
}

pub enum ButtonState {
    Pressed(char),
    Released(char),      // Generated just once after button is released.
}

impl InputHandler {
    pub fn new(input_sources: Vec<Box<dyn InputSource>>) -> Self {
        InputHandler { input_sources }
    }

    // Add a new input source dynamically
    pub fn add_input_source(&mut self, source: Box<dyn InputSource>) {
        self.input_sources.push(source);
    }

    // Return the state of the first pressed or released button, if any.
    pub fn button_state(&mut self) -> Option<ButtonState> {
        for source in &mut self.input_sources {
            match source.button_state() {
                Some(state) => return Some(state),
                None => continue,
            }
        }
        None
    }
}

pub trait InputSource {
    fn button_state(&mut self) -> Option<ButtonState>;
}

/// Buttons B0..B7 from the STM32 ADC module, one debounced digital sensor chain
/// per button (see `main.rs::setup_button_sensors`). Edge detection (press/release)
/// happens here rather than in the chain itself, since a sensor chain only reports
/// current level and InputSource can only return one ButtonState per poll.
const BUTTON_INPUTS: [HWInput; 8] = [
    HWInput::HwButton0, HWInput::HwButton1, HWInput::HwButton2, HWInput::HwButton3,
    HWInput::HwButton4, HWInput::HwButton5, HWInput::HwButton6, HWInput::HwButton7,
];
const BUTTON_KEYS: [char; 8] = ['1', '2', '3', '4', '5', '6', '7', '8'];

pub struct PhysicalButtonInput {
    sensor_manager: SensorManager,
    prev_states: [bool; 8],
}

impl PhysicalButtonInput {
    pub fn new(sensor_manager: SensorManager) -> Self {
        PhysicalButtonInput { sensor_manager, prev_states: [false; 8] }
    }
}

impl InputSource for PhysicalButtonInput {
    fn button_state(&mut self) -> Option<ButtonState> {
        if let Err(e) = self.sensor_manager.read_all_sensors() {
            // Suppress: while the ADC link is down, every button read chain fails with
            // "channel not in frame" until AdcDataProvider's reconnect loop recovers it.
            if !self.sensor_manager.adc_link_down() {
                log::error!("Button sensor read error: {}", e);
            }
            return None;
        }

        for i in 0..8 {
            let active = self.sensor_manager.get_sensor_value(&BUTTON_INPUTS[i])
                .map(|v| v.is_active())
                .unwrap_or(false);
            if active != self.prev_states[i] {
                self.prev_states[i] = active;
                return Some(if active {
                    ButtonState::Pressed(BUTTON_KEYS[i])
                } else {
                    ButtonState::Released(BUTTON_KEYS[i])
                });
            }
        }
        None
    }
}

pub struct KeyboardInput {
    _private: (),          // Really, Rust...
}

impl KeyboardInput {
    pub fn try_new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        restore_output_newline_translation()?;
        Ok(KeyboardInput { _private: () })
    }
}

// crossterm's raw mode clears OPOST (termios `cfmakeraw` behavior), which disables the
// tty driver's \n -> \r\n translation on output. Left alone, every bare \n written after
// this point (println!, log macros, ...) moves the cursor down a row without returning
// it to column 0, producing a staircase effect in the terminal. ICANON/ECHO/ISIG must
// stay off for the per-keystroke reads in `button_state`, so only the output-processing
// flags are restored here rather than undoing raw mode as a whole.
fn restore_output_newline_translation() -> std::io::Result<()> {
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDOUT_FILENO, &mut termios) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        termios.c_oflag |= libc::OPOST | libc::ONLCR;
        if libc::tcsetattr(libc::STDOUT_FILENO, libc::TCSANOW, &termios) != 0 {
          return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

impl Drop for KeyboardInput {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
    }
}

impl InputSource for KeyboardInput {
    fn button_state(&mut self) -> Option<ButtonState> {
        // Integrate crossterm events.
        if event::poll(Duration::from_millis(0)).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    KeyCode::Char(c) => {
                        // Linux terminal does not support separate key released events,
                        // so we generate a released event when the key is pressed.
                        return Some(ButtonState::Released(c));
                    }
                    _ => {}
                }
            }
        }
        None
    }
}