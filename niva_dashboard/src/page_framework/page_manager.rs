use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::diag_page::DiagPage;
use crate::page_framework::events::{UIEvent, EventSender, EventReceiver, EventBus, create_event_bus};
use crate::page_framework::input::{InputHandler, ButtonState};
use crate::page_framework::main_page::MainPage;
use crate::hardware::sensor_manager::SensorManager;

use std::collections::HashMap;
use std::time::{Duration, Instant};

const STATUS_LINE_MARGIN: f32 = 25.0;

pub const MAIN_PAGE_ID: u32 = 0;
pub const DIAG_PAGE_ID: u32 = 1;

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
    ui_style: UIStyle,
}

impl PageBase {
    pub fn new(id: u32, name: String, ui_style: UIStyle) -> Self {
        PageBase {
            id,
            name,
            buttons: Vec::new(),
            ui_style,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
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

    pub fn ui_style(&self) -> &UIStyle {
        &self.ui_style
    }
}

pub trait Page {
    fn id(&self) -> u32;
    fn name(&self) -> &str;
    // Render page-specific stuff (except button labels, which are PageManager responsibility).
    fn render(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager) -> Result<(), String>;
    // Trigger once on switching to this page.
    fn on_enter(&mut self) -> Result<(), String>;
    // Trigger once on switching from this page.
    fn on_exit(&mut self) -> Result<(), String>;
    // If PageManager does not handle button press, this will be called.
    fn on_button(&mut self, button: char) -> Result<(), String>;
    // Process events specific to this page (MPMC allows each page to have its own receiver)
    fn process_events(&mut self) {}

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>>;
    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>);

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>>;
    
    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>>;

    fn ui_style(&self) -> &UIStyle;
}

// Helper struct to manage collection of pages.
// Introduced to avoid borrowing issues in PageManager.
struct Pages {
    pages: Vec<Box<dyn Page>>,
}

impl Pages {
    pub fn new() -> Self {
        Pages { pages: Vec::new() }
    }

    pub fn add_page(&mut self, page: Box<dyn Page>) {
        self.pages.push(page);
    }

    pub fn get_page(&self, id: u32) -> Option<&Box<dyn Page>> {
        for page in &self.pages {
            if page.id() == id {
                return Some(page);
            }
        }
        None
    }

    pub fn get_page_mut(&mut self, id: u32) -> Option<&mut Box<dyn Page>> {
        for page in &mut self.pages {
            if page.id() == id {
                return Some(page);
            }
        }
        None
    }
}

// PageManager is responsible for managing multiple pages and their transitions,
// as well as rendering button labels.
pub struct PageManager {
    // Rendering-related stuff.
    context: GraphicsContext,
    // Global UI style settings.
    ui_style: UIStyle,

    // Takes care of low-level hw input, signal processing, and conversion
    // to actual sensor values.
    sensor_manager: SensorManager,

    // UI pages related stuff.
    pg_id: u32,             // Page incremental id, depends on page creation order.
    current_page: Option<u32>,
    pages: Pages,

    // Input handling from gpio buttons, external keyboard, etc.
    input_handler: InputHandler,

    // Map hardware keys with UI buttons positions.
    buttons_map: HashMap<char, ButtonPosition>,

    // Event system for UI communication (MPMC).
    event_bus: EventBus,
    event_receiver: EventReceiver,
    event_sender: EventSender,

    fps_counter: FpsCounter,
    start_time: Instant,

