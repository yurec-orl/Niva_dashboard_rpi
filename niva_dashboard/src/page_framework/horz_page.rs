#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_value::SensorValue;
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID};
use crate::hardware::sensor_manager::SensorManager;
use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::indicators::pitch_indicator::PitchIndicator;
use crate::indicators::roll_indicator::{RollIndicator, RollScaleDecorator};
use crate::util::bno085_data_provider::Bno085Frame;

/// Fraction of screen width/height the pitch indicator occupies.
const PITCH_INDICATOR_WIDTH_FRAC: f32 = 0.25;
const PITCH_INDICATOR_HEIGHT_FRAC: f32 = 0.75;

/// Roll indicator's total width, relative to the pitch indicator's width -- see
/// RollIndicator's doc comment for why its pivot is placed at the pitch indicator's center.
const ROLL_INDICATOR_WIDTH_FRAC: f32 = 1.5;

const INFO_X_MARGIN: f32 = 30.0;
const INFO_Y: f32 = 5.0;

/// Standard gravity, used to convert Bno085Acceleration's m/s^2 (chip's native unit) to g's
/// for the INS info block. Duplicated from gnss_page.rs, which has its own page-local copy.
const STANDARD_GRAVITY_MPS2: f32 = 9.80665;

/// Inclinometer page: aircraft-style artificial horizon (pitch ladder + roll pointer), both
/// driven by the BNO085 Game Rotation Vector.
pub struct HorzPage {
    base: PageBase,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,
    /// None when the BNO085 data provider failed to start.
    bno_frame: Option<Bno085Frame>,
    pitch_indicator: PitchIndicator,
    roll_indicator: RollIndicator,
}

impl HorzPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver, bno_frame: Option<Bno085Frame>) -> Self {
        let mut page = HorzPage {
            base: PageBase::new(id, "Horz".to_string()),
            smart_event_sender,
            event_receiver,
            bno_frame,
            pitch_indicator: PitchIndicator::new().with_visible_span_deg(50.0),
            roll_indicator: RollIndicator::new().with_decorators(vec![Box::new(RollScaleDecorator::new())]),
        };

        page.setup_buttons();
        page
    }

    /// Game Rotation Vector pitch/roll in degrees (Bno085Frame::game_orientation() already
    /// applies the persisted КАЛИБР correction), or None while the BNO085 link is down /
    /// hasn't reported yet -- see Bno085Frame::game_orientation_is_stale's doc comment for why
    /// that check (not the general is_stale()) is the correct one for Game RV specifically.
    fn pitch_roll_deg(&self) -> Option<(f32, f32)> {
        match &self.bno_frame {
            Some(frame) if !frame.game_orientation_is_stale() => {
                let o = frame.game_orientation();
                Some((o.pitch_deg, o.roll_deg))
            },
            _ => None,
        }
    }

    /// Pitch in degrees; 0.0 while stale.
    fn pitch_deg(&self) -> f32 {
        self.pitch_roll_deg().map(|(p, _)| p).unwrap_or(0.0)
    }

    /// Roll in degrees; 0.0 while stale.
    fn roll_deg(&self) -> f32 {
        self.pitch_roll_deg().map(|(_, r)| r).unwrap_or(0.0)
    }

    fn na() -> String {
        "н/д".to_string()
    }

    /// INS info block (link status, pitch/roll, acceleration), mirroring GnssPage's
    /// InsLinkStatus + InsData blocks -- lines are (text, is_header, is_warning) tuples for
    /// render_info_lines to color.
    fn get_ins_info_lines(&self) -> Vec<(String, bool, bool)> {
        let stale = self.bno_frame.as_ref().map(|f| f.game_orientation_is_stale()).unwrap_or(true);
        let link_str = if stale { "НЕТ СВЯЗИ" } else { "НОРМА" }.to_string();

        let (pitch_str, roll_str) = if stale {
            (Self::na(), Self::na())
        } else {
            (format!("{:.1}\u{00B0}", self.pitch_deg()), format!("{:.1}\u{00B0}", self.roll_deg()))
        };

        let (accel_x_str, accel_y_str, accel_z_str) = match &self.bno_frame {
            Some(f) if !f.is_stale() => {
                let a = f.acceleration();
                (
                    format!("{:.1} g", a.x_mps2 / STANDARD_GRAVITY_MPS2),
                    format!("{:.1} g", a.y_mps2 / STANDARD_GRAVITY_MPS2),
                    format!("{:.1} g", a.z_mps2 / STANDARD_GRAVITY_MPS2),
                )
            },
            _ => (Self::na(), Self::na(), Self::na()),
        };

        vec![
            (format!("ИНС:   {}", link_str), false, stale),
            (String::new(), false, false),
            (format!("ТАНГАЖ:{}", pitch_str), false, false),
            (format!("КРЕН:  {}", roll_str), false, false),
            (String::new(), false, false),
            (format!("УСК X: {}", accel_x_str), false, false),
            (format!("УСК Y: {}", accel_y_str), false, false),
            (format!("УСК Z: {}", accel_z_str), false, false),
        ]
    }

    fn render_info_lines(&self, lines: &Vec<(String, bool, bool)>, position: (f32, f32), context: &mut GraphicsContext, colors: &[(f32, f32, f32)], font: &String, font_size: u32) -> Result<(), String> {
        let mut y = position.1;
        let line_height = context.get_line_height_with_font(1.0, &font, font_size)?;

        for (text, is_header, is_warning) in lines {
            if !text.is_empty() {
                let color = if *is_header { colors[2] } else if *is_warning { colors[1] } else { colors[0] };
                context.render_text_with_font(text, position.0, y, 1.0, color, &font, font_size)?;
            }
            y += line_height;
        }

        Ok(())
    }

    fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "КАЛИБР".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::HorzCalibrate)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ВОЗВР".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(MAIN_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }
}

