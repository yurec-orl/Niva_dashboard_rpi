// SDL2-based gauge rendering for Niva Dashboard
// This demonstrates how high-level SDL2 can create professional dashboard gauges

use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::f64::consts::PI;
use crate::graphics::context::GraphicsContext;

/// SDL2-based gauge renderer using high-level 2D graphics
pub struct SDL2GaugeRenderer {
    canvas: Canvas<Window>,
    texture_creator: TextureCreator<WindowContext>,
}

impl SDL2GaugeRenderer {
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, String> {
        let sdl_context = sdl2::init().map_err(|e| e.to_string())?;
        let video_subsystem = sdl_context.video().map_err(|e| e.to_string())?;
        
        let window = video_subsystem
            .window(title, width, height)
            .position_centered()
            .build()
            .map_err(|e| e.to_string())?;
        
        let canvas = window.into_canvas()
            .accelerated()
            .present_vsync()
            .build()
            .map_err(|e| e.to_string())?;
        
        let texture_creator = canvas.texture_creator();
        
        Ok(SDL2GaugeRenderer {
            canvas,
            texture_creator,
        })
    }
    
    /// Render a complete automotive-style speedometer
    pub fn render_speedometer(&mut self, center_x: i32, center_y: i32, radius: i32, 
                             speed: f64, max_speed: f64) -> Result<(), String> {
        // Draw gauge background circle
        self.draw_filled_circle(center_x, center_y, radius, Color::RGB(20, 20, 30))?;
        self.draw_circle_outline(center_x, center_y, radius, Color::RGB(100, 100, 120), 3)?;
        
        // Draw speed markings (0 to max_speed)
        let num_major_ticks = 8;
        let num_minor_ticks = 40;
        
        // Major tick marks and numbers
        for i in 0..=num_major_ticks {
            let angle = -225.0 + (270.0 * i as f64 / num_major_ticks as f64);
            let tick_value = (max_speed * i as f64 / num_major_ticks as f64) as i32;
            
            self.draw_gauge_tick(center_x, center_y, radius, angle, 15, 4, 
                               Color::RGB(200, 200, 220))?;
            
            // Add speed numbers
            let text_radius = radius - 25;
            let text_x = center_x + (text_radius as f64 * angle.to_radians().sin()) as i32;
            let text_y = center_y - (text_radius as f64 * angle.to_radians().cos()) as i32;
            
            // Note: You would use TTF here for actual text rendering
            self.draw_small_rect(text_x - 2, text_y - 2, 4, 4, Color::RGB(255, 255, 255))?;
        }
        
        // Minor tick marks
        for i in 0..num_minor_ticks {
            let angle = -225.0 + (270.0 * i as f64 / num_minor_ticks as f64);
            self.draw_gauge_tick(center_x, center_y, radius, angle, 8, 2, 
                               Color::RGB(120, 120, 140))?;
        }
        
        // Draw speed needle
        let needle_angle = -225.0 + (270.0 * speed / max_speed);
        self.draw_gauge_needle(center_x, center_y, radius - 20, needle_angle, 
                              Color::RGB(255, 50, 50))?;
        
        // Draw center hub
        self.draw_filled_circle(center_x, center_y, 8, Color::RGB(150, 150, 150))?;
        
        Ok(())
    }
    
    /// Render an RPM gauge (tachometer)
    pub fn render_rpm_gauge(&mut self, center_x: i32, center_y: i32, radius: i32, 
                           rpm: f64, max_rpm: f64) -> Result<(), String> {
        // Similar to speedometer but with different styling
        self.draw_filled_circle(center_x, center_y, radius, Color::RGB(30, 15, 15))?;
        self.draw_circle_outline(center_x, center_y, radius, Color::RGB(150, 100, 100), 3)?;
        
        // RPM-specific color zones
        let redline_start = 0.85; // 85% of max RPM
        
        // Draw RPM zones with colors
        let num_zones = 8;
        for i in 0..=num_zones {
            let angle = -225.0 + (270.0 * i as f64 / num_zones as f64);
            let zone_ratio = i as f64 / num_zones as f64;
            
            let color = if zone_ratio >= redline_start {
                Color::RGB(255, 100, 100) // Red zone
            } else if zone_ratio >= 0.7 {
                Color::RGB(255, 200, 100) // Yellow zone  
            } else {
                Color::RGB(100, 255, 100) // Green zone
            };
            
            self.draw_gauge_tick(center_x, center_y, radius, angle, 12, 3, color)?;
        }
        
        // Draw RPM needle
        let needle_angle = -225.0 + (270.0 * rpm / max_rpm);
        let needle_color = if rpm / max_rpm >= redline_start {
            Color::RGB(255, 100, 100)
        } else {
            Color::RGB(255, 200, 50)
        };
        
        self.draw_gauge_needle(center_x, center_y, radius - 15, needle_angle, needle_color)?;
        self.draw_filled_circle(center_x, center_y, 6, Color::RGB(180, 140, 100))?;
        
        Ok(())
    }
    
