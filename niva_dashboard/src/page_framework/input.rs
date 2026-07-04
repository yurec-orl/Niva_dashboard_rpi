#![allow(dead_code)]
use std::time::Duration;
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use crossterm::event::{self, Event, KeyCode};

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
    pub fn new() -> Self {
        let mut sources: Vec<Box<dyn InputSource>> = vec![Box::new(PhysicalButtonInput {})];
        match KeyboardInput::try_new() {
            Ok(kb) => sources.push(Box::new(kb)),
            Err(e) => log::info!("Keyboard input unavailable (no TTY?): {}", e),
        }
        InputHandler { input_sources: sources }
    }

    // Add a new input source dynamically
    pub fn add_input_source(&mut self, source: Box<dyn InputSource>) {
        self.input_sources.push(source);
    }

    // Return the state of the first pressed or released button, if any.
    pub fn button_state(&self) -> Option<ButtonState> {
        for source in &self.input_sources {
            match source.button_state() {
                Some(state) => return Some(state),
                None => continue,
            }
        }
        None
    }
}

pub trait InputSource {
    fn button_state(&self) -> Option<ButtonState>;
}

struct PhysicalButtonInput {
}

impl InputSource for PhysicalButtonInput {
    fn button_state(&self) -> Option<ButtonState> {
        None
    }
}

struct KeyboardInput {
    _private: (),          // Really, Rust...
}

impl KeyboardInput {
    fn try_new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        Ok(KeyboardInput { _private: () })
    }
}

impl Drop for KeyboardInput {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
    }
}

impl InputSource for KeyboardInput {
    fn button_state(&self) -> Option<ButtonState> {
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