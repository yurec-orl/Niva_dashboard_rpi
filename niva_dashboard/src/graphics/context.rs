// Graphics context manager for shared SDL2/OpenGL resources
use std::ffi::CString;

// SDL2 and OpenGL ES bindings (reused from opengl_test.rs)
extern "C" {
    // SDL2 functions
    fn SDL_Init(flags: u32) -> i32;
    fn SDL_CreateWindow(title: *const std::ffi::c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut std::ffi::c_void;
    fn SDL_GL_CreateContext(window: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn SDL_GL_SetAttribute(attr: i32, value: i32) -> i32;
    fn SDL_GL_SwapWindow(window: *mut std::ffi::c_void);
    fn SDL_PollEvent(event: *mut SDL_Event) -> i32;
    fn SDL_Quit();
    fn SDL_DestroyWindow(window: *mut std::ffi::c_void);
    fn SDL_GL_DeleteContext(context: *mut std::ffi::c_void);
    fn glViewport(x: i32, y: i32, width: i32, height: i32);
}

// SDL2 constants
const SDL_INIT_VIDEO: u32 = 0x00000020;
const SDL_WINDOW_OPENGL: u32 = 0x00000002;
const SDL_WINDOWPOS_CENTERED: i32 = 0x2FFF0000;

// SDL2 GL attributes
const SDL_GL_CONTEXT_MAJOR_VERSION: i32 = 17;
const SDL_GL_CONTEXT_MINOR_VERSION: i32 = 18;
const SDL_GL_CONTEXT_PROFILE_MASK: i32 = 21;
const SDL_GL_CONTEXT_PROFILE_ES: i32 = 4;

const SDL_QUIT: u32 = 0x100;

// SDL Event structure (simplified)
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct SDL_Event {
    pub type_: u32,
    pub padding: [u8; 52], // Simplified event structure
}

/// Shared graphics context for all dashboard tests
pub struct GraphicsContext {
    pub window: *mut std::ffi::c_void,
    pub gl_context: *mut std::ffi::c_void,
    pub width: i32,
    pub height: i32,
    pub sdl_initialized: bool,
}

impl GraphicsContext {
    /// Create a new graphics context with the specified window title and dimensions
    pub fn new(title: &str, width: i32, height: i32) -> Result<Self, String> {
        unsafe {
            // Initialize SDL2 if not already done
            if SDL_Init(SDL_INIT_VIDEO) < 0 {
                return Err("Failed to initialize SDL2".to_string());
            }
            
            // Set OpenGL ES attributes
            SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 2);
            SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 0);
            SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_ES);
            
            // Create window
            let window_title = CString::new(title).unwrap();
            let window = SDL_CreateWindow(
                window_title.as_ptr(),
                SDL_WINDOWPOS_CENTERED,
                SDL_WINDOWPOS_CENTERED,
                width,
                height,
                SDL_WINDOW_OPENGL,
            );
            
            if window.is_null() {
                SDL_Quit();
                return Err("Failed to create SDL2 window".to_string());
            }
            
            // Create OpenGL context
            let gl_context = SDL_GL_CreateContext(window);
            if gl_context.is_null() {
                SDL_DestroyWindow(window);
                SDL_Quit();
                return Err("Failed to create OpenGL context".to_string());
            }
            
            // Set viewport
            glViewport(0, 0, width, height);
            
            println!("Graphics context created successfully: {}x{}", width, height);
            
            Ok(GraphicsContext {
                window,
                gl_context,
                width,
                height,
                sdl_initialized: true,
            })
        }
    }
    
    /// Create a context specifically for dashboard applications (800x480)
    pub fn new_dashboard(title: &str) -> Result<Self, String> {
        Self::new(title, 800, 480)
    }
    
    /// Swap the front and back buffers
    pub fn swap_buffers(&self) {
        unsafe {
            SDL_GL_SwapWindow(self.window);
        }
    }
    
    /// Poll for SDL events
    pub fn poll_events(&self) -> Vec<SDL_Event> {
        let mut events = Vec::new();
        unsafe {
            let mut event = SDL_Event { type_: 0, padding: [0; 52] };
            while SDL_PollEvent(&mut event) != 0 {
                events.push(event);
                event = SDL_Event { type_: 0, padding: [0; 52] };
            }
        }
        events
    }
    
    /// Check if a quit event was received
    pub fn should_quit(&self) -> bool {
        let events = self.poll_events();
        events.iter().any(|event| event.type_ == SDL_QUIT)
    }

}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        unsafe {
            if !self.gl_context.is_null() {
                SDL_GL_DeleteContext(self.gl_context);
            }
            if !self.window.is_null() {
                SDL_DestroyWindow(self.window);
            }
            if self.sdl_initialized {
                SDL_Quit();
            }
        }
        println!("Graphics context cleaned up");
    }
}
