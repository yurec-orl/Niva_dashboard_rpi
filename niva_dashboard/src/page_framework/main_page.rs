use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, DIAG_PAGE_ID};
use crate::page_framework::events::{EventSender, EventReceiver, SmartEventSender};
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::hw_providers::{*};
use crate::hardware::sensor_value::SensorValue;
use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::text_indicator::{TextIndicator, TextAlignment};
use crate::indicator_builders::{
    build_speedometer_gauge, build_fuel_level_gauge, build_oil_pressure_gauge, build_temperature_gauge, build_voltage_gauge,
    build_oil_pressure_bar, build_fuel_level_bar, build_temperature_bar, build_voltage_bar,
    build_speed_digital
};
use crate::page_framework::events::UIEvent;

struct IndicatorSet {
    indicators: Vec<Box<dyn Indicator>>,
    inputs: Vec<HWInput>, // Corresponding hardware inputs for each indicator
    indicator_bounds: Vec<IndicatorBounds>,
}

pub struct MainPage {
    base: PageBase,
    current_indicator_set: usize,
    indicator_sets: Vec<IndicatorSet>,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,
}

impl MainPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver, context: &GraphicsContext, ui_style: &UIStyle) -> Self {
        let test_indicator_set = Self::setup_test_indicators(ui_style);
        let gauge_indicator_set = Self::setup_gauge_indicators(context, ui_style);
        let bar_indicator_set = Self::setup_bar_indicators(context, ui_style);

        let mut main_page = MainPage {
            base: PageBase::new(id, "Main".to_string()),
            smart_event_sender: smart_event_sender.clone(),
            event_receiver,
            indicator_sets: vec![bar_indicator_set, gauge_indicator_set, test_indicator_set],
            current_indicator_set: 0,
        };

        // Set up default buttons for the main page
        main_page.setup_buttons();
        
