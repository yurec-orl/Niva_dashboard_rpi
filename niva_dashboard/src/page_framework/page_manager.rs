use crate::graphics::context::GraphicsContext;
use crate::page_framework::input::{InputHandler, ButtonState};
use crate::page_framework::main_page::MainPage;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::rc::Rc;
use std::cell::RefCell;

// ButtonPosition correspond to physical 2x4 buttons layout.
#[derive(Debug, PartialEq, Eq, Hash)]
enum ButtonPosition {
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
    label: String,
    callback: CB,
}

impl<CB> PageButton<CB>
where
    CB: FnMut(),
{
    pub fn new(pos: ButtonPosition, label: String, callback: CB) -> Self {
        PageButton { pos, label, callback }
    }

    pub fn trigger(&mut self) {
        (self.callback)();
    }

    pub fn position(&self) -> &ButtonPosition {
        &self.pos
    }
}

pub struct PageBase {
    id: u32,
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

    pub fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.buttons = buttons;
    }

    pub fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        &self.buttons
    }
}

pub trait Page {
    fn render(&self, context: &mut GraphicsContext) -> Result<(), String>;
    fn on_enter(&mut self) -> Result<(), String>;
    fn on_exit(&mut self) -> Result<(), String>;
    fn on_button(&mut self, button: char) -> Result<(), String>;
    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>>;
}

pub struct PageManager {
    context: GraphicsContext,
    pg_id: u32,
    current_page: Option<Rc<RefCell<dyn Page>>>, // Shared reference to current page
    pages: Vec<Rc<RefCell<dyn Page>>>, // Store shared references
    button_keymap: HashMap<char, usize>,
    input_handler: InputHandler,
    fps_counter: FpsCounter,
    start_time: Instant,
    running: bool,
}

impl PageManager {
    pub fn new(context: GraphicsContext) -> Self {
        PageManager {
            context,
            pg_id: 0,
            current_page: None,
            pages: Vec::new(),
            button_keymap: HashMap::new(),
            input_handler: InputHandler::new(),
            fps_counter: FpsCounter::new(),
            start_time: Instant::now(),
            running: false,
        }
    }

    pub fn setup(&mut self) -> Result<(), String> {
        let main_page = MainPage::new(self.get_page_id(), "Main".into());
        let _main_buttons = {
            vec![
                PageButton::new(ButtonPosition::Left1, "Button 1".into(), Box::new(|| {
                    print!("Button 1 pressed\r\n");
                }) as Box<dyn FnMut()>),
                PageButton::new(ButtonPosition::Left4, "Button 2".into(), Box::new(|| {
                    print!("Button 2 pressed\r\n");
                }) as Box<dyn FnMut()>),
            ]
        };
        
        // Add the page and switch to it using shared reference
        let main_page_ref = self.add_page(main_page);
        self.switch_page(main_page_ref)?;

        Ok(())
    }

    fn get_page_id(&mut self) -> u32 {
        let id = self.pg_id;
        self.pg_id += 1;
        id
    }

    pub fn add_page<T: Page + 'static>(&mut self, page: T) -> Rc<RefCell<dyn Page>> {
        let shared_page = Rc::new(RefCell::new(page));
        self.pages.push(shared_page.clone());
        shared_page // Return the shared reference
    }

    /// Add a page from an already created Rc<RefCell<dyn Page>>
    pub fn add_shared_page(&mut self, page: Rc<RefCell<dyn Page>>) -> Rc<RefCell<dyn Page>> {
        self.pages.push(page.clone());
        page
    }

    pub fn switch_page(&mut self, page: Rc<RefCell<dyn Page>>) -> Result<(), String> {
        self.current_page = Some(page);
        self.button_keymap = self.create_button_keymap();
        Ok(())
    }

    /// Get the current page reference
    pub fn current_page(&self) -> Option<Rc<RefCell<dyn Page>>> {
        self.current_page.clone()
    }

    pub fn start(&mut self) -> Result<(), String> {
        // Hide mouse cursor for dashboard
        if let Err(e) = self.context.hide_cursor() {
            eprintln!("Warning: Failed to hide cursor: {}", e);
        }
        
        // Initialize text renderer in the graphics context
        self.context.initialize_text_renderer("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 16)?;
        
        print!("Dashboard initialized successfully!\r\n");
        self.running = true;
        self.start_time = Instant::now();
        self.event_loop()
    }

    fn create_button_keymap(&self) -> HashMap<char, usize> {
        let mut button_map = HashMap::new();

        let mut key_position_map = HashMap::new();
        key_position_map.insert(ButtonPosition::Left1, '1');
        key_position_map.insert(ButtonPosition::Left2, '2');
        key_position_map.insert(ButtonPosition::Left3, '3');
        key_position_map.insert(ButtonPosition::Left4, '4');
        key_position_map.insert(ButtonPosition::Right1, '5');
        key_position_map.insert(ButtonPosition::Right2, '6');
        key_position_map.insert(ButtonPosition::Right3, '7');
        key_position_map.insert(ButtonPosition::Right4, '8');

        if let Some(current_page) = &self.current_page {
            let page_ref = current_page.borrow();
            let buttons = page_ref.buttons();
            for (index, button) in buttons.iter().enumerate() {
                if let Some(key) = key_position_map.get(button.position()) {
                    button_map.insert(*key, index);
                }
            }
        }

        button_map
    }

    fn event_loop(&mut self) -> Result<(), String> {
        const TARGET_FPS: u64 = 60;
        const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

        let button_keymap = self.create_button_keymap();

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
            if let Some(current_page) = &self.current_page {
                let page_ref = current_page.borrow();
                let _ = page_ref.render(&mut self.context);
            }
            
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
        let status_y = self.context.height as f32 - 25.0; // 25 pixels from bottom
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
