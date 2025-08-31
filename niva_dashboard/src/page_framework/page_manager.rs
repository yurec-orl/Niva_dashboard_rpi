use crate::graphics::context::GraphicsContext;
use crate::page_framework::input::{InputHandler, ButtonState};
use crate::page_framework::main_page::MainPage;
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

        PageManager {
            context,
            pg_id: 0,
            current_page: None,
            pages: Vec::new(),
            input_handler: InputHandler::new(),
            buttons_map,
            fps_counter: FpsCounter::new(),
            start_time: Instant::now(),
            running: false,
        }
    }

    // Set up pages and buttons.
    pub fn setup(&mut self) -> Result<(), String> {
        let mut main_page = Box::new(MainPage::new(self.get_page_mut_id(), "Main".into()));
        let main_buttons = vec![
            PageButton::new(ButtonPosition::Left1, "Button 1".into(), Box::new(|| {
                print!("Button 1 pressed\r\n");
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "Button 2".into(), Box::new(|| {
                print!("Button 2 pressed\r\n");
            }) as Box<dyn FnMut()>),
        ];
        
        main_page.set_buttons(main_buttons);
        
        let main_page_idx = self.add_page(main_page);
        self.switch_page(main_page_idx)?;

        Ok(())
    }

    fn get_page_mut_id(&mut self) -> u32 {
        let id = self.pg_id;
        self.pg_id += 1;
        id
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
            
            // Clear screen
            unsafe {
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
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

            // Exit condition (for now, run for 30 seconds)
            if self.start_time.elapsed() > Duration::from_secs(10) {
                self.running = false;
            }
        }
        
        print!("Event loop finished\r\n");
        
        Ok(())
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
