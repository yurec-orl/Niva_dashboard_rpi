#![allow(dead_code)]
use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::hardware::hw_providers::HWInput;
use crate::hardware::sensor_calibration::{self, CalibrationRecord};
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID, ADC_TERM_PAGE_ID, LOG_PAGE_ID, GNSS_TERM_PAGE_ID, OSC_PAGE_ID};
use crate::hardware::sensor_manager::SensorManager;
use crate::util::adc_data_provider::AdcVersionFrame;
use crate::util::diagnostics::{self, ThrottleStatus};

// Build identity, embedded at compile time by build.rs. Useful because this project
// self-restarts onto a freshly built binary while running — this is how you confirm on
// the physical screen which build actually came up.
const GIT_HASH: &str = env!("NIVA_GIT_HASH");
const BUILD_TIME: &str = env!("NIVA_BUILD_TIME");

// vcgencmd/df/procfs reads are cheap but not free — no need to poll every frame.
const DIAG_REFRESH_INTERVAL: Duration = Duration::from_secs(3);

const CONTENT_X_MARGIN: f32 = 40.0;
const TITLE_Y: f32 = 20.0;
const TITLE_CONTENT_GAP: f32 = 10.0;

/// One entry per field-calibratable sensor (see SENSOR_CALIBRATION_DESIGN.md's Anchor-point
/// procedure) -- `id` must match that sensor's `id` in sensor_config.json / the key
/// PageManager's `calib_offsets` registry uses. `step`/`decimals` are round, hand-picked
/// per-sensor nudge sizes, not derived from the sensor's constraints range.
struct CalibSensorInfo {
    input: HWInput,
    id: &'static str,
    label: &'static str,
    step: f32,
    decimals: usize,
}

const CALIB_SENSORS: [CalibSensorInfo; 3] = [
    CalibSensorInfo { input: HWInput::HwEngineCoolantTemp, id: "HwEngineCoolantTemp", label: "ТЕМП", step: 1.0, decimals: 0 },
    CalibSensorInfo { input: HWInput::HwOilPress, id: "HwOilPress", label: "МАСЛО", step: 0.1, decimals: 1 },
    CalibSensorInfo { input: HWInput::HwFuelLvl, id: "HwFuelLvl", label: "ТОПЛ", step: 1.0, decimals: 0 },
];

/// Field calibration UI sub-mode (SENSOR_CALIBRATION_DESIGN.md's Field calibration UI
/// sketch), layered on top of the normal diagnostics screen -- entered via ДИАГ's "КАЛИБР"
/// button, own button set per state.
#[derive(Clone, PartialEq)]
enum CalibMode {
    Off,
    SelectSensor,
    /// Index into `CALIB_SENSORS`. The (reported, target) baseline lives in
    /// `DiagPage::calib_baseline`, not here -- it's seeded from a live sensor reading, which
    /// only `render()` has access to (see that field's doc comment).
    Adjust(usize),
}

pub struct DiagPage {
    base: PageBase,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,

    // STM32 ADC firmware commit hash ($VER reply). None if the ADC provider didn't start
    // or the firmware predates the $VER command.
    adc_version_frame: Option<AdcVersionFrame>,

    // Snapshots refreshed at most every DIAG_REFRESH_INTERVAL — see `refresh`.
    kernel_version: Option<String>,
    os_pretty_name: Option<String>,
    disk_usage_mb: Option<(u64, u64)>, // (total, available)
    throttle_status: Option<ThrottleStatus>,
    core_voltage: Option<f32>,
    arm_clock_mhz: Option<u32>,
    last_refresh: Instant,

    // Field calibration UI (SENSOR_CALIBRATION_DESIGN.md) -- see CalibMode/CALIB_SENSORS
    // above. `calib_offsets` is a clone of PageManager's registry: each calibrated sensor's
    // live value_offset cell, shared with the actual running `CalibratedVariableResistanceAnalogSensor`
    // (empty if the ADC was unavailable at startup, in which case CalibSelect is a no-op).
    calib_offsets: HashMap<String, Arc<AtomicU32>>,
    // Where a confirmed capture is persisted (sensor_calibration.json).
    calib_path: PathBuf,
    calib_mode: CalibMode,
    // (reported, target) for the sensor named by `calib_mode`'s `Adjust` index. `None` until
    // `render()` seeds it from a live reading -- `render` only has `&self`, so this needs
    // interior mutability rather than being set directly from `process_events`/`on_button`
    // (which don't get a `&SensorManager`). Reset to `None` on every mode transition.
    calib_baseline: Cell<Option<(f32, f32)>>,
}

