#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID};
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::hw_providers::HWInput;
use crate::util::gnss_data_provider::GnssFrame;
use crate::util::nmea::{FixQuality, GnssFix};
use crate::indicators::compass_indicator::{CompassHeadingMarkerDecorator, CompassIndicator};
use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::indicators::text_indicator::{TextIndicator, TextAlignment};
use crate::indicators::decorator::*;
use crate::util::gnss_data_provider::TestGnssDataProvider;

const CONTENT_X_MARGIN: f32 = 40.0;
const TITLE_Y: f32 = 5.0;
const TITLE_CONTENT_GAP: f32 = 10.0;

pub enum GnssMode {
    Info,
    PNP,        // ПНП (планово-навигационный прибор) imitation mode - heading, track and basic waypoint navigation
    Map,        // map mode - not implemented yet
}

struct PnpMode {
    compass_indicator: CompassIndicator,
    heading_indicator: TextIndicator,
}

#[derive(PartialEq, Clone, Copy)]
enum InfoBlocks {
    LinkStatus,
    FixQuality,
    Position,
    Movement,
    TimeAndDate
}

/// Structured GNSS status page: parsed fix (position/speed/heading/time) pulled straight
/// from GnssFrame, unlike the raw-NMEA `TerminalPage::new_gnss` view reachable from the
/// diag page.
pub struct GnssPage {
    base: PageBase,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,
    frame: GnssFrame,
    mode: GnssMode,
    pnp_mode: PnpMode,
    gnss: TestGnssDataProvider,
}

