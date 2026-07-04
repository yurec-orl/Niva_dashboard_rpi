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