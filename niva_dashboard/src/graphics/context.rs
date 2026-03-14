// Graphics context manager for KMS/DRM OpenGL ES backend
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;
use std::collections::HashMap;
use freetype_sys as ft;
use crate::graphics::ui_style::UIStyle;

// EGL types and constants
type EGLDisplay = *mut c_void;
type EGLContext = *mut c_void;
type EGLSurface = *mut c_void;
type EGLConfig = *mut c_void;
type EGLint = c_int;
type EGLBoolean = c_uint;

const EGL_SUCCESS: EGLint = 0x3000;
const EGL_TRUE: EGLBoolean = 1;
const EGL_FALSE: EGLBoolean = 0;
const EGL_DEFAULT_DISPLAY: *mut c_void = ptr::null_mut();
const EGL_NO_CONTEXT: EGLContext = ptr::null_mut();
const EGL_NO_SURFACE: EGLSurface = ptr::null_mut();

// EGL configuration attributes
const EGL_SURFACE_TYPE: EGLint = 0x3033;
const EGL_WINDOW_BIT: EGLint = 0x0004;
const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
const EGL_OPENGL_ES2_BIT: EGLint = 0x0004;
const EGL_RED_SIZE: EGLint = 0x3024;
const EGL_GREEN_SIZE: EGLint = 0x3023;
const EGL_BLUE_SIZE: EGLint = 0x3022;
const EGL_ALPHA_SIZE: EGLint = 0x3021;
const EGL_DEPTH_SIZE: EGLint = 0x3025;
const EGL_NONE: EGLint = 0x3038;

// EGL context attributes
const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;

// EGL platform constants
const EGL_PLATFORM_GBM_MESA: EGLint = 0x31D7;

// EGL/OpenGL ES external functions
#[repr(C)]
union GbmBoHandle {
    ptr: *mut c_void,
    s32: i32,
    u32: u32,
    s64: i64,
    u64: u64,
}