impl GnssPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver, frame: GnssFrame, mode: GnssMode, ui_style: &UIStyle) -> Self {
        let mut page = GnssPage {
            base: PageBase::new(id, "GNSS".to_string()),
            smart_event_sender,
            event_receiver,
            frame,
            mode,
            pnp_mode: GnssPage::setup_pnp_mode(ui_style),
            gnss: TestGnssDataProvider::start()         // TODO: Temporary data provider - remove after testing
        };
        page.setup_buttons();
        page
    }

    fn setup_pnp_mode(ui_style: &UIStyle) -> PnpMode {
        let visible_half_angle_deg = 120.0;
        let ring_margin = 24.0;
        let major_mark_length = 18.0;

        let heading_label_color = ui_style.get_color(COMPASS_HEADING_COLOR, (0.9, 0.9, 1.0));
        let heading_label_font = ui_style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);

        PnpMode {
            compass_indicator: CompassIndicator::new().with_decorators(vec![
                Box::new(CompassHeadingMarkerDecorator::new(visible_half_angle_deg, ring_margin, major_mark_length))]),
            heading_indicator: TextIndicator::new().with_font(heading_label_font, 36, 1.0).with_colors(heading_label_color, (1.0, 1.0, 0.0), (1.0, 0.0, 0.0)).
                with_parameters(TextAlignment::Center, false, false).with_decorators(vec![
                    Box::new(BoxDecorator::new(2.0, COMPASS_HEADING_COLOR, 0.0)),
                    Box::new(TriangleDecorator::new([(0.5, 1.5), (0.35, 1.2), (0.65, 1.2)], 2.0, COMPASS_HEADING_COLOR, true)),
                ]),
        }
    }

    fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ПНП".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::NavPnpMode)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ИНФ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::NavInfoMode)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(MAIN_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }

    fn na() -> String {
        "н/д".to_string()
    }

    fn fix_quality_label(q: FixQuality) -> String {
        match q {
            FixQuality::Invalid => "НЕТ".to_string(),
            FixQuality::Gps => "GPS".to_string(),
            FixQuality::DGps => "DGPS".to_string(),
            FixQuality::PpsFix => "PPS".to_string(),
            FixQuality::RtkFixed => "RTK ФИКС".to_string(),
            FixQuality::RtkFloat => "RTK ПЛАВ".to_string(),
            FixQuality::Estimated => "ОЦЕНКА".to_string(),
            FixQuality::Manual => "РУЧН".to_string(),
            FixQuality::Simulation => "СИМ".to_string(),
            FixQuality::Unknown(code) => format!("? ({})", code),
        }
    }

    fn lat_str(fix: &GnssFix) -> String {
        match fix.latitude_deg {
            Some(v) => format!("{:.6}\u{00B0} {}", v.abs(), if v >= 0.0 { "С" } else { "Ю" }),
            None => Self::na(),
        }
    }

    fn lon_str(fix: &GnssFix) -> String {
        match fix.longitude_deg {
            Some(v) => format!("{:.6}\u{00B0} {}", v.abs(), if v >= 0.0 { "В" } else { "З" }),
            None => Self::na(),
        }
    }

    fn date_str(fix: &GnssFix) -> String {
        match (fix.date) {
            (Some(d)) => format!("{:02}.{:02}.{}", d.day, d.month, d.year),
            _ => Self::na(),
        }
    }

    fn time_str(fix: &GnssFix) -> String {
        match (fix.time) {
            (Some(t)) => format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second as u8),
            _ => Self::na(),
        }
    }

    fn get_info_text(&self, frame: &GnssFrame, blocks: &[InfoBlocks]) -> Vec<(String, bool, bool)> {
        let stale = self.frame.is_stale();
        let fix = self.frame.fix();

        let link_str = if stale { "НЕТ СВЯЗИ" } else { "НОРМА" }.to_string();
        let quality_str = fix.fix_quality.map(Self::fix_quality_label).unwrap_or_else(Self::na);
        let satellites_str = fix.satellites.map(|s| s.to_string()).unwrap_or_else(Self::na);
        let hdop_str = fix.hdop.map(|v| format!("{:.2}", v)).unwrap_or_else(Self::na);
        let alt_str = fix.altitude_m.map(|v| format!("{:.0} м", v)).unwrap_or_else(Self::na);
        let speed_str = fix.speed_kmh.map(|v| format!("{:.1} км/ч", v)).unwrap_or_else(Self::na);
        let course_str = fix.course_deg.map(|v| format!("{:.1}\u{00B0}", v)).unwrap_or_else(Self::na);
        let heading_str = fix.heading_deg.map(|v| format!("{:.1}\u{00B0}", v)).unwrap_or_else(Self::na);

        let mut lines: Vec<(String, bool, bool)> = vec![];

        for &block in blocks {
            match block {
                InfoBlocks::LinkStatus => {
                    lines.append(&mut vec![
                        (format!("ГНСС:   {}", link_str), false, stale),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::FixQuality => {
                    lines.append(&mut vec![
                        (format!("Фикс:   {}", quality_str), false, false),
                        (format!("Спутн:  {}", satellites_str), false, false),
                        (format!("HDOP:   {}", hdop_str), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::Position => {
                    lines.append(&mut vec![
                        (format!("Шир:    {}", Self::lat_str(&fix)), false, false),
                        (format!("Дол:    {}", Self::lon_str(&fix)), false, false),
                        (format!("Выс:    {}", alt_str), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::Movement => {
                    lines.append(&mut vec![
                        (format!("Скор:   {}", speed_str), false, false),
                        (format!("Курс:   {}", course_str), false, false),
                        (format!("Азимут: {}", heading_str), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::TimeAndDate => {
                    lines.append(&mut vec![
                        (format!("UTC:    {}", Self::time_str(&fix)), false, false),
                        (format!("        {}", Self::date_str(&fix)), false, false),
                        (String::new(), false, false),
                    ]);
                },
            }
        }

        lines
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

    fn render_info_mode(&self, context: &mut GraphicsContext, _sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        let title_font = ui_style.get_string(TEXT_PRIMARY_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let title_font_size = ui_style.get_integer(TEXT_PRIMARY_FONT_SIZE, 24);
        let title_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));
        let header_color = title_color;
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (0.8, 0.8, 0.8));
        let warning_color = ui_style.get_color(TEXT_WARNING_COLOR, (1.0, 1.0, 0.0));

        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);

        context.render_text_with_font(
            "НАВИГАЦИЯ", CONTENT_X_MARGIN, TITLE_Y, 1.0, title_color, &title_font, title_font_size,
        )?;

        let title_height = context.calculate_text_height_with_font("НАВИГАЦИЯ", 1.0, &title_font, title_font_size)?;
        let y = TITLE_Y + title_height + TITLE_CONTENT_GAP;

        // Read directly from the frame rather than through the sensor chain — lat/lon/
        // time/date are composite fields GnssChannelProvider doesn't carry (see
        // hw_providers.rs), so this page is the sole consumer of the full GnssFix.

        let lines = self.get_info_text(&self.frame, &[InfoBlocks::LinkStatus, InfoBlocks::FixQuality, InfoBlocks::Position, InfoBlocks::Movement, InfoBlocks::TimeAndDate]);

        self.render_info_lines(&lines, (CONTENT_X_MARGIN, y), context, &[text_color, warning_color, header_color], &font, font_size)?;

        Ok(())
    }

    fn render_pnp_mode(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        use crate::hardware::sensor_value::SensorValue;

        let header_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));;
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (0.8, 0.8, 0.8));
        let warning_color = ui_style.get_color(TEXT_WARNING_COLOR, (1.0, 1.0, 0.0));

        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);

        let w = context.width as f32;
        let h = context.height as f32;
        let bounds = IndicatorBounds::new(w * 0.2, h * 0.1, w * 0.6, h * 0.8);

        let lines = self.get_info_text(&self.frame, &[InfoBlocks::LinkStatus, InfoBlocks::Position, InfoBlocks::FixQuality]);

        self.render_info_lines(&lines, (CONTENT_X_MARGIN, TITLE_Y), context, &[text_color, warning_color, header_color], &font, font_size)?;

        let lines = self.get_info_text(&self.frame, &[InfoBlocks::TimeAndDate, InfoBlocks::Movement]);

        self.render_info_lines(&lines, (w * 0.75, TITLE_Y), context, &[text_color, warning_color, header_color], &font, font_size)?;

        // if let Some(heading_value) = sensor_manager.get_sensor_value(&HWInput::HwGnssHeading) {
        //     self.pnp_mode.compass_indicator.render(heading_value, bounds, &ui_style, context)?;
        // }
        
        // TODO: remove test gnss provider usage after testing
        let fix = self.frame.fix();
        let heading = fix.heading_deg.unwrap_or(0.0);
        let mut heading_value = SensorValue::analog(0.0, 0.0, 359.999, "\u{00B0}", "Курс", "test_heading");
        heading_value.value = crate::hardware::sensor_value::ValueData::Analog(heading);

        // Heading indicator sits directly above the compass's drawn arc, centered over the
        // same horizontal span, derived from the compass's own geometry so the two can't
        // drift out of alignment.
        
        let heading_font = ui_style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let heading_font_height = context.get_line_height_with_font(1.0, &heading_font, 36)?;
        let heading_font_width = context.calculate_text_width_with_font("0000", 1.0, &heading_font, 36)?;

        let (_cx, cy, radius) = CompassIndicator::geometry(bounds, self.pnp_mode.compass_indicator.visible_half_angle_deg());
        let compass_top_y = cy - (radius - self.pnp_mode.compass_indicator.ring_margin());
        let heading_bounds = IndicatorBounds::new((w - heading_font_width) / 2.0, (compass_top_y - heading_font_height - 20.0).max(0.0), heading_font_width, heading_font_height);

        self.pnp_mode.heading_indicator.render(&heading_value, heading_bounds, &ui_style, context)?;
        self.pnp_mode.compass_indicator.render(&heading_value, bounds, &ui_style, context)?;

        Ok(())
    }
}

impl Page for GnssPage {
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

        match self.mode {
            GnssMode::Info => {
                self.render_info_mode(context, sensor_manager, ui_style)?;
            },
            GnssMode::PNP => {
                self.render_pnp_mode(context, sensor_manager, ui_style)?;
            }
            _ => {}
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
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                UIEvent::NavPnpMode => {
                    self.mode = GnssMode::PNP;
                },
                UIEvent::NavInfoMode => {
                    self.mode = GnssMode::Info;
                },
                _ => {}
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