        main_page
    }

    fn setup_test_indicators(ui_style: &UIStyle) -> IndicatorSet {
        let mut indicators: Vec<Box<dyn Indicator>> = Vec::new();
        let inputs: Vec<HWInput> = vec![
            HWInput::Hw12v,
            HWInput::HwFuelLvl,
            HWInput::HwOilPress,
            HWInput::HwEngineCoolantTemp,
            HWInput::HwBrakeFluidLvlLow,
            HWInput::HwCharge,
            HWInput::HwCheckEngine,
            HWInput::HwDiffLock,
            HWInput::HwExtLights,
            HWInput::HwFuelLvlLow,
            HWInput::HwHighBeam,
            HWInput::HwOilPressLow,
            HWInput::HwParkBrake,
            HWInput::HwSpeed,
            HWInput::HwTacho,
            HWInput::HwTurnSignal
        ];
        let mut indicator_bounds: Vec<IndicatorBounds> = Vec::new();

        // Screen layout: assuming 800x480 resolution
        // Grid: 4 columns x 4 rows for 16 sensors
        // Left and right margins: 10 chars * 12px = 120px each side
        // Available width: 800 - 240 = 560px
        // Column width: 560 / 4 = 140px
        // Row height: 480 / 4 = 120px
        let margin_left = 120.0;
        let margin_top = 80.0;
        let col_width = 140.0;
        let row_height = 64.0;
        let indicator_width = 64.0;
        let indicator_height = 40.0;

        let mut col = 0;
        let mut row = 0;

        // Helper function to create indicator bounds and advance grid position
        let create_bounds_and_advance = |col: &mut usize, row: &mut usize| -> IndicatorBounds {
            let x = margin_left + (*col as f32 * col_width);
            let y = margin_top + (*row as f32 * row_height);
            let bounds = IndicatorBounds::new(x, y, indicator_width, indicator_height);
            
            *col += 1;
            if *col >= 4 {
                *col = 0;
                *row += 1;
            }
            bounds
        };

        let indicator_font = ui_style.get_string(TEXT_SECONDARY_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let indicator_font_size = ui_style.get_integer(TEXT_SECONDARY_FONT_SIZE, 10) as u32;
        let indicator_color = ui_style.get_color(TEXT_SECONDARY_COLOR, (1.0, 1.0, 1.0));
        let indicator_warning_color = ui_style.get_color(TEXT_WARNING_COLOR, (1.0, 1.0, 0.0));
        let indicator_error_color = ui_style.get_color(TEXT_ERROR_COLOR, (1.0, 0.0, 0.0));

        // Digital sensors (12 total)
        for _ in 0..12 {
            indicators.push(Box::new(TextIndicator::new(
                0, true, true, TextAlignment::Center,
                indicator_font.clone(),
                indicator_font_size, 1.0,
                indicator_color, indicator_warning_color, indicator_error_color,
            )));
            indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));
        }

        // Analog sensors (4 total) - with different precision settings
        // 12V (1 decimal place)
        indicators.push(Box::new(TextIndicator::new(
            1, true, true, TextAlignment::Center,
            indicator_font.clone(),
            indicator_font_size, 1.0,
            indicator_color, indicator_warning_color, indicator_error_color,
        )));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));

        // Fuel Level (1 decimal place)
        indicators.push(Box::new(TextIndicator::new(
            1, true, true, TextAlignment::Center,
            indicator_font.clone(),
            indicator_font_size, 1.0,
            indicator_color, indicator_warning_color, indicator_error_color,
        )));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));

        // Oil Pressure (2 decimal places)
        indicators.push(Box::new(TextIndicator::new(
            2, true, true, TextAlignment::Center,
            indicator_font.clone(),
            indicator_font_size, 1.0,
            indicator_color, indicator_warning_color, indicator_error_color,
        )));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));

        // Temperature (1 decimal place)
        indicators.push(Box::new(TextIndicator::new(
            1, true, true, TextAlignment::Center,
            indicator_font.clone(),
            indicator_font_size, 1.0,
            indicator_color, indicator_warning_color, indicator_error_color,
        )));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));
        IndicatorSet { indicators, inputs, indicator_bounds }
    }

    fn setup_gauge_indicators(context: &GraphicsContext, ui_style: &UIStyle) -> IndicatorSet {
        let mut indicators: Vec<Box<dyn Indicator>> = Vec::new();
        let inputs: Vec<HWInput> = vec![
            HWInput::HwSpeed,
            HWInput::HwFuelLvl,
            HWInput::HwOilPress,
            HWInput::HwEngineCoolantTemp,
            HWInput::Hw12v,
        ];
        let mut indicator_bounds: Vec<IndicatorBounds> = Vec::new();

        // Main indicator set layout:
        // 1. Large central speedometer (gauge)
        // 2. Smaller fuel level and oil pressure gauges on the left
        // 3. Smaller temperature and battery voltage gauges on the right

        let screen_width = context.width as f32;
        let screen_height = context.height as f32;
        
        // Layout parameters
        let button_margin = 60.0; // Space for buttons on left/right
        let top_margin = 40.0;

        // Central speedometer - large gauge (RPM/Speed)
        let center_gauge_radius = 150.0;
        let center_x = screen_width / 2.0;
        let center_y = top_margin + center_gauge_radius;
        
        let (speedometer, speedometer_bounds) = build_speedometer_gauge(center_x, center_y, center_gauge_radius, ui_style);
        indicators.push(speedometer);
        indicator_bounds.push(speedometer_bounds);

        // Left side gauges - smaller gauges
        let side_gauge_radius = 90.0;
        let left_x = button_margin + side_gauge_radius;
        
        // Fuel level gauge (left top)
        let fuel_y = top_margin + side_gauge_radius;
        let (fuel_gauge, fuel_bounds) = build_fuel_level_gauge(left_x, fuel_y, side_gauge_radius, ui_style);
        indicators.push(fuel_gauge);
        indicator_bounds.push(fuel_bounds);
        
        // Oil pressure gauge (left bottom)
        let oil_y = fuel_y + side_gauge_radius * 2.0 + 20.0;
        let (oil_gauge, oil_bounds) = build_oil_pressure_gauge(left_x, oil_y, side_gauge_radius, ui_style);
        indicators.push(oil_gauge);
        indicator_bounds.push(oil_bounds);

        // Right side gauges - smaller gauges
        let right_x = screen_width - button_margin - side_gauge_radius;
        
        // Temperature gauge (right top)
        let temp_y = top_margin + side_gauge_radius;
        let (temp_gauge, temp_bounds) = build_temperature_gauge(right_x, temp_y, side_gauge_radius, ui_style);
        indicators.push(temp_gauge);
        indicator_bounds.push(temp_bounds);
        
        // Battery voltage gauge (right bottom)
        let battery_y = temp_y + side_gauge_radius * 2.0 + 20.0;
        let (voltage_gauge, voltage_bounds) = build_voltage_gauge(right_x, battery_y, side_gauge_radius, ui_style);
        indicators.push(voltage_gauge);
        indicator_bounds.push(voltage_bounds);

        IndicatorSet { indicators, inputs, indicator_bounds }
    }

    fn setup_bar_indicators(context: &GraphicsContext, ui_style: &UIStyle) -> IndicatorSet {
        let mut indicators: Vec<Box<dyn Indicator>> = Vec::new();
        let inputs: Vec<HWInput> = vec![
            HWInput::HwOilPress,
            HWInput::HwFuelLvl,
            HWInput::HwEngineCoolantTemp,
            HWInput::Hw12v,
            HWInput::HwSpeed,
        ];
        let mut indicator_bounds: Vec<IndicatorBounds> = Vec::new();

        // Layout parameters
        let screen_width = context.width as f32;
        let screen_height = context.height as f32;
        let button_margin = 40.0; // Space for buttons on left/right
        let top_margin = 40.0;
        let available_width = screen_width - 2.0 * button_margin;
        
        // Arrange vertical bar indicators in a row
        let bar_width = 52.0;
        let bar_height = 200.0;

        // Oil pressure indicator (leftmost)
        let (oil_pressure_bar, oil_pressure_bounds) = build_oil_pressure_bar(
            button_margin + bar_width,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        indicators.push(oil_pressure_bar);
        indicator_bounds.push(oil_pressure_bounds);
        
        // Fuel level indicator
        let (fuel_level_bar, fuel_level_bounds) = build_fuel_level_bar(
            button_margin + bar_width * 2.0 + 50.0,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        indicators.push(fuel_level_bar);
        indicator_bounds.push(fuel_level_bounds);

        // Temperature indicator
        let (temperature_bar, temperature_bounds) = build_temperature_bar(
            available_width - bar_width * 2.0 - 50.0,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        indicators.push(temperature_bar);
        indicator_bounds.push(temperature_bounds);

        // Voltage indicator (rightmost)
        let (voltage_bar, voltage_bounds) = build_voltage_bar(
            available_width - bar_width,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        indicators.push(voltage_bar);
        indicator_bounds.push(voltage_bounds);

        // Speed digital display (centered)
        let (speed_digital, speed_bounds) = build_speed_digital(
            (screen_width - 200.0) / 2.0,
            top_margin,
            200.0,
            80.0,
            ui_style
        );
        indicators.push(speed_digital);
        indicator_bounds.push(speed_bounds);

        IndicatorSet { indicators, inputs, indicator_bounds }
    }

    // Setup default buttons for main page using event system
    fn setup_buttons(&mut self) {
        let smart_sender = self.smart_event_sender.clone();
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ВИД+".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::NextIndicatorSet)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ВИД-".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::PreviousIndicatorSet)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "СБРОС".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SuppressAlerts)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "ЯРК+".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::BrightnessUp)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right2, "ЯРК-".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::BrightnessDown)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ДИАГ".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(DIAG_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];

        self.base.set_buttons(buttons);
    }

    // Event handler methods for indicator set navigation
    fn next_indicator_set(&mut self) {
        if self.indicator_sets.len() > 1 {
            self.current_indicator_set = (self.current_indicator_set + 1) % self.indicator_sets.len();
            print!("MainPage: Switched to indicator set {}\r\n", self.current_indicator_set);
        }
    }

    fn previous_indicator_set(&mut self) {
        if self.indicator_sets.len() > 1 {
            if self.current_indicator_set == 0 {
                self.current_indicator_set = self.indicator_sets.len() - 1;
            } else {
                self.current_indicator_set -= 1;
            }
            print!("MainPage: Switched to indicator set {}\r\n", self.current_indicator_set);
        }
    }

    fn reset_to_first_indicator_set(&mut self) {
        self.current_indicator_set = 0;
        print!("MainPage: Reset to first indicator set\r\n");
    }
}

