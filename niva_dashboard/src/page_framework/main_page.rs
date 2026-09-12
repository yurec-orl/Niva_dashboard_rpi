use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, DIAG_PAGE_ID, GNSS_PAGE_ID, HORZ_PAGE_ID, TEMP_PAGE_ID};
use crate::page_framework::events::{EventReceiver, SmartEventSender};
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::sensor_value::{SensorValue, ValueData};
use crate::hardware::hw_providers::{*};
use std::cell::RefCell;
use std::collections::HashMap;
use crate::indicators::{Indicator, IndicatorBounds};
use crate::indicators::text_indicator::TextIndicator;
use crate::indicator_builders::{
    build_speedometer_gauge, build_fuel_level_gauge, build_oil_pressure_gauge, build_temperature_gauge, build_voltage_gauge,
    build_oil_pressure_bar, build_fuel_level_bar, build_temperature_bar, build_voltage_bar,
    build_speed_digital
};
use crate::page_framework::events::UIEvent;

// One indicator bound to the single hardware input that feeds it and the screen
// area it draws in. Pairing them in one struct removes the positional coupling
// that previously let a reorder in any one list mismatch the others.
struct IndicatorEntry {
    input: HWInput,
    indicator: Box<dyn Indicator>,
    bounds: IndicatorBounds,
}

struct IndicatorSet {
    entries: Vec<IndicatorEntry>,
}

