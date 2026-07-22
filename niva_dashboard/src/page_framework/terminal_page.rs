#![allow(dead_code)]
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::time::{Duration, Instant};

use crate::graphics::context::GraphicsContext;
use crate::graphics::text_box::TextBoxRenderer;
use crate::graphics::ui_style::*;
use crate::hardware::sensor_manager::SensorManager;
use crate::page_framework::events::{EventReceiver, SmartEventSender, UIEvent};
use crate::page_framework::page_manager::{ButtonPosition, Page, PageBase, PageButton, DIAG_PAGE_ID};
use crate::util::adc_data_provider::ADCFrame;

// Ring buffer capacity: only needs to comfortably outlast one screen's worth of wrapped
// rows, since manual scroll-back isn't supported — the box always shows the newest content.
const MAX_BUFFERED_LINES: usize = 200;
// How much log history to preload on construction, so the page isn't empty on first entry.
const LOG_SEED_LINES: usize = 100;
// The STM32 stream updates far faster than a human can read; throttle how often a fresh
// ADC frame gets turned into a new terminal line.
const ADC_PUSH_INTERVAL: Duration = Duration::from_millis(200);

// Content area insets: clears the vertical button-label columns on both sides, the top
// screen edge, and the bottom status line.
const CONTENT_X_MARGIN: f32 = 40.0;
const CONTENT_TOP_MARGIN: f32 = 10.0;
const CONTENT_BOTTOM_MARGIN: f32 = 35.0;

enum TerminalSource {
    Log { path: String, offset: u64 },
    Adc { frame: ADCFrame, last_push: Instant },
}

/// Generic scrolling-terminal diagnostic page. Backed by a `TextBoxRenderer` and driven by
/// either a tailed log file or the live ADC frame, depending on how it was constructed.
pub struct TerminalPage {
    base: PageBase,
    smart_event_sender: SmartEventSender,
    event_receiver: EventReceiver,
    text_box: TextBoxRenderer,
    source: TerminalSource,
}

impl TerminalPage {
    pub fn new_log(id: u32, name: &str, smart_event_sender: SmartEventSender, event_receiver: EventReceiver) -> Self {
        let path = Self::current_log_path();
        let mut text_box = TextBoxRenderer::new(MAX_BUFFERED_LINES);
        let offset = Self::seed_log_tail(&path, &mut text_box);

        let mut page = TerminalPage {
            base: PageBase::new(id, name.to_string()),
            smart_event_sender,
            event_receiver,
            text_box,
            source: TerminalSource::Log { path, offset },
        };
        page.setup_buttons();
        page
    }

    pub fn new_adc(id: u32, name: &str, smart_event_sender: SmartEventSender, event_receiver: EventReceiver, frame: ADCFrame) -> Self {
        let mut page = TerminalPage {
            base: PageBase::new(id, name.to_string()),
            smart_event_sender,
            event_receiver,
            text_box: TextBoxRenderer::new(MAX_BUFFERED_LINES),
            // Backdated so the first poll pushes a line immediately instead of waiting out
            // a full interval after page construction.
            source: TerminalSource::Adc { frame, last_push: Instant::now() - ADC_PUSH_INTERVAL },
        };
        page.setup_buttons();
        page
    }

    fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Right4, "ВОЗВ".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(DIAG_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }

    /// Same path construction as `util::logging::init_logging` — the file each run's log
    /// lines land in, after startup rotation.
    fn current_log_path() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs/niva_dashboard_rCURRENT.log")
    }

    /// Preloads the tail of the current log file into `text_box` and returns the byte
    /// offset to resume tailing from (end of file at seed time).
    fn seed_log_tail(path: &str, text_box: &mut TextBoxRenderer) -> u64 {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return 0;
        };
        let len = contents.len() as u64;
        let tail: Vec<&str> = contents.lines().rev().take(LOG_SEED_LINES).collect();
        for line in tail.into_iter().rev() {
            text_box.push_line(line.to_string());
        }
        len
    }

    /// Reads only newly-appended bytes since `offset` and pushes complete lines. Any
    /// trailing partial line is left unconsumed so it's picked up whole next poll.
    fn poll_log(path: &str, offset: &mut u64, text_box: &mut TextBoxRenderer) {
        let Ok(mut file) = File::open(path) else { return };
        let Ok(metadata) = file.metadata() else { return };
        let len = metadata.len();

        // File shrank — rotated or truncated since the last poll. Restart from the top.
        if len < *offset {
            *offset = 0;
        }
        if len == *offset {
            return;
        }
        if file.seek(SeekFrom::Start(*offset)).is_err() {
            return;
        }

        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            return;
        }

        if let Some(idx) = buf.rfind('\n') {
            for line in buf[..idx].lines() {
                text_box.push_line(line.to_string());
            }
            *offset += (idx + 1) as u64;
        }
    }

    fn poll_adc(frame: &ADCFrame, last_push: &mut Instant, text_box: &mut TextBoxRenderer) {
        if last_push.elapsed() < ADC_PUSH_INTERVAL {
            return;
        }
        *last_push = Instant::now();

        if frame.is_stale() {
            return;
        }

        let data = frame.get_data();
        let formatted = data.iter()
            .enumerate()
            .map(|(i, v)| format!("ch{i}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        text_box.push_line(formatted);
    }
}

impl Page for TerminalPage {
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
        let x = CONTENT_X_MARGIN;
        let y = CONTENT_TOP_MARGIN;
        let width = (context.width as f32 - 2.0 * CONTENT_X_MARGIN).max(0.0);
        let height = (context.height as f32 - CONTENT_TOP_MARGIN - CONTENT_BOTTOM_MARGIN).max(0.0);
        self.text_box.render(context, ui_style, x, y, width, height)
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
        // Drain page-scoped events even though none are currently handled, so they don't
        // pile up in the channel while this page is active.
        while self.event_receiver.try_recv().is_ok() {}

        match &mut self.source {
            TerminalSource::Log { path, offset } => Self::poll_log(path, offset, &mut self.text_box),
            TerminalSource::Adc { frame, last_push } => Self::poll_adc(frame, last_push, &mut self.text_box),
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