impl Page for HorzPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }

    fn render(&self, context: &mut GraphicsContext, _sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        let screen_width = context.width as f32;
        let screen_height = context.height as f32;
        let width = screen_width * PITCH_INDICATOR_WIDTH_FRAC;
        let height = screen_height * PITCH_INDICATOR_HEIGHT_FRAC;
        let bounds = IndicatorBounds::new((screen_width - width) / 2.0, (screen_height - height) / 2.0, width, height);
        let (pitch_cx, pitch_cy) = bounds.center();

        let pitch_value = SensorValue::analog(self.pitch_deg(), -90.0, 90.0, "\u{00B0}", "ТАНГАЖ", "bno085_game_pitch");
        self.pitch_indicator.render(&pitch_value, bounds, ui_style, context)?;

        // Pivot sits exactly on the pitch indicator's center -- see RollIndicator's doc
        // comment for why. Height is arbitrary (unused by RollIndicator beyond bounds.center()).
        let roll_width = width * ROLL_INDICATOR_WIDTH_FRAC;
        let roll_height = roll_width / 6.0;
        let roll_bounds = IndicatorBounds::new(pitch_cx - roll_width / 2.0, pitch_cy - roll_height / 2.0, roll_width, roll_height);

        let roll_value = SensorValue::analog(self.roll_deg(), -180.0, 180.0, "\u{00B0}", "КРЕН", "bno085_game_roll");
        self.roll_indicator.render(&roll_value, roll_bounds, ui_style, context)?;

        let header_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (0.8, 0.8, 0.8));
        let warning_color = ui_style.get_color(TEXT_WARNING_COLOR, (1.0, 1.0, 0.0));
        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);

        let ins_lines = self.get_ins_info_lines();
        self.render_info_lines(&ins_lines, (INFO_X_MARGIN, INFO_Y), context, &[text_color, warning_color, header_color], &font, font_size)?;

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
        while let Ok(event) = self.event_receiver.try_recv() {
            if let UIEvent::HorzCalibrate = event {
                if let Some(frame) = &self.bno_frame {
                    frame.calibrate_pitch_roll();
                }
            }
        }
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