    // If set to false, main loop will exit.
    running: bool,
}

impl PageManager {
    pub fn new(context: GraphicsContext, sensor_manager: SensorManager) -> Self {
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

        let ui_style = UIStyle::new();
        // ui_style.read_from_file("/etc/niva_dashboard/ui_style.json").unwrap_or_else(|e| {
        //     print!("Warning: Failed to read UI style config: {}\r\n", e);
        // });

        PageManager {
            context,
            ui_style,
            sensor_manager,
            pg_id: 0,
            current_page: None,
            pages: Pages::new(),
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

    fn get_page_id(&mut self) -> u32 {
        let id = self.pg_id;
        while (self.get_page(id)).is_some() {
            self.pg_id += 1;
        }
        self.pg_id
    }

    /// Get a new event receiver for pages (MPMC allows multiple receivers)
    pub fn get_event_receiver(&self) -> EventReceiver {
        self.event_bus.receiver()
    }

    /// Get the event sender for UI components
    pub fn get_event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    fn get_page(&self, id: u32) -> Option<&Box<dyn Page>> {
        self.pages.get_page(id)
    }

    fn get_page_mut(&mut self, id: u32) -> Option<&mut Box<dyn Page>> {
        self.pages.get_page_mut(id)
    }

    fn get_current_page(&self) -> Option<&Box<dyn Page>> {
        if let Some(page_id) = self.current_page {
            self.get_page(page_id)
        } else {
            None
        }
    }

    fn get_current_page_mut(&mut self) -> Option<&mut Box<dyn Page>> {
        if let Some(page_id) = self.current_page {
            self.get_page_mut(page_id)
        } else {
            None
        }
    }

    fn render_current_page(&mut self) -> Result<(), String> {
        if let Some(page_id) = self.current_page {
            match self.pages.get_page_mut(page_id) {
                Some(page) => page.render(&mut self.context, &self.sensor_manager),
                None => Err(format!("Current page id {} not found", page_id)),
            }?;
        }

        Ok(())
    }

    pub fn add_page(&mut self, page: Box<dyn Page>) -> u32 {
        let page_id = page.id();
        // Check if page with same id already exists - abort if so,
        // because page switching would be ambiguous.
        if self.get_page(page_id).is_some() {
            print!("Warning: Page with id {} already exists, exiting\r\n", page_id);
            std::process::exit(1);
        }
        self.pages.add_page(page);
        page_id
    }

    // Switch to a page by index
    pub fn switch_page(&mut self, page_id: u32) -> Result<(), String> {
        // Call on_exit for old page first.
        if let Some(current) = self.get_current_page_mut() {
            current.on_exit()?;
        }

        self.current_page = Some(page_id);

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

        // Create and add pages first to get their IDs
        let mut main_page_style = self.ui_style.clone();
        main_page_style.set(TEXT_PRIMARY_FONT_SIZE, UIStyleValue::Integer(14));
        main_page_style.set(TEXT_PRIMARY_COLOR, UIStyleValue::Color("#FFFFFF".to_string())); // White color
        let mut main_page = Box::new(MainPage::new(MAIN_PAGE_ID,
                                                   main_page_style,
                                                   event_sender.clone(),
                                                   self.get_event_receiver(),
                                                   &self.context));

        // let mut diag_page_style = UIStyle::new();
        // diag_page_style.set(TEXT_PRIMARY_FONT_SIZE, UIStyleValue::Integer(20));
        // diag_page_style.set(TEXT_PRIMARY_COLOR, UIStyleValue::Color("#00FF00".to_string())); // Green color
        // diag_page_style.set(TEXT_PRIMARY_FONT, UIStyleValue::String("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".to_string()));
        let mut diag_page = Box::new(DiagPage::new(DIAG_PAGE_ID,
                                                   self.ui_style.clone(),
                                                   event_sender.clone(),
                                                   self.get_event_receiver()));

        self.add_page(main_page);
        self.switch_page(MAIN_PAGE_ID)?;

        self.add_page(diag_page);

        Ok(())
    }

    // Do first-time initialization and start main loop.
    // Normally, main loop should not exit until device shutdown on external power loss.
    pub fn start(&mut self) -> Result<(), String> {
        // Hide mouse cursor for dashboard
        if let Err(e) = self.context.hide_cursor() {
            print!("Warning: Failed to hide cursor: {}\r\n", e);
        }
        
        // Fonts are now created on-demand when needed
        
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

            // Read sensor values
            self.sensor_manager.read_all_sensors()?;
            
            // Render current page
            self.render_current_page()?;

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
            UIEvent::SwitchToPage(page_id) => {
                if let Err(e) = self.switch_page(page_id) {
                    print!("Failed to switch to page {}: {}\r\n", page_id, e);
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
            _ => {}
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
                let text_width = self.context.calculate_text_width_with_font(
                    label, 
                    label_scale,
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                    16
                )?;
                x - text_width
            }
            // Left side buttons are left-aligned
            _ => x,
        };
        
        self.context.render_text_with_font(
            label, 
            render_x, 
            y, 
            label_scale, 
            label_color,
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            16
        )?;
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
        
        self.context.render_text_with_font(
            &status_text,
            status_x,
            status_y,
            1.0, // scale
            (0.7, 0.7, 0.7), // gray color
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            14 // smaller font for status
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