    /// Render a fuel gauge (horizontal or vertical bar style)
    pub fn render_fuel_gauge(&mut self, x: i32, y: i32, width: i32, height: i32, 
                            fuel_percent: f64) -> Result<(), String> {
        // Background
        self.canvas.set_draw_color(Color::RGB(25, 25, 35));
        self.canvas.fill_rect(Rect::new(x, y, width as u32, height as u32))
            .map_err(|e| e.to_string())?;
        
        // Border
        self.canvas.set_draw_color(Color::RGB(100, 120, 100));
        self.canvas.draw_rect(Rect::new(x, y, width as u32, height as u32))
            .map_err(|e| e.to_string())?;
        
        // Fuel level fill
        let fill_height = (height as f64 * fuel_percent / 100.0) as i32;
        let fill_y = y + height - fill_height;
        
        let fuel_color = if fuel_percent < 15.0 {
            Color::RGB(255, 100, 100) // Low fuel - red
        } else if fuel_percent < 25.0 {
            Color::RGB(255, 200, 100) // Warning - yellow
        } else {
            Color::RGB(100, 255, 100) // Normal - green
        };
        
        self.canvas.set_draw_color(fuel_color);
        self.canvas.fill_rect(Rect::new(x + 2, fill_y, (width - 4) as u32, fill_height as u32))
            .map_err(|e| e.to_string())?;
        
        // Fuel level markers
        for i in 0..=4 {
            let marker_y = y + (height * i / 4);
            self.canvas.set_draw_color(Color::RGB(200, 200, 200));
            self.canvas.draw_line(
                Point::new(x + width - 10, marker_y),
                Point::new(x + width - 5, marker_y)
            ).map_err(|e| e.to_string())?;
        }
        
        Ok(())
    }
    
    /// Render a temperature gauge (circular arc style)
    pub fn render_temperature_gauge(&mut self, center_x: i32, center_y: i32, radius: i32, 
                                   temp_celsius: f64, max_temp: f64) -> Result<(), String> {
        // Temperature gauge as a partial circle (bottom half)
        let start_angle = -180.0;
        let end_angle = 0.0;
        let temp_ratio = temp_celsius / max_temp;
        
        // Background arc
        self.draw_arc(center_x, center_y, radius, start_angle, end_angle, 
                     Color::RGB(30, 30, 40), 8)?;
        
        // Temperature zones
        let normal_temp = 90.0; // Normal operating temperature
        let warning_temp = 105.0; // Warning temperature
        
        let temp_color = if temp_celsius >= warning_temp {
            Color::RGB(255, 50, 50) // Overheating - red
        } else if temp_celsius >= normal_temp {
            Color::RGB(255, 200, 50) // Warm - yellow
        } else {
            Color::RGB(50, 150, 255) // Cold - blue
        };
        
        // Temperature level arc
        let temp_end_angle = start_angle + (end_angle - start_angle) * temp_ratio;
        self.draw_arc(center_x, center_y, radius, start_angle, temp_end_angle, 
                     temp_color, 6)?;
        
        // Temperature markings
        let num_marks = 6;
        for i in 0..=num_marks {
            let angle = start_angle + (end_angle - start_angle) * i as f64 / num_marks as f64;
            self.draw_gauge_tick(center_x, center_y, radius, angle, 10, 2, 
                               Color::RGB(180, 180, 200))?;
        }
        
        Ok(())
    }
    
