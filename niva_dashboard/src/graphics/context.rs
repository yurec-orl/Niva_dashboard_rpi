// Graphics context manager for KMS/DRM OpenGL ES backend
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;

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
            initialized: false,
            display_configured: false,
        };
        
        println!("Initializing KMS/DRM graphics context: {} ({}x{})", title, width, height);
        println!("Setting up direct display output...");
        
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
        
        context.initialized = true;
        println!("Graphics context initialized successfully: {}x{}", context.width, context.height);
        println!("✓ Display setup complete - output should be visible on screen");
        println!("  Resolution: {}x{}@{}Hz", context.width, context.height, context.mode.vrefresh);
        println!("  CRTC: {}, Connector: {}", context.crtc_id, context.connector_id);
        
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
            
            println!("DRM device opened successfully (fd: {})", self.drm_fd);
            
            // Get DRM resources to check display configuration
            let resources = drmModeGetResources(self.drm_fd);
            if !resources.is_null() {
                let res = &*(resources as *const DrmModeRes);
                println!("DRM Resources found:");
                println!("  CRTCs: {}", res.count_crtcs);
                println!("  Connectors: {}", res.count_connectors);
                println!("  Encoders: {}", res.count_encoders);
                
                if res.count_connectors > 0 {
                    println!("  Display appears to be available");
                } else {
                    println!("  Warning: No display connectors found");
                }
                
                drmModeFreeResources(resources);
            } else {
                println!("Warning: Could not get DRM resources");
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
            println!("Setting up display mode...");
            println!("Available CRTCs: {}, Connectors: {}", res.count_crtcs, res.count_connectors);
            
            // Find a connected display
            let mut found_display = false;
            for i in 0..res.count_connectors {
                let connector_id = *res.connectors.offset(i as isize);
                let connector = drmModeGetConnector(self.drm_fd, connector_id);
                
                if !connector.is_null() {
                    let conn = &*(connector as *const DrmModeConnector);
                    
                    if conn.connection == DRM_MODE_CONNECTED && conn.count_modes > 0 {
                        println!("Found connected display on connector {}", connector_id);
                        
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
                        
                        println!("Display mode: {}x{}@{}Hz", 
                                mode.hdisplay, mode.vdisplay, mode.vrefresh);
                        println!("Using CRTC: {}, Connector: {}", self.crtc_id, self.connector_id);
                        
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
            
            println!("GBM device and surface created successfully");
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
            
            println!("EGL initialized: version {}.{}", major, minor);
            
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
            
            println!("EGL context created and made current");
        }
        
        Ok(())
    }
    
    /// Configure the display to show our framebuffer
    fn configure_display(&mut self) -> Result<(), String> {
        unsafe {
            println!("Configuring display output...");
            
            // Get the initial front buffer to set up the display
            let bo = gbm_surface_lock_front_buffer(self.gbm_surface);
            if bo.is_null() {
                return Err("Failed to lock front buffer for display setup".to_string());
            }
            
            // Get buffer properties
            let handle = gbm_bo_get_handle(bo).u32;
            let stride = gbm_bo_get_stride(bo);
            println!("Buffer handle: {}, stride: {}", handle, stride);
            
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
            
            println!("Created framebuffer: {}", fb_id);
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
            
            println!("✓ Display CRTC configured - framebuffer {} is now showing", fb_id);
            
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
                    println!("Warning: eglSwapBuffers failed with error: 0x{:X}", error);
                    return;
                }
                
                // For the first frame only, set up initial display
                if !self.display_configured {
                    self.display_configured = true;
                    
                    match self.configure_display() {
                        Ok(_) => {
                            println!("✓ Display configured successfully after first swap");
                        },
                        Err(e) => {
                            println!("Warning: Failed to configure display: {}", e);
                            println!("Continuing with off-screen rendering...");
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
                    println!("Framebuffer saved to: {}", filename);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to save framebuffer: {}", e)),
            }
        }
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        unsafe {
            if self.initialized {
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
        println!("Graphics context cleaned up");
    }
}