pub struct MainPage {
    base: PageBase,
    current_indicator_set: usize,
    indicator_sets: Vec<IndicatorSet>,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,
    // Last known-good value per HWInput, kept across ticks so that when a chain read fails
    // and the input drops out of sensor_values, the fault placeholder rendered in its place
    // can still carry the real label/unit/constraints -- otherwise TextIndicator's "---"
    // would be indistinguishable from every other missing reading (see issue #28). Render()
    // takes &self, hence the RefCell.
    last_known_values: RefCell<HashMap<HWInput, SensorValue>>,
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
            indicator_sets: vec![gauge_indicator_set, bar_indicator_set, test_indicator_set],
            current_indicator_set: 0,
            last_known_values: RefCell::new(HashMap::new()),
        };

        // Set up default buttons for the main page
        main_page.setup_buttons();
        
        main_page
    }

    fn setup_test_indicators(ui_style: &UIStyle) -> IndicatorSet {
        let mut entries: Vec<IndicatorEntry> = Vec::new();

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

        // Digital sensors - plain text readout, no precision setting
        let digital_inputs = [
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
            HWInput::HwTurnSignal,
        ];
        for input in digital_inputs {
            entries.push(IndicatorEntry {
                input,
                indicator: Box::new(
                    TextIndicator::new()
                        .with_font(indicator_font.clone(), indicator_font_size, 1.0)
                        .with_colors(indicator_color, indicator_warning_color, indicator_error_color),
                ),
                bounds: create_bounds_and_advance(&mut col, &mut row),
            });
        }

        // Analog sensors - per-sensor decimal precision
        let analog_inputs = [
            (HWInput::Hw12v, 2),
            (HWInput::HwFuelLvl, 2),
            (HWInput::HwOilPress, 2),
            (HWInput::HwEngineCoolantTemp, 1),
        ];
        for (input, precision) in analog_inputs {
            entries.push(IndicatorEntry {
                input,
                indicator: Box::new(
                    TextIndicator::new()
                        .with_precision(precision)
                        .with_font(indicator_font.clone(), indicator_font_size, 1.0)
                        .with_colors(indicator_color, indicator_warning_color, indicator_error_color),
                ),
                bounds: create_bounds_and_advance(&mut col, &mut row),
            });
        }

        IndicatorSet { entries }
    }

    fn setup_gauge_indicators(context: &GraphicsContext, ui_style: &UIStyle) -> IndicatorSet {
        let mut entries: Vec<IndicatorEntry> = Vec::new();

        // Main indicator set layout:
        // 1. Large central speedometer (gauge)
        // 2. Smaller fuel level and oil pressure gauges on the left
        // 3. Smaller temperature and battery voltage gauges on the right

        let screen_width = context.width as f32;
        let _screen_height = context.height as f32;
        
        // Layout parameters
        let button_margin = 60.0; // Space for buttons on left/right
        let top_margin = 8.0;

        // Central speedometer - large gauge (RPM/Speed)
        let center_gauge_radius = 150.0;
        let center_x = screen_width / 2.0;
        let center_y = top_margin + center_gauge_radius;
        
        let (speedometer, speedometer_bounds) = build_speedometer_gauge(center_x, center_y, center_gauge_radius, ui_style);
        entries.push(IndicatorEntry { input: HWInput::HwSpeed, indicator: speedometer, bounds: speedometer_bounds });

        // Left side gauges - smaller gauges
        let side_gauge_radius = 90.0;
        let left_x = button_margin + side_gauge_radius;

        // Fuel level gauge (left top)
        let fuel_y = top_margin + side_gauge_radius;
        let (fuel_gauge, fuel_bounds) = build_fuel_level_gauge(left_x, fuel_y, side_gauge_radius, ui_style);
        entries.push(IndicatorEntry { input: HWInput::HwFuelLvl, indicator: fuel_gauge, bounds: fuel_bounds });

        // Oil pressure gauge (left bottom)
        let oil_y = fuel_y + side_gauge_radius * 2.0 + 20.0;
        let (oil_gauge, oil_bounds) = build_oil_pressure_gauge(left_x, oil_y, side_gauge_radius, ui_style);
        entries.push(IndicatorEntry { input: HWInput::HwOilPress, indicator: oil_gauge, bounds: oil_bounds });

        // Right side gauges - smaller gauges
        let right_x = screen_width - button_margin - side_gauge_radius;

        // Temperature gauge (right top)
        let temp_y = top_margin + side_gauge_radius;
        let (temp_gauge, temp_bounds) = build_temperature_gauge(right_x, temp_y, side_gauge_radius, ui_style);
        entries.push(IndicatorEntry { input: HWInput::HwEngineCoolantTemp, indicator: temp_gauge, bounds: temp_bounds });

        // Battery voltage gauge (right bottom)
        let battery_y = temp_y + side_gauge_radius * 2.0 + 20.0;
        let (voltage_gauge, voltage_bounds) = build_voltage_gauge(right_x, battery_y, side_gauge_radius, ui_style);
        entries.push(IndicatorEntry { input: HWInput::Hw12v, indicator: voltage_gauge, bounds: voltage_bounds });

        IndicatorSet { entries }
    }

    fn setup_bar_indicators(context: &GraphicsContext, ui_style: &UIStyle) -> IndicatorSet {
        let mut entries: Vec<IndicatorEntry> = Vec::new();

        // Layout parameters
        let screen_width = context.width as f32;
        let _screen_height = context.height as f32;
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
        entries.push(IndicatorEntry { input: HWInput::HwOilPress, indicator: oil_pressure_bar, bounds: oil_pressure_bounds });

        // Fuel level indicator
        let (fuel_level_bar, fuel_level_bounds) = build_fuel_level_bar(
            button_margin + bar_width * 2.0 + 50.0,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        entries.push(IndicatorEntry { input: HWInput::HwFuelLvl, indicator: fuel_level_bar, bounds: fuel_level_bounds });

        // Temperature indicator
        let (temperature_bar, temperature_bounds) = build_temperature_bar(
            available_width - bar_width * 2.0 - 50.0,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        entries.push(IndicatorEntry { input: HWInput::HwEngineCoolantTemp, indicator: temperature_bar, bounds: temperature_bounds });

        // Voltage indicator (rightmost)
        let (voltage_bar, voltage_bounds) = build_voltage_bar(
            available_width - bar_width,
            top_margin,
            bar_width,
            bar_height,
            ui_style
        );
        entries.push(IndicatorEntry { input: HWInput::Hw12v, indicator: voltage_bar, bounds: voltage_bounds });

        // Speed digital display (centered)
        let (speed_digital, speed_bounds) = build_speed_digital(
            (screen_width - 200.0) / 2.0,
            top_margin,
            200.0,
            80.0,
            ui_style
        );
        entries.push(IndicatorEntry { input: HWInput::HwSpeed, indicator: speed_digital, bounds: speed_bounds });

        IndicatorSet { entries }
    }

    // Primary button set, shown on entering the page and after returning from the
    // secondary set (ВОЗВР). УСТАНОВ swaps to the secondary set below.
    fn setup_buttons(&mut self) {
        let smart_sender = self.smart_event_sender.clone();
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "НАВ".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(GNSS_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ГОРИЗ".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(HORZ_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left3, "ТЕМП".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(TEMP_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "СБРОС".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SuppressAlerts)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right3, "УСТАНОВ".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::MainSecondaryButtons)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ДИАГ".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(DIAG_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];

        self.base.set_buttons(buttons);
    }

    // Secondary button set, entered via УСТАНОВ. Holds the view/brightness controls freed
    // from the primary set's left1/left2/right1/right2 slots; ВОЗВР returns to primary.
    fn setup_secondary_buttons(&mut self) {
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
            PageButton::new(ButtonPosition::Right1, "ЯРК+".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::BrightnessUp)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right2, "ЯРК-".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::BrightnessDown)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ВОЗВР".into(), Box::new({
                let sender = smart_sender.clone();
                move || sender.send(UIEvent::MainPrimaryButtons)
            }) as Box<dyn FnMut()>),
        ];

        self.base.set_buttons(buttons);
    }

    // Event handler methods for indicator set navigation
    fn next_indicator_set(&mut self) {
        if self.indicator_sets.len() > 1 {
            self.current_indicator_set = (self.current_indicator_set + 1) % self.indicator_sets.len();
            log::info!("MainPage: Switched to indicator set {}", self.current_indicator_set);
        }
    }

    fn previous_indicator_set(&mut self) {
        if self.indicator_sets.len() > 1 {
            if self.current_indicator_set == 0 {
                self.current_indicator_set = self.indicator_sets.len() - 1;
            } else {
                self.current_indicator_set -= 1;
            }
            log::info!("MainPage: Switched to indicator set {}", self.current_indicator_set);
        }
    }

    fn reset_to_first_indicator_set(&mut self) {
        self.current_indicator_set = 0;
        log::info!("MainPage: Reset to first indicator set");
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

        // Render each indicator with the sensor value from its paired hardware input.
        // A missing entry (chain read failed this tick, or hasn't produced one yet) still
        // gets rendered, with an Empty value so the indicator shows its defined fault visual
        // instead of vanishing (see issue #28) -- but carrying the last known-good
        // metadata/constraints, so e.g. TextIndicator's "---" still shows which sensor it is
        // rather than going blank.
        let mut last_known = self.last_known_values.borrow_mut();
        for entry in &self.indicator_sets[self.current_indicator_set].entries {
            let sensor_value = match sensor_values.get(&entry.input) {
                Some(value) => {
                    last_known.insert(entry.input, value.clone());
                    value.clone()
                }
                None => match last_known.get(&entry.input) {
                    Some(prev) => SensorValue::new(ValueData::Empty, prev.constraints.clone(), prev.metadata.clone()),
                    None => SensorValue::empty(),
                },
            };
            entry.indicator.render(&sensor_value, entry.bounds.clone(), ui_style, context)?;
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
                    log::info!("MainPage: NextIndicatorSet event received");
                    self.next_indicator_set();
                }
                crate::page_framework::events::UIEvent::PreviousIndicatorSet => {
                    log::info!("MainPage: PreviousIndicatorSet event received");
                    self.previous_indicator_set();
                }
                crate::page_framework::events::UIEvent::MainSecondaryButtons => {
                    self.setup_secondary_buttons();
                }
                crate::page_framework::events::UIEvent::MainPrimaryButtons => {
                    self.setup_buttons();
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
                    log::info!("MainPage: Ignoring unknown page event: {:?}", event);
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