use crate::graphics::context::GraphicsContext;
use crate::graphics::opengl_test::OpenGLTextRenderer;
use std::time::{Duration, Instant};

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

struct PageButton<CB> {
    label: String,
    callback: CB,
}

impl<CB> PageButton<CB>
where
    CB: FnMut(),
{
    pub fn new(label: String, callback: CB) -> Self {
        PageButton { label, callback }
    }

    pub fn trigger(&mut self) {
        (self.callback)();
    }
}

pub struct PageBase {
    id: u32,
    name: String,
    buttons: Vec<PageButton<Box<dyn FnMut()>>>,
}

pub trait Page {
    fn render(&self, context: &mut GraphicsContext);
}

pub struct PageManager {
    context: GraphicsContext,
    pages: Vec<Box<dyn Page>>,
    text_renderer: Option<OpenGLTextRenderer>,
    fps_counter: FpsCounter,
    start_time: Instant,
    running: bool,
}

impl PageManager {
    pub fn new(context: GraphicsContext) -> Self {
        PageManager { 
            context, 
            pages: Vec::new(),
            text_renderer: None,
            fps_counter: FpsCounter::new(),
            start_time: Instant::now(),
            running: false,
        }
    }

    pub fn add_page(&mut self, page: Box<dyn Page>) {
        self.pages.push(page);
    }

    pub fn start(&mut self) -> Result<(), String> {
        // Hide mouse cursor for dashboard
        if let Err(e) = self.context.hide_cursor() {
            eprintln!("Warning: Failed to hide cursor: {}", e);
        }
        
        // Initialize text renderer
        unsafe {
            self.text_renderer = Some(OpenGLTextRenderer::new("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 16)?);
        }
        
        println!("Dashboard initialized successfully!");
        self.running = true;
        self.start_time = Instant::now();
        self.event_loop()
    }

    fn event_loop(&mut self) -> Result<(), String> {
        const TARGET_FPS: u64 = 60;
        const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);
        
        println!("Starting event loop (target: {} FPS)", TARGET_FPS);
        
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
            
            // Render all pages
            for page in &self.pages {
                page.render(&mut self.context);
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

            // Exit condition (for now, run for 30 seconds)
            if self.start_time.elapsed() > Duration::from_secs(30) {
                self.running = false;
            }
        }
        
        println!("Event loop finished");
        
        // Clean up text renderer before context cleanup
        self.text_renderer.take();
        
        Ok(())
    }
    
    fn render_status_line(&mut self) -> Result<(), String> {
        if let Some(text_renderer) = &mut self.text_renderer {
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
            
            unsafe {
                text_renderer.render_text(
                    &status_text,
                    status_x,
                    status_y,
                    1.0, // scale
                    (0.7, 0.7, 0.7), // gray color
                    self.context.width as f32,
                    self.context.height as f32,
                )?;
            }
        }
        
        Ok(())
    }
    
    pub fn stop(&mut self) {
        self.running = false;
        // Clean up text renderer before context is dropped
        self.text_renderer.take();
    }
}