use crossbeam_channel::{bounded, Sender, Receiver};
use crate::hardware::hw_providers::HWInput;

/// Events that can be triggered by UI components
#[derive(Debug, Clone)]
pub enum UIEvent {
    // Brightness control
    BrightnessUp,
    BrightnessDown,
    SetBrightness(f32),
    
    // Page navigation
    SwitchToPage(u32),

    // Main page events
    NextIndicatorSet,
    PreviousIndicatorSet,

    // System events
    Shutdown,
    Restart,
    
    // Custom button events
    ButtonPressed(String), // Generic button with custom action name

    // Diagnostic page events
    ShowSensorInfo,
    ShowECUInfo,
    ShowOSCInfo,
    ShowLog,

    // Oscilloscope page events
    OscStart,
    OscStop,
    OscSetSampleRate(f32),
    OscSetTimeScale(f32),
    OscSetVoltageScale(f32),
    OscSetTriggerLevel(f32),
    OscToggleChannel(u8),

    // Alert events
    SuppressAlerts,
}

/// Event bus that manages dual-channel communication for global and page events
pub struct EventBus {
    // Global events channel (only PageManager listens)
    global_sender: Sender<UIEvent>,
    global_receiver: Receiver<UIEvent>,
    // Page events channel (only current page listens)  
    page_sender: Sender<UIEvent>,
    page_receiver: Receiver<UIEvent>,
}

impl EventBus {
    /// Create a new event bus with bounded capacity
    pub fn new(capacity: usize) -> Self {
        let (global_sender, global_receiver) = bounded(capacity);
        let (page_sender, page_receiver) = bounded(capacity);
        Self { 
            global_sender, 
            global_receiver,
            page_sender,
            page_receiver
        }
    }
    
    /// Create a new event bus with unbounded capacity
    pub fn unbounded() -> Self {
        let (global_sender, global_receiver) = crossbeam_channel::unbounded();
        let (page_sender, page_receiver) = crossbeam_channel::unbounded();
        Self { 
            global_sender, 
            global_receiver,
            page_sender,
            page_receiver
        }
    }
    
    /// Get a sender for global events (handled by PageManager)
    pub fn global_sender(&self) -> EventSender {
        EventSender::new(self.global_sender.clone())
    }
    
    /// Get a receiver for global events (PageManager only)
    pub fn global_receiver(&self) -> EventReceiver {
        EventReceiver::new(self.global_receiver.clone())
    }
    
    /// Get a sender for page-specific events
    pub fn page_sender(&self) -> EventSender {
        EventSender::new(self.page_sender.clone())
    }
    
    /// Get a receiver for page-specific events (current page only)
    pub fn page_receiver(&self) -> EventReceiver {
        EventReceiver::new(self.page_receiver.clone())
    }
    
    /// Get a smart sender that routes events to appropriate channels
    pub fn smart_sender(&self) -> SmartEventSender {
        SmartEventSender::new(
            EventSender::new(self.global_sender.clone()),
            EventSender::new(self.page_sender.clone())
        )
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
        if let Err(e) = self.sender.send(event) {
            print!("Failed to send UI event: {:?}\r\n", e);
        }
    }
    
    /// Send an event (blocking)
    pub fn send_blocking(&self, event: UIEvent) {
        if let Err(e) = self.sender.send(event) {
            eprintln!("Failed to send UI event (blocking): {:?}", e);
        }
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

/// Smart sender that routes events to appropriate channels based on event type
#[derive(Clone)]
pub struct SmartEventSender {
    global_sender: EventSender,
    page_sender: EventSender,
}

impl SmartEventSender {
    pub fn new(global_sender: EventSender, page_sender: EventSender) -> Self {
        Self { global_sender, page_sender }
    }
    
    /// Send an event to the appropriate channel based on event type
    pub fn send(&self, event: UIEvent) {
        match event {
            // Global events go to PageManager
            UIEvent::Shutdown |
            UIEvent::Restart |
            UIEvent::BrightnessUp |
            UIEvent::BrightnessDown |
            UIEvent::SetBrightness(_) |
            UIEvent::SwitchToPage(_) |
            UIEvent::SuppressAlerts => {
                self.global_sender.send(event);
            }
            // Page-specific events go to current page
            UIEvent::NextIndicatorSet |
            UIEvent::PreviousIndicatorSet |
            UIEvent::ButtonPressed(_) |
            UIEvent::ShowSensorInfo |
            UIEvent::ShowECUInfo |
            UIEvent::ShowOSCInfo |
            UIEvent::ShowLog |
            UIEvent::OscStart |
            UIEvent::OscStop |
            UIEvent::OscSetSampleRate(_) |
            UIEvent::OscSetTimeScale(_) |
            UIEvent::OscSetVoltageScale(_) |
            UIEvent::OscSetTriggerLevel(_) |
            UIEvent::OscToggleChannel(_) => {
                self.page_sender.send(event);
            }
        }
    }
}

/// Create a new event bus with default settings
pub fn create_event_bus() -> EventBus {
    EventBus::new(1000) // Bounded channel with 1000 event capacity
}