impl DiagPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver,
               adc_version_frame: Option<AdcVersionFrame>,
               calib_offsets: HashMap<String, Arc<AtomicU32>>,
               calib_path: PathBuf) -> Self {
        let mut diag_page = DiagPage {
            base: PageBase::new(id, "Diag".to_string()),
            smart_event_sender,
            event_receiver,
            adc_version_frame,
            kernel_version: None,
            os_pretty_name: None,
            disk_usage_mb: None,
            throttle_status: None,
            core_voltage: None,
            arm_clock_mhz: None,
            last_refresh: Instant::now(),
            calib_offsets,
            calib_path,
            calib_mode: CalibMode::Off,
            calib_baseline: Cell::new(None),
        };

        diag_page.setup_buttons();
        diag_page.refresh();

        diag_page
    }

    pub fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ДАТЧ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(ADC_TERM_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ЖУРН".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(LOG_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left3, "ГНСС".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(GNSS_TERM_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "ОСЦ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(OSC_PAGE_ID))
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "ПЕРЕЗАГР".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::Restart)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right3, "КАЛИБР".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::CalibEnter)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(MAIN_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }

    /// Sensor-select screen of the field calibration UI -- one button per `CALIB_SENSORS`
    /// entry plus a back button. Only 3 entries today, so no scrolling/paging needed.
    fn setup_calib_select_buttons(&mut self) {
        let positions = [ButtonPosition::Left1, ButtonPosition::Left2, ButtonPosition::Left3];
        let mut buttons: Vec<PageButton<Box<dyn FnMut()>>> = CALIB_SENSORS.iter().zip(positions).map(|(info, pos)| {
            let sender = self.smart_event_sender.clone();
            let id = info.id.to_string();
            PageButton::new(pos, info.label.into(), Box::new(move || sender.send(UIEvent::CalibSelect(id.clone()))) as Box<dyn FnMut()>)
        }).collect();
        buttons.push(PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
            let sender = self.smart_event_sender.clone();
            move || sender.send(UIEvent::CalibBack)
        }) as Box<dyn FnMut()>));
        self.base.set_buttons(buttons);
    }

    /// Adjust screen: nudge the target value with +/- (held-repeat, see PageManager's
    /// BUTTON_HOLD_REPEAT_INTERVAL -- same pattern as GnssPage's КУРС+/КУРС-), then confirm
    /// or back out. +/- only act on press/repeat, not release, so a tap nudges once.
    fn setup_calib_adjust_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "−".into(), Box::new(|| {}) as Box<dyn FnMut()>)
                .with_onpress(Box::new({
                    let sender = self.smart_event_sender.clone();
                    move || sender.send(UIEvent::CalibDecrease)
                }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "+".into(), Box::new(|| {}) as Box<dyn FnMut()>)
                .with_onpress(Box::new({
                    let sender = self.smart_event_sender.clone();
                    move || sender.send(UIEvent::CalibIncrease)
                }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "ПОДТВ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::CalibConfirm)
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ОТМЕНА".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::CalibBack)
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }

    fn refresh(&mut self) {
        self.kernel_version = diagnostics::kernel_version();
        self.os_pretty_name = diagnostics::os_pretty_name();
        self.disk_usage_mb = diagnostics::root_disk_usage_mb();
        self.throttle_status = diagnostics::throttle_status();
        self.core_voltage = diagnostics::core_voltage();
        self.arm_clock_mhz = diagnostics::arm_clock_mhz();
        self.last_refresh = Instant::now();
    }

    fn na() -> String {
        "н/д".to_string()
    }

    /// Back to the main diagnostics screen, abandoning any in-progress calibration attempt
    /// (nothing is persisted unless CalibConfirm ran) -- used by CalibBack from the
    /// sensor-select screen and defensively on enter/exit.
    fn calib_reset(&mut self) {
        self.calib_mode = CalibMode::Off;
        self.calib_baseline.set(None);
        self.setup_buttons();
    }

    fn calib_nudge(&mut self, direction: f32) {
        if let CalibMode::Adjust(idx) = self.calib_mode {
            if let Some((reported, target)) = self.calib_baseline.get() {
                let step = CALIB_SENSORS[idx].step;
                self.calib_baseline.set(Some((reported, target + direction * step)));
            }
        }
    }

    /// Commits the adjust screen's current (reported, target) as this sensor's new
    /// value_offset -- live (the running sensor picks it up on its next read(), see
    /// CalibratedVariableResistanceAnalogSensor) and persisted to sensor_calibration.json so
    /// it survives a restart. A missing baseline (no live reading arrived yet -- see
    /// `calib_baseline`'s doc comment) or an id absent from the registry (ADC was
    /// unavailable at startup) just means there's nothing to commit; either way this always
    /// returns to the sensor-select screen.
    fn calib_confirm(&mut self) {
        if let CalibMode::Adjust(idx) = self.calib_mode {
            if let Some((reported, target)) = self.calib_baseline.get() {
                let info = &CALIB_SENSORS[idx];
                if let Some(cell) = self.calib_offsets.get(info.id) {
                    let record = CalibrationRecord { reported, true_value: target };
                    cell.store(record.offset().to_bits(), Ordering::Relaxed);

                    match sensor_calibration::load(&self.calib_path) {
                        Ok(mut records) => {
                            records.insert(info.id.to_string(), record);
                            if let Err(e) = sensor_calibration::save(&self.calib_path, &records) {
                                log::error!("sensor calibration: failed to persist {}: {}", info.id, e);
                            }
                        }
                        Err(e) => log::error!("sensor calibration: failed to reload before saving {}: {}", info.id, e),
                    }
                }
            }
        }
        self.calib_mode = CalibMode::SelectSensor;
        self.calib_baseline.set(None);
        self.setup_calib_select_buttons();
    }
}

impl Page for DiagPage {
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
        let title_font = ui_style.get_string(TEXT_PRIMARY_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let title_font_size = ui_style.get_integer(TEXT_PRIMARY_FONT_SIZE, 24);
        let title_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));
        let header_color = title_color;
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (0.8, 0.8, 0.8));

        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);

        let title = match self.calib_mode {
            CalibMode::Off => "ДИАГНОСТИКА",
            CalibMode::SelectSensor | CalibMode::Adjust(_) => "КАЛИБРОВКА ДАТЧИКОВ",
        };
        context.render_text_with_font(
            title, CONTENT_X_MARGIN, TITLE_Y, 1.0, title_color, &title_font, title_font_size,
        )?;

        let title_height = context.calculate_text_height_with_font(title, 1.0, &title_font, title_font_size)?;
        let line_height = context.get_line_height_with_font(1.0, &font, font_size)?;
        let mut y = TITLE_Y + title_height + TITLE_CONTENT_GAP;

        match self.calib_mode {
            CalibMode::Off => {
                let disk = self.disk_usage_mb.map(|(total, avail)| format!("{} / {} МБ своб.", avail, total)).unwrap_or_else(Self::na);
                let adc_version = self.adc_version_frame.as_ref().and_then(AdcVersionFrame::get).unwrap_or_else(Self::na);
                let lines: [(String, bool); 13] = [
                    ("СБОРКА:".to_string(), true),
                    (format!("  коммит {}  {}", GIT_HASH, BUILD_TIME), false),
                    (format!("  верс.АЦП  {}", adc_version), false),
                    (String::new(), false),
                    ("ПИТАНИЕ:".to_string(), true),
                    (format!("  троттл:  {}", self.throttle_status.as_ref().map(ThrottleStatus::summary).unwrap_or_else(Self::na)), false),
                    (format!("  напряж:  {}", self.core_voltage.map(|v| format!("{:.2} В", v)).unwrap_or_else(Self::na)), false),
                    (format!("  такт:    {}", self.arm_clock_mhz.map(|c| format!("{} МГц", c)).unwrap_or_else(Self::na)), false),
                    (String::new(), false),
                    ("ОС:".to_string(), true),
                    (format!("  ядро:    {}", self.kernel_version.clone().unwrap_or_else(Self::na)), false),
                    (format!("  сборка:  {}", self.os_pretty_name.clone().unwrap_or_else(Self::na)), false),
                    (format!("  диск:    {}", disk), false),
                ];

                for (text, is_header) in &lines {
                    if !text.is_empty() {
                        let color = if *is_header { header_color } else { text_color };
                        context.render_text_with_font(text, CONTENT_X_MARGIN, y, 1.0, color, &font, font_size)?;
                    }
                    y += line_height;
                }
            }
            CalibMode::SelectSensor => {
                context.render_text_with_font("ВЫБЕРИТЕ ДАТЧИК:", CONTENT_X_MARGIN, y, 1.0, header_color, &font, font_size)?;
                y += line_height * 1.5;
                for info in CALIB_SENSORS.iter() {
                    let value_str = sensor_manager.get_sensor_value(&info.input)
                        .map(|v| format!("{:.prec$} {}", v.as_f32(), v.metadata.unit, prec = info.decimals))
                        .unwrap_or_else(Self::na);
                    context.render_text_with_font(
                        &format!("  {:<8}{}", info.label, value_str), CONTENT_X_MARGIN, y, 1.0, text_color, &font, font_size,
                    )?;
                    y += line_height;
                }
            }
            CalibMode::Adjust(idx) => {
                let info = &CALIB_SENSORS[idx];
                let live = sensor_manager.get_sensor_value(&info.input);

                // Seed (reported, target) from the live reading the first time this screen
                // renders after CalibSelect -- see `calib_baseline`'s doc comment for why this
                // can't happen in process_events/on_button instead.
                if self.calib_baseline.get().is_none() {
                    if let Some(live) = live {
                        let displayed = live.as_f32();
                        let current_offset = self.calib_offsets.get(info.id)
                            .map(|cell| f32::from_bits(cell.load(Ordering::Relaxed)))
                            .unwrap_or(0.0);
                        // Curve-only value (offset subtracted back out) -- see
                        // CalibrationRecord::reported's doc comment for why.
                        self.calib_baseline.set(Some((displayed - current_offset, displayed)));
                    }
                }

                context.render_text_with_font(&format!("ДАТЧИК: {}", info.label), CONTENT_X_MARGIN, y, 1.0, header_color, &font, font_size)?;
                y += line_height * 1.5;

                match (live, self.calib_baseline.get()) {
                    (Some(live), Some((_reported, target))) => {
                        let unit = &live.metadata.unit;
                        context.render_text_with_font(
                            &format!("  ТЕКУЩЕЕ:  {:.prec$} {}", live.as_f32(), unit, prec = info.decimals),
                            CONTENT_X_MARGIN, y, 1.0, text_color, &font, font_size,
                        )?;
                        y += line_height;
                        context.render_text_with_font(
                            &format!("  ЦЕЛЬ:     {:.prec$} {}", target, unit, prec = info.decimals),
                            CONTENT_X_MARGIN, y, 1.0, text_color, &font, font_size,
                        )?;
                        y += line_height * 1.5;
                        context.render_text_with_font("  -/+ : ИЗМЕНИТЬ ЦЕЛЬ, ПОДТВ : СОХРАНИТЬ", CONTENT_X_MARGIN, y, 1.0, text_color, &font, font_size)?;
                    }
                    _ => {
                        context.render_text_with_font("  ОЖИДАНИЕ ПОКАЗАНИЯ ДАТЧИКА...", CONTENT_X_MARGIN, y, 1.0, text_color, &font, font_size)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        // Defensive: land on the main screen even if a previous session was left mid
        // calibration (e.g. an unclean exit) -- see calib_reset.
        self.calib_reset();
        self.refresh();
        Ok(())
    }

    fn on_exit(&mut self) -> Result<(), String> {
        // Abandon any in-progress (unconfirmed) calibration attempt rather than leaving the
        // page in a calibration sub-screen for the next on_enter to inherit.
        self.calib_reset();
        Ok(())
    }

    fn on_button(&mut self, _button: char) -> Result<(), String> {
        Ok(())
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                UIEvent::CalibEnter => {
                    self.calib_mode = CalibMode::SelectSensor;
                    self.calib_baseline.set(None);
                    self.setup_calib_select_buttons();
                }
                UIEvent::CalibSelect(id) => {
                    if let Some(idx) = CALIB_SENSORS.iter().position(|s| s.id == id) {
                        self.calib_mode = CalibMode::Adjust(idx);
                        self.calib_baseline.set(None);
                        self.setup_calib_adjust_buttons();
                    }
                }
                UIEvent::CalibIncrease => self.calib_nudge(1.0),
                UIEvent::CalibDecrease => self.calib_nudge(-1.0),
                UIEvent::CalibConfirm => self.calib_confirm(),
                UIEvent::CalibBack => match self.calib_mode {
                    CalibMode::Adjust(_) => {
                        self.calib_mode = CalibMode::SelectSensor;
                        self.calib_baseline.set(None);
                        self.setup_calib_select_buttons();
                    }
                    _ => self.calib_reset(),
                },
                _ => {}
            }
        }

        if self.last_refresh.elapsed() >= DIAG_REFRESH_INTERVAL {
            self.refresh();
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
