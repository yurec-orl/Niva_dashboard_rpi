#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID};
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::hw_providers::HWInput;
use crate::util::gnss_data_provider::GnssFrame;
use crate::util::bno085_data_provider::Bno085Frame;
use crate::util::nmea::{FixQuality, GnssFix};
use crate::indicators::compass_indicator::{CompassHeadingMarkerDecorator, CompassIndicator, HdopIndicator, UP_ANGLE};
use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::indicators::text_indicator::{TextIndicator, TextAlignment};
use crate::indicators::needle_indicator::NeedleIndicator;
use crate::indicators::needle_shape::MarkNeedleShape;
use crate::indicators::decorator::*;
use crate::util::gnss_data_provider::TestGnssDataProvider;

const CONTENT_X_MARGIN: f32 = 30.0;
const TITLE_Y: f32 = 5.0;
const TITLE_CONTENT_GAP: f32 = 10.0;
/// Matches CompassIndicator's own (hardcoded, not style-driven) tape label size, so the
/// INS/GNSS status boxes below the compass read as part of the same label family.
const STATUS_LABEL_FONT_SIZE: u32 = 32;
/// Half-width of the heading-accuracy marks' angular swing: heading_std_dev_deg/2 beyond
/// this is visually clamped to the mark's max spread. Matches CompassIndicator's default
/// minor-mark spacing (5°) so a maxed-out mark reaches one minor tick's width from the
/// lubber line.
const HEADING_ACCURACY_MAX_HALF_SPREAD_DEG: f32 = 90.0;
/// Standard gravity, used to convert Bno085Acceleration's m/s^2 (chip's native unit) to g's
/// for the InsData info block.
const STANDARD_GRAVITY_MPS2: f32 = 9.80665;

pub enum GnssMode {
    Info,
    PNP,        // ПНП (планово-навигационный прибор) imitation mode - heading, track and basic waypoint navigation
    Map,        // map mode - not implemented yet
}

struct PnpMode {
    compass_indicator: CompassIndicator,
    heading_indicator: TextIndicator,
    hdop_indicator: HdopIndicator,
    /// Renders a single small tick mark at heading_deg ± heading_std_dev_deg/2 (rendered
    /// twice per frame, once per side) to visualize GNSS heading uncertainty -- a
    /// NeedleIndicator with a MarkNeedleShape in place of the default blade.
    heading_accuracy_needle: NeedleIndicator,
    /// BNO085 IMU link status box, bottom-left of the compass. Green when the BNO085 has
    /// reported a fresh orientation reading recently, red otherwise (see
    /// Bno085LinkStatusProvider / HwBno085Link).
    ins_link_indicator: TextIndicator,
    /// GNSS link status box, bottom-right of the compass. Green when the link is up and a
    /// fix is held, red otherwise.
    gnss_link_indicator: TextIndicator,
    /// "ТЕСТ" label shown below the GNSS status box while test_provider is active. Only
    /// ever rendered in that one state, so it has a single fixed color rather than
    /// status-driven ones like its neighbors.
    test_mode_indicator: TextIndicator,
}

#[derive(PartialEq, Clone, Copy)]
enum InfoBlocks {
    GnssLinkStatus,
    GnssFixQuality,
    GnssPosition,
    GnssMovement,
    GnssTimeAndDate,
    /// BNO085 link health, mirrors GnssLinkStatus's format.
    InsLinkStatus,
    /// Raw BNO085 readings bypassing the fused heading chain: Game Rotation Vector yaw,
    /// Geomagnetic Rotation Vector yaw, and instantaneous acceleration per axis in g's.
    InsData,
}