impl Page for MainPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn render(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        // Read sensor values and create SensorValue objects
        let sensor_values = sensor_manager.get_sensor_values();

        // Render each indicator with its corresponding sensor value
        let indicators = self.indicator_sets[self.current_indicator_set].indicators.iter();
        let current_inputs = &self.indicator_sets[self.current_indicator_set].inputs;
        let indicator_bounds = &self.indicator_sets[self.current_indicator_set].indicator_bounds;
        
        for (i, indicator) in indicators.enumerate() {
            if let Some(sensor_value) = sensor_values.get(&current_inputs[i]) {
                //print!("Rendering indicator {} for sensor {:?} with value {:?}\r\n", indicator.indicator_type(), sensor_value.metadata.sensor_id, sensor_value.value);
                if let Some(bounds) = indicator_bounds.get(i) {
                    indicator.render(sensor_value, bounds.clone(), ui_style, context)?;
                }
            }
        }

        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_exit(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_button(&mut self, _button: char) -> Result<(), String> {
        Ok(())
    }

    fn process_events(&mut self) {
        // Process events specific to the main page
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                crate::page_framework::events::UIEvent::NextIndicatorSet => {
                    print!("MainPage: NextIndicatorSet event received\r\n");
                    self.next_indicator_set();
                }
                crate::page_framework::events::UIEvent::PreviousIndicatorSet => {
                    print!("MainPage: PreviousIndicatorSet event received\r\n");
                    self.previous_indicator_set();
                }
                crate::page_framework::events::UIEvent::ButtonPressed(action) => {
                    match action.as_str() {
                        "next_view" => self.next_indicator_set(),
                        "prev_view" => self.previous_indicator_set(),
                        "reset_view" => self.reset_to_first_indicator_set(),
                        _ => {} // Ignore unknown actions
                    }
                }
                // With dual-channel system, MainPage only receives page-specific events
                // Global events go directly to PageManager via global channel
                _ => {
                    print!("MainPage: Ignoring unknown page event: {:?}\r\n", event);
                }
            }
        }
    }

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        self.base.buttons()
    }

    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position(pos)
    }

    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position_mut(pos)
    }
}