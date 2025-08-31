use crossbeam_channel::{bounded, Sender, Receiver};

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

    // Diagnostic page events
    ShowSensorInfo,
    ShowECUInfo,
    ShowOSCInfo,

    // Oscilloscope page events
    OscStart,
    OscStop,
    OscSetSampleRate(f32),
    OscSetTimeScale(f32),
    OscSetVoltageScale(f32),
    OscSetTriggerLevel(f32),
    OscToggleChannel(u8),
}

/// Event bus that manages MPMC communication
pub struct EventBus {
    sender: Sender<UIEvent>,
    receiver: Receiver<UIEvent>,
}

impl EventBus {
    /// Create a new event bus with bounded capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self { sender, receiver }
    }
    
    /// Create a new event bus with unbounded capacity
    pub fn unbounded() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
    
    /// Get a sender that can be cloned and distributed
    pub fn sender(&self) -> EventSender {
        EventSender::new(self.sender.clone())
    }
    
    /// Get a receiver that can be cloned for multiple consumers
    pub fn receiver(&self) -> EventReceiver {
        EventReceiver::new(self.receiver.clone())
    }
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
        if let Err(e) = self.sender.try_send(event) {
            eprintln!("Failed to send UI event: {:?}", e);
        }
    }
    
    /// Send an event (blocking)
    pub fn send_blocking(&self, event: UIEvent) {
        if let Err(e) = self.sender.send(event) {
            eprintln!("Failed to send UI event (blocking): {:?}", e);
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
    
    /// Send page switch event
    pub fn switch_to_page(&self, page_id: u32) {
        self.send(UIEvent::SwitchToPage(page_id));
    }
}

/// Event receiver for processing events (can be cloned for MPMC)
#[derive(Clone)]
pub struct EventReceiver {
    receiver: Receiver<UIEvent>,
}

impl EventReceiver {
    pub fn new(receiver: Receiver<UIEvent>) -> Self {
        Self { receiver }
    }
    
    /// Try to receive an event (non-blocking)
    pub fn try_recv(&self) -> Result<UIEvent, crossbeam_channel::TryRecvError> {
        self.receiver.try_recv()
    }
    
    /// Receive an event (blocking)
    pub fn recv(&self) -> Result<UIEvent, crossbeam_channel::RecvError> {
        self.receiver.recv()
    }
    
    /// Receive an event with timeout
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<UIEvent, crossbeam_channel::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
    
    /// Create an iterator over received events
    pub fn iter(&self) -> crossbeam_channel::Iter<UIEvent> {
        self.receiver.iter()
    }
    
    /// Create a non-blocking iterator over received events
    pub fn try_iter(&self) -> crossbeam_channel::TryIter<UIEvent> {
        self.receiver.try_iter()
    }
}

/// Create a new event bus with default settings
pub fn create_event_bus() -> EventBus {
    EventBus::new(1000) // Bounded channel with 1000 event capacity
}
