#![allow(dead_code)]
use std::time::{Duration, Instant};

use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, MAIN_PAGE_ID, ADC_TERM_PAGE_ID, LOG_PAGE_ID};
use crate::hardware::sensor_manager::SensorManager;
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

pub struct DiagPage {
    base: PageBase,
    event_receiver: EventReceiver,
    smart_event_sender: SmartEventSender,

    // Snapshots refreshed at most every DIAG_REFRESH_INTERVAL — see `refresh`.
    kernel_version: Option<String>,
    os_pretty_name: Option<String>,
    disk_usage_mb: Option<(u64, u64)>, // (total, available)
    throttle_status: Option<ThrottleStatus>,
    core_voltage: Option<f32>,
    arm_clock_mhz: Option<u32>,
    last_refresh: Instant,
}

impl DiagPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver) -> Self {
        let mut diag_page = DiagPage {
            base: PageBase::new(id, "Diag".to_string()),
            smart_event_sender,
            event_receiver,
            kernel_version: None,
            os_pretty_name: None,
            disk_usage_mb: None,
            throttle_status: None,
            core_voltage: None,
            arm_clock_mhz: None,
            last_refresh: Instant::now(),
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
            PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(MAIN_PAGE_ID))
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

    fn render(&self, context: &mut GraphicsContext, _sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        let title_font = ui_style.get_string(TEXT_PRIMARY_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let title_font_size = ui_style.get_integer(TEXT_PRIMARY_FONT_SIZE, 24);
        let title_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (1.0, 1.0, 1.0));
        let header_color = title_color;
        let text_color = ui_style.get_color(TERMINAL_TEXT_COLOR, (0.8, 0.8, 0.8));

        let font = ui_style.get_string(TEXT_MONOSPACE_FONT, TERMINAL_FONT_PATH);
        let font_size = ui_style.get_integer(TEXT_MONOSPACE_FONT_SIZE, 16);

        context.render_text_with_font(
            "ДИАГНОСТИКА", CONTENT_X_MARGIN, TITLE_Y, 1.0, title_color, &title_font, title_font_size,
        )?;

        let title_height = context.calculate_text_height_with_font("ДИАГНОСТИКА", 1.0, &title_font, title_font_size)?;
        let line_height = context.get_line_height_with_font(1.0, &font, font_size)?;
        let mut y = TITLE_Y + title_height + TITLE_CONTENT_GAP;

        let disk = self.disk_usage_mb.map(|(total, avail)| format!("{} / {} МБ своб.", avail, total)).unwrap_or_else(Self::na);
        let lines: [(String, bool); 12] = [
            ("СБОРКА:".to_string(), true),
            (format!("  commit {}  {}", GIT_HASH, BUILD_TIME), false),
            (String::new(), false),
            ("ПИТАНИЕ:".to_string(), true),
            (format!("  троттл:  {}", self.throttle_status.as_ref().map(ThrottleStatus::summary).unwrap_or_else(Self::na)), false),
            (format!("  напряж:  {}", self.core_voltage.map(|v| format!("{:.2} В", v)).unwrap_or_else(Self::na)), false),
            (format!("  такт:    {}", self.arm_clock_mhz.map(|c| format!("{} МГц", c)).unwrap_or_else(Self::na)), false),
            (String::new(), false),
            ("ОС:".to_string(), true),
            (format!("  ядро:    {}", self.kernel_version.clone().unwrap_or_else(Self::na)), false),
            (format!("  дистр:   {}", self.os_pretty_name.clone().unwrap_or_else(Self::na)), false),
            (format!("  диск:    {}", disk), false),
        ];

        for (text, is_header) in &lines {
            if !text.is_empty() {
                let color = if *is_header { header_color } else { text_color };
                context.render_text_with_font(text, CONTENT_X_MARGIN, y, 1.0, color, &font, font_size)?;
            }
            y += line_height;
        }

        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        self.refresh();
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
