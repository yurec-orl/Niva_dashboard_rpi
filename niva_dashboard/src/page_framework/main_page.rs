use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, DIAG_PAGE_ID};
use crate::page_framework::events::{EventSender, EventReceiver};
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::hw_providers::{*};
use crate::indicators::{Indicator, SensorValue, IndicatorBounds};
use crate::indicators::text_indicator::{TextIndicator, TextAlignment};
use crate::page_framework::events::UIEvent;

struct IndicatorSet {
    indicators: Vec<Box<dyn Indicator>>,
    indicator_bounds: Vec<IndicatorBounds>,
}

pub struct MainPage {
    base: PageBase,
    current_indicator_set: usize,
    indicator_sets: Vec<IndicatorSet>,
    event_receiver: EventReceiver,
    event_sender: EventSender,
}

impl MainPage {
    fn setup_indicators() -> IndicatorSet {
        let mut indicators: Vec<Box<dyn Indicator>> = Vec::new();
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

        // Digital sensors (12 total) - all use basic configuration
        for _ in 0..12 {
            indicators.push(Box::new(TextIndicator::with_config(0, true, true, TextAlignment::Center)));
            indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));
        }

        // Analog sensors (4 total) - with different precision settings
        // 12V (1 decimal place)
        indicators.push(Box::new(TextIndicator::with_config(1, true, true, TextAlignment::Center)));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));

        // Fuel Level (1 decimal place)
        indicators.push(Box::new(TextIndicator::with_config(1, true, true, TextAlignment::Center)));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));

        // Oil Pressure (2 decimal places)
        indicators.push(Box::new(TextIndicator::with_config(2, true, true, TextAlignment::Center)));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));

        // Temperature (1 decimal place)
        indicators.push(Box::new(TextIndicator::with_config(1, true, true, TextAlignment::Center)));
        indicator_bounds.push(create_bounds_and_advance(&mut col, &mut row));
        IndicatorSet { indicators, indicator_bounds }
    }

    pub fn new(id: u32, ui_style: UIStyle, event_sender: EventSender, event_receiver: EventReceiver) -> Self {
        let indicator_set = Self::setup_indicators();

        let mut main_page = MainPage {
            base: PageBase::new(id, "Main".to_string(), ui_style),
            event_sender: event_sender.clone(),
            event_receiver,
            indicator_sets: vec![indicator_set],
            current_indicator_set: 0,
        };

        // Set up default buttons for the main page
        main_page.setup_buttons();
        
        main_page
    }

    // Setup default buttons for main page using event system
    fn setup_buttons(&mut self) {
        let event_sender = self.event_sender.clone();
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ВИД+".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::NextIndicatorSet)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ВИД-".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::PreviousIndicatorSet)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "ВЫХ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::Shutdown)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "ЯРК+".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::BrightnessUp)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right2, "ЯРК-".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::BrightnessDown)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ДИАГ".into(), Box::new({
                let sender = event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(DIAG_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];

        self.base.set_buttons(buttons);
    }

    // Helper method to read all sensor values and convert to SensorValue objects
    fn read_all_sensors(&self, sensor_manager: &SensorManager) -> Result<Vec<SensorValue>, String> {
        let mut sensor_values = Vec::new();

        // Read digital sensors using the new interface
        let digital_values = sensor_manager.get_digital_sensor_values();
        for (sensor_input, value) in digital_values {
            let (label, sensor_name) = match sensor_input {
                input if *input == HW_BRAKE_FLUID_LVL_LOW_INPUT => ("Brake Fluid", "brake_fluid"),
                input if *input == HW_CHARGE_INPUT => ("Charge", "charge"),
                input if *input == HW_CHECK_ENGINE_INPUT => ("Check Eng", "check_engine"),
                input if *input == HW_DIFF_LOCK_INPUT => ("Diff Lock", "diff_lock"),
                input if *input == HW_EXT_LIGHTS_INPUT => ("Ext Lights", "ext_lights"),
                input if *input == HW_FUEL_LVL_LOW_INPUT => ("Fuel Low", "fuel_low"),
                input if *input == HW_HIGH_BEAM_INPUT => ("High Beam", "high_beam"),
                input if *input == HW_INSTR_ILLUM_INPUT => ("Illum", "instr_illum"),
                input if *input == HW_OIL_PRESS_LOW_INPUT => ("Oil Press Low", "oil_press_low"),
                input if *input == HW_PARK_BRAKE_INPUT => ("Park Brake", "park_brake"),
                input if *input == HW_SPEED_INPUT => ("Speed", "speed"),
                input if *input == HW_TACHO_INPUT => ("Tacho", "tacho"),
                input if *input == HW_TURN_SIGNAL_INPUT => ("Turn Signal", "turn_signal"),
                _ => ("Unknown", "unknown"),
            };
            
            sensor_values.push(SensorValue::digital(*value, label, sensor_name));
        }

        // Read analog sensors using the new interface
        let analog_values = sensor_manager.get_analog_sensor_values();
        for (sensor_input, value) in analog_values {
            match sensor_input {
                input if *input == HW_12V_INPUT => {
                    sensor_values.push(SensorValue::analog_with_thresholds(
                        *value, 0.0, 20.0, Some(11.5), Some(13.8), Some(10.5), None, "battery_12v", "V", "12V"
                    ));
                },
                input if *input == HW_FUEL_LVL_INPUT => {
                    sensor_values.push(SensorValue::analog_with_thresholds(
                        *value, 0.0, 100.0, Some(20.0), None, Some(10.0), None, "fuel_level", "%", "Fuel Level"
                    ));
                },
                input if *input == HW_OIL_PRESS_INPUT => {
                    sensor_values.push(SensorValue::analog_with_thresholds(
                        *value, 0.0, 8.0, Some(1.0), Some(6.0), Some(0.5), None, "oil_pressure", "kgf/cm²", "Oil Press"
                    ));
                },
                input if *input == HW_TEMP_INPUT => {
                    sensor_values.push(SensorValue::analog_with_thresholds(
                        *value, 0.0, 120.0, Some(90.0), None, None, Some(105.0), "temperature", "°C", "Temperature"
                    ));
                },
                _ => {
                    sensor_values.push(SensorValue::analog_with_thresholds(
                        *value, 0.0, 100.0, Some(50.0), None, None, None, "unknown_analog", "", "Unknown"
                    ));
                },
            }
        }

        Ok(sensor_values)
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

    // Public method to get current indicator set index (for debugging/status)
    pub fn current_indicator_set(&self) -> usize {
        self.current_indicator_set
    }

    // Public method to get total number of indicator sets
    pub fn indicator_sets_count(&self) -> usize {
        self.indicator_sets.len()
    }
}

impl Page for MainPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn render(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager) -> Result<(), String> {
        // Render page title
        context.render_text_with_font(
            "Main Page", 
            400.0, 
            40.0, 
            1.0, 
            self.ui_style().get_color(TEXT_PRIMARY_COLOR, (1.0, 1.0, 1.0)),
            &self.ui_style().get_string(TEXT_PRIMARY_FONT, "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
            self.ui_style().get_integer(TEXT_PRIMARY_FONT_SIZE, 24)
        )?;

        // Read sensor values and create SensorValue objects
        let sensor_values = self.read_all_sensors(sensor_manager)?;

        // Render each indicator with its corresponding sensor value
        let indicators = self.indicator_sets[self.current_indicator_set].indicators.iter();
        let indicator_bounds = &self.indicator_sets[self.current_indicator_set].indicator_bounds;
        
        for (i, indicator) in indicators.enumerate() {
            if let Some(sensor_value) = sensor_values.get(i) {
                if let Some(bounds) = indicator_bounds.get(i) {
                    indicator.render(sensor_value, bounds.clone(), self.ui_style(), context)?;
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
                    self.next_indicator_set();
                }
                crate::page_framework::events::UIEvent::PreviousIndicatorSet => {
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
                // Let other events pass through to the page manager
                _ => {
                    // Re-send event to page manager for global handling
                    self.event_sender.send(event);
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

    fn ui_style(&self) -> &UIStyle {
        self.base.ui_style()
    }
}