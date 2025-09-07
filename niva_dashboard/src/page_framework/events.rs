use std::sync::mpsc::{Sender, Receiver, channel};

/// Events that can be triggered by UI components
#[derive(Debug, Clone)]
pub enum UIEvent {
    // Brightness control
    BrightnessUp,
    BrightnessDown,
    SetBrightness(f32),
    
    // Page navigation
    SwitchToPage(u32),

    // System events
    Shutdown,
    Restart,
    
    // Custom button events
    ButtonPressed(String), // Generic button with custom action name
    
    // Diagnostic events
    ShowSensorsInfo,
    ShowLog,
    RunSelfTest,
}

/// Event sender that can be cloned and passed to UI components
#[derive(Clone)]
pub struct EventSender {
    sender: Sender<UIEvent>,
}

impl EventSender {
    pub fn new(sender: Sender<UIEvent>) -> Self {
        Self { sender }
    }
    
    /// Send an event (non-blocking)
    pub fn send(&self, event: UIEvent) {
        if let Err(e) = self.sender.send(event) {
            print!("Failed to send UI event: {:?}\r\n", e);
        }
    }
    
    /// Send brightness up event
    pub fn brightness_up(&self) {
        self.send(UIEvent::BrightnessUp);
    }
    
    /// Send brightness down event
    pub fn brightness_down(&self) {
        self.send(UIEvent::BrightnessDown);
    }
    
    /// Send shutdown event
    pub fn shutdown(&self) {
        self.send(UIEvent::Shutdown);
    }
}

/// Event receiver for processing events
pub struct EventReceiver {
    receiver: Receiver<UIEvent>,
}

impl EventReceiver {
    pub fn new(receiver: Receiver<UIEvent>) -> Self {
        Self { receiver }
    }
    
    /// Try to receive an event (non-blocking)
    pub fn try_recv(&self) -> Result<UIEvent, std::sync::mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
    
    /// Receive an event (blocking)
    pub fn recv(&self) -> Result<UIEvent, std::sync::mpsc::RecvError> {
        self.receiver.recv()
    }
}

/// Create a new event channel
pub fn create_event_channel() -> (EventSender, EventReceiver) {
    let (sender, receiver) = channel();
    (EventSender::new(sender), EventReceiver::new(receiver))
}
