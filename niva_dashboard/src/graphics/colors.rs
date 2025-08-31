// Color constants
const CLR_WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);
const CLR_BLACK: (f32, f32, f32) = (0.0, 0.0, 0.0);
const CLR_RED: (f32, f32, f32) = (1.0, 0.0, 0.0);
const CLR_GREEN: (f32, f32, f32) = (0.0, 1.0, 0.0);
const CLR_BLUE: (f32, f32, f32) = (0.0, 0.0, 1.0);
const CLR_YELLOW: (f32, f32, f32) = (1.0, 1.0, 0.0);
const CLR_CYAN: (f32, f32, f32) = (0.0, 1.0, 1.0);
const CLR_MAGENTA: (f32, f32, f32) = (1.0, 0.0, 1.0);
const CLR_GRAY: (f32, f32, f32) = (0.5, 0.5, 0.5);
const CLR_LIGHT_GRAY: (f32, f32, f32) = (0.75, 0.75, 0.75);
const CLR_DARK_GRAY: (f32, f32, f32) = (0.25, 0.25, 0.25);

/// Color management with software brightness control
pub struct ColorManager {
    brightness: f32, // 0.0 (black) to 1.0 (full brightness)
}

impl ColorManager {
    /// Create a new ColorManager with default brightness
    pub fn new() -> Self {
        Self {
            brightness: 1.0, // Full brightness by default
        }
    }

    /// Set brightness level (0.0 to 1.0)
    /// 0.0 = completely black (display off)
    /// 1.0 = full brightness
    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.0, 1.0);
    }

    /// Get current brightness level
    pub fn get_brightness(&self) -> f32 {
        self.brightness
    }

    /// Apply brightness to a color
    pub fn apply_brightness(&self, color: (f32, f32, f32)) -> (f32, f32, f32) {
        (
            color.0 * self.brightness,
            color.1 * self.brightness,
            color.2 * self.brightness,
        )
    }

    /// Apply brightness to a color with alpha
    pub fn apply_brightness_rgba(&self, color: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        (
            color.0 * self.brightness,
            color.1 * self.brightness,
            color.2 * self.brightness,
            color.3, // Alpha channel is not affected by brightness
        )
    }

    /// Get standard colors with brightness applied
    pub fn white(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_WHITE)
    }

    pub fn black(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_BLACK)
    }

    pub fn red(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_RED)
    }

    pub fn green(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_GREEN)
    }

    pub fn blue(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_BLUE)
    }

    pub fn yellow(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_YELLOW)
    }

    pub fn cyan(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_CYAN)
    }

    pub fn magenta(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_MAGENTA)
    }

    pub fn gray(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_GRAY)
    }

    pub fn light_gray(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_LIGHT_GRAY)
    }

    pub fn dark_gray(&self) -> (f32, f32, f32) {
        self.apply_brightness(CLR_DARK_GRAY)
    }

    /// Increase brightness by a step (useful for brightness controls)
    pub fn increase_brightness(&mut self, step: f32) {
        self.set_brightness(self.brightness + step);
    }

    /// Decrease brightness by a step
    pub fn decrease_brightness(&mut self, step: f32) {
        self.set_brightness(self.brightness - step);
    }

    /// Get a brightness-adjusted color from RGB values
    pub fn rgb(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        self.apply_brightness((r, g, b))
    }

    /// Get a brightness-adjusted color from RGBA values
    pub fn rgba(&self, r: f32, g: f32, b: f32, a: f32) -> (f32, f32, f32, f32) {
        self.apply_brightness_rgba((r, g, b, a))
    }
}

impl Default for ColorManager {
    fn default() -> Self {
        Self::new()
    }
}