/// Structured GNSS status page: parsed fix (position/speed/heading/time) pulled straight
/// from GnssFrame, unlike the raw-NMEA `TerminalPage::new_gnss` view reachable from the
/// diag page.
pub struct GnssPage {
    base: PageBase,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,
    frame: GnssFrame,
    /// BNO085 frame, read directly for the raw INS diagnostics block (InfoBlocks::InsData)
    /// the same way `frame` above is read directly for GNSS's composite fields -- neither
    /// goes through the HWInput/sensor-chain pipeline. None when the BNO085 data provider
    /// failed to start.
    bno_frame: Option<Bno085Frame>,
    mode: GnssMode,
    pnp_mode: PnpMode,
    /// Synthetic GNSS provider, active only while test mode is toggled on (see
    /// UIEvent::NavToggleGnssTest / active_frame()). `None` means the page is showing
    /// live data from `frame`.
    test_provider: Option<TestGnssDataProvider>,
}

impl GnssPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver, frame: GnssFrame, bno_frame: Option<Bno085Frame>, mode: GnssMode, ui_style: &UIStyle) -> Self {
        let mut page = GnssPage {
            base: PageBase::new(id, "GNSS".to_string()),
            smart_event_sender,
            event_receiver,
            frame,
            bno_frame,
            mode,
            pnp_mode: GnssPage::setup_pnp_mode(ui_style),
            test_provider: None,
        };
        page.setup_buttons();
        page
    }

    fn setup_pnp_mode(ui_style: &UIStyle) -> PnpMode {
        let visible_half_angle_deg = 120.0;
        let ring_margin = 24.0;
        let major_mark_length = 18.0;
        // Matches CompassIndicator::new()'s own default minor_mark_length -- duplicated
        // here the same way major_mark_length/ring_margin above are, since the accuracy
        // marks' shape is baked in at construction rather than read back from the indicator.
        let minor_mark_length = 10.0;
        // Matches CompassHeadingMarkerDecorator::new()'s own default arrow_width, made
        // explicit (via with_arrow_width below) so the accuracy marks' width can share it
        // instead of silently drifting if that default ever changes.
        let heading_marker_arrow_width = 3.0;

        let heading_label_color = ui_style.get_color(COMPASS_HEADING_COLOR, (0.9, 0.9, 1.0));
        let heading_label_font = ui_style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let status_label_font = ui_style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);

        let accuracy_start_angle = UP_ANGLE - HEADING_ACCURACY_MAX_HALF_SPREAD_DEG.to_radians();
        let accuracy_end_angle = UP_ANGLE + HEADING_ACCURACY_MAX_HALF_SPREAD_DEG.to_radians();

        PnpMode {
            compass_indicator: CompassIndicator::new().with_decorators(vec![
                Box::new(CompassHeadingMarkerDecorator::new(visible_half_angle_deg, ring_margin, major_mark_length)
                    .with_arrow_width(heading_marker_arrow_width))]),
            heading_accuracy_needle: NeedleIndicator::new(
                accuracy_start_angle, accuracy_end_angle, 1.0,
                heading_marker_arrow_width, heading_marker_arrow_width,
                COMPASS_ARROW_COLOR,
            ).with_shape(Box::new(MarkNeedleShape::new(heading_marker_arrow_width, minor_mark_length))),
            heading_indicator: TextIndicator::new().with_font(heading_label_font, 36, 1.0).with_colors(heading_label_color, (1.0, 1.0, 0.0), (1.0, 0.0, 0.0)).
                with_parameters(TextAlignment::Center, false, false, true).with_decorators(vec![
                    Box::new(BoxDecorator::new(2.0, COMPASS_HEADING_COLOR, 0.0)),
                    Box::new(TriangleDecorator::new([(0.5, 1.5), (0.35, 1.2), (0.65, 1.2)], 2.0, COMPASS_HEADING_COLOR, true)),
                ]),
            hdop_indicator: HdopIndicator::new(),
            ins_link_indicator: TextIndicator::new()
                .with_font(status_label_font.clone(), STATUS_LABEL_FONT_SIZE, 1.0)
                .with_colors((0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (1.0, 0.0, 0.0))
                .with_parameters(TextAlignment::Center, false, true, false),
            gnss_link_indicator: TextIndicator::new()
                .with_font(status_label_font.clone(), STATUS_LABEL_FONT_SIZE, 1.0)
                .with_colors((0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (1.0, 0.0, 0.0))
                .with_parameters(TextAlignment::Center, false, true, false),
            test_mode_indicator: TextIndicator::new()
                .with_font(status_label_font, STATUS_LABEL_FONT_SIZE, 1.0)
                .with_colors((1.0, 1.0, 0.0), (1.0, 1.0, 0.0), (1.0, 1.0, 0.0))
                .with_parameters(TextAlignment::Center, false, true, false),
        }
    }

    /// The GnssFrame currently driving the page's display: the synthetic one while test
    /// mode is on, otherwise the live frame backed by the real GNSS receiver.
    fn active_frame(&self) -> GnssFrame {
        self.test_provider.as_ref().map(|p| p.frame()).unwrap_or_else(|| self.frame.clone())
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
            PageButton::new(ButtonPosition::Right3, "ТЕСТ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::NavToggleGnssTest)
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
        match fix.date {
            Some(d) => format!("{:02}.{:02}.{}", d.day, d.month, d.year),
            _ => Self::na(),
        }
    }

    fn time_str(fix: &GnssFix) -> String {
        match fix.time {
            Some(t) => format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second as u8),
            _ => Self::na(),
        }
    }

    fn hdg_sensor_str(sensor_manager: &SensorManager) -> String {
        match sensor_manager.get_sensor_value(&HWInput::HwHeadingConfidence) {
            Some(sensor_value) => 
                match sensor_value.as_f32() {
                    1.0 => "УСТ".to_string(),
                    2.0 => "ИНС".to_string(),
                    3.0 => "ГНСС".to_string(),
                    _ => Self::na(),
                },
            None => Self::na(),
        }
    }

    fn dead_reckoning_elapsed_str(sensor_manager: &SensorManager) -> String {
        let seconds = sensor_manager.get_sensor_value(&HWInput::HwDeadReckoningElapsed);

        match seconds {
            Some(seconds) => {
                let total_seconds = seconds.as_f32() as u32;

                if total_seconds > 0 {
                    let hours = total_seconds / 3600;
                    let minutes = (total_seconds % 3600) / 60;
                    let secs = total_seconds % 60;

                    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
                } else {
                    Self::na()
                }
            },
            None => Self::na(),
        }
    }

    fn get_info_text(&self, sensor_manager: &SensorManager, frame: &GnssFrame, blocks: &[InfoBlocks]) -> Vec<(String, bool, bool)> {
        let stale = frame.is_stale();
        let fix = frame.fix();

        let link_str = if stale { "НЕТ СВЯЗИ" } else { "НОРМА" }.to_string();
        let quality_str = fix.fix_quality.map(Self::fix_quality_label).unwrap_or_else(Self::na);
        let satellites_str = fix.satellites.map(|s| s.to_string()).unwrap_or_else(Self::na);
        let hdop_str = fix.hdop.map(|v| format!("{:.2}", v)).unwrap_or_else(Self::na);
        let alt_str = fix.altitude_m.map(|v| format!("{:.0} м", v)).unwrap_or_else(Self::na);
        let speed_str = fix.speed_kmh.map(|v| format!("{:.1} км/ч", v)).unwrap_or_else(Self::na);
        let heading_sats_str = fix.heading_satellites.map(|s| format!(" [{}]", s)).unwrap_or_default();
        let heading_str = fix.heading_deg.map(|v| format!("{:.1}\u{00B0}{}", v, heading_sats_str)).unwrap_or_else(Self::na);
        let heading_std_dev_str = fix.heading_std_dev_deg.map(|v| format!("{:.2}\u{00B0}", v)).unwrap_or_else(Self::na);

        let mut lines: Vec<(String, bool, bool)> = vec![];

        for &block in blocks {
            match block {
                InfoBlocks::GnssLinkStatus => {
                    lines.append(&mut vec![
                        (format!("ГНСС:    {}", link_str), false, stale),
                    ]);
                    if self.test_provider.is_some() {
                        lines.push(("ТЕСТ".to_string(), false, true));
                    }
                    lines.push((String::new(), false, false));
                },
                InfoBlocks::GnssFixQuality => {
                    lines.append(&mut vec![
                        (format!("Фикс:     {}", quality_str), false, false),
                        (format!("Спутн:    {}", satellites_str), false, false),
                        (format!("HDOP:     {}", hdop_str), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::GnssPosition => {
                    lines.append(&mut vec![
                        (format!("Шир:      {}", Self::lat_str(&fix)), false, false),
                        (format!("Дол:      {}", Self::lon_str(&fix)), false, false),
                        (format!("Выс:      {}", alt_str), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::GnssMovement => {
                    lines.append(&mut vec![
                        (format!("Скор:     {}", speed_str), false, false),
                        (format!("Курс:     {}", heading_str), false, false),
                        (format!("СКО кур:  {}", heading_std_dev_str), false, false),
                        (format!("КУРС ДАТЧ:{}", Self::hdg_sensor_str(sensor_manager)), false, false),
                        (format!("ОТ ИНС:   {}", Self::dead_reckoning_elapsed_str(sensor_manager)), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::GnssTimeAndDate => {
                    lines.append(&mut vec![
                        (format!("UTC:      {}", Self::time_str(&fix)), false, false),
                        (format!("          {}", Self::date_str(&fix)), false, false),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::InsLinkStatus => {
                    let ins_stale = self.bno_frame.as_ref().map(|f| f.is_stale()).unwrap_or(true);
                    let ins_link_str = if ins_stale { "НЕТ СВЯЗИ" } else { "НОРМА" }.to_string();
                    lines.append(&mut vec![
                        (format!("ИНС: {}", ins_link_str), false, ins_stale),
                        (String::new(), false, false),
                    ]);
                },
                InfoBlocks::InsData => {
                    // Game RV has its own staleness flag (see Bno085Frame::game_orientation_is_stale
                    // doc comment) -- the general is_stale() only tells you *some* report type
                    // arrived recently, not specifically this one.
                    let game_yaw_str = match &self.bno_frame {
                        Some(f) if !f.game_orientation_is_stale() => format!("{:.1}\u{00B0}", f.game_orientation().heading_deg),
                        _ => Self::na(),
                    };
                    let geo_yaw_str = match &self.bno_frame {
                        Some(f) if !f.is_stale() => format!("{:.1}\u{00B0}", f.geomagnetic_orientation().heading_deg),
                        _ => Self::na(),
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
                    lines.append(&mut vec![
                        (format!("К/ИНС: {}", game_yaw_str), false, false),
                        (format!("К/МАГ: {}", geo_yaw_str), false, false),
                        (format!("УСК X: {}", accel_x_str), false, false),
                        (format!("УСК Y: {}", accel_y_str), false, false),
                        (format!("УСК Z: {}", accel_z_str), false, false),
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

    fn render_info_mode(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
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

        let lines = self.get_info_text(sensor_manager, &self.active_frame(), &[
            InfoBlocks::GnssLinkStatus, InfoBlocks::GnssFixQuality, InfoBlocks::GnssPosition, InfoBlocks::GnssMovement, InfoBlocks::GnssTimeAndDate,
            InfoBlocks::InsLinkStatus, InfoBlocks::InsData,
        ]);

        self.render_info_lines(&lines, (CONTENT_X_MARGIN, y), context, &[text_color, warning_color, header_color], &font, font_size)?;

        Ok(())
    }

    fn render_pnp_mode(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueMetadata};

        let header_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (0.8, 0.8, 0.8));
        let warning_color = ui_style.get_color(TEXT_WARNING_COLOR, (1.0, 1.0, 0.0));

        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);

        let w = context.width as f32;
        let h = context.height as f32;
        let bounds = IndicatorBounds::new(w * 0.2, h * 0.1, w * 0.6, h * 0.8);

        let active_frame = self.active_frame();

        // Gnss data, left side
        let lines = self.get_info_text(sensor_manager, &active_frame, &[InfoBlocks::GnssLinkStatus, InfoBlocks::GnssPosition, InfoBlocks::GnssFixQuality]);
        self.render_info_lines(&lines, (CONTENT_X_MARGIN, TITLE_Y), context, &[text_color, warning_color, header_color], &font, font_size)?;

        // Gnss data, right side
        let lines = self.get_info_text(sensor_manager, &active_frame, &[InfoBlocks::GnssTimeAndDate, InfoBlocks::GnssMovement]);
        self.render_info_lines(&lines, (w * 0.75, TITLE_Y), context, &[text_color, warning_color, header_color], &font, font_size)?;

        // Ins data, left bottom side
        let lines = self.get_info_text(sensor_manager, &active_frame, &[InfoBlocks::InsLinkStatus, InfoBlocks::InsData]);
        self.render_info_lines(&lines, (CONTENT_X_MARGIN, h * 0.6), context, &[text_color, warning_color, header_color], &font, font_size)?;

        let fix = active_frame.fix();

        // The fused HwHeading sensor (BNO085 + GNSS, see hardware::heading_fusion_sensor)
        // always reads the *real* GNSS frame, not this page's synthetic test_provider frame
        // -- so while test mode is on, bypass it and read the test frame's heading directly,
        // the same way the rest of this page's fields already fall back to active_frame().
        // TestGnssDataProvider sweeps heading through the full 0-360° range specifically to
        // exercise the compass, so this keeps the "ТЕСТ" button doing what it's for.
        let heading_value = if self.test_provider.is_some() {
            SensorValue::analog(fix.heading_deg.unwrap_or(0.0), 0.0, 359.999, "\u{00B0}", "КУРС", "gnss_test_heading")
        } else {
            match sensor_manager.get_sensor_value(&HWInput::HwHeading) {
                Some(value) if value.value != crate::hardware::sensor_value::ValueData::Empty => value.clone(),
                // No BNO085 or GNSS heading available -- park the compass at 0° rather than
                // feed NaN (SensorValue::empty().as_f32()) into CompassIndicator::render.
                _ => SensorValue::analog(0.0, 0.0, 359.999, "\u{00B0}", "КУРС", "heading_fused"),
            }
        };

        // Heading indicator sits directly above the compass's drawn arc, centered over the
        // same horizontal span, derived from the compass's own geometry so the two can't
        // drift out of alignment.
        
        let heading_font = ui_style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let heading_font_height = context.get_line_height_with_font(1.0, &heading_font, 36)?;
        let heading_font_width = context.calculate_text_width_with_font("0000", 1.0, &heading_font, 36)?;

        let (cx, cy, radius) = CompassIndicator::geometry(bounds, self.pnp_mode.compass_indicator.visible_half_angle_deg());
        let outer_r = radius - self.pnp_mode.compass_indicator.ring_margin();
        let compass_top_y = cy - outer_r;
        let heading_bounds = IndicatorBounds::new((w - heading_font_width) / 2.0, (compass_top_y - heading_font_height - 20.0).max(0.0), heading_font_width, heading_font_height);

        self.pnp_mode.heading_indicator.render(&heading_value, heading_bounds, &ui_style, context)?;
        self.pnp_mode.compass_indicator.render(&heading_value, bounds, &ui_style, context)?;
        self.pnp_mode.hdop_indicator.render(cx, cy, fix.hdop, &ui_style, context)?;

        // Two small marks flanking the lubber line at heading_deg ± heading_std_dev_deg/2,
        // visualizing fused heading sensor uncertainty. Skipped (not shown at 0 spread) when no std
        // dev is reported, since a collapsed mark would misleadingly read as "perfect fix".
        if let Some(heading_std_dev_value) = sensor_manager.get_sensor_value(&HWInput::HwHeadingAccuracy) {
            let heading_std_dev_deg = heading_std_dev_value.as_f32();
            if heading_std_dev_deg > 5.0 {
                let half_dev_deg = heading_std_dev_deg / 2.0;
                let accuracy_radius = outer_r - self.pnp_mode.compass_indicator.major_mark_length();
                let accuracy_bounds = IndicatorBounds::new(
                    cx - accuracy_radius, cy - accuracy_radius, accuracy_radius * 2.0, accuracy_radius * 2.0,
                );
                let plus_value = SensorValue::analog(
                    half_dev_deg, -HEADING_ACCURACY_MAX_HALF_SPREAD_DEG, HEADING_ACCURACY_MAX_HALF_SPREAD_DEG,
                    "\u{00B0}", "", "heading_accuracy_plus");
                let minus_value = SensorValue::analog(
                    -half_dev_deg, -HEADING_ACCURACY_MAX_HALF_SPREAD_DEG, HEADING_ACCURACY_MAX_HALF_SPREAD_DEG,
                    "\u{00B0}", "", "heading_accuracy_minus");
                self.pnp_mode.heading_accuracy_needle.render(&plus_value, accuracy_bounds, &ui_style, context)?;
                self.pnp_mode.heading_accuracy_needle.render(&minus_value, accuracy_bounds, &ui_style, context)?;
            }
        }

        // INS/GNSS link status boxes, tucked under the compass's two side tips (derived from
        // the same geometry as the compass itself, so they track it if bounds ever change).
        let half_angle_rad = self.pnp_mode.compass_indicator.visible_half_angle_deg().to_radians();
        let compass_bottom_y = cy - outer_r * half_angle_rad.cos();

        let status_font = ui_style.get_string(COMPASS_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let status_box_height = context.get_line_height_with_font(1.0, &status_font, STATUS_LABEL_FONT_SIZE)? ;
        let status_box_y = (compass_bottom_y - status_box_height).min(h - status_box_height - 4.0);

        let ins_box_width = context.calculate_text_width_with_font("ИНС", 1.0, &status_font, STATUS_LABEL_FONT_SIZE)?;
        let gnss_box_width = context.calculate_text_width_with_font("ГНСС", 1.0, &status_font, STATUS_LABEL_FONT_SIZE)?;

        let ins_bounds = IndicatorBounds::new(cx - ins_box_width / 2.0 - w / 8.0, status_box_y, ins_box_width, status_box_height);
        let gnss_bounds = IndicatorBounds::new(cx - gnss_box_width / 2.0 + w / 8.0, status_box_y, gnss_box_width, status_box_height);

        // Red when the BNO085 link is down (never connected, or a live link went stale) --
        // see Bno085LinkStatusProvider. Independent of test mode: BNO085 is real hardware
        // with no synthetic-frame stand-in, unlike the GNSS box below.
        let ins_problem = sensor_manager.get_sensor_value(&HWInput::HwBno085Link)
            .map(|v| v.is_active())
            .unwrap_or(true);
        let ins_value = SensorValue::digital_with_constraints_and_metadata(
            ins_problem, ValueConstraints::digital_critical(), ValueMetadata::new("", "ИНС", "ins_link"));
        // Red for either "no serial link" (frame stale) or "link up but no fix yet"
        // (fix_quality Invalid or never seen) — both mean the heading/position aren't trustworthy.
        let gnss_problem = active_frame.is_stale() || fix.fix_quality.map_or(true, |q| q == FixQuality::Invalid);
        let gnss_value = SensorValue::digital_with_constraints_and_metadata(
            gnss_problem, ValueConstraints::digital_critical(), ValueMetadata::new("", "ГНСС", "gnss_link"));

        self.pnp_mode.ins_link_indicator.render(&ins_value, ins_bounds, &ui_style, context)?;
        self.pnp_mode.gnss_link_indicator.render(&gnss_value, gnss_bounds, &ui_style, context)?;

        if self.test_provider.is_some() {
            let test_bounds = IndicatorBounds::new(
                gnss_bounds.x, gnss_bounds.y + status_box_height,
                gnss_box_width, status_box_height,
            );
            let test_value = SensorValue::digital_with_constraints_and_metadata(
                false, ValueConstraints::digital_critical(), ValueMetadata::new("", "ТЕСТ", "gnss_test"));
            self.pnp_mode.test_mode_indicator.render(&test_value, test_bounds, &ui_style, context)?;
        }

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
                UIEvent::NavToggleGnssTest => {
                    if self.test_provider.is_some() {
                        self.test_provider = None; // Drop stops + joins the thread
                    } else {
                        self.test_provider = Some(TestGnssDataProvider::start());
                    }
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
