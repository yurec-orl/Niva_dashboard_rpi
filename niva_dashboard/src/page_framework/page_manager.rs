use crate::graphics::context::GraphicsContext;
use crate::page_framework::input::{InputHandler, ButtonState};
use crate::page_framework::main_page::MainPage;
use crate::page_framework::diag_page::DiagPage;
use crate::page_framework::osc_page::OscPage;
use crate::page_framework::events::{UIEvent, EventSender, EventReceiver, EventBus, create_event_bus};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const STATUS_LINE_MARGIN: f32 = 25.0;

// ButtonPosition correspond to physical 2x4 buttons layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ButtonPosition {
    Left1,
    Left2,
    Left3,
    Left4,
    Right1,
    Right2,
    Right3,
    Right4,
}

// PageButton represents UI button element on MFI page.
// It does not handle actual input.
pub struct PageButton<CB> {
    pos: ButtonPosition,
    pub label: String,
    callback: CB,
}

impl<CB> PageButton<CB>
where
    CB: FnMut(),
{
    pub fn new(pos: ButtonPosition, label: String, callback: CB) -> Self {
        PageButton { pos, label, callback }
    }

    // Invokes button-specific callback.
    pub fn trigger(&mut self) {
        (self.callback)();
    }

    // Used to match buttons with hardware input.
    pub fn position(&self) -> &ButtonPosition {
        &self.pos
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

// Shared data for MFI pages.
pub struct PageBase {
    id: u32,            // Incremental id, depends on page creation order.
    name: String,
    buttons: Vec<PageButton<Box<dyn FnMut()>>>,
}

impl PageBase {
    pub fn new(id: u32, name: String) -> Self {
        PageBase {
            id,
            name,
            buttons: Vec::new(),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn set_buttons(&mut self, mut buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        // Sort buttons by position to ensure correct order
        buttons.sort_by_key(|button| button.position().clone());
        self.buttons = buttons;
    }

    pub fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        &self.buttons
    }

    pub fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.buttons.iter().find(|button| button.position() == &pos)
    }

    pub fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.buttons.iter_mut().find(|button| button.position() == &pos)
    }
}

pub trait Page {
    fn id(&self) -> u32;
    // Render page-specific stuff (except button labels, which are PageManager responsibility).
    fn render(&self, context: &mut GraphicsContext) -> Result<(), String>;
    // Trigger once on switching to this page.
    fn on_enter(&mut self) -> Result<(), String>;
    // Trigger once on switching from this page.
    fn on_exit(&mut self) -> Result<(), String>;
    // If PageManager does not handle button press, this will be called.
    fn on_button(&mut self, button: char) -> Result<(), String>;
    // Process events specific to this page (MPMC allows each page to have its own receiver)
    fn process_events(&mut self) {}

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>>;

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>>;
    
    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>>;
}

// PageManager is responsible for managing multiple pages and their transitions,
// as well as rendering button labels.
pub struct PageManager {
    context: GraphicsContext,
    pg_id: u32,             // Page incremental id, depends on page creation order.
    current_page: Option<usize>,
    pages: Vec<Box<dyn Page>>,

    // Input handling from gpio buttons, external keyboard, etc.
    input_handler: InputHandler,

    // Map hardware keys with UI buttons positions.
    buttons_map: HashMap<char, ButtonPosition>,

    // Event system for UI communication (MPMC)
    event_bus: EventBus,
    event_receiver: EventReceiver,
    event_sender: EventSender,

    fps_counter: FpsCounter,
    start_time: Instant,

    // If set to false, main loop will exit.
    running: bool,
}

impl PageManager {
    pub fn new(context: GraphicsContext) -> Self {
        let mut buttons_map = HashMap::new();
        buttons_map.insert('1', ButtonPosition::Left1);
        buttons_map.insert('2', ButtonPosition::Left2);
        buttons_map.insert('3', ButtonPosition::Left3);
        buttons_map.insert('4', ButtonPosition::Left4);
        buttons_map.insert('5', ButtonPosition::Right1);
        buttons_map.insert('6', ButtonPosition::Right2);
        buttons_map.insert('7', ButtonPosition::Right3);
        buttons_map.insert('8', ButtonPosition::Right4);

        // Create event bus and get sender/receiver
        let event_bus = create_event_bus();
        let event_sender = event_bus.sender();
        let event_receiver = event_bus.receiver();

        PageManager {
            context,
            pg_id: 0,
            current_page: None,
            pages: Vec::new(),
            input_handler: InputHandler::new(),
            buttons_map,
            event_bus,
            event_receiver,
            event_sender,
            fps_counter: FpsCounter::new(),
            start_time: Instant::now(),
            running: false,
        }
    }

    fn get_page_mut_id(&mut self) -> u32 {
        let id = self.pg_id;
        self.pg_id += 1;
        id
    }

    /// Get a new event receiver for pages (MPMC allows multiple receivers)
    pub fn get_event_receiver(&self) -> EventReceiver {
        self.event_bus.receiver()
    }

    /// Get the event sender for UI components
    pub fn get_event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    /// Add a page with event processing capability
    pub fn add_page_with_events(&mut self, page: Box<dyn Page>) -> usize {
        // Pages can process their own events using their receivers
        self.add_page(page)
    }

    fn get_page(&self, index: Option<usize>) -> Option<&Box<dyn Page>> {
        index.and_then(|i| self.pages.get(i))
    }

    fn get_page_mut(&mut self, index: Option<usize>) -> Option<&mut Box<dyn Page>> {
        index.and_then(|i| self.pages.get_mut(i))
    }

    fn get_current_page(&self) -> Option<&Box<dyn Page>> {
        self.get_page(self.current_page)
    }

    fn get_current_page_mut(&mut self) -> Option<&mut Box<dyn Page>> {
        self.get_page_mut(self.current_page)
    }

    pub fn add_page(&mut self, page: Box<dyn Page>) -> usize {
        self.pages.push(page);
        self.pages.len() - 1
    }

    // Switch to a page by index
    pub fn switch_page(&mut self, page_index: usize) -> Result<(), String> {
        // Call on_exit for old page first.
        if let Some(current) = self.get_current_page_mut() {
            current.on_exit()?;
        }

        self.current_page = Some(page_index);

        // Call on_enter for new page.
        if let Some(current) = self.get_current_page_mut() {
            current.on_enter()?;
        }

        Ok(())
    }

    fn button_by_key(&mut self, key: &char) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        let pos = self.buttons_map.get(key).copied()?;
        self.get_current_page_mut()?.button_by_position_mut(pos)
    }

    // Set up pages and buttons.
    pub fn setup(&mut self) -> Result<(), String> {
        // Get event sender for button callbacks
        let event_sender = self.event_sender.clone();

        // Create pages with their own event receivers (MPMC allows this)
        let mut main_page = Box::new(MainPage::new(
            self.get_page_mut_id(), 
            "Main".into(),
            event_sender.clone(), 
            self.get_event_receiver()
        ));
        let diag_page = Box::new(DiagPage::new(
            self.get_page_mut_id(), 
            "Diagnostics".into(),
            event_sender.clone(), 
            self.get_event_receiver()
        ));
        let mut osc_page = Box::new(OscPage::new(
            self.get_page_mut_id(), 
            "Oscilloscope".into(),
            event_sender.clone(), 
            self.get_event_receiver()
        ));

        // Create diagnostic page buttons before adding to page manager
        let mut diag_page_mut = diag_page;
        
        let diag_buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ДАТЧ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::ShowSensorInfo)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ЭБУ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::ShowECUInfo)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "ОСЦ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::ShowOSCInfo)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "НАЗАД".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(0))
            }) as Box<dyn FnMut()>),
        ];
        
        diag_page_mut.set_buttons(diag_buttons);

        // Create buttons for main page 
        let main_buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ВИД+".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::ButtonPressed("view_up".into()))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ВИД-".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::ButtonPressed("view_down".into()))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "ВЫХ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::Shutdown)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "ЯРК+".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::BrightnessUp)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right2, "ЯРК-".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::BrightnessDown)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ДИАГ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(1))
            }) as Box<dyn FnMut()>),
        ];

        main_page.set_buttons(main_buttons);

        // Add pages to manager in order: main(0), diag(1), osc(2)
        let actual_main_page_idx = self.add_page(main_page);
        let actual_diag_page_idx = self.add_page(diag_page_mut);
        let actual_osc_page_idx = self.add_page(osc_page);
        
        self.switch_page(actual_main_page_idx)?;

        Ok(())
    }

    // Do first-time initialization and start main loop.
    // Normally, main loop should not exit until device shutdown on external power loss.
    pub fn start(&mut self) -> Result<(), String> {
        // Hide mouse cursor for dashboard
        if let Err(e) = self.context.hide_cursor() {
            eprintln!("Warning: Failed to hide cursor: {}", e);
        }
        
        self.context.initialize_text_renderer("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 16)?;
        
        print!("Dashboard initialized successfully!\r\n");
        self.running = true;
        self.start_time = Instant::now();
        self.event_loop()
    }

    fn event_loop(&mut self) -> Result<(), String> {
        const TARGET_FPS: u64 = 60;
        const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

        print!("Starting event loop (target: {} FPS)\r\n", TARGET_FPS);
        
        while self.running {
            let frame_start = Instant::now();
            
            // Update FPS counter
            self.fps_counter.update();
            
            // Clear screen with black
            self.context.clear_screen();
            unsafe {
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            }
            
            // Render current page
            if let Some(page_idx) = self.current_page {
                // Create a temporary borrow to avoid conflicts
                let page = &mut self.pages[page_idx];
                page.render(&mut self.context)?;
            } else {
                return Err("No current page to render".into());
            }

            // Render button labels on left and right sides
            self.render_button_labels()?;
            
            // Render status line
            self.render_status_line()?;
            
            // Swap buffers
            self.context.swap_buffers();
            
            // Frame timing control
            let frame_time = frame_start.elapsed();
            if frame_time < FRAME_DURATION {
                std::thread::sleep(FRAME_DURATION - frame_time);
            }

            // Check for button state changes
            if let Some(state) = self.input_handler.button_state() {
                match state {
                    ButtonState::Pressed(key) => {
                        print!("Button pressed: {}\r\n", key);
                    }
                    ButtonState::Released(key) => {
                        print!("Button released: {}\r\n", key);
                        if let Some(button) = self.button_by_key(&key) {
                            button.trigger();
                        }
                    }
                }
            }

            // Process UI events from buttons and other sources (PageManager events)
            while let Ok(event) = self.event_receiver.try_recv() {
                self.handle_ui_event(event);
            }

            // Let the current page process its own events
            if let Some(current_page) = self.get_current_page_mut() {
                current_page.process_events();
            }

            // Exit condition (for now, run for 30 seconds)
            if self.start_time.elapsed() > Duration::from_secs(10) {
                self.running = false;
            }
        }
        
        print!("Event loop finished\r\n");
        
        Ok(())
    }

    /// Handle UI events sent by buttons and other components
    fn handle_ui_event(&mut self, event: UIEvent) {
        print!("Processing UI event: {:?}\r\n", event);
        
        match event {
            UIEvent::BrightnessUp => {
                self.brightness_up();
            }
            UIEvent::BrightnessDown => {
                self.brightness_down();
            }
            UIEvent::SetBrightness(level) => {
                self.set_brightness(level);
            }
            UIEvent::SwitchToPage(page_index) => {
                if let Err(e) = self.switch_page(page_index) {
                    print!("Failed to switch to page {}: {}\r\n", page_index, e);
                }
            }
            UIEvent::Shutdown => {
                print!("Shutdown event received\r\n");
                self.running = false;
            }
            UIEvent::Restart => {
                print!("Restart event received (not implemented)\r\n");
            }
            UIEvent::ButtonPressed(action) => {
                print!("Custom button action: {}\r\n", action);
                // Handle custom button actions here
                match action.as_str() {
                    "view_up" => print!("View up action\r\n"),
                    "view_down" => print!("View down action\r\n"),
                    "engine_data" => print!("Engine data diagnostic action\r\n"),
                    "clear_codes" => print!("Clear diagnostic codes action\r\n"),
                    _ => print!("Unknown action: {}\r\n", action),
                }
            }
            // Page-specific events are handled by individual pages automatically
            UIEvent::ShowSensorInfo => {
                print!("Sensor info event (handled by DiagPage)\r\n");
            }
            UIEvent::ShowECUInfo => {
                print!("ECU info event (handled by DiagPage)\r\n");
            }
            UIEvent::ShowOSCInfo => {
                print!("ECU info event (handled by DiagPage)\r\n");
            }
            // Oscilloscope events (for future use)
            UIEvent::OscStart => {
                print!("Oscilloscope start event\r\n");
            }
            UIEvent::OscStop => {
                print!("Oscilloscope stop event\r\n");
            }
            UIEvent::OscSetSampleRate(rate) => {
                print!("Oscilloscope sample rate: {} Hz\r\n", rate);
            }
            UIEvent::OscSetTimeScale(scale) => {
                print!("Oscilloscope time scale: {}\r\n", scale);
            }
            UIEvent::OscSetVoltageScale(scale) => {
                print!("Oscilloscope voltage scale: {}\r\n", scale);
            }
            UIEvent::OscSetTriggerLevel(level) => {
                print!("Oscilloscope trigger level: {}\r\n", level);
            }
            UIEvent::OscToggleChannel(channel) => {
                print!("Oscilloscope toggle channel: {}\r\n", channel);
            }
        }
    }
    
    fn get_button_position(&self, pos: &ButtonPosition) -> (f32, f32) {
        let screen_width = self.context.width as f32;
        let screen_height = self.context.height as f32 - STATUS_LINE_MARGIN;
        let x_margin = 0.0;   // No horizontal margin
        let y_margin = 30.0;  // Small vertical margin from screen edges
        
        // Define fixed Y positions for each button row (1-4)
        // First button near top, last button near bottom, middle two evenly spaced
        let available_height = screen_height - 2.0 * y_margin;
        let y_positions = [
            y_margin,                                    // Row 1 - near top
            y_margin + available_height / 3.0,           // Row 2 - 1/3 down
            y_margin + 2.0 * available_height / 3.0,     // Row 3 - 2/3 down
            screen_height - y_margin,                    // Row 4 - near bottom
        ];
        
        match pos {
            ButtonPosition::Left1 => (x_margin, y_positions[0]),
            ButtonPosition::Left2 => (x_margin, y_positions[1]),
            ButtonPosition::Left3 => (x_margin, y_positions[2]),
            ButtonPosition::Left4 => (x_margin, y_positions[3]),
            ButtonPosition::Right1 => (screen_width - x_margin, y_positions[0]),
            ButtonPosition::Right2 => (screen_width - x_margin, y_positions[1]),
            ButtonPosition::Right3 => (screen_width - x_margin, y_positions[2]),
            ButtonPosition::Right4 => (screen_width - x_margin, y_positions[3]),
        }
    }
    
    fn render_button_at_position(&mut self, pos: &ButtonPosition, label: &str, label_scale: f32, label_color: (f32, f32, f32)) -> Result<(), String> {
        let (x, y) = self.get_button_position(pos);
        
        let render_x = match pos {
            // Right side buttons are right-aligned
            ButtonPosition::Right1 | ButtonPosition::Right2 | 
            ButtonPosition::Right3 | ButtonPosition::Right4 => {
                let text_width = self.context.calculate_text_width(label, label_scale)?;
                x - text_width
            }
            // Left side buttons are left-aligned
            _ => x,
        };
        
        self.context.render_text(label, render_x, y, label_scale, label_color)?;
        Ok(())
    }
    
    fn render_button_labels(&mut self) -> Result<(), String> {
        // Check if there's a current page
        if self.current_page.is_none() {
            return Ok(());
        }

        // Render settings
        let label_scale = 1.2;
        let label_color = (1.0, 1.0, 1.0);

        // Collect button data first to avoid borrowing conflicts
        let button_data: Vec<(ButtonPosition, String)> = {
            let current_page = self.get_current_page().unwrap();
            current_page.buttons()
                .iter()
                .map(|button| (*button.position(), button.label().to_string()))
                .collect()
        };

        // Now render each button at its fixed position
        for (position, label) in button_data {
            self.render_button_at_position(&position, &label, label_scale, label_color)?;
        }

        Ok(())
    }
    
    fn render_status_line(&mut self) -> Result<(), String> {
        let elapsed = self.start_time.elapsed();
        let fps = self.fps_counter.get_fps();
        let frame_count = self.fps_counter.get_frame_count();
        
        // Format status information
        let status_text = format!(
            "Time: {:.1}s | FPS: {:.1} | Frame: {} | Resolution: {}x{}",
            elapsed.as_secs_f32(),
            fps,
            frame_count,
            self.context.width,
            self.context.height
        );
        
        // Render status line at bottom of screen
        let status_y = self.context.height as f32 - STATUS_LINE_MARGIN; // 25 pixels from bottom
        let status_x = 10.0; // 10 pixels from left
        
        self.context.render_text(
            &status_text,
            status_x,
            status_y,
            1.0, // scale
            (0.7, 0.7, 0.7), // gray color
        )?;
        
        Ok(())
    }
    
    pub fn stop(&mut self) {
        self.running = false;
    }

    // =============================================================================
    // Brightness Control for UI
    // =============================================================================

    /// Set display brightness (0.0 to 1.0)
    pub fn set_brightness(&mut self, brightness: f32) {
        self.context.set_brightness(brightness);
        print!("Display brightness set to: {:.1}%\r\n", brightness * 100.0);
    }

    /// Get current brightness level
    pub fn get_brightness(&self) -> f32 {
        self.context.get_brightness()
    }

    /// Increase brightness by 10%
    pub fn brightness_up(&mut self) {
        self.context.increase_brightness(0.1);
        let current = self.get_brightness();
        print!("Brightness increased to: {:.1}%\r\n", current * 100.0);
    }

    /// Decrease brightness by 10%
    pub fn brightness_down(&mut self) {
        self.context.decrease_brightness(0.1);
        let current = self.get_brightness();
        print!("Brightness decreased to: {:.1}%\r\n", current * 100.0);
    }

}

#[derive(Debug)]
pub struct FpsCounter {
    frame_count: u32,
    last_time: Instant,
    current_fps: f32,
    frame_times: Vec<Duration>,
    max_samples: usize,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            last_time: Instant::now(),
            current_fps: 0.0,
            frame_times: Vec::new(),
            max_samples: 60, // Track last 60 frames for smoothing
        }
    }
    
    pub fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_time);
        
        self.frame_times.push(delta);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
        
        self.frame_count += 1;
        self.last_time = now;
        
        // Calculate FPS from average frame time
        if !self.frame_times.is_empty() {
            let avg_frame_time: Duration = self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;
            self.current_fps = 1.0 / avg_frame_time.as_secs_f32();
        }
    }
    
    pub fn get_fps(&self) -> f32 {
        self.current_fps
    }
    
    pub fn get_frame_count(&self) -> u32 {
        self.frame_count
    }
}
