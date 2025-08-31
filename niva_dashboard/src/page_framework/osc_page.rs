use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::{Page, PageButton, ButtonPosition};
use crate::page_framework::events::{UIEvent, EventSender, EventReceiver};

/// Base page structure for common functionality
pub struct PageBase {
    id: u32,
    name: String,
    buttons: Vec<PageButton<Box<dyn FnMut()>>>,
}

impl PageBase {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            buttons: Vec::new(),
        }
    }
    
    pub fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.buttons = buttons;
    }
}

/// Oscilloscope page for signal visualization
pub struct OscPage {
    base: PageBase,
    event_sender: EventSender,
    event_receiver: EventReceiver,
    
    // Oscilloscope state
    is_running: bool,
    sample_rate: f32,
    time_scale: f32,
    voltage_scale: f32,
    trigger_level: f32,
    channel_enabled: [bool; 4],
}

impl OscPage {
    pub fn new(id: u32, name: String, event_sender: EventSender, event_receiver: EventReceiver) -> Self {
        Self {
            base: PageBase::new(id, name),
            event_sender,
            event_receiver,
            is_running: false,
            sample_rate: 1000.0,
            time_scale: 1.0,
            voltage_scale: 1.0,
            trigger_level: 0.0,
            channel_enabled: [true, false, false, false],
        }
    }
    
    /// Process oscilloscope-specific events
    pub fn process_events(&mut self) {
        // Process events relevant to this page
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                UIEvent::OscStart => {
                    self.is_running = true;
                    print!("Oscilloscope started\n");
                }
                UIEvent::OscStop => {
                    self.is_running = false;
                    print!("Oscilloscope stopped\n");
                }
                UIEvent::OscSetSampleRate(rate) => {
                    self.sample_rate = rate;
                    print!("Sample rate set to: {} Hz\n", rate);
                }
                UIEvent::OscSetTimeScale(scale) => {
                    self.time_scale = scale;
                    print!("Time scale set to: {}\n", scale);
                }
                UIEvent::OscSetVoltageScale(scale) => {
                    self.voltage_scale = scale;
                    print!("Voltage scale set to: {}\n", scale);
                }
                UIEvent::OscSetTriggerLevel(level) => {
                    self.trigger_level = level;
                    print!("Trigger level set to: {}\n", level);
                }
                UIEvent::OscToggleChannel(channel) => {
                    if (channel as usize) < self.channel_enabled.len() {
                        self.channel_enabled[channel as usize] = !self.channel_enabled[channel as usize];
                        print!("Channel {} toggled: {}\n", channel, self.channel_enabled[channel as usize]);
                    }
                }
                _ => {} // Ignore other events
            }
        }
    }

    pub fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }
}

impl Page for OscPage {
    fn id(&self) -> u32 {
        self.base.id
    }

    fn render(&self, context: &mut GraphicsContext) -> Result<(), String> {
        // Render oscilloscope UI
        let status = if self.is_running { "RUNNING" } else { "STOPPED" };
        let status_text = format!("OSC: {} | Rate: {:.0}Hz | Time: {:.1} | Volt: {:.1} | Trig: {:.2}", 
                                 status, self.sample_rate, self.time_scale, self.voltage_scale, self.trigger_level);
        
        context.render_text(&status_text, 50.0, 50.0, 20.0, (1.0, 1.0, 1.0))?;
        
        // Render channel status
        for (i, &enabled) in self.channel_enabled.iter().enumerate() {
            let channel_text = format!("CH{}: {}", i + 1, if enabled { "ON" } else { "OFF" });
            let color = if enabled { (0.0, 1.0, 0.0) } else { (0.5, 0.5, 0.5) };
            context.render_text(&channel_text, 50.0 + (i as f32 * 100.0), 80.0, 16.0, color)?;
        }
        
        // TODO: Render actual oscilloscope waveform
        
        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        print!("Entering Oscilloscope page\n");
        Ok(())
    }

    fn on_exit(&mut self) -> Result<(), String> {
        print!("Exiting Oscilloscope page\n");
        Ok(())
    }

    fn on_button(&mut self, button: char) -> Result<(), String> {
        // Handle oscilloscope-specific button presses
        match button {
            '1' => self.event_sender.send(UIEvent::OscStart),
            '2' => self.event_sender.send(UIEvent::OscStop),
            '3' => self.event_sender.send(UIEvent::OscToggleChannel(0)),
            '4' => self.event_sender.send(UIEvent::OscToggleChannel(1)),
            _ => {}
        }
        Ok(())
    }

    fn process_events(&mut self) {
        self.process_events();
    }

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        &self.base.buttons
    }

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.base.buttons.iter().find(|button| *button.position() == pos)
    }

    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.base.buttons.iter_mut().find(|button| *button.position() == pos)
    }
}