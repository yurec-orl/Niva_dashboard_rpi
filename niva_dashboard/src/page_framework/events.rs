#![allow(dead_code)]
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

    // Nav page events
    NavPnpMode,
    NavInfoMode,
    NavMapMode,
    NavToggleGnssTest,
    NavHeadingSetMode,     // Enter the КУРС+/КУРС- manual heading button set
    NavHeadingSetExit,     // Leave the manual heading button set, back to the primary one
    NavHeadingIncrease,    // Nudge the manual heading anchor +1 deg (needs PageManager's heading_fusion)
    NavHeadingDecrease,    // Nudge the manual heading anchor -1 deg (needs PageManager's heading_fusion)

    // Alert events
    SuppressAlerts,

    // Switch sensors event
    SwitchSensorSet,
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
            log::error!("Failed to send UI event: {:?}", e);
        }
    }
    
    /// Send an event (blocking)
    pub fn send_blocking(&self, event: UIEvent) {
        if let Err(e) = self.sender.send(event) {
            log::error!("Failed to send UI event (blocking): {:?}", e);
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
    pub fn iter(&self) -> crossbeam_channel::Iter<'_, UIEvent> {
        self.receiver.iter()
    }
    
    /// Create a non-blocking iterator over received events
    pub fn try_iter(&self) -> crossbeam_channel::TryIter<'_, UIEvent> {
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
            UIEvent::SuppressAlerts |
            UIEvent::SwitchSensorSet |
            // Manual heading anchor lives in PageManager's heading_fusion, not the page.
            UIEvent::NavHeadingIncrease |
            UIEvent::NavHeadingDecrease => {
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
            UIEvent::OscToggleChannel(_) |
            UIEvent::NavPnpMode |
            UIEvent::NavInfoMode |
            UIEvent::NavMapMode |
            UIEvent::NavToggleGnssTest |
            UIEvent::NavHeadingSetMode |
            UIEvent::NavHeadingSetExit => {
                self.page_sender.send(event);
            }
        }
    }
}

/// Create a new event bus with default settings
pub fn create_event_bus() -> EventBus {
    EventBus::new(1000) // Bounded channel with 1000 event capacity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_and_page_channels_are_independent() {
        let bus = EventBus::new(4);

        bus.global_sender().send(UIEvent::Shutdown);
        bus.page_sender().send(UIEvent::ShowLog);

        assert!(matches!(bus.global_receiver().try_recv(), Ok(UIEvent::Shutdown)),
               "global channel should only see what was sent on the global sender");
        assert!(matches!(bus.page_receiver().try_recv(), Ok(UIEvent::ShowLog)),
               "page channel should only see what was sent on the page sender");

        // Each channel is now drained.
        assert!(bus.global_receiver().try_recv().is_err());
        assert!(bus.page_receiver().try_recv().is_err());
    }

    #[test]
    fn test_unbounded_event_bus_round_trip() {
        let bus = EventBus::unbounded();
        bus.global_sender().send(UIEvent::Restart);
        assert!(matches!(bus.global_receiver().try_recv(), Ok(UIEvent::Restart)));
    }

    #[test]
    fn test_create_event_bus_has_working_bounded_channel() {
        let bus = create_event_bus();
        bus.global_sender().send(UIEvent::BrightnessUp);
        assert!(matches!(bus.global_receiver().try_recv(), Ok(UIEvent::BrightnessUp)));
    }

    #[test]
    fn test_receiver_recv_timeout_expires_on_empty_channel() {
        let bus = EventBus::new(4);
        let result = bus.global_receiver().recv_timeout(std::time::Duration::from_millis(10));
        assert!(result.is_err(), "recv_timeout on an empty channel must time out rather than block forever");
    }

    // Every UIEvent variant that SmartEventSender::send routes to the global channel
    // (handled by PageManager), per the match in `send` above.
    fn global_events() -> Vec<UIEvent> {
        vec![
            UIEvent::BrightnessUp,
            UIEvent::BrightnessDown,
            UIEvent::SetBrightness(0.5),
            UIEvent::SwitchToPage(2),
            UIEvent::Shutdown,
            UIEvent::Restart,
            UIEvent::SuppressAlerts,
            UIEvent::SwitchSensorSet,
            UIEvent::NavHeadingIncrease,
            UIEvent::NavHeadingDecrease,
        ]
    }

    // Every UIEvent variant that SmartEventSender::send routes to the page channel
    // (handled by the current page).
    fn page_events() -> Vec<UIEvent> {
        vec![
            UIEvent::NextIndicatorSet,
            UIEvent::PreviousIndicatorSet,
            UIEvent::ButtonPressed("test".to_string()),
            UIEvent::ShowSensorInfo,
            UIEvent::ShowECUInfo,
            UIEvent::ShowOSCInfo,
            UIEvent::ShowLog,
            UIEvent::OscStart,
            UIEvent::OscStop,
            UIEvent::OscSetSampleRate(1.0),
            UIEvent::OscSetTimeScale(1.0),
            UIEvent::OscSetVoltageScale(1.0),
            UIEvent::OscSetTriggerLevel(1.0),
            UIEvent::OscToggleChannel(0),
            UIEvent::NavHeadingSetMode,
            UIEvent::NavHeadingSetExit,
        ]
    }

    #[test]
    fn test_smart_sender_routes_global_events_to_global_channel_only() {
        let bus = EventBus::new(global_events().len());
        let smart_sender = bus.smart_sender();
        let global_receiver = bus.global_receiver();
        let page_receiver = bus.page_receiver();

        for event in global_events() {
            smart_sender.send(event);
        }

        assert_eq!(global_receiver.try_iter().count(), 10, "every global-tagged event must land on the global channel");
        assert!(page_receiver.try_recv().is_err(), "no global-tagged event should leak onto the page channel");
    }

    #[test]
    fn test_smart_sender_routes_page_events_to_page_channel_only() {
        let bus = EventBus::new(page_events().len());
        let smart_sender = bus.smart_sender();
        let global_receiver = bus.global_receiver();
        let page_receiver = bus.page_receiver();

        for event in page_events() {
            smart_sender.send(event);
        }

        assert_eq!(page_receiver.try_iter().count(), 16, "every page-tagged event must land on the page channel");
        assert!(global_receiver.try_recv().is_err(), "no page-tagged event should leak onto the global channel");
    }
}
