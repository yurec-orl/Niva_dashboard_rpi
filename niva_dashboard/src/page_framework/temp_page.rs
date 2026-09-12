#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_manager::SensorManager;
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID};

const CONTENT_X_MARGIN: f32 = 40.0;
const TITLE_Y: f32 = 20.0;
const TITLE_CONTENT_GAP: f32 = 16.0;
const VALUE_COLUMN_X: f32 = 260.0;

/// The one-wire temperature readings this page shows, with the label to print when the
/// chain has never produced a value (there is no SensorValue to read a label off then).
/// Kept in sync with sensor_config.json's `one_wire_temp` entries by hand -- adding an oil
/// / gearbox sensor there means adding its HWInput + label row here.
const TEMP_ROWS: [(HWInput, &str); 2] = [
    (HWInput::HwTempOut, "НАРУЖ"),
    (HWInput::HwTempInt, "САЛОН"),
];

/// Read-only page listing the DS18B20 one-wire bus temperatures (see
/// ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md). Reachable from MainPage's Left3 slot; always
/// registered, rendering "НЕТ ДАТЧИКОВ" when no sensor currently has a value.
///
/// Indicator/decorator styling is deliberately minimal for now -- plain threshold-coloured
/// text rows. A richer layout waits until the real installed sensor set is known.
pub struct TempPage {
    base: PageBase,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,
}

impl TempPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver) -> Self {
        let mut page = TempPage {
            base: PageBase::new(id, "Temp".to_string()),
            smart_event_sender,
            event_receiver,
        };
        page.setup_buttons();
        page
    }

    fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(MAIN_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }
}

impl Page for TempPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }

    fn render(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        let title_font = ui_style.get_string(StyleKey::TextPrimaryFont);
        let title_font_size = ui_style.get_integer(StyleKey::TextPrimaryFontSize);
        let title_color = ui_style.get_color(StyleKey::TerminalTextColor);

        let font = ui_style.get_string(StyleKey::TextMonospaceFont);
        let font_size = ui_style.get_integer(StyleKey::TextMonospaceFontSize);

        let label_color = ui_style.get_color(StyleKey::TextPrimaryColor);
        let stale_color = ui_style.get_color(StyleKey::TextSecondaryColor);
        let normal_color = ui_style.get_color(StyleKey::TextPrimaryColor);
        let warning_color = ui_style.get_color(StyleKey::BarWarningColor);
        let critical_color = ui_style.get_color(StyleKey::BarCriticalColor);

        context.render_text_with_font(
            "ТЕМПЕРАТУРА", CONTENT_X_MARGIN, TITLE_Y, 1.0, title_color, &title_font, title_font_size,
        )?;

        let title_height = context.calculate_text_height_with_font("ТЕМПЕРАТУРА", 1.0, &title_font, title_font_size)?;
        let line_height = context.get_line_height_with_font(1.0, &font, font_size)?;
        let mut y = TITLE_Y + title_height + TITLE_CONTENT_GAP;

        let sensor_values = sensor_manager.get_sensor_values();
        let mut any_value = false;

        for (input, label) in TEMP_ROWS {
            context.render_text_with_font(label, CONTENT_X_MARGIN, y, 1.0, label_color, &font, font_size)?;

            match sensor_values.get(&input) {
                Some(value) => {
                    any_value = true;
                    let color = if value.is_critical() {
                        critical_color
                    } else if value.is_warning() {
                        warning_color
                    } else {
                        normal_color
                    };
                    let text = format!("{:+.1} °C", value.as_f32());
                    context.render_text_with_font(&text, VALUE_COLUMN_X, y, 1.0, color, &font, font_size)?;
                }
                None => {
                    context.render_text_with_font("—  НЕТ СВЯЗИ", VALUE_COLUMN_X, y, 1.0, stale_color, &font, font_size)?;
                }
            }
            y += line_height * 1.5;
        }

        if !any_value {
            context.render_text_with_font(
                "НЕТ ДАТЧИКОВ", CONTENT_X_MARGIN, y + line_height, 1.0, stale_color, &font, font_size,
            )?;
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
        while self.event_receiver.try_recv().is_ok() {}
    }

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        self.base.buttons()
    }

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position(pos)
    }

    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position_mut(pos)
    }
}