    // Helper drawing methods
    fn draw_filled_circle(&mut self, x: i32, y: i32, radius: i32, color: Color) -> Result<(), String> {
        // SDL2 doesn't have built-in circle drawing, so we approximate with filled rects
        self.canvas.set_draw_color(color);
        
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    self.canvas.draw_point(Point::new(x + dx, y + dy))
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
    
    fn draw_circle_outline(&mut self, x: i32, y: i32, radius: i32, color: Color, 
                          thickness: i32) -> Result<(), String> {
        self.canvas.set_draw_color(color);
        
        for t in 0..thickness {
            let r = radius - t;
            for angle in 0..360 {
                let rad = (angle as f64 * PI / 180.0);
                let px = x + (r as f64 * rad.cos()) as i32;
                let py = y + (r as f64 * rad.sin()) as i32;
                self.canvas.draw_point(Point::new(px, py))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    
    fn draw_gauge_tick(&mut self, center_x: i32, center_y: i32, radius: i32, 
                      angle_degrees: f64, length: i32, thickness: i32, 
                      color: Color) -> Result<(), String> {
        self.canvas.set_draw_color(color);
        
        let angle_rad = angle_degrees.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();
        
        let start_x = center_x + ((radius - length) as f64 * sin_a) as i32;
        let start_y = center_y - ((radius - length) as f64 * cos_a) as i32;
        let end_x = center_x + (radius as f64 * sin_a) as i32;
        let end_y = center_y - (radius as f64 * cos_a) as i32;
        
        // Draw thick line by drawing multiple parallel lines
        for t in 0..thickness {
            let offset_x = if thickness > 1 { t - thickness/2 } else { 0 };
            let offset_y = if thickness > 1 { t - thickness/2 } else { 0 };
            
            self.canvas.draw_line(
                Point::new(start_x + offset_x, start_y + offset_y),
                Point::new(end_x + offset_x, end_y + offset_y)
            ).map_err(|e| e.to_string())?;
        }
        
        Ok(())
    }
    
    fn draw_gauge_needle(&mut self, center_x: i32, center_y: i32, length: i32, 
                        angle_degrees: f64, color: Color) -> Result<(), String> {
        self.canvas.set_draw_color(color);
        
        let angle_rad = angle_degrees.to_radians();
        let end_x = center_x + (length as f64 * angle_rad.sin()) as i32;
        let end_y = center_y - (length as f64 * angle_rad.cos()) as i32;
        
        // Draw needle as thick line
        for thickness in 0..3 {
            let offset = thickness - 1;
            self.canvas.draw_line(
                Point::new(center_x + offset, center_y + offset),
                Point::new(end_x + offset, end_y + offset)
            ).map_err(|e| e.to_string())?;
        }
        
        Ok(())
    }
    
    fn draw_arc(&mut self, center_x: i32, center_y: i32, radius: i32, 
               start_angle: f64, end_angle: f64, color: Color, thickness: i32) -> Result<(), String> {
        self.canvas.set_draw_color(color);
        
        let steps = ((end_angle - start_angle).abs() * 2.0) as i32;
        
        for step in 0..steps {
            let angle = start_angle + (end_angle - start_angle) * step as f64 / steps as f64;
            let angle_rad = angle.to_radians();
            
            for t in 0..thickness {
                let r = radius - t;
                let x = center_x + (r as f64 * angle_rad.cos()) as i32;
                let y = center_y + (r as f64 * angle_rad.sin()) as i32;
                self.canvas.draw_point(Point::new(x, y))
                    .map_err(|e| e.to_string())?;
            }
        }
        
        Ok(())
    }
    
    fn draw_small_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) -> Result<(), String> {
        self.canvas.set_draw_color(color);
        self.canvas.fill_rect(Rect::new(x, y, w as u32, h as u32))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    
    pub fn clear(&mut self, color: Color) {
        self.canvas.set_draw_color(color);
        self.canvas.clear();
    }
    
    pub fn present(&mut self) {
        self.canvas.present();
    }
}

/// Run SDL2-based gauge test
pub fn run_sdl2_gauges_test(_context: &GraphicsContext) -> Result<(), String> {
    println!("Starting SDL2 High-Level Gauge Rendering Test...");
    println!("Note: This test creates its own SDL2 context separate from OpenGL");
    
    // Initialize SDL2 separately for gauge rendering
    let sdl_context = sdl2::init().map_err(|e| e.to_string())?;
    let video_subsystem = sdl_context.video().map_err(|e| e.to_string())?;
    
    // Create window for gauge rendering
    let window = video_subsystem
        .window("Niva Dashboard - SDL2 Gauges", 800, 480)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;
    
    let mut canvas = window.into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    
    let mut event_pump = sdl_context.event_pump().map_err(|e| e.to_string())?;
    
    let mut frame_count = 0;
    let total_frames = 600; // 10 seconds at 60fps
    
    'running: loop {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } |
                sdl2::event::Event::KeyDown { keycode: Some(sdl2::keyboard::Keycode::Escape), .. } => {
                    break 'running;
                }
                _ => {}
            }
        }
        
        // Clear with dashboard background
        canvas.set_draw_color(Color::RGB(8, 8, 12));
        canvas.clear();
        
        // Animate gauge values
        let time = frame_count as f64 / 60.0; // Time in seconds
        let speed = 40.0 + 45.0 * (time * 0.5).sin().abs(); // 40-85 km/h
        let rpm = 1500.0 + 2000.0 * (time * 0.3).sin().abs(); // 1500-3500 RPM
        let fuel = 75.0 - 50.0 * (time * 0.1).sin().abs(); // 25-75% fuel
        let temp = 85.0 + 15.0 * (time * 0.2).sin(); // 70-100°C temperature
        
        // Draw gauges directly using canvas
        draw_speedometer(&mut canvas, 150, 150, 80, speed, 120.0)?;
        draw_rpm_gauge(&mut canvas, 650, 150, 80, rpm, 6000.0)?;
        draw_fuel_gauge(&mut canvas, 50, 300, 30, 120, fuel)?;
        draw_temperature_gauge(&mut canvas, 650, 350, 70, temp, 120.0)?;
        
        // Present the frame
        canvas.present();
        
        frame_count += 1;
        if frame_count >= total_frames {
            break 'running;
        }
        
        // Print status occasionally
        if frame_count % 60 == 0 {
            println!("Frame {} - Speed: {:.1} km/h, RPM: {:.0}, Fuel: {:.1}%, Temp: {:.1}°C", 
                    frame_count, speed, rpm, fuel, temp);
        }
        
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
    }
    
    println!("SDL2 gauge rendering test completed successfully!");
    Ok(())
}

// Standalone gauge drawing functions that work directly with SDL2 canvas
// These avoid the SDL2 initialization issue by not creating their own context

/// Draw a speedometer directly on the canvas
fn draw_speedometer(canvas: &mut Canvas<Window>, center_x: i32, center_y: i32, radius: i32, 
                   speed: f64, max_speed: f64) -> Result<(), String> {
    // Draw gauge background circle
    draw_filled_circle(canvas, center_x, center_y, radius, Color::RGB(20, 20, 30))?;
    draw_circle_outline(canvas, center_x, center_y, radius, Color::RGB(100, 100, 120), 3)?;
    
    // Draw speed markings (0 to max_speed)
    let num_major_ticks = 8;
    let num_minor_ticks = 40;
    
    // Major tick marks and numbers
    for i in 0..=num_major_ticks {
        let angle = -225.0 + (270.0 * i as f64 / num_major_ticks as f64);
        
        draw_gauge_tick(canvas, center_x, center_y, radius, angle, 15, 4, 
                       Color::RGB(200, 200, 220))?;
        
        // Add speed numbers (placeholder dots)
        let text_radius = radius - 25;
        let text_x = center_x + (text_radius as f64 * angle.to_radians().sin()) as i32;
        let text_y = center_y - (text_radius as f64 * angle.to_radians().cos()) as i32;
        
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        canvas.fill_rect(Rect::new(text_x - 2, text_y - 2, 4, 4))
            .map_err(|e| e.to_string())?;
    }
    
    // Minor tick marks
    for i in 0..num_minor_ticks {
        let angle = -225.0 + (270.0 * i as f64 / num_minor_ticks as f64);
        draw_gauge_tick(canvas, center_x, center_y, radius, angle, 8, 2, 
                       Color::RGB(120, 120, 140))?;
    }
    
    // Draw speed needle
    let needle_angle = -225.0 + (270.0 * speed / max_speed);
    draw_gauge_needle(canvas, center_x, center_y, radius - 20, needle_angle, 
                      Color::RGB(255, 50, 50))?;
    
    // Draw center hub
    draw_filled_circle(canvas, center_x, center_y, 8, Color::RGB(150, 150, 150))?;
    
    Ok(())
}

/// Draw an RPM gauge directly on the canvas
fn draw_rpm_gauge(canvas: &mut Canvas<Window>, center_x: i32, center_y: i32, radius: i32, 
                 rpm: f64, max_rpm: f64) -> Result<(), String> {
    // Similar to speedometer but with different styling
    draw_filled_circle(canvas, center_x, center_y, radius, Color::RGB(30, 15, 15))?;
    draw_circle_outline(canvas, center_x, center_y, radius, Color::RGB(150, 100, 100), 3)?;
    
    // RPM-specific color zones
    let redline_start = 0.85; // 85% of max RPM
    
    // Draw RPM zones with colors
    let num_zones = 8;
    for i in 0..=num_zones {
        let angle = -225.0 + (270.0 * i as f64 / num_zones as f64);
        let zone_ratio = i as f64 / num_zones as f64;
        
        let color = if zone_ratio >= redline_start {
            Color::RGB(255, 100, 100) // Red zone
        } else if zone_ratio >= 0.7 {
            Color::RGB(255, 200, 100) // Yellow zone  
        } else {
            Color::RGB(100, 255, 100) // Green zone
        };
        
        draw_gauge_tick(canvas, center_x, center_y, radius, angle, 12, 3, color)?;
    }
    
    // Draw RPM needle
    let needle_angle = -225.0 + (270.0 * rpm / max_rpm);
    let needle_color = if rpm / max_rpm >= redline_start {
        Color::RGB(255, 100, 100)
    } else {
        Color::RGB(255, 200, 50)
    };
    
    draw_gauge_needle(canvas, center_x, center_y, radius - 15, needle_angle, needle_color)?;
    draw_filled_circle(canvas, center_x, center_y, 6, Color::RGB(180, 140, 100))?;
    
    Ok(())
}

/// Draw a fuel gauge directly on the canvas
fn draw_fuel_gauge(canvas: &mut Canvas<Window>, x: i32, y: i32, width: i32, height: i32, 
                  fuel_percent: f64) -> Result<(), String> {
    // Background
    canvas.set_draw_color(Color::RGB(25, 25, 35));
    canvas.fill_rect(Rect::new(x, y, width as u32, height as u32))
        .map_err(|e| e.to_string())?;
    
    // Border
    canvas.set_draw_color(Color::RGB(100, 120, 100));
    canvas.draw_rect(Rect::new(x, y, width as u32, height as u32))
        .map_err(|e| e.to_string())?;
    
    // Fuel level fill
    let fill_height = (height as f64 * fuel_percent / 100.0) as i32;
    let fill_y = y + height - fill_height;
    
    let fuel_color = if fuel_percent < 15.0 {
        Color::RGB(255, 100, 100) // Low fuel - red
    } else if fuel_percent < 25.0 {
        Color::RGB(255, 200, 100) // Warning - yellow
    } else {
        Color::RGB(100, 255, 100) // Normal - green
    };
    
    canvas.set_draw_color(fuel_color);
    canvas.fill_rect(Rect::new(x + 2, fill_y, (width - 4) as u32, fill_height as u32))
        .map_err(|e| e.to_string())?;
    
    // Fuel level markers
    for i in 0..=4 {
        let marker_y = y + (height * i / 4);
        canvas.set_draw_color(Color::RGB(200, 200, 200));
        canvas.draw_line(
            Point::new(x + width - 10, marker_y),
            Point::new(x + width - 5, marker_y)
        ).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

/// Draw a temperature gauge directly on the canvas
fn draw_temperature_gauge(canvas: &mut Canvas<Window>, center_x: i32, center_y: i32, radius: i32, 
                         temp_celsius: f64, max_temp: f64) -> Result<(), String> {
    // Temperature gauge as a partial circle (bottom half)
    let start_angle = -180.0;
    let end_angle = 0.0;
    let temp_ratio = temp_celsius / max_temp;
    
    // Background arc
    draw_arc(canvas, center_x, center_y, radius, start_angle, end_angle, 
             Color::RGB(30, 30, 40), 8)?;
    
    // Temperature zones
    let normal_temp = 90.0; // Normal operating temperature
    let warning_temp = 105.0; // Warning temperature
    
    let temp_color = if temp_celsius >= warning_temp {
        Color::RGB(255, 50, 50) // Overheating - red
    } else if temp_celsius >= normal_temp {
        Color::RGB(255, 200, 50) // Warm - yellow
    } else {
        Color::RGB(50, 150, 255) // Cold - blue
    };
    
    // Temperature level arc
    let temp_end_angle = start_angle + (end_angle - start_angle) * temp_ratio;
    draw_arc(canvas, center_x, center_y, radius, start_angle, temp_end_angle, 
             temp_color, 6)?;
    
    // Temperature markings
    let num_marks = 6;
    for i in 0..=num_marks {
        let angle = start_angle + (end_angle - start_angle) * i as f64 / num_marks as f64;
        draw_gauge_tick(canvas, center_x, center_y, radius, angle, 10, 2, 
                       Color::RGB(180, 180, 200))?;
    }
    
    Ok(())
}

// Helper drawing functions for standalone use
fn draw_filled_circle(canvas: &mut Canvas<Window>, x: i32, y: i32, radius: i32, color: Color) -> Result<(), String> {
    canvas.set_draw_color(color);
    
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                canvas.draw_point(Point::new(x + dx, y + dy))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn draw_circle_outline(canvas: &mut Canvas<Window>, x: i32, y: i32, radius: i32, color: Color, 
                      thickness: i32) -> Result<(), String> {
    canvas.set_draw_color(color);
    
    for t in 0..thickness {
        let r = radius - t;
        for angle in 0..360 {
            let rad = (angle as f64 * PI / 180.0);
            let px = x + (r as f64 * rad.cos()) as i32;
            let py = y + (r as f64 * rad.sin()) as i32;
            canvas.draw_point(Point::new(px, py))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn draw_gauge_tick(canvas: &mut Canvas<Window>, center_x: i32, center_y: i32, radius: i32, 
                  angle_degrees: f64, length: i32, thickness: i32, 
                  color: Color) -> Result<(), String> {
    canvas.set_draw_color(color);
    
    let angle_rad = angle_degrees.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    
    let start_x = center_x + ((radius - length) as f64 * sin_a) as i32;
    let start_y = center_y - ((radius - length) as f64 * cos_a) as i32;
    let end_x = center_x + (radius as f64 * sin_a) as i32;
    let end_y = center_y - (radius as f64 * cos_a) as i32;
    
    // Draw thick line by drawing multiple parallel lines
    for t in 0..thickness {
        let offset_x = if thickness > 1 { t - thickness/2 } else { 0 };
        let offset_y = if thickness > 1 { t - thickness/2 } else { 0 };
        
        canvas.draw_line(
            Point::new(start_x + offset_x, start_y + offset_y),
            Point::new(end_x + offset_x, end_y + offset_y)
        ).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

fn draw_gauge_needle(canvas: &mut Canvas<Window>, center_x: i32, center_y: i32, length: i32, 
                    angle_degrees: f64, color: Color) -> Result<(), String> {
    canvas.set_draw_color(color);
    
    let angle_rad = angle_degrees.to_radians();
    let end_x = center_x + (length as f64 * angle_rad.sin()) as i32;
    let end_y = center_y - (length as f64 * angle_rad.cos()) as i32;
    
    // Draw needle as thick line
    for thickness in 0..3 {
        let offset = thickness - 1;
        canvas.draw_line(
            Point::new(center_x + offset, center_y + offset),
            Point::new(end_x + offset, end_y + offset)
        ).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

fn draw_arc(canvas: &mut Canvas<Window>, center_x: i32, center_y: i32, radius: i32, 
           start_angle: f64, end_angle: f64, color: Color, thickness: i32) -> Result<(), String> {
    canvas.set_draw_color(color);
    
    let steps = ((end_angle - start_angle).abs() * 2.0) as i32;
    
    for step in 0..steps {
        let angle = start_angle + (end_angle - start_angle) * step as f64 / steps as f64;
        let angle_rad = angle.to_radians();
        
        for t in 0..thickness {
            let r = radius - t;
            let x = center_x + (r as f64 * angle_rad.cos()) as i32;
            let y = center_y + (r as f64 * angle_rad.sin()) as i32;
            canvas.draw_point(Point::new(x, y))
                .map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}