extern "C" {
    // EGL functions
    fn eglGetDisplay(display_id: *mut c_void) -> EGLDisplay;
    fn eglGetPlatformDisplay(platform: EGLint, native_display: *mut c_void, attrib_list: *const EGLint) -> EGLDisplay;
    fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    fn eglTerminate(dpy: EGLDisplay) -> EGLBoolean;
    fn eglChooseConfig(
        dpy: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean;
    fn eglCreateContext(
        dpy: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    fn eglCreateWindowSurface(
        dpy: EGLDisplay,
        config: EGLConfig,
        win: *mut c_void,
        attrib_list: *const EGLint,
    ) -> EGLSurface;
    fn eglMakeCurrent(
        dpy: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        ctx: EGLContext,
    ) -> EGLBoolean;
    fn eglSwapBuffers(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    fn eglSwapInterval(dpy: EGLDisplay, interval: EGLint) -> EGLBoolean;
    fn eglDestroySurface(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
    fn eglGetError() -> EGLint;
    fn eglGetProcAddress(procname: *const c_char) -> *mut c_void;
    
    // OpenGL ES functions
    fn glViewport(x: c_int, y: c_int, width: c_int, height: c_int);
    fn glClearColor(red: f32, green: f32, blue: f32, alpha: f32);
    fn glClear(mask: c_uint);
    
    // DRM functions
    fn drmOpen(name: *const c_char, busid: *const c_char) -> c_int;
    fn drmClose(fd: c_int) -> c_int;
    fn drmModeGetResources(fd: c_int) -> *mut c_void;
    fn drmModeFreeResources(ptr: *mut c_void);
    fn drmModeGetConnector(fd: c_int, connector_id: u32) -> *mut c_void;
    fn drmModeFreeConnector(ptr: *mut c_void);
    fn drmModeGetEncoder(fd: c_int, encoder_id: u32) -> *mut c_void;
    fn drmModeFreeEncoder(ptr: *mut c_void);
    fn drmModeGetCrtc(fd: c_int, crtc_id: u32) -> *mut c_void;
    fn drmModeFreeCrtc(ptr: *mut c_void);
    fn drmModeSetCrtc(
        fd: c_int,
        crtc_id: u32,
        buffer_id: u32,
        x: u32,
        y: u32,
        connectors: *mut u32,
        count: c_int,
        mode: *mut DrmModeModeInfo,
    ) -> c_int;
    fn drmModeAddFB(
        fd: c_int,
        width: u32,
        height: u32,
        depth: u8,
        bpp: u8,
        pitch: u32,
        bo_handle: u32,
        buf_id: *mut u32,
    ) -> c_int;
    fn drmModeRmFB(fd: c_int, bufferId: u32) -> c_int;
    fn drmModePageFlip(
        fd: c_int,
        crtc_id: u32,
        fb_id: u32,
        flags: u32,
        user_data: *mut c_void,
    ) -> c_int;
    
    // GBM functions
    fn gbm_create_device(fd: c_int) -> *mut c_void;
    fn gbm_device_destroy(gbm: *mut c_void);
    fn gbm_surface_create(
        gbm: *mut c_void,
        width: u32,
        height: u32,
        format: u32,
        flags: u32,
    ) -> *mut c_void;
    fn gbm_surface_destroy(surface: *mut c_void);
    fn gbm_surface_lock_front_buffer(surface: *mut c_void) -> *mut c_void;
    fn gbm_surface_release_buffer(surface: *mut c_void, buffer: *mut c_void);
    fn gbm_bo_get_handle(bo: *mut c_void) -> GbmBoHandle;
    fn gbm_bo_get_stride(bo: *mut c_void) -> u32;
}

// OpenGL constants
const GL_COLOR_BUFFER_BIT: c_uint = 0x00004000;

// GBM constants
const GBM_FORMAT_XRGB8888: u32 = 0x34325258;
const GBM_BO_USE_SCANOUT: u32 = 1 << 0;
const GBM_BO_USE_RENDERING: u32 = 1 << 2;

// DRM connector states
const DRM_MODE_CONNECTED: u32 = 1;

// DRM page flip flags
const DRM_MODE_PAGE_FLIP_EVENT: u32 = 0x01;

// Basic DRM structures (simplified)
#[repr(C)]
#[derive(Clone, Copy)]
struct DrmModeRes {
    count_fbs: i32,
    fbs: *mut u32,
    count_crtcs: i32,
    crtcs: *mut u32,
    count_connectors: i32,
    connectors: *mut u32,
    count_encoders: i32,
    encoders: *mut u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DrmModeConnector {
    connector_id: u32,
    encoder_id: u32,
    connector_type: u32,
    connector_type_id: u32,
    connection: u32,
    mm_width: u32,
    mm_height: u32,
    subpixel: u32,
    count_modes: i32,
    modes: *mut DrmModeModeInfo,
    count_props: i32,
    props: *mut u32,
    prop_values: *mut u64,
    count_encoders: i32,
    encoders: *mut u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DrmModeEncoder {
    encoder_id: u32,
    encoder_type: u32,
    crtc_id: u32,
    possible_crtcs: u32,
    possible_clones: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DrmModeModeInfo {
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,
    vrefresh: u32,
    flags: u32,
    type_: u32,
    name: [i8; 32],
}

/// Represents cached glyph data for efficient text rendering
#[derive(Clone)]
struct CachedGlyph {
    texture_id: u32,
    width: f32,
    height: f32,
    bearing_x: f32,
    bearing_y: f32,
    advance: f32,
}

/// OpenGL text renderer using FreeType with glyph caching
pub struct OpenGLTextRenderer {
    ft_library: ft::FT_Library,
    ft_face: ft::FT_Face,
    shader_program: u32,
    vao: u32,
    vbo: u32,
    font_size: u32,
    glyph_cache: HashMap<char, CachedGlyph>,
    projection_width: f32,
    projection_height: f32,
    projection_matrix: [f32; 16],
    // Cached uniform and attribute locations for performance
    projection_uniform: i32,
    color_uniform: i32,
    texture_uniform: i32,
    vertex_attr: i32,
}

/// Event structure for input handling
#[derive(Debug, Clone)]
pub struct InputEvent {
    pub event_type: InputEventType,
}

#[derive(Debug, Clone)]
pub enum InputEventType {
    Quit,
    KeyPress(u32),
    KeyRelease(u32),
}

/// Text orientation options for rendering
#[derive(Debug, Clone, Copy)]
pub enum TextOrientation {
    Horizontal,  // Normal left-to-right text
    Vertical,    // Characters stacked vertically (top-to-bottom, not rotated)
}

/// Graphics context using KMS/DRM backend with OpenGL ES
pub struct GraphicsContext {
    // DRM/KMS handles
    drm_fd: c_int,
    gbm_device: *mut c_void,
    gbm_surface: *mut c_void,
    
    // EGL handles
    egl_display: EGLDisplay,
    egl_context: EGLContext,
    egl_surface: EGLSurface,
    egl_config: EGLConfig,
    
    // Display configuration
    connector_id: u32,
    crtc_id: u32,
    mode: DrmModeModeInfo,
    previous_crtc: *mut c_void,
    
    // Framebuffer management
    current_fb: u32,
    previous_fb: u32,
    
    // Display properties
    pub width: i32,
    pub height: i32,
    
    // Text rendering - font management with HashMap
    pub text_renderers: HashMap<String, OpenGLTextRenderer>,
    
    // UI style with brightness control and theming
    pub ui_style: UIStyle,
    
    // Cached shader programs for performance
    rectangle_shader: Option<u32>,
    
    // Bloom post-processing effect
    bloom_enabled: bool,
    bloom_intensity: f32,
    bloom_threshold: f32,
    bloom_framebuffer: Option<u32>,
    bloom_texture: Option<u32>,
    bloom_shader: Option<u32>,
    
    // State
    initialized: bool,
    display_configured: bool,
}

impl GraphicsContext {
    /// Create a new graphics context with KMS/DRM backend
    pub fn new(title: &str, width: i32, height: i32) -> Result<Self, String> {
        let mut context = GraphicsContext {
            drm_fd: -1,
            gbm_device: ptr::null_mut(),
            gbm_surface: ptr::null_mut(),
            egl_display: ptr::null_mut(),
            egl_context: EGL_NO_CONTEXT,
            egl_surface: EGL_NO_SURFACE,
            egl_config: ptr::null_mut(),
            connector_id: 0,
            crtc_id: 0,
            mode: unsafe { std::mem::zeroed() },
            previous_crtc: ptr::null_mut(),
            current_fb: 0,
            previous_fb: 0,
            width,
            height,
            text_renderers: HashMap::new(),
            ui_style: UIStyle::new(),
            rectangle_shader: None,
            bloom_enabled: true,
            bloom_intensity: 0.5,  // Increased for more visible glow
            bloom_threshold: 0.3,  // Lowered to catch more bright pixels
            bloom_framebuffer: None,
            bloom_texture: None,
            bloom_shader: None,
            initialized: false,
            display_configured: false,
        };

        // Load OpenGL function pointers
        gl::load_with(|name| {
            let c_str = std::ffi::CString::new(name).unwrap();
            context.get_proc_address(c_str.as_ptr()) as *const _
        });
        
        print!("Initializing KMS/DRM graphics context: {} ({}x{})\r\n", title, width, height);
        print!("Setting up direct display output...\r\n");
        
        // Initialize DRM
        context.init_drm()?;
        
        // Set up display mode
        context.setup_display()?;
        
        // Initialize GBM with display dimensions
        context.init_gbm()?;
        
        // Initialize EGL
        context.init_egl()?;
        
        // Note: Display will be configured on first swap_buffers call
        
        // Set up OpenGL viewport
        unsafe {
            glViewport(0, 0, context.width, context.height);
            glClearColor(0.0, 0.0, 0.0, 1.0);
        }
        
        // Initialize bloom effect
        if let Err(e) = context.init_bloom() {
            print!("Warning: Failed to initialize bloom effect: {}\r\n", e);
            context.bloom_enabled = false;
        }
        
        context.initialized = true;
        print!("Graphics context initialized successfully: {}x{}\r\n", context.width, context.height);
        print!("✓ Display setup complete - output should be visible on screen\r\n");
        print!("  Resolution: {}x{}@{}Hz\r\n", context.width, context.height, context.mode.vrefresh);
        print!("  CRTC: {}, Connector: {}\r\n", context.crtc_id, context.connector_id);
        
        Ok(context)
    }
    
    /// Create a context specifically for dashboard applications (800x480)
    pub fn new_dashboard(title: &str) -> Result<Self, String> {
        Self::new(title, 800, 480)
    }
    
    /// Initialize DRM (Direct Rendering Manager)
    fn init_drm(&mut self) -> Result<(), String> {
        unsafe {
            // Try to open the primary DRM device
            let card_name = CString::new("card0").unwrap();
            self.drm_fd = drmOpen(card_name.as_ptr(), ptr::null());
            
            if self.drm_fd < 0 {
                // Fallback to vc4 driver for Raspberry Pi
                let vc4_name = CString::new("vc4").unwrap();
                self.drm_fd = drmOpen(vc4_name.as_ptr(), ptr::null());
                
                if self.drm_fd < 0 {
                    return Err("Failed to open DRM device. Make sure you have access to /dev/dri/card* devices.".to_string());
                }
            }
            
            print!("DRM device opened successfully (fd: {})\r\n", self.drm_fd);
            
            // Get DRM resources to check display configuration
            let resources = drmModeGetResources(self.drm_fd);
            if !resources.is_null() {
                let res = &*(resources as *const DrmModeRes);
                print!("DRM Resources found:\r\n");
                print!("  CRTCs: {}\r\n", res.count_crtcs);
                print!("  Connectors: {}\r\n", res.count_connectors);
                print!("  Encoders: {}\r\n", res.count_encoders);
                
                if res.count_connectors > 0 {
                    print!("  Display appears to be available\r\n");
                } else {
                    print!("  Warning: No display connectors found\r\n");
                }
                
                drmModeFreeResources(resources);
            } else {
                print!("Warning: Could not get DRM resources\r\n");
            }
        }
        
        Ok(())
    }
    
    /// Find and configure display mode
    fn setup_display(&mut self) -> Result<(), String> {
        unsafe {
            let resources = drmModeGetResources(self.drm_fd);
            if resources.is_null() {
                return Err("Failed to get DRM resources".to_string());
            }
            
            let res = &*(resources as *const DrmModeRes);
            print!("Setting up display mode...\r\n");
            print!("Available CRTCs: {}, Connectors: {}\r\n", res.count_crtcs, res.count_connectors);
            
            // Find a connected display
            let mut found_display = false;
            for i in 0..res.count_connectors {
                let connector_id = *res.connectors.offset(i as isize);
                let connector = drmModeGetConnector(self.drm_fd, connector_id);
                
                if !connector.is_null() {
                    let conn = &*(connector as *const DrmModeConnector);
                    
                    if conn.connection == DRM_MODE_CONNECTED && conn.count_modes > 0 {
                        print!("Found connected display on connector {}\r\n", connector_id);
                        
                        // Use the first mode (usually the preferred mode)
                        let mode = &*conn.modes;
                        self.mode = *mode;
                        self.connector_id = connector_id;
                        
                        // Find encoder and CRTC
                        if conn.encoder_id != 0 {
                            let encoder = drmModeGetEncoder(self.drm_fd, conn.encoder_id);
                            if !encoder.is_null() {
                                let enc = &*(encoder as *const DrmModeEncoder);
                                self.crtc_id = enc.crtc_id;
                                drmModeFreeEncoder(encoder);
                            }
                        }
                        
                        // If no CRTC found, use the first available one
                        if self.crtc_id == 0 && res.count_crtcs > 0 {
                            self.crtc_id = *res.crtcs;
                        }
                        
                        // Save current CRTC configuration for restoration
                        self.previous_crtc = drmModeGetCrtc(self.drm_fd, self.crtc_id);
                        
                        print!("Display mode: {}x{}@{}Hz\r\n", 
                                mode.hdisplay, mode.vdisplay, mode.vrefresh);
                        print!("Using CRTC: {}, Connector: {}\r\n", self.crtc_id, self.connector_id);
                        
                        // Update dimensions to match display mode
                        self.width = mode.hdisplay as i32;
                        self.height = mode.vdisplay as i32;
                        
                        found_display = true;
                        drmModeFreeConnector(connector);
                        break;
                    }
                    
                    drmModeFreeConnector(connector);
                }
            }
            
            drmModeFreeResources(resources);
            
            if !found_display {
                return Err("No connected display found".to_string());
            }
        }
        
        Ok(())
    }
    
    /// Initialize GBM (Generic Buffer Management)
    fn init_gbm(&mut self) -> Result<(), String> {
        unsafe {
            // Create GBM device
            self.gbm_device = gbm_create_device(self.drm_fd);
            if self.gbm_device.is_null() {
                return Err("Failed to create GBM device".to_string());
            }
            
            // Create GBM surface
            self.gbm_surface = gbm_surface_create(
                self.gbm_device,
                self.width as u32,
                self.height as u32,
                GBM_FORMAT_XRGB8888,
                GBM_BO_USE_SCANOUT | GBM_BO_USE_RENDERING,
            );
            
            if self.gbm_surface.is_null() {
                return Err("Failed to create GBM surface".to_string());
            }
            
            print!("GBM device and surface created successfully\r\n");
        }
        
        Ok(())
    }
    
    /// Initialize EGL (Embedded-System Graphics Library)
    fn init_egl(&mut self) -> Result<(), String> {
        unsafe {
            // Try to get platform display first (preferred method)
            self.egl_display = eglGetPlatformDisplay(EGL_PLATFORM_GBM_MESA, self.gbm_device, ptr::null());
            if self.egl_display.is_null() {
                // Fallback to traditional method
                self.egl_display = eglGetDisplay(self.gbm_device);
                if self.egl_display.is_null() {
                    return Err("Failed to get EGL display".to_string());
                }
            }
            
            // Initialize EGL
            let mut major = 0;
            let mut minor = 0;
            if eglInitialize(self.egl_display, &mut major, &mut minor) == EGL_FALSE {
                return Err(format!("Failed to initialize EGL: error {}", eglGetError()));
            }
            
            print!("EGL initialized: version {}.{}\r\n", major, minor);
            
            // Choose EGL configuration
            let config_attribs = [
                EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
                EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
                EGL_RED_SIZE, 8,
                EGL_GREEN_SIZE, 8,
                EGL_BLUE_SIZE, 8,
                EGL_ALPHA_SIZE, 8,
                EGL_DEPTH_SIZE, 16,
                EGL_NONE,
            ];
            
            let mut config = ptr::null_mut();
            let mut num_configs = 0;
            
            if eglChooseConfig(
                self.egl_display,
                config_attribs.as_ptr(),
                &mut config,
                1,
                &mut num_configs,
            ) == EGL_FALSE || num_configs == 0 {
                return Err("Failed to choose EGL config".to_string());
            }
            
            self.egl_config = config;
            
            // Create EGL context
            let context_attribs = [
                EGL_CONTEXT_CLIENT_VERSION, 2,
                EGL_NONE,
            ];
            
            self.egl_context = eglCreateContext(
                self.egl_display,
                self.egl_config,
                EGL_NO_CONTEXT,
                context_attribs.as_ptr(),
            );
            
            if self.egl_context == EGL_NO_CONTEXT {
                return Err("Failed to create EGL context".to_string());
            }
            
            // Create EGL surface
            self.egl_surface = eglCreateWindowSurface(
                self.egl_display,
                self.egl_config,
                self.gbm_surface,
                ptr::null(),
            );
            
            if self.egl_surface == EGL_NO_SURFACE {
                return Err("Failed to create EGL surface".to_string());
            }
            
            // Make context current
            if eglMakeCurrent(
                self.egl_display,
                self.egl_surface,
                self.egl_surface,
                self.egl_context,
            ) == EGL_FALSE {
                return Err("Failed to make EGL context current".to_string());
            }
            
            // Enable vsync to prevent tearing
            eglSwapInterval(self.egl_display, 1);
            
            print!("EGL context created and made current\r\n");
        }
        
        Ok(())
    }
    
    /// Configure the display to show our framebuffer
    fn configure_display(&mut self) -> Result<(), String> {
        unsafe {
            print!("Configuring display output...\r\n");
            
            // Get the initial front buffer to set up the display
            let bo = gbm_surface_lock_front_buffer(self.gbm_surface);
            if bo.is_null() {
                return Err("Failed to lock front buffer for display setup".to_string());
            }
            
            // Get buffer properties
            let handle = gbm_bo_get_handle(bo).u32;
            let stride = gbm_bo_get_stride(bo);
            print!("Buffer handle: {}, stride: {}\r\n", handle, stride);
            
            // Create DRM framebuffer
            let mut fb_id = 0;
            let result = drmModeAddFB(
                self.drm_fd,
                self.width as u32,
                self.height as u32,
                24, // depth
                32, // bpp
                stride,
                handle,
                &mut fb_id,
            );
            
            if result != 0 {
                gbm_surface_release_buffer(self.gbm_surface, bo);
                return Err(format!("Failed to create framebuffer: error {}", result));
            }
            
            print!("Created framebuffer: {}\r\n", fb_id);
            self.current_fb = fb_id;
            
            // Set the CRTC to display our framebuffer
            let mut connector_id = self.connector_id;
            let mut mode = self.mode;
            let result = drmModeSetCrtc(
                self.drm_fd,
                self.crtc_id,
                fb_id,
                0, // x
                0, // y
                &mut connector_id,
                1, // connector count
                &mut mode,
            );
            
            if result != 0 {
                drmModeRmFB(self.drm_fd, fb_id);
                gbm_surface_release_buffer(self.gbm_surface, bo);
                return Err(format!("Failed to set CRTC: error {}", result));
            }
            
            print!("✓ Display CRTC configured - framebuffer {} is now showing\r\n", fb_id);
            
            // Release the buffer back to GBM
            gbm_surface_release_buffer(self.gbm_surface, bo);
        }
        
        Ok(())
    }
    
    /// Swap the front and back buffers and update display
    pub fn swap_buffers(&mut self) {
        unsafe {
            if self.initialized {
                // Swap the EGL buffers first to render content
                let result = eglSwapBuffers(self.egl_display, self.egl_surface);
                if result == EGL_FALSE {
                    let error = eglGetError();
                    print!("Warning: eglSwapBuffers failed with error: 0x{:X}\r\n", error);
                    return;
                }
                
                // For the first frame only, set up initial display
                if !self.display_configured {
                    self.display_configured = true;
                    
                    match self.configure_display() {
                        Ok(_) => {
                            print!("✓ Display configured successfully after first swap\r\n");
                        },
                        Err(e) => {
                            print!("Warning: Failed to configure display: {}\r\n", e);
                            print!("Continuing with off-screen rendering...\r\n");
                        }
                    }
                } else {
                    // For subsequent frames, use page flipping for smooth updates
                    self.page_flip_display();
                }
            }
        }
    }
    
    /// Handle page flipping for smooth double buffering
    fn page_flip_display(&mut self) {
        unsafe {
            // Get the current front buffer from GBM
            let bo = gbm_surface_lock_front_buffer(self.gbm_surface);
            if bo.is_null() {
                return; // Skip this frame if buffer isn't ready
            }
            
            // Get buffer properties
            let handle = gbm_bo_get_handle(bo).u32;
            let stride = gbm_bo_get_stride(bo);
            
            // Create a new framebuffer for this buffer
            let mut new_fb_id = 0;
            let result = drmModeAddFB(
                self.drm_fd,
                self.width as u32,
                self.height as u32,
                24, // depth
                32, // bpp
                stride,
                handle,
                &mut new_fb_id,
            );
            
            if result == 0 {
                // Try page flip first (smooth, async)
                let flip_result = drmModePageFlip(
                    self.drm_fd,
                    self.crtc_id,
                    new_fb_id,
                    DRM_MODE_PAGE_FLIP_EVENT,
                    ptr::null_mut(),
                );
                
                if flip_result == 0 {
                    // Page flip successful - clean up old framebuffer
                    if self.previous_fb != 0 {
                        drmModeRmFB(self.drm_fd, self.previous_fb);
                    }
                    self.previous_fb = self.current_fb;
                    self.current_fb = new_fb_id;
                } else {
                    // Page flip failed - fallback to immediate mode set (might flicker)
                    let mut connector_id = self.connector_id;
                    let mut mode = self.mode;
                    let crtc_result = drmModeSetCrtc(
                        self.drm_fd,
                        self.crtc_id,
                        new_fb_id,
                        0, // x
                        0, // y
                        &mut connector_id,
                        1, // connector count
                        &mut mode,
                    );
                    
                    if crtc_result == 0 {
                        // Clean up old framebuffer
                        if self.current_fb != 0 {
                            drmModeRmFB(self.drm_fd, self.current_fb);
                        }
                        self.current_fb = new_fb_id;
                    } else {
                        // Both failed - clean up new framebuffer
                        drmModeRmFB(self.drm_fd, new_fb_id);
                    }
                }
            }
            
            // Release the buffer back to GBM
            gbm_surface_release_buffer(self.gbm_surface, bo);
        }
    }
    
    /// Clear the screen with black color
    pub fn clear(&self) {
        unsafe {
            glClear(GL_COLOR_BUFFER_BIT);
        }
    }
    
    /// Poll for input events (basic implementation)
    pub fn poll_events(&self) -> Vec<InputEvent> {
        // For a basic implementation, we'll return an empty vector
        // In a real implementation, this would poll for keyboard/GPIO events
        Vec::new()
    }
    
    /// Check if a quit event was received
    pub fn should_quit(&self) -> bool {
        let events = self.poll_events();
        events.iter().any(|event| matches!(event.event_type, InputEventType::Quit))
    }

    /// Get OpenGL function pointer (needed for gl::load_with)
    pub fn get_proc_address(&self, proc: *const c_char) -> *mut c_void {
        unsafe { eglGetProcAddress(proc) }
    }
    
    /// Load OpenGL function pointers
    pub fn load_gl_functions(&self) {
        gl::load_with(|name| {
            let c_str = CString::new(name).unwrap();
            self.get_proc_address(c_str.as_ptr())
        });
    }
    
    /// Save the current framebuffer to an image file (for testing)
    pub fn save_framebuffer(&self, filename: &str) -> Result<(), String> {
        unsafe {
            let mut pixels = vec![0u8; (self.width * self.height * 4) as usize];
            gl::ReadPixels(
                0, 0, 
                self.width, self.height,
                gl::RGBA, gl::UNSIGNED_BYTE,
                pixels.as_mut_ptr() as *mut std::ffi::c_void
            );
            
            // Flip image vertically (OpenGL has origin at bottom-left)
            let mut flipped_pixels = vec![0u8; pixels.len()];
            let row_size = (self.width * 4) as usize;
            for y in 0..self.height as usize {
                let src_row = &pixels[y * row_size..(y + 1) * row_size];
                let dst_y = (self.height as usize - 1 - y) * row_size;
                flipped_pixels[dst_y..dst_y + row_size].copy_from_slice(src_row);
            }
            
            // Convert RGBA to RGB (remove alpha channel)
            let mut rgb_pixels = Vec::with_capacity((self.width * self.height * 3) as usize);
            for chunk in flipped_pixels.chunks(4) {
                rgb_pixels.push(chunk[0]); // R
                rgb_pixels.push(chunk[1]); // G
                rgb_pixels.push(chunk[2]); // B
            }
            
            match image::save_buffer(
                filename,
                &rgb_pixels,
                self.width as u32,
                self.height as u32,
                image::ColorType::Rgb8,
            ) {
                Ok(()) => {
                    print!("Framebuffer saved to: {}\r\n", filename);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to save framebuffer: {}", e)),
            }
        }
    }
    
    /// Hide the mouse cursor for dashboard applications
    pub fn hide_cursor(&self) -> Result<(), String> {
        use std::fs::File;
        use std::io::Write;
        
        // Method 1: Hide cursor via console escape sequence
        print!("\x1b[?25l"); // ANSI escape sequence to hide cursor
        std::io::stdout().flush().map_err(|e| format!("Failed to flush stdout: {}", e))?;
        
        // Method 2: Try to hide cursor via /dev/tty
        if let Ok(mut tty) = File::options().write(true).open("/dev/tty") {
            let _ = tty.write_all(b"\x1b[?25l");
            let _ = tty.flush();
        }
        
        // Method 3: Try to disable cursor via kernel parameter (best effort)
        if let Ok(mut file) = File::options().write(true).open("/sys/class/graphics/fbcon/cursor_blink") {
            let _ = file.write_all(b"0");
        }
        
        Ok(())
    }
    
    /// Show the mouse cursor (restore visibility)
    pub fn show_cursor(&self) -> Result<(), String> {
        use std::fs::File;
        use std::io::Write;
        
        // Method 1: Show cursor via console escape sequence
        print!("\x1b[?25h"); // ANSI escape sequence to show cursor
        std::io::stdout().flush().map_err(|e| format!("Failed to flush stdout: {}", e))?;
        
        // Method 2: Try to show cursor via /dev/tty
        if let Ok(mut tty) = File::options().write(true).open("/dev/tty") {
            let _ = tty.write_all(b"\x1b[?25h");
            let _ = tty.flush();
        }
        
        // Method 3: Try to enable cursor via kernel parameter (best effort)
        if let Ok(mut file) = File::options().write(true).open("/sys/class/graphics/fbcon/cursor_blink") {
            let _ = file.write_all(b"1");
        }
        
        Ok(())
    }

    // =============================================================================
    // Brightness Control Methods
    // =============================================================================

    /// Set display brightness (0.0 to 1.0)
    pub fn set_brightness(&mut self, brightness: f32) {
        self.ui_style.set_brightness(brightness);
    }

    /// Get current brightness level
    pub fn get_brightness(&self) -> f32 {
        self.ui_style.get_brightness()
    }

    /// Increase brightness by a step
    pub fn increase_brightness(&mut self, step: f32) {
        self.ui_style.increase_brightness(step);
    }

    /// Decrease brightness by a step
    pub fn decrease_brightness(&mut self, step: f32) {
        self.ui_style.decrease_brightness(step);
    }

    /// Clear the screen with black
    pub fn clear_screen(&mut self) {
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    // =============================================================================
    // RECTANGLE RENDERING METHODS
    // =============================================================================
    
    /// Render a rectangle with specified properties
    /// 
    /// # Arguments
    /// * `x` - X coordinate of top-left corner
    /// * `y` - Y coordinate of top-left corner  
    /// * `width` - Rectangle width
    /// * `height` - Rectangle height
    /// * `color` - RGB color as (r, g, b) tuple, values 0.0-1.0
    /// * `filled` - If true, fills the rectangle; if false, draws outline only
    /// * `thickness` - Line thickness for outline (ignored if filled=true)
    /// * `corner_radius` - Corner radius for rounded rectangles (0.0 for sharp corners)
    pub fn render_rectangle(
        &mut self,
        x: f32, 
        y: f32, 
        width: f32, 
        height: f32, 
        color: (f32, f32, f32),
        filled: bool,
        thickness: f32,
        corner_radius: f32,
    ) -> Result<(), String> {
        unsafe {
            if corner_radius > 0.0 {
                // Render rounded rectangle
                if filled {
                    self.render_filled_rounded_rectangle(x, y, width, height, color, corner_radius)
                } else {
                    self.render_rounded_rectangle_outline(x, y, width, height, color, thickness, corner_radius)
                }
            } else {
                // Render regular rectangle
                if filled {
                    self.render_filled_rectangle(x, y, width, height, color)
                } else {
                    self.render_rectangle_outline(x, y, width, height, color, thickness)
                }
            }
        }
    }
    
    /// Render a filled rectangle (solid color)
    unsafe fn render_filled_rectangle(
        &mut self,
        x: f32, 
        y: f32, 
        width: f32, 
        height: f32, 
        color: (f32, f32, f32)
    ) -> Result<(), String> {
        // Create simple rectangle shader program if needed
        let shader_program = self.get_or_create_rectangle_shader()?;
        gl::UseProgram(shader_program);
        
        // Set up projection matrix for 2D rendering
        let projection_matrix = self.create_2d_projection_matrix();
        let projection_uniform = gl::GetUniformLocation(shader_program, b"projection\0".as_ptr());
        gl::UniformMatrix4fv(projection_uniform, 1, gl::FALSE, projection_matrix.as_ptr());
        
        // Set color uniform
        let color_uniform = gl::GetUniformLocation(shader_program, b"color\0".as_ptr());
        gl::Uniform3f(color_uniform, color.0, color.1, color.2);
        
        // Define rectangle vertices (2 triangles)
        let vertices: [f32; 12] = [
            x,         y,          // Top-left
            x + width, y,          // Top-right  
            x,         y + height, // Bottom-left
            
            x + width, y,          // Top-right
            x + width, y + height, // Bottom-right
            x,         y + height, // Bottom-left
        ];
        
        // Create and bind VAO/VBO
        let mut vao = 0u32;
        let mut vbo = 0u32;
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        
        // Upload vertex data
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const std::ffi::c_void,
            gl::STATIC_DRAW,
        );
        
        // Set up vertex attributes
        let position_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr()) as u32;
        gl::VertexAttribPointer(position_attr, 2, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
        gl::EnableVertexAttribArray(position_attr);
        
        // Render
        gl::DrawArrays(gl::TRIANGLES, 0, 6);
        
        // Clean up
        gl::DeleteBuffers(1, &vbo);
        gl::DeleteVertexArrays(1, &vao);
        
        Ok(())
    }
    
    /// Render rectangle outline with specified thickness
    unsafe fn render_rectangle_outline(
        &mut self,
        x: f32, 
        y: f32, 
        width: f32, 
        height: f32, 
        color: (f32, f32, f32),
        thickness: f32
    ) -> Result<(), String> {
        // Draw 4 filled rectangles for the outline
        let half_thickness = thickness / 2.0;
        
        // Top edge
        self.render_filled_rectangle(x - half_thickness, y - half_thickness, width + thickness, thickness, color)?;
        
        // Bottom edge  
        self.render_filled_rectangle(x - half_thickness, y + height - half_thickness, width + thickness, thickness, color)?;
        
        // Left edge
        self.render_filled_rectangle(x - half_thickness, y + half_thickness, thickness, height - thickness, color)?;
        
        // Right edge
        self.render_filled_rectangle(x + width - half_thickness, y + half_thickness, thickness, height - thickness, color)?;
        
        Ok(())
    }
    
    /// Render filled rectangle with rounded corners
    unsafe fn render_filled_rounded_rectangle(
        &mut self,
        x: f32, 
        y: f32, 
        width: f32, 
        height: f32, 
        color: (f32, f32, f32),
        corner_radius: f32
    ) -> Result<(), String> {
        let radius = corner_radius.min(width / 2.0).min(height / 2.0);
        
        // Draw main rectangle (without corners)
        self.render_filled_rectangle(x + radius, y, width - 2.0 * radius, height, color)?;
        self.render_filled_rectangle(x, y + radius, radius, height - 2.0 * radius, color)?;
        self.render_filled_rectangle(x + width - radius, y + radius, radius, height - 2.0 * radius, color)?;
        
        // Draw rounded corners using circle segments
        self.render_circle_segment(x + radius, y + radius, radius, color, 180.0, 270.0)?; // Top-left
        self.render_circle_segment(x + width - radius, y + radius, radius, color, 270.0, 360.0)?; // Top-right
        self.render_circle_segment(x + width - radius, y + height - radius, radius, color, 0.0, 90.0)?; // Bottom-right
        self.render_circle_segment(x + radius, y + height - radius, radius, color, 90.0, 180.0)?; // Bottom-left
        
        Ok(())
    }
    
    /// Render rounded rectangle outline
    unsafe fn render_rounded_rectangle_outline(
        &mut self,
        x: f32, 
        y: f32, 
        width: f32, 
        height: f32, 
        color: (f32, f32, f32),
        thickness: f32,
        corner_radius: f32
    ) -> Result<(), String> {
        let radius = corner_radius.min(width / 2.0).min(height / 2.0);
        let half_thickness = thickness / 2.0;
        
        // Draw straight edges
        // Top edge
        self.render_filled_rectangle(x + radius, y - half_thickness, width - 2.0 * radius, thickness, color)?;
        // Bottom edge
        self.render_filled_rectangle(x + radius, y + height - half_thickness, width - 2.0 * radius, thickness, color)?;
        // Left edge
        self.render_filled_rectangle(x - half_thickness, y + radius, thickness, height - 2.0 * radius, color)?;
        // Right edge
        self.render_filled_rectangle(x + width - half_thickness, y + radius, thickness, height - 2.0 * radius, color)?;
        
        // Draw rounded corner outlines using circle arcs
        self.render_circle_arc_outline(x + radius, y + radius, radius, thickness, color, 180.0_f32.to_radians(), 270.0_f32.to_radians(), 16)?; // Top-left
        self.render_circle_arc_outline(x + width - radius, y + radius, radius, thickness, color, 270.0_f32.to_radians(), 360.0_f32.to_radians(), 16)?; // Top-right
        self.render_circle_arc_outline(x + width - radius, y + height - radius, radius, thickness, color, 0.0_f32.to_radians(), 90.0_f32.to_radians(), 16)?; // Bottom-right
        self.render_circle_arc_outline(x + radius, y + height - radius, radius, thickness, color, 90.0_f32.to_radians(), 180.0_f32.to_radians(), 16)?; // Bottom-left

        Ok(())
    }
    
    /// Render a filled circle segment (for rounded corners)
    unsafe fn render_circle_segment(
        &mut self,
        center_x: f32, 
        center_y: f32, 
        radius: f32, 
        color: (f32, f32, f32),
        start_angle: f32, 
        end_angle: f32
    ) -> Result<(), String> {
        let shader_program = self.get_or_create_rectangle_shader()?;
        gl::UseProgram(shader_program);
        
        // Set up projection matrix
        let projection_matrix = self.create_2d_projection_matrix();
        let projection_uniform = gl::GetUniformLocation(shader_program, b"projection\0".as_ptr());
        gl::UniformMatrix4fv(projection_uniform, 1, gl::FALSE, projection_matrix.as_ptr());
        
        // Set color uniform
        let color_uniform = gl::GetUniformLocation(shader_program, b"color\0".as_ptr());
        gl::Uniform3f(color_uniform, color.0, color.1, color.2);
        
        // Generate vertices for circle segment
        let segments = 16; // Number of triangular segments for smooth curve
        let mut vertices = Vec::with_capacity((segments + 2) * 2); // Center + arc points
        
        // Add center point
        vertices.push(center_x);
        vertices.push(center_y);
        
        // Add arc points
        let angle_step = (end_angle - start_angle) / segments as f32;
        for i in 0..=segments {
            let angle = (start_angle + i as f32 * angle_step).to_radians();
            vertices.push(center_x + radius * angle.cos());
            vertices.push(center_y + radius * angle.sin());
        }
        
        // Create and bind VAO/VBO
        let mut vao = 0u32;
        let mut vbo = 0u32;
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        
        // Upload vertex data
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const std::ffi::c_void,
            gl::STATIC_DRAW,
        );
        
        // Set up vertex attributes
        let position_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr()) as u32;
        gl::VertexAttribPointer(position_attr, 2, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
        gl::EnableVertexAttribArray(position_attr);
        
        // Render as triangle fan
        gl::DrawArrays(gl::TRIANGLE_FAN, 0, vertices.len() as i32 / 2);
        
        // Clean up
        gl::DeleteBuffers(1, &vbo);
        gl::DeleteVertexArrays(1, &vao);
        
        Ok(())
    }
    
    /// Render a circle arc outline (for rounded corner borders)
    pub fn render_circle_arc_outline(
        &mut self,
        center_x: f32, 
        center_y: f32, 
        radius: f32, 
        thickness: f32,
        color: (f32, f32, f32),
        start_angle: f32, 
        end_angle: f32,
        segments: usize,
    ) -> Result<(), String> {
        unsafe {
            // For thick arcs, we render the difference between outer and inner arcs
            let outer_radius = radius + thickness / 2.0;
            let inner_radius = radius - thickness / 2.0;
            
            let shader_program = self.get_or_create_rectangle_shader()?;
            gl::UseProgram(shader_program);
            
            // Set up projection matrix
            let projection_matrix = self.create_2d_projection_matrix();
            let projection_uniform = gl::GetUniformLocation(shader_program, b"projection\0".as_ptr());
            gl::UniformMatrix4fv(projection_uniform, 1, gl::FALSE, projection_matrix.as_ptr());
            
            // Set color uniform
            let color_uniform = gl::GetUniformLocation(shader_program, b"color\0".as_ptr());
            gl::Uniform3f(color_uniform, color.0, color.1, color.2);
            
            // Generate vertices for arc ring (triangle strip)
            let mut vertices = Vec::with_capacity(segments * 4 * 2); // 2 points per segment * 2 coords
            
            let angle_step = (end_angle - start_angle) / (segments - 1) as f32;
            for i in 0..segments {
                let angle = start_angle + i as f32 * angle_step; // Angles already in radians
                let cos_a = angle.cos();
                let sin_a = angle.sin();
                
                // Inner point
                vertices.push(center_x + inner_radius * cos_a);
                vertices.push(center_y + inner_radius * sin_a);
                
                // Outer point
                vertices.push(center_x + outer_radius * cos_a);
                vertices.push(center_y + outer_radius * sin_a);
            }
            
            // Create and bind VAO/VBO
            let mut vao = 0u32;
            let mut vbo = 0u32;
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            
            // Upload vertex data
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                gl::STATIC_DRAW,
            );
            
            // Set up vertex attributes
            let position_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr()) as u32;
            gl::VertexAttribPointer(position_attr, 2, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
            gl::EnableVertexAttribArray(position_attr);
            
            // Render as triangle strip
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, vertices.len() as i32 / 2);
            
            // Clean up
            gl::DeleteBuffers(1, &vbo);
            gl::DeleteVertexArrays(1, &vao);
        }
        
        Ok(())
    }
    
    /// Get or create the rectangle shader program (cached)
    unsafe fn get_or_create_rectangle_shader(&mut self) -> Result<u32, String> {
        if let Some(shader) = self.rectangle_shader {
            Ok(shader)
        } else {
            let shader = self.create_rectangle_shader_program()?;
            self.rectangle_shader = Some(shader);
            print!("Rectangle shader program cached for reuse\r\n");
            Ok(shader)
        }
    }
    
    /// Create shader program for rectangle rendering
    unsafe fn create_rectangle_shader_program(&self) -> Result<u32, String> {
        let vertex_shader_source = b"
attribute vec2 position;
uniform mat4 projection;

void main() {
    gl_Position = projection * vec4(position, 0.0, 1.0);
}
\0";
        
        let fragment_shader_source = b"
precision mediump float;
uniform vec3 color;

void main() {
    gl_FragColor = vec4(color, 1.0);
}
\0";
        
        // Create and compile vertex shader
        let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
        if vertex_shader == 0 {
            return Err("Failed to create rectangle vertex shader".to_string());
        }
        
        let vertex_src_ptr = vertex_shader_source.as_ptr();
        gl::ShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
        gl::CompileShader(vertex_shader);
        
        let mut compile_status = 0i32;
        gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut compile_status);
        if compile_status == 0 {
            gl::DeleteShader(vertex_shader);
            return Err("Rectangle vertex shader compilation failed".to_string());
        }
        
        // Create and compile fragment shader
        let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        if fragment_shader == 0 {
            gl::DeleteShader(vertex_shader);
            return Err("Failed to create rectangle fragment shader".to_string());
        }
        
        let fragment_src_ptr = fragment_shader_source.as_ptr();
        gl::ShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
        gl::CompileShader(fragment_shader);
        
        let mut compile_status = 0i32;
        gl::GetShaderiv(fragment_shader, gl::COMPILE_STATUS, &mut compile_status);
        if compile_status == 0 {
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
            return Err("Rectangle fragment shader compilation failed".to_string());
        }
        
        // Create and link shader program
        let program = gl::CreateProgram();
        if program == 0 {
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
            return Err("Failed to create rectangle shader program".to_string());
        }
        
        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);
        
        let mut link_status = 0i32;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut link_status);
        if link_status == 0 {
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
            gl::DeleteProgram(program);
            return Err("Rectangle shader program linking failed".to_string());
        }
        
        // Clean up individual shaders (they're now linked to the program)
        gl::DeleteShader(vertex_shader);
        gl::DeleteShader(fragment_shader);
        
        Ok(program)
    }
    
    /// Create 2D projection matrix for screen coordinates
    fn create_2d_projection_matrix(&self) -> [f32; 16] {
        // Create orthographic projection matrix for 2D rendering
        // Maps screen coordinates (0,0) to (width, height) to NDC (-1,-1) to (1,1)
        [
            2.0 / self.width as f32, 0.0,                      0.0, 0.0,
            0.0,                     -2.0 / self.height as f32, 0.0, 0.0,  // Negative Y to flip coordinates
            0.0,                     0.0,                      -1.0, 0.0,
            -1.0,                    1.0,                       0.0, 1.0,
        ]
    }
    
    // =============================================================================
    // CONVENIENCE RECTANGLE RENDERING METHODS
    // =============================================================================
    
    /// Render a simple filled rectangle (convenience method)
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (f32, f32, f32)) -> Result<(), String> {
        self.render_rectangle(x, y, width, height, color, true, 0.0, 0.0)
    }
    
    /// Render a simple rectangle outline (convenience method)
    pub fn stroke_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (f32, f32, f32), thickness: f32) -> Result<(), String> {
        self.render_rectangle(x, y, width, height, color, false, thickness, 0.0)
    }
    
    /// Render a filled rounded rectangle (convenience method)
    pub fn fill_rounded_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (f32, f32, f32), corner_radius: f32) -> Result<(), String> {
        self.render_rectangle(x, y, width, height, color, true, 0.0, corner_radius)
    }
    
    /// Render a rounded rectangle outline (convenience method)
    pub fn stroke_rounded_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (f32, f32, f32), thickness: f32, corner_radius: f32) -> Result<(), String> {
        self.render_rectangle(x, y, width, height, color, false, thickness, corner_radius)
    }
    
    /// Render a rectangle using UI style colors (convenience method for dashboard components)
    pub fn render_ui_rect(&mut self, x: f32, y: f32, width: f32, height: f32, style: &str, filled: bool, thickness: f32) -> Result<(), String> {
        let color = match style {
            "primary" => self.ui_style.get_color("global_brand_primary_color", (1.0, 0.0, 0.0)),
            "secondary" => self.ui_style.get_color("global_brand_secondary_color", (0.5, 0.5, 0.5)), 
            "accent" => self.ui_style.get_color("global_brand_accent_color", (1.0, 0.4, 0.0)),
            "warning" => self.ui_style.get_color("text_warning_color", (1.0, 0.67, 0.0)),
            "error" | "danger" => self.ui_style.get_color("text_error_color", (1.0, 0.0, 0.0)),
            "critical" => self.ui_style.get_color("indicator_critical_color", (1.0, 0.0, 0.0)),
            "success" | "normal" => self.ui_style.get_color("indicator_normal_color", (0.0, 1.0, 0.0)),
            "background" => self.ui_style.get_color("global_background_color", (0.0, 0.0, 0.0)),
            "text_primary" => self.ui_style.get_color("text_primary_color", (1.0, 1.0, 1.0)),
            "text_secondary" => self.ui_style.get_color("text_secondary_color", (0.75, 0.75, 0.75)),
            "gauge_border" => self.ui_style.get_color("gauge_border_color", (1.0, 1.0, 1.0)),
            "bar_fill" => self.ui_style.get_color("bar_fill_color", (0.0, 1.0, 0.0)),
            _ => (1.0, 1.0, 1.0), // Default to white
        };
        
        self.render_rectangle(x, y, width, height, color, filled, thickness, 0.0)
    }
    
    /// Cleanup rectangle shader when context is destroyed
    unsafe fn cleanup_rectangle_shader(&mut self) {
        if let Some(shader) = self.rectangle_shader.take() {
            gl::DeleteProgram(shader);
            print!("Rectangle shader program cleaned up\r\n");
        }
    }

    // =============================================================================
    // NEW FONT MANAGEMENT SYSTEM
    // =============================================================================
    
    /// Create a font key from font path and size
    fn create_font_key(font_path: &str, font_size: u32) -> String {
        format!("{}_{}", font_path, font_size)
    }
    
    /// Get or create a text renderer for a specific font
    pub fn get_text_renderer(&mut self, font_path: &str, font_size: u32) -> Result<&mut OpenGLTextRenderer, String> {
        let key = Self::create_font_key(font_path, font_size);
        
        // Check if renderer already exists
        if !self.text_renderers.contains_key(&key) {
            // Create new renderer
            let renderer = unsafe { OpenGLTextRenderer::new(font_path, font_size)? };
            self.text_renderers.insert(key.clone(), renderer);
            print!("Created new text renderer for font: {} (size: {})\r\n", font_path, font_size);
        }
        
        Ok(self.text_renderers.get_mut(&key).unwrap())
    }
    
    /// Private method to render text with orientation support
    fn render_text(
        &mut self, 
        text: &str, 
        x: f32, 
        y: f32, 
        scale: f32, 
        color: (f32, f32, f32),
        font_path: &str,
        font_size: u32,
        orientation: TextOrientation
    ) -> Result<(), String> {
        // Apply brightness adjustment to the color
        let adjusted_color = self.ui_style.apply_brightness(color);
        
        // Capture dimensions before borrowing renderer
        let width = self.width as f32;
        let height = self.height as f32;
        
        // Get the text renderer for this font
        let renderer = self.get_text_renderer(font_path, font_size)?;
        
        // Render the text with orientation
        unsafe {
            renderer.render_text(text, x, y, scale, adjusted_color, width, height, orientation)
        }
    }
    
    /// Render text using a specific font (horizontal orientation)
    pub fn render_text_with_font(
        &mut self, 
        text: &str, 
        x: f32, 
        y: f32, 
        scale: f32, 
        color: (f32, f32, f32),
        font_path: &str,
        font_size: u32
    ) -> Result<(), String> {
        self.render_text(text, x, y, scale, color, font_path, font_size, TextOrientation::Horizontal)
    }
    
    /// Render text using a specific font (vertical orientation)
    pub fn render_text_with_font_vert(
        &mut self, 
        text: &str, 
        x: f32, 
        y: f32, 
        scale: f32, 
        color: (f32, f32, f32),
        font_path: &str,
        font_size: u32
    ) -> Result<(), String> {
        self.render_text(text, x, y, scale, color, font_path, font_size, TextOrientation::Vertical)
    }
    
    /// Private method to calculate text width with orientation
    fn calculate_text_width(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32,
        orientation: TextOrientation
    ) -> Result<f32, String> {
        let renderer = self.get_text_renderer(font_path, font_size)?;
        unsafe {
            renderer.calculate_text_width(text, scale, orientation)
        }
    }
    
    /// Private method to calculate text height with orientation
    fn calculate_text_height(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32,
        orientation: TextOrientation
    ) -> Result<f32, String> {
        let renderer = self.get_text_renderer(font_path, font_size)?;
        unsafe {
            renderer.calculate_text_height(text, scale, orientation)
        }
    }
    
    /// Private method to calculate text dimensions with orientation
    fn calculate_text_dimensions(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32,
        orientation: TextOrientation
    ) -> Result<(f32, f32), String> {
        let renderer = self.get_text_renderer(font_path, font_size)?;
        unsafe {
            renderer.calculate_text_dimensions(text, scale, orientation)
        }
    }
    
    /// Calculate text width using a specific font (horizontal orientation)
    pub fn calculate_text_width_with_font(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<f32, String> {
        self.calculate_text_width(text, scale, font_path, font_size, TextOrientation::Horizontal)
    }

    pub fn calculate_text_width_with_font_vert(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<f32, String> {
        self.calculate_text_width(text, scale, font_path, font_size, TextOrientation::Vertical)
    }
    
    /// Calculate text height using a specific font (horizontal orientation)
    pub fn calculate_text_height_with_font(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<f32, String> {
        self.calculate_text_height(text, scale, font_path, font_size, TextOrientation::Horizontal)
    }

    pub fn calculate_text_height_with_font_vert(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<f32, String> {
        self.calculate_text_height(text, scale, font_path, font_size, TextOrientation::Vertical)
    }

    /// Calculate text dimensions using a specific font (horizontal orientation)
    pub fn calculate_text_dimensions_with_font(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<(f32, f32), String> {
        self.calculate_text_dimensions(text, scale, font_path, font_size, TextOrientation::Horizontal)
    }

    pub fn calculate_text_dimensions_with_font_vert(
        &mut self, 
        text: &str, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<(f32, f32), String> {
        self.calculate_text_dimensions(text, scale, font_path, font_size, TextOrientation::Vertical)
    }

    /// Get line height for a specific font
    pub fn get_line_height_with_font(
        &mut self, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<f32, String> {
        let renderer = self.get_text_renderer(font_path, font_size)?;
        Ok(renderer.get_line_height(scale))
    }
    
    /// Get line spacing for a specific font
    pub fn get_line_spacing_with_font(
        &mut self, 
        scale: f32,
        font_path: &str,
        font_size: u32
    ) -> Result<f32, String> {
        let renderer = self.get_text_renderer(font_path, font_size)?;
        Ok(renderer.get_line_spacing(scale))
    }
    
    /// Initialize bloom post-processing effect
    pub fn init_bloom(&mut self) -> Result<(), String> {
        if self.bloom_framebuffer.is_some() {
            return Ok(()); // Already initialized
        }
        
        unsafe {
            // Create framebuffer
            let mut framebuffer = 0;
            gl::GenFramebuffers(1, &mut framebuffer);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
            
            // Create texture for framebuffer
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexImage2D(
                gl::TEXTURE_2D, 0, gl::RGBA as i32, 
                self.width, self.height, 0, 
                gl::RGBA, gl::UNSIGNED_BYTE, 
                ptr::null()
            );
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            
            // Attach texture to framebuffer
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, 
                gl::TEXTURE_2D, texture, 0
            );
            
            // Check framebuffer completeness
            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                return Err("Failed to create bloom framebuffer".to_string());
            }
            
            // Create bloom shader
            let shader = self.create_bloom_shader()?;
            
            self.bloom_framebuffer = Some(framebuffer);
            self.bloom_texture = Some(texture);
            self.bloom_shader = Some(shader);
            
            // Restore default framebuffer
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        
        print!("✓ Bloom effect initialized\r\n");
        Ok(())
    }
    
    /// Create bloom post-processing shader
    fn create_bloom_shader(&self) -> Result<u32, String> {
        let vertex_shader_source = b"
            #version 300 es
            precision mediump float;
            
            in vec2 position;
            in vec2 texCoord;
            
            out vec2 vTexCoord;
            
            void main() {
                gl_Position = vec4(position, 0.0, 1.0);
                vTexCoord = texCoord;
            }
        \0";
        
        let fragment_shader_source = format!("
            #version 300 es
            precision mediump float;
            
            in vec2 vTexCoord;
            out vec4 fragColor;
            
            uniform sampler2D uTexture;
            uniform float uIntensity;
            uniform float uThreshold;
            
            void main() {{
                vec3 originalColor = texture(uTexture, vTexCoord).rgb;
                vec2 texelSize = 1.0 / vec2({}, {});
                
                vec3 bloom = vec3(0.0);
                
                // Simple gaussian-like blur for bloom effect
                // Sample surrounding pixels with decreasing weights
                for(int x = -3; x <= 3; x++) {{
                    for(int y = -3; y <= 3; y++) {{
                        vec2 offset = vec2(float(x), float(y)) * texelSize;
                        vec3 sampleColor = texture(uTexture, vTexCoord + offset).rgb;
                        
                        // Extract bright pixels above threshold
                        float brightness = dot(sampleColor, vec3(0.299, 0.587, 0.114));
                        if(brightness > uThreshold) {{
                            float distance = length(vec2(float(x), float(y)));
                            float weight = exp(-distance * 0.5);
                            bloom += sampleColor * weight * (brightness - uThreshold);
                        }}
                    }}
                }}
                
                // Apply bloom with intensity control
                vec3 finalColor = originalColor + bloom * uIntensity;
                fragColor = vec4(finalColor, 1.0);
            }}
        \0", self.width, self.height);
        
        unsafe {
            // Compile vertex shader
            let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
            gl::ShaderSource(vertex_shader, 1, &vertex_shader_source.as_ptr(), ptr::null());
            gl::CompileShader(vertex_shader);
            
            // Check compilation
            let mut success = 0;
            gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut success);
            if success == 0 {
                let mut log = [0u8; 512];
                gl::GetShaderInfoLog(vertex_shader, 512, ptr::null_mut(), log.as_mut_ptr());
                return Err(format!("Vertex shader compilation failed: {}", 
                    String::from_utf8_lossy(&log)));
            }
            
            // Compile fragment shader
            let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
            let fragment_source_ptr = fragment_shader_source.as_ptr();
            gl::ShaderSource(fragment_shader, 1, &fragment_source_ptr, ptr::null());
            gl::CompileShader(fragment_shader);
            
            // Check compilation
            gl::GetShaderiv(fragment_shader, gl::COMPILE_STATUS, &mut success);
            if success == 0 {
                let mut log = [0u8; 512];
                gl::GetShaderInfoLog(fragment_shader, 512, ptr::null_mut(), log.as_mut_ptr());
                return Err(format!("Fragment shader compilation failed: {}", 
                    String::from_utf8_lossy(&log)));
            }
            
            // Link shader program
            let shader_program = gl::CreateProgram();
            gl::AttachShader(shader_program, vertex_shader);
            gl::AttachShader(shader_program, fragment_shader);
            gl::LinkProgram(shader_program);
            
            // Check linking
            gl::GetProgramiv(shader_program, gl::LINK_STATUS, &mut success);
            if success == 0 {
                let mut log = [0u8; 512];
                gl::GetProgramInfoLog(shader_program, 512, ptr::null_mut(), log.as_mut_ptr());
                return Err(format!("Shader program linking failed: {}", 
                    String::from_utf8_lossy(&log)));
            }
            
            // Clean up shaders
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
            
            Ok(shader_program)
        }
    }
    
    /// Begin rendering to bloom framebuffer
    pub fn begin_bloom_render(&self) -> Result<(), String> {
        if let Some(framebuffer) = self.bloom_framebuffer {
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
                gl::Viewport(0, 0, self.width, self.height);
                gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            Ok(())
        } else {
            Err("Bloom not initialized".to_string())
        }
    }
    
    /// End bloom rendering and apply bloom effect to screen
    pub fn end_bloom_render(&self) -> Result<(), String> {
        if let (Some(texture), Some(shader)) = (self.bloom_texture, self.bloom_shader) {
            unsafe {
                // Restore default framebuffer
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl::Viewport(0, 0, self.width, self.height);
                
                // Use bloom shader
                gl::UseProgram(shader);
                
                // Set uniforms
                let intensity_loc = gl::GetUniformLocation(shader, b"uIntensity\0".as_ptr());
                let threshold_loc = gl::GetUniformLocation(shader, b"uThreshold\0".as_ptr());
                let texture_loc = gl::GetUniformLocation(shader, b"uTexture\0".as_ptr());
                
                gl::Uniform1f(intensity_loc, self.bloom_intensity);
                gl::Uniform1f(threshold_loc, self.bloom_threshold);
                gl::Uniform1i(texture_loc, 0);
                
                // Bind bloom texture
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, texture);
                
                // Render fullscreen quad
                self.render_fullscreen_quad();
            }
            Ok(())
        } else {
            Err("Bloom not initialized".to_string())
        }
    }
    
    /// Render a fullscreen quad for post-processing
    fn render_fullscreen_quad(&self) {
        unsafe {
            // Simple fullscreen quad vertices
            let vertices: [f32; 24] = [
                // Position    // TexCoord
                -1.0, -1.0,    0.0, 0.0,  // Bottom-left
                 1.0, -1.0,    1.0, 0.0,  // Bottom-right
                 1.0,  1.0,    1.0, 1.0,  // Top-right
                
                -1.0, -1.0,    0.0, 0.0,  // Bottom-left
                 1.0,  1.0,    1.0, 1.0,  // Top-right
                -1.0,  1.0,    0.0, 1.0,  // Top-left
            ];
            
            let mut vbo = 0;
            let mut vao = 0;
            
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            
            gl::BindVertexArray(vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const _,
                gl::STATIC_DRAW
            );
            
            // Position attribute
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 4 * std::mem::size_of::<f32>() as i32, ptr::null());
            gl::EnableVertexAttribArray(0);
            
            // TexCoord attribute
            gl::VertexAttribPointer(1, 2, gl::FLOAT, gl::FALSE, 4 * std::mem::size_of::<f32>() as i32, 
                (2 * std::mem::size_of::<f32>()) as *const _);
            gl::EnableVertexAttribArray(1);
            
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            
            // Cleanup
            gl::DeleteVertexArrays(1, &vao);
            gl::DeleteBuffers(1, &vbo);
        }
    }
    
    /// Set bloom parameters
    pub fn set_bloom_intensity(&mut self, intensity: f32) {
        self.bloom_intensity = intensity.clamp(0.0, 2.0);
    }
    
    pub fn set_bloom_threshold(&mut self, threshold: f32) {
        self.bloom_threshold = threshold.clamp(0.0, 1.0);
    }
    
    pub fn set_bloom_enabled(&mut self, enabled: bool) {
        self.bloom_enabled = enabled;
    }

    /// Begin selective bloom rendering - only elements drawn between this and end_selective_bloom_render will bloom
    pub fn begin_selective_bloom_render(&self) -> Result<(), String> {
        if let Some(framebuffer) = self.bloom_framebuffer {
            unsafe {
                // Switch to bloom framebuffer and clear it
                gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
                gl::ClearColor(0.0, 0.0, 0.0, 1.0); // Clear to black
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            Ok(())
        } else {
            Err("Bloom framebuffer not initialized".to_string())
        }
    }

    /// End selective bloom rendering and return to main framebuffer
    pub fn end_selective_bloom_render(&self) -> Result<(), String> {
        unsafe {
            // Return to main framebuffer (0 = screen)
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        Ok(())
    }

    /// Apply bloom from selective rendering to the current scene
    pub fn apply_selective_bloom(&self) -> Result<(), String> {
        if let (Some(texture), Some(shader)) = (self.bloom_texture, self.bloom_shader) {
            unsafe {
                // Use bloom shader
                gl::UseProgram(shader);
                
                // Bind bloom texture
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, texture);
                gl::Uniform1i(gl::GetUniformLocation(shader, b"uTexture\0".as_ptr()), 0);
                
                // Set bloom parameters
                gl::Uniform1f(gl::GetUniformLocation(shader, b"uIntensity\0".as_ptr()), self.bloom_intensity);
                gl::Uniform1f(gl::GetUniformLocation(shader, b"uThreshold\0".as_ptr()), self.bloom_threshold);
                
                // Enable additive blending for bloom overlay
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::ONE, gl::ONE); // Additive blending
                
                // Render fullscreen quad
                self.render_fullscreen_quad();
                
                // Restore normal blending
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            }
            Ok(())
        } else {
            Err("Bloom not properly initialized".to_string())
        }
    }

    /// Draw text with bloom effect
    pub fn draw_text_with_bloom(&mut self, text: &str, x: f32, y: f32, color: (f32, f32, f32), font_path: &str, font_size: u32) -> Result<(), String> {
        // First, draw normally to main framebuffer
        self.render_text_with_font(text, x, y, 1.0, color, font_path, font_size)?;
        
        // Then draw to bloom framebuffer for glow effect
        if self.bloom_enabled {
            self.begin_selective_bloom_render()?;
            // Draw with enhanced brightness for bloom
            let bloom_color = (color.0 * 2.0, color.1 * 2.0, color.2 * 2.0);
            self.render_text_with_font(text, x, y, 1.0, bloom_color, font_path, font_size)?;
            self.end_selective_bloom_render()?;
            self.apply_selective_bloom()?;
        }
        
        Ok(())
    }

    /// Draw rectangle with bloom effect
    pub fn draw_rect_with_bloom(&mut self, x: f32, y: f32, width: f32, height: f32, color: (f32, f32, f32)) -> Result<(), String> {
        // First, draw normally to main framebuffer
        self.fill_rect(x, y, width, height, color)?;
        
        // Then draw to bloom framebuffer for glow effect
        if self.bloom_enabled {
            self.begin_selective_bloom_render()?;
            // Draw with enhanced brightness for bloom
            let bloom_color = (color.0 * 1.5, color.1 * 1.5, color.2 * 1.5);
            self.fill_rect(x, y, width, height, bloom_color)?;
            self.end_selective_bloom_render()?;
            self.apply_selective_bloom()?;
        }
        
        Ok(())
    }

    /// Begin custom bloom element group - for complex elements
    pub fn begin_bloom_element(&self) -> Result<(), String> {
        if self.bloom_enabled {
            self.begin_selective_bloom_render()
        } else {
            Ok(())
        }
    }

    /// End custom bloom element group
    pub fn end_bloom_element(&self) -> Result<(), String> {
        if self.bloom_enabled {
            self.end_selective_bloom_render()?;
            self.apply_selective_bloom()
        } else {
            Ok(())
        }
    }
    
    pub fn is_bloom_enabled(&self) -> bool {
        self.bloom_enabled
    }
    
    /// Cleanup text renderer before destroying OpenGL context
    fn cleanup_text_renderer(&mut self) {
        if !self.text_renderers.is_empty() {
            print!("Cleaning up {} text renderer(s)...\r\n", self.text_renderers.len());
            self.text_renderers.clear(); // This will trigger Drop for all OpenGLTextRenderer instances
        }
    }
    
    /// Cleanup bloom effect resources
    fn cleanup_bloom(&mut self) {
        unsafe {
            if let Some(framebuffer) = self.bloom_framebuffer.take() {
                gl::DeleteFramebuffers(1, &framebuffer);
            }
            if let Some(texture) = self.bloom_texture.take() {
                gl::DeleteTextures(1, &texture);
            }
            if let Some(shader) = self.bloom_shader.take() {
                gl::DeleteProgram(shader);
            }
        }
        print!("Cleaned up bloom effect resources\r\n");
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        unsafe {
            if self.initialized {
                // Clean up shaders FIRST while OpenGL context is still valid
                self.cleanup_text_renderer();
                self.cleanup_rectangle_shader();
                self.cleanup_bloom();
                
                // Restore previous CRTC configuration
                if !self.previous_crtc.is_null() {
                    // This would restore the original display state
                    // For now, we'll just free the saved CRTC
                    drmModeFreeCrtc(self.previous_crtc);
                }
                
                // Clean up EGL
                if self.egl_display != ptr::null_mut() {
                    if self.egl_surface != EGL_NO_SURFACE {
                        eglDestroySurface(self.egl_display, self.egl_surface);
                    }
                    if self.egl_context != EGL_NO_CONTEXT {
                        eglDestroyContext(self.egl_display, self.egl_context);
                    }
                    eglTerminate(self.egl_display);
                }
                
                // Clean up GBM
                if !self.gbm_surface.is_null() {
                    gbm_surface_destroy(self.gbm_surface);
                }
                if !self.gbm_device.is_null() {
                    gbm_device_destroy(self.gbm_device);
                }
                
                // Clean up DRM
                if self.drm_fd >= 0 {
                    drmClose(self.drm_fd);
                }
            }
        }
        print!("Graphics context cleaned up\r\n");
    }
}



impl OpenGLTextRenderer {
    unsafe fn new(font_path: &str, font_size: u32) -> Result<Self, String> {
        // Initialize FreeType
        let mut ft_library: ft::FT_Library = std::ptr::null_mut();
        if ft::FT_Init_FreeType(&mut ft_library) != 0 {
            return Err("Failed to initialize FreeType library".to_string());
        }
        
        // Load font face
        let mut ft_face: ft::FT_Face = std::ptr::null_mut();
        let font_path_cstr = std::ffi::CString::new(font_path).map_err(|_| "Invalid font path")?;
        
        if ft::FT_New_Face(ft_library, font_path_cstr.as_ptr(), 0, &mut ft_face) != 0 {
            ft::FT_Done_FreeType(ft_library);
            return Err(format!("Failed to load font: {}", font_path));
        }
        
        // Set font size
        if ft::FT_Set_Pixel_Sizes(ft_face, 0, font_size) != 0 {
            ft::FT_Done_Face(ft_face);
            ft::FT_Done_FreeType(ft_library);
            return Err("Failed to set font size".to_string());
        }
        
        // Create text rendering shader
        let shader_program = Self::create_text_shader_program()?;
        
        // Cache uniform and attribute locations for performance
        let projection_uniform = gl::GetUniformLocation(shader_program, b"projection\0".as_ptr());
        let color_uniform = gl::GetUniformLocation(shader_program, b"text_color\0".as_ptr());
        let texture_uniform = gl::GetUniformLocation(shader_program, b"text_texture\0".as_ptr());
        let vertex_attr = gl::GetAttribLocation(shader_program, b"vertex\0".as_ptr());
        
        // Create VAO and VBO for text quads
        let mut vao = 0u32;
        let mut vbo = 0u32;
        gl::GenBuffers(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        
        print!("OpenGL text renderer initialized with FreeType + glyph caching\r\n");
        print!("Font: {}, Size: {}px\r\n", font_path, font_size);
        
        Ok(OpenGLTextRenderer {
            ft_library,
            ft_face,
            shader_program,
            vao,
            vbo,
            font_size,
            glyph_cache: HashMap::new(),
            projection_width: 0.0,
            projection_height: 0.0,
            projection_matrix: [0.0; 16],
            projection_uniform,
            color_uniform,
            texture_uniform,
            vertex_attr,
        })
    }
    
    unsafe fn create_text_shader_program() -> Result<u32, String> {
        let vertex_shader_source = b"
attribute vec4 vertex; // <vec2 pos, vec2 tex>
varying vec2 tex_coords;
uniform mat4 projection;

void main() {
    gl_Position = projection * vec4(vertex.xy, 0.0, 1.0);
    tex_coords = vertex.zw;
}
\0";
        
        let fragment_shader_source = b"
precision mediump float;
varying vec2 tex_coords;
uniform sampler2D text_texture;
uniform vec3 text_color;

void main() {
    vec4 sampled = vec4(1.0, 1.0, 1.0, texture2D(text_texture, tex_coords).r);
    gl_FragColor = vec4(text_color, 1.0) * sampled;
}
\0";
        
        // Create and compile vertex shader
        let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
        if vertex_shader == 0 {
            return Err("Failed to create text vertex shader".to_string());
        }
        
        let vertex_src_ptr = vertex_shader_source.as_ptr();
        gl::ShaderSource(vertex_shader, 1, &vertex_src_ptr, std::ptr::null());
        gl::CompileShader(vertex_shader);
        
        let mut compile_status = 0i32;
        gl::GetShaderiv(vertex_shader, gl::COMPILE_STATUS, &mut compile_status);
        if compile_status == 0 {
            return Err("Text vertex shader compilation failed".to_string());
        }
        
        // Create and compile fragment shader
        let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        if fragment_shader == 0 {
            return Err("Failed to create text fragment shader".to_string());
        }
        
        let fragment_src_ptr = fragment_shader_source.as_ptr();
        gl::ShaderSource(fragment_shader, 1, &fragment_src_ptr, std::ptr::null());
        gl::CompileShader(fragment_shader);
        
        let mut compile_status = 0i32;
        gl::GetShaderiv(fragment_shader, gl::COMPILE_STATUS, &mut compile_status);
        if compile_status == 0 {
            return Err("Text fragment shader compilation failed".to_string());
        }
        
        // Create and link shader program
        let program = gl::CreateProgram();
        if program == 0 {
            return Err("Failed to create text shader program".to_string());
        }
        
        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);
        
        let mut link_status = 0i32;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut link_status);
        if link_status == 0 {
            return Err("Text shader program linking failed".to_string());
        }
        
        print!("Text rendering shader program created successfully!\r\n");
        Ok(program)
    }
    
    unsafe fn render_text(&mut self, text: &str, x: f32, y: f32, scale: f32, color: (f32, f32, f32), width: f32, height: f32, orientation: TextOrientation) -> Result<(), String> {
        // Use cached program state
        gl::UseProgram(self.shader_program);
        
        // Only update projection matrix if dimensions changed
        if self.projection_width != width || self.projection_height != height {
            self.projection_width = width;
            self.projection_height = height;
            
            // Calculate projection matrix once
            self.projection_matrix = [
                2.0/width, 0.0,         0.0, 0.0,
                0.0,       -2.0/height, 0.0, 0.0,  // Negative Y scaling to flip coordinate system
                0.0,       0.0,         -1.0, 0.0,
                -1.0,      1.0,         0.0, 1.0,  // Y translation adjusted for flipped coordinates
            ];
            
            // Upload to GPU using cached uniform location
            gl::UniformMatrix4fv(self.projection_uniform, 1, 0, self.projection_matrix.as_ptr());
        }
        
        // Set text color using cached uniform location
        gl::Uniform3f(self.color_uniform, color.0, color.1, color.2);
        
        // Set up texture uniform using cached location
        gl::Uniform1i(self.texture_uniform, 0);
        
        // Set up vertex attributes using cached location
        gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
        gl::EnableVertexAttribArray(self.vertex_attr as u32);
        gl::VertexAttribPointer(self.vertex_attr as u32, 4, gl::FLOAT, 0, 0, std::ptr::null());
        
        // Render each character using cached glyphs with orientation-based positioning
        match orientation {
            TextOrientation::Horizontal => {
                // Traditional horizontal text - advance cursor in X direction
                let mut cursor_x = x;
                for ch in text.chars() {
                    cursor_x += self.render_cached_character(ch, cursor_x, y, scale)?;
                }
            },
            TextOrientation::Vertical => {
                // Vertical text - advance cursor in Y direction, characters remain upright
                let mut cursor_y = y;
                for ch in text.chars() {
                    // For vertical text, we need to calculate the character's advance in Y direction
                    let glyph = self.get_or_cache_glyph(ch)?;
                    
                    // Render character at current position
                    self.render_cached_character(ch, x, cursor_y, scale)?;
                    
                    // Advance cursor downward by the character height plus small spacing
                    let char_height = glyph.height * scale;
                    cursor_y += char_height + scale * 2.0; // Add some spacing between characters
                }
            }
        }
        
        Ok(())
    }
    
    unsafe fn get_or_cache_glyph(&mut self, ch: char) -> Result<CachedGlyph, String> {
        // Check if glyph is already cached
        if let Some(cached_glyph) = self.glyph_cache.get(&ch) {
            return Ok(cached_glyph.clone());
        }
        
        // Load character glyph
        if ft::FT_Load_Char(self.ft_face, ch as u64, ft::FT_LOAD_RENDER as i32) != 0 {
            return Err(format!("Failed to load character: {}", ch));
        }
        
        // Get glyph slot
        let glyph = (*self.ft_face).glyph;
        
        // Create a dedicated texture for this glyph
        let mut texture_id = 0u32;
        gl::GenTextures(1, &mut texture_id);
        gl::BindTexture(gl::TEXTURE_2D, texture_id);
        
        // Set pixel alignment to 1 byte to handle FreeType's bitmap format
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RED as i32,
            (*glyph).bitmap.width as i32,
            (*glyph).bitmap.rows as i32,
            0,
            gl::RED,
            gl::UNSIGNED_BYTE,
            (*glyph).bitmap.buffer as *const std::ffi::c_void,
        );
        
        // Reset pixel alignment to default
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);
        
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        
        // Cache the glyph data
        let cached_glyph = CachedGlyph {
            texture_id,
            width: (*glyph).bitmap.width as f32,
            height: (*glyph).bitmap.rows as f32,
            bearing_x: (*glyph).bitmap_left as f32,
            bearing_y: (*glyph).bitmap_top as f32,
            advance: ((*glyph).advance.x >> 6) as f32,
        };
        
        self.glyph_cache.insert(ch, cached_glyph.clone());
        Ok(cached_glyph)
    }
    
    unsafe fn render_cached_character(&mut self, ch: char, x: f32, y: f32, scale: f32) -> Result<f32, String> {
        // Get cached glyph (or create if not cached)
        let glyph = self.get_or_cache_glyph(ch)?;
        
        // Bind the glyph's texture
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, glyph.texture_id);
        
        // Calculate quad vertices
        let w = glyph.width * scale;
        let h = glyph.height * scale;
        let xrel = x + glyph.bearing_x * scale;
        
        // Get font ascender to convert from top-of-line to baseline coordinates
        let face_ref = &*self.ft_face;
        let ascender = face_ref.size.as_ref().unwrap().metrics.ascender as f32 / 64.0 * scale;
        
        // Calculate y position: y is top of line, so add ascender to get baseline, then subtract bearing_y
        let yrel = y + ascender - glyph.bearing_y * scale;
        
        // Create quad vertices (x, y, tex_x, tex_y)
        let vertices: [f32; 24] = [
            xrel,     yrel + h, 0.0, 1.0,  // Top-left corner, tex coords (0,1) - flipped V
            xrel,     yrel,     0.0, 0.0,  // Bottom-left corner, tex coords (0,0) - flipped V
            xrel + w, yrel,     1.0, 0.0,  // Bottom-right corner, tex coords (1,0) - flipped V
            
            xrel,     yrel + h, 0.0, 1.0,  // Top-left corner, tex coords (0,1) - flipped V
            xrel + w, yrel,     1.0, 0.0,  // Bottom-right corner, tex coords (1,0) - flipped V
            xrel + w, yrel + h, 1.0, 1.0,  // Top-right corner, tex coords (1,1) - flipped V
        ];
        
        // Upload vertex data
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const std::ffi::c_void,
            gl::STATIC_DRAW,
        );
        
        // Render quad
        gl::DrawArrays(gl::TRIANGLES, 0, 6);
        
        // Return advance for next character
        Ok(glyph.advance * scale)
    }
    
    /// Calculate the total width of a text string with the current font and scale
    unsafe fn calculate_text_width(&mut self, text: &str, scale: f32, orientation: TextOrientation) -> Result<f32, String> {
        match orientation {
            TextOrientation::Horizontal => {
                // For horizontal text, width is the sum of character advances
                let mut total_width = 0.0;
                for ch in text.chars() {
                    let glyph = self.get_or_cache_glyph(ch)?;
                    total_width += glyph.advance * scale;
                }
                Ok(total_width)
            },
            TextOrientation::Vertical => {
                // For vertical text, width is the maximum character width
                let mut max_width = 0.0;
                for ch in text.chars() {
                    let glyph = self.get_or_cache_glyph(ch)?;
                    let char_width = glyph.width * scale;
                    if char_width > max_width {
                        max_width = char_width;
                    }
                }
                Ok(max_width)
            }
        }
    }
    
    /// Calculate the maximum height of a text string with the current font and scale
    unsafe fn calculate_text_height(&mut self, text: &str, scale: f32, orientation: TextOrientation) -> Result<f32, String> {
        match orientation {
            TextOrientation::Horizontal => {
                // For horizontal text, height is the maximum character height
                let mut max_height = 0.0;
                let mut max_descent = 0.0;
                
                for ch in text.chars() {
                    let glyph = self.get_or_cache_glyph(ch)?;
                    let char_height = glyph.bearing_y * scale;
                    let char_descent = (glyph.height - glyph.bearing_y) * scale;
                    
                    if char_height > max_height {
                        max_height = char_height;
                    }
                    if char_descent > max_descent {
                        max_descent = char_descent;
                    }
                }
                
                Ok(max_height + max_descent)
            },
            TextOrientation::Vertical => {
                // For vertical text, height is the sum of character heights plus spacing
                let mut total_height = 0.0;
                for ch in text.chars() {
                    let glyph = self.get_or_cache_glyph(ch)?;
                    total_height += glyph.height * scale + scale * 2.0; // Add spacing
                }
                Ok(total_height)
            }
        }
    }
    
    /// Calculate both width and height of a text string (convenience function)
    unsafe fn calculate_text_dimensions(&mut self, text: &str, scale: f32, orientation: TextOrientation) -> Result<(f32, f32), String> {
        let width = self.calculate_text_width(text, scale, orientation)?;
        let height = self.calculate_text_height(text, scale, orientation)?;
        Ok((width, height))
    }
    
    /// Get the line height for the current font (useful for multi-line text)
    fn get_line_height(&self, scale: f32) -> f32 {
        unsafe {
            let face_ref = &*self.ft_face;
            (face_ref.size as *const ft::FT_SizeRec).as_ref().unwrap().metrics.height as f32 / 64.0 * scale
        }
    }
    
    /// Get the baseline-to-baseline distance for the current font
    fn get_line_spacing(&self, scale: f32) -> f32 {
        // Use line height as default line spacing
        self.get_line_height(scale)
    }
}

impl Drop for OpenGLTextRenderer {
    fn drop(&mut self) {
        unsafe {
            if !self.ft_face.is_null() {
                ft::FT_Done_Face(self.ft_face);
            }
            if !self.ft_library.is_null() {
                ft::FT_Done_FreeType(self.ft_library);
            }
            
            // Clean up cached glyph textures
            for cached_glyph in self.glyph_cache.values() {
                gl::DeleteTextures(1, &cached_glyph.texture_id);
            }
            // Note: VAO/VBO cleanup would need proper OpenGL context
        }
    }
}
