#![allow(dead_code)]
use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::diag_page::DiagPage;
use crate::page_framework::events::{UIEvent, EventReceiver, EventBus, SmartEventSender, create_event_bus};
use crate::page_framework::input::{InputHandler, InputSource, ButtonState};
use crate::page_framework::main_page::MainPage;
use crate::page_framework::terminal_page::TerminalPage;
use crate::page_framework::gnss_page::{GnssPage, GnssMode};
use crate::page_framework::horz_page::HorzPage;
use crate::page_framework::osc_page::OscPage;
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::hw_providers::HWInput;
use crate::hardware::heading_fusion_sensor::HeadingFusionSensor;
use crate::alerts::alert_manager::{AlertManager, Severity};
use crate::alerts::watchdog::Watchdog;
use crate::util::adc_data_provider::{ADCFrame, OscFrame};
use crate::util::gnss_data_provider::GnssFrame;
use crate::util::bno085_data_provider::Bno085Frame;
use crate::util::ups_monitor::UpsMonitor;
use crate::util::config::Config;

use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::fs;
use std::sync::mpsc::{Sender, Receiver};

const STATUS_LINE_X_MARGIN : f32 = 20.0;
const STATUS_LINE_Y_MARGIN : f32 = 25.0;

const PAGE_BUTTON_X_MARGIN: f32 = 4.0;      // Move a little from screen edge for better visibility.

// While a button stays physically held past this interval, its on_press callback fires
// again (e.g. GNSS page's КУРС+/КУРС- continuous heading adjust).
const BUTTON_HOLD_REPEAT_INTERVAL: Duration = Duration::from_millis(150);

pub const MAIN_PAGE_ID: u32 = 0;
pub const DIAG_PAGE_ID: u32 = 1;
pub const ADC_TERM_PAGE_ID: u32 = 2;
pub const LOG_PAGE_ID: u32 = 3;
pub const GNSS_TERM_PAGE_ID: u32 = 4;
pub const GNSS_PAGE_ID: u32 = 5;
pub const HORZ_PAGE_ID: u32 = 6;
pub const OSC_PAGE_ID: u32 = 7;

/// Config section key PageManager persists the last-open page under.
const LAST_PAGE_CONFIG_KEY: &str = "last_page";

/// Config section key a page's own persisted settings are stored under -- Config only knows
/// about opaque string keys, so PageManager (the only thing that knows pages have numeric ids)
/// turns each page's id into one.
fn page_config_key(page_id: u32) -> String {
    format!("page_{}", page_id)
}

// ButtonPosition correspond to physical 2x4 buttons layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ButtonPosition {
    Left1,
    Left2,
    Left3,
    Left4,
    Right1,
    Right2,
    Right3,
    Right4,
}

// PageButton represents UI button element on MFI page.
// It does not handle actual input.
pub struct PageButton<CB> {
    pos: ButtonPosition,
    pub label: String,
    callback: CB,
    on_press: Option<CB>,
}

impl<CB> PageButton<CB>
where
    CB: FnMut(),
{
    pub fn new(pos: ButtonPosition, label: String, callback: CB) -> Self {
        PageButton { pos, label, callback, on_press: None }
    }

    // Attaches a callback fired on press, and repeatedly while held (see PageManager's
    // held-button repeat sweep) -- independent of the release-triggered callback above.
    pub fn with_onpress(mut self, on_press: CB) -> Self {
        self.on_press = Some(on_press);
        self
    }

    // Invokes button-specific callback.
    pub fn trigger(&mut self) {
        (self.callback)();
    }

    // Invokes the on-press callback, if any.
    pub fn trigger_press(&mut self) {
        if let Some(on_press) = &mut self.on_press {
            on_press();
        }
    }

    // Used to match buttons with hardware input.
    pub fn position(&self) -> &ButtonPosition {
        &self.pos
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

// Shared data for MFI pages.
pub struct PageBase {
    id: u32,            // Incremental id, depends on page creation order.
    name: String,
    buttons: Vec<PageButton<Box<dyn FnMut()>>>,
}

impl PageBase {
    pub fn new(id: u32, name: String) -> Self {
        PageBase {
            id,
            name,
            buttons: Vec::new(),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_buttons(&mut self, mut buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        // Sort buttons by position to ensure correct order
        buttons.sort_by_key(|button| button.position().clone());
        self.buttons = buttons;
    }

    pub fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        &self.buttons
    }

    pub fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.buttons.iter().find(|button| button.position() == &pos)
    }

    pub fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.buttons.iter_mut().find(|button| button.position() == &pos)
    }
}

pub trait Page {
    fn id(&self) -> u32;
    fn name(&self) -> &str;
    // Render page-specific stuff (except button labels, which are PageManager responsibility).
    fn render(&self, context: &mut GraphicsContext, sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String>;
    // Trigger once on switching to this page.
    fn on_enter(&mut self) -> Result<(), String>;
    // Trigger once on switching from this page.
    fn on_exit(&mut self) -> Result<(), String>;
    // If PageManager does not handle button press, this will be called.
    fn on_button(&mut self, button: char) -> Result<(), String>;
    // Process events specific to this page (MPMC allows each page to have its own receiver)
    fn process_events(&mut self) {}

    // Returns this page's persistable settings as JSON, or `Value::Null` if it has none.
    // PageManager stores/restores this opaquely (see util::config::Config), without knowing
    // any page's own settings shape.
    fn get_config(&self) -> serde_json::Value { serde_json::Value::Null }
    // Restores settings previously returned by `get_config` (or `Value::Null` on first run /
    // no prior entry) -- implementations must tolerate Null and missing/stale fields.
    fn set_config(&mut self, _config: &serde_json::Value) {}

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>>;
    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>);

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>>;
    
    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>>;
}

// Helper struct to manage collection of pages.
// Introduced to avoid borrowing issues in PageManager.
struct Pages {
    pages: Vec<Box<dyn Page>>,
}

impl Pages {
    pub fn new() -> Self {
        Pages { pages: Vec::new() }
    }

    pub fn add_page(&mut self, page: Box<dyn Page>) {
        self.pages.push(page);
    }

    pub fn get_page(&self, id: u32) -> Option<&Box<dyn Page>> {
        for page in &self.pages {
            if page.id() == id {
                return Some(page);
            }
        }
        None
    }

    pub fn get_page_mut(&mut self, id: u32) -> Option<&mut Box<dyn Page>> {
        for page in &mut self.pages {
            if page.id() == id {
                return Some(page);
            }
        }
        None
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Box<dyn Page>> {
        self.pages.iter_mut()
    }
}

// PageManager is responsible for managing multiple pages and their transitions,
// as well as rendering button labels.
pub struct PageManager {
    // Rendering-related stuff.
    context: GraphicsContext,
    // Global UI style settings.
    ui_style: UIStyle,

    // Takes care of low-level hw input, signal processing, and conversion
    // to actual sensor values.
    sensor_manager: SensorManager,

    // UI pages related stuff.
    pg_id: u32,             // Page incremental id, depends on page creation order.
    current_page: Option<u32>,
    pages: Pages,

    // Per-page persisted settings + last-opened page, loaded once at startup and updated on
    // every page switch (see Page::get_config/set_config).
    config: Config,

    // Input handling from gpio buttons, external keyboard, etc.
    input_handler: InputHandler,

    // Map hardware keys with UI buttons positions.
    buttons_map: HashMap<char, ButtonPosition>,

    // Positions currently held down, mapped to when their on_press callback last fired.
    // Used both to draw a frame around the label and to drive the held-button repeat sweep.
    pressed_positions: HashMap<ButtonPosition, Instant>,

    // Event system for UI communication (dual-channel).
    event_bus: EventBus,
    global_event_receiver: EventReceiver,  // PageManager listens to global events
    smart_event_sender: SmartEventSender,  // Smart sender routes events automatically

    // Sensors switching event channel
    sensor_config_rx: Receiver<SensorManager>,
    sensor_config_tx: Sender<SensorManager>,

    // Alert system
    alert_manager: AlertManager,

    // Tracks on-battery duration from the UPS sensor chain and triggers a shutdown once it
    // has persisted too long. Its check() is a no-op when no UPS sensor value is available
    // (e.g. I2C unavailable) — see UpsMonitor::check.
    ups_monitor: UpsMonitor,

    // Handle to the shared ADC frame, used to build the ADC diagnostic terminal page.
    // None when the ADC data provider failed to start.
    adc_frame: Option<ADCFrame>,

    // Handle for requesting oscilloscope burst captures, used to build the osc page. None
    // under the same condition as adc_frame (they come from the same provider).
    osc_frame: Option<OscFrame>,

    // Handle to the shared GNSS line buffer, used to build the GNSS diagnostic terminal
    // page. None when the GNSS data provider failed to start.
    gnss_frame: Option<GnssFrame>,

    // Handle to the shared BNO085 frame, passed straight into GnssPage for its raw INS
    // diagnostics block (same rationale as gnss_frame above -- composite data the
    // HWInput/sensor-chain pipeline doesn't carry). None when the BNO085 data provider
    // failed to start.
    bno_frame: Option<Bno085Frame>,

    // Reads GnssFrame/Bno085Frame directly and is ticked once per loop iteration below,
    // independent of `sensor_manager`'s self-test/real handoff -- see
    // hardware::heading_fusion_sensor. None when either source's data provider failed to
    // start (see main.rs::setup_sensors).
    heading_fusion: Option<HeadingFusionSensor>,

    fps_counter: FpsCounter,
    start_time: Instant,

    // Cached /proc/stat snapshot for non-blocking CPU load calculation.
    // Every frame a new sample is taken and appended to cpu_load_samples.
    // Every CPU_LOAD_UPDATE_INTERVAL the samples are averaged into cpu_load.
    last_cpu_stat: Option<(u64, u64)>,  // (idle, total)
    cpu_load_samples: Vec<f32>,         // per-frame samples since last update
    cpu_load_last_update: Instant,      // time of the last average flush
    cpu_load: f32,                      // last published average, shown in status line

    // CPU temperature is read from sysfs at most once every 3 seconds.
    cpu_temp: Option<f32>,
    cpu_temp_last_update: Instant,

    // If set to false, main loop will exit.
    running: bool,
}

impl PageManager {
    pub fn new(context: GraphicsContext, sensor_manager: SensorManager, ui_style: UIStyle,
               input_sources: Vec<Box<dyn InputSource>>, ups_monitor: UpsMonitor,
               adc_frame: Option<ADCFrame>, osc_frame: Option<OscFrame>, gnss_frame: Option<GnssFrame>,
               bno_frame: Option<Bno085Frame>,
               alert_manager: AlertManager, heading_fusion: Option<HeadingFusionSensor>) -> Self {
        let mut buttons_map = HashMap::new();
        buttons_map.insert('1', ButtonPosition::Left1);
        buttons_map.insert('2', ButtonPosition::Left2);
        buttons_map.insert('3', ButtonPosition::Left3);
        buttons_map.insert('4', ButtonPosition::Left4);
        buttons_map.insert('5', ButtonPosition::Right1);
        buttons_map.insert('6', ButtonPosition::Right2);
        buttons_map.insert('7', ButtonPosition::Right3);
        buttons_map.insert('8', ButtonPosition::Right4);

        // Create event bus with dual-channel system
        let event_bus = create_event_bus();
        let global_event_receiver = event_bus.global_receiver();
        let smart_event_sender = event_bus.smart_sender();

        // Event channel for switching self-test sequence sensors to real ones
        let (sensor_config_tx, sensor_config_rx) = std::sync::mpsc::channel::<SensorManager>();

        PageManager {
            context,
            ui_style,
            sensor_manager,
            pg_id: 0,
            current_page: None,
            pages: Pages::new(),
            config: Config::load(),
            input_handler: InputHandler::new(input_sources),
            buttons_map,
            pressed_positions: HashMap::new(),
            event_bus,
            global_event_receiver,
            smart_event_sender,
            sensor_config_rx,
            sensor_config_tx,
            alert_manager,
            ups_monitor,
            adc_frame,
            osc_frame,
            gnss_frame,
            bno_frame,
            heading_fusion,
            fps_counter: FpsCounter::new(),
            start_time: Instant::now(),
            last_cpu_stat: None,
            cpu_load_samples: Vec::new(),
            cpu_load_last_update: Instant::now(),
            cpu_load: 0.0,
            cpu_temp: None,
            cpu_temp_last_update: Instant::now(),
            running: false,
        }
    }

    fn get_page_id(&mut self) -> u32 {
        let id = self.pg_id;
        while (self.get_page(id)).is_some() {
            self.pg_id += 1;
        }
        self.pg_id
    }

    /// Get a new event receiver for pages (page-specific events only)
    pub fn get_event_receiver(&self) -> EventReceiver {
        self.event_bus.page_receiver()
    }

    /// Get the smart event sender for UI components (auto-routes events)
    pub fn get_smart_event_sender(&self) -> SmartEventSender {
        self.smart_event_sender.clone()
    }

    // Set sensor manager switch receiver
    pub fn get_sensor_config_tx(&self) -> Sender<SensorManager>  {
        self.sensor_config_tx.clone()
    }

    fn get_page(&self, id: u32) -> Option<&Box<dyn Page>> {
        self.pages.get_page(id)
    }

    fn get_page_mut(&mut self, id: u32) -> Option<&mut Box<dyn Page>> {
        self.pages.get_page_mut(id)
    }

    fn get_current_page(&self) -> Option<&Box<dyn Page>> {
        if let Some(page_id) = self.current_page {
            self.get_page(page_id)
        } else {
            None
        }
    }

    fn get_current_page_mut(&mut self) -> Option<&mut Box<dyn Page>> {
        if let Some(page_id) = self.current_page {
            self.get_page_mut(page_id)
        } else {
            None
        }
    }

    fn render_current_page(&mut self) -> Result<(), String> {
        if let Some(page_id) = self.current_page {
            match self.pages.get_page_mut(page_id) {
                Some(page) => page.render(&mut self.context, &self.sensor_manager, &self.ui_style),
                None => Err(format!("Current page id {} not found", page_id)),
            }?;
        }

        Ok(())
    }

    pub fn add_page(&mut self, page: Box<dyn Page>) -> u32 {
        let page_id = page.id();
        // Check if page with same id already exists - abort if so,
        // because page switching would be ambiguous.
        if self.get_page(page_id).is_some() {
            log::warn!("Warning: Page with id {} already exists, exiting", page_id);
            std::process::exit(1);
        }
        self.pages.add_page(page);
        page_id
    }

    // Switch to a page by index
    pub fn switch_page(&mut self, page_id: u32) -> Result<(), String> {
        if self.get_page(page_id).is_none() {
            log::warn!("Cannot switch to page {}: not registered", page_id);
            return Ok(());
        }

        // Call on_exit for old page first.
        let outgoing_id = self.current_page;
        if let Some(current) = self.get_current_page_mut() {
            current.on_exit()?;
        }

        // Persist the outgoing page's config (post on_exit, so its freshest state is
        // captured) and record the page we're switching to as the one to reopen next
        // launch -- done on every switch, not just at shutdown, so an unclean exit right
        // after doesn't lose either.
        if let Some((id, config)) = outgoing_id.and_then(|id| self.get_page(id).map(|p| (id, p.get_config()))) {
            self.config.set_section(&page_config_key(id), config);
        }
        self.config.set_section(LAST_PAGE_CONFIG_KEY, serde_json::json!(page_id));

        self.current_page = Some(page_id);

        // Call on_enter for new page.
        if let Some(current) = self.get_current_page_mut() {
            current.on_enter()?;
        }

        Ok(())
    }

    fn button_by_key(&mut self, key: &char) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        let pos = self.buttons_map.get(key).copied()?;
        self.get_current_page_mut()?.button_by_position_mut(pos)
    }

    // Track currently held-down positions, used to frame their label while pressed.
    fn set_button_pressed(&mut self, key: char, pressed: bool) {
        if let Some(pos) = self.buttons_map.get(&key).copied() {
            if pressed {
                self.pressed_positions.insert(pos, Instant::now());
            } else {
                self.pressed_positions.remove(&pos);
            }
        }
    }

    // Set up pages, buttons and watchdogs.
    pub fn setup(&mut self) -> Result<(), String> {
        // Get smart event sender for button callbacks
        let smart_sender = self.smart_event_sender.clone();

        // Create and add pages first to get their IDs
        let main_page = Box::new(MainPage::new(MAIN_PAGE_ID,
                                               smart_sender.clone(),
                                               self.get_event_receiver(),
                                               &self.context,
                                               &self.ui_style));

        let diag_page = Box::new(DiagPage::new(DIAG_PAGE_ID,
                                               smart_sender.clone(),
                                               self.get_event_receiver()));

        let log_page = Box::new(TerminalPage::new_log(LOG_PAGE_ID, "Log",
                                                       smart_sender.clone(),
                                                       self.get_event_receiver()));

        let horz_page = Box::new(HorzPage::new(HORZ_PAGE_ID,
                                                smart_sender.clone(),
                                                self.get_event_receiver(),
                                                self.bno_frame.clone(),
                                                &self.ui_style));

        self.add_page(main_page);
        self.add_page(diag_page);
        self.add_page(log_page);
        self.add_page(horz_page);

        // ADC terminal page only exists when the ADC data provider actually started —
        // without a frame handle there is nothing for it to display.
        if let Some(frame) = self.adc_frame.clone() {
            let adc_page = Box::new(TerminalPage::new_adc(ADC_TERM_PAGE_ID, "ADC",
                                                           smart_sender.clone(),
                                                           self.get_event_receiver(),
                                                           frame));
            self.add_page(adc_page);
        }

        // Osc page is always registered, even without an osc_frame handle (ADC data
        // provider unavailable) -- it just renders "АЦП НЕДОСТУПНО" instead of a graph in
        // that case, and the diag page's ОСЦ button is unconditional (same lenient pattern
        // as its ДАТЧ/ГНСС buttons, which no-op if their target page isn't registered).
        let osc_page = Box::new(OscPage::new(OSC_PAGE_ID,
                                              smart_sender.clone(),
                                              self.get_event_receiver(),
                                              self.osc_frame.clone()));
        self.add_page(osc_page);

        // GNSS terminal page only exists when the GNSS data provider actually started —
        // without a frame handle there is nothing for it to display.
        if let Some(frame) = self.gnss_frame.clone() {
            let gnss_term_page = Box::new(TerminalPage::new_gnss(GNSS_TERM_PAGE_ID, "GNSS",
                                                             smart_sender.clone(),
                                                             self.get_event_receiver(),
                                                             frame.clone()));
            self.add_page(gnss_term_page);

            let gnss_page = Box::new(GnssPage::new(GNSS_PAGE_ID,
                                                     smart_sender.clone(),
                                                     self.get_event_receiver(),
                                                     frame,
                                                     self.bno_frame.clone(),
                                                     GnssMode::PNP,
                                                     &self.ui_style));
            self.add_page(gnss_page);
        }

        // Restore every page's persisted config in one place, rather than at each add_page
        // call site above -- a page added here without going through this loop would
        // silently lose its settings on every restart.
        for page in self.pages.iter_mut() {
            let config = self.config.section(&page_config_key(page.id()));
            page.set_config(&config);
        }

        // Reopen wherever the user left off last run, falling back to Main if that page
        // no longer exists (e.g. GNSS was persisted as last-open but the receiver failed
        // to start this run) or this is a first run with nothing persisted yet.
        let start_page_id = self.config.section(LAST_PAGE_CONFIG_KEY).as_u64()
            .map(|id| id as u32)
            .filter(|id| self.get_page(*id).is_some())
            .unwrap_or(MAIN_PAGE_ID);
        self.switch_page(start_page_id)?;

        // Set up watchdogs for alert manager.
        // Display timeout: how long alert is displayed on screen before automatically hidden. None means displayed until manually suppressed.
        // Remove timeout: how long alert stays in queue after is was suppressed and is no longer visible. Suppressed alerts, which are not removed from queue,
        // prevent alerts of the same type from triggering. Used to prevent alerts flooding. None means no timeout -> removed immediately.
        // Trigger duration: how long a condition has to persist to trigger an alert.
        let engine_temp_watchdog = Watchdog::new(
            HWInput::HwEngineCoolantTemp,
            "ТЕМПЕРАТУРА ДВИГАТЕЛЯ".to_string(),
            Severity::Critical,
            None,                                           // No display timeout
            Some(std::time::Duration::from_secs(60)),       // Wait 60 s before displaying again
            None,                                           // Eng temp doesn't fluctuate, if reached critical, display alert immediately
        );
        let oil_press_low_watchdog = Watchdog::new(
            HWInput::HwOilPressLow,
            "ДАВЛЕНИЕ МАСЛА".to_string(),
            Severity::Critical,
            None,                                           // No display timeout
            Some(std::time::Duration::from_secs(30)),       // Wait 30 s before displaying again (engine should not run on low oil pressure long)
            None,                                           // Trigger immediately to catch even brief sensor triggers
        );

        let adc_link_watchdog = Watchdog::new(
            HWInput::HwAdcLink,
            "ОШИБКА СВЯЗИ АЦП".to_string(),
            Severity::Critical,
            None,                                           // No display timeout - stays visible for as long as the link is down
            Some(std::time::Duration::from_secs(10)),
            None,                                           // Trigger immediately, no persistence delay
        );

        let gnss_link_watchdog = Watchdog::new(
            HWInput::HwGnssLink,
            "ОШИБКА СВЯЗИ ГНСС".to_string(),
            Severity::Warning,
            Some(std::time::Duration::from_secs(30)),       // Display for 30 s
            Some(std::time::Duration::from_secs(10*60)),    // Suppress for 10 minutes
            None,                                           // Trigger immediately, no persistence delay
        );

        // "On battery" is a Warning here — the actual shutdown decision has its own,
        // longer-delayed timer in UpsMonitor::check. This just surfaces it to the driver.
        let ups_on_battery_watchdog = Watchdog::new(
            HWInput::HwUPSCurrent,
            "РЕЗЕРВНОЕ ПИТАНИЕ".to_string(),
            Severity::Warning,
            Some(std::time::Duration::from_secs(30)),       // Display for 30 s
            Some(std::time::Duration::from_secs(60)),       // Wait 60 s before displaying again
            Some(std::time::Duration::from_secs(10)),       // Ignore brief transients
        );
        // 2 stages: warning on 25% charge and critical on 15% charge (controlled by sensor value constraints).
        let ups_low_charge_watchdog = Watchdog::new(
            HWInput::HwUPSChargeState,
            "ЗАРЯД РЕЗЕРВ АКБ".to_string(),
            Severity::Warning,
            Some(std::time::Duration::from_secs(60)),       // Display for 60 s
            Some(std::time::Duration::from_secs(60*3)),     // Wait 3 min before displaying again
            Some(std::time::Duration::from_secs(5)),        // Ignore brief transients
        );
        let ups_crit_charge_watchdog = Watchdog::new(
            HWInput::HwUPSChargeState,
            "ЗАРЯД РЕЗЕРВ АКБ".to_string(),
            Severity::Critical,
            None,                                           // No display timeout - stays visible while charge is critically low
            Some(std::time::Duration::from_secs(60*3)),     // Wait 3 min before displaying again
            Some(std::time::Duration::from_secs(5)),        // Ignore brief transients
        );

        self.alert_manager.add_watchdog(engine_temp_watchdog);
        self.alert_manager.add_watchdog(oil_press_low_watchdog);
        self.alert_manager.add_watchdog(adc_link_watchdog);
        self.alert_manager.add_watchdog(gnss_link_watchdog);
        self.alert_manager.add_watchdog(ups_on_battery_watchdog);
        self.alert_manager.add_watchdog(ups_low_charge_watchdog);
        self.alert_manager.add_watchdog(ups_crit_charge_watchdog);

        Ok(())
    }

    // Do first-time initialization and start main loop.
    // Normally, main loop should not exit until device shutdown on external power loss.
    pub fn start(&mut self) -> Result<(), String> {
        // Hide mouse cursor for dashboard
        if let Err(e) = self.context.hide_cursor() {
            log::warn!("Warning: Failed to hide cursor: {}", e);
        }
        
        // Fonts are now created on-demand when needed
        
        log::info!("Dashboard initialized successfully!");
        self.running = true;
        self.start_time = Instant::now();
        self.event_loop()
    }

    fn event_loop(&mut self) -> Result<(), String> {
        log::info!("Starting event loop");
        self.toggle_bloom();    // Turn off for now
        
        while self.running {
            if crate::util::shutdown::shutdown_requested() {
                log::info!("Shutdown signal received (SIGTERM/SIGINT)");
                self.running = false;
                continue;
            }
            if crate::util::shutdown::binary_updated() {
                log::info!("New binary detected on disk");
                self.running = false;
                continue;
            }
            if crate::util::shutdown::restart_requested() {
                log::info!("Restart requested (SIGUSR1)");
                self.running = false;
                continue;
            }

            // Continuous sensor polling - poll sensors every loop iteration
            // This ensures sensor data is always up to date regardless of render timing
            if let Err(e) = self.sensor_manager.read_all_sensors() {
                // Suppress: while the ADC link is down, ADC-backed chains fail with
                // "channel not in frame" until AdcDataProvider's reconnect loop recovers
                // it — expected and already surfaced via the ADC LINK alert.
                //
                // Also suppress GnssChannelProvider's "no GNSS ... in current fix" —
                // unlike the ADC case this isn't tied to link health at all (see
                // GnssLinkStatusProvider vs GnssChannelProvider in hw_providers.rs): a
                // fully-connected receiver can legitimately go without a field (heading
                // especially, which needs an RTK dual-antenna fix) for extended, routine
                // periods. Already surfaced per-field by the corresponding indicator
                // simply showing no data, so it doesn't need error-level logging either.
                let gnss_field_unavailable = e.starts_with("no GNSS ");
                if !self.sensor_manager.adc_link_down() && !gnss_field_unavailable {
                    log::error!("Sensor read error: {}", e);
                }
            }
            // Ticked directly rather than through a sensor chain -- see
            // hardware::heading_fusion_sensor's module doc for why -- and independent of
            // sensor_manager's self-test/real handoff, so its state (anchor, confidence,
            // persistence timer) isn't disturbed by that swap.
            if let Some(heading_fusion) = &mut self.heading_fusion {
                let fusion_output = heading_fusion.tick();
                self.sensor_manager.set_external_value(HWInput::HwHeading, fusion_output.heading);
                self.sensor_manager.set_external_value(HWInput::HwHeadingConfidence, fusion_output.confidence);
                self.sensor_manager.set_external_value(HWInput::HwHeadingAccuracy, fusion_output.accuracy);
                self.sensor_manager.set_external_value(HWInput::HwDeadReckoningElapsed, fusion_output.dead_reckoning_elapsed);
            }

            self.alert_manager.check_watchdogs(&self.sensor_manager);
            self.ups_monitor.check(&self.sensor_manager);

            // Update FPS counter
            self.fps_counter.update();
            
            // Begin bloom rendering if enabled
            let bloom_enabled = self.context.is_bloom_enabled();
            if bloom_enabled {
                if let Err(e) = self.context.begin_bloom_render() {
                    log::error!("Bloom render error: {}", e);
                }
            } else {
                // Clear screen with black for normal rendering
                self.context.clear_screen();
            }
        
            unsafe {
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            }

            // Render the frame - sensors were already read above
            self.render_current_page()?;

            self.alert_manager.render_alerts(&mut self.context);

            self.render_button_labels()?;
            
            self.render_status_line()?;
            
            // Apply bloom effect and swap buffers
            if bloom_enabled {
                if let Err(e) = self.context.end_bloom_render() {
                    log::error!("Bloom end render error: {}", e);
                }
            }
            
            // Swap buffers - pacing is handled by the DRM page flip
            self.context.swap_buffers();

            // Check for button state changes (processed every loop iteration for responsiveness)
            if let Some(state) = self.input_handler.button_state() {
                match state {
                    ButtonState::Pressed(key) => {
                        log::info!("Button pressed: {}", key);
                        self.set_button_pressed(key, true);
                        if let Some(button) = self.button_by_key(&key) {
                            button.trigger_press();
                        }
                    }
                    ButtonState::Released(key) => {
                        log::info!("Button released: {}", key);
                        self.set_button_pressed(key, false);
                        if let Some(button) = self.button_by_key(&key) {
                            button.trigger();
                        } else if key == 'q' {
                            // For debugging, allow 'q' key to quit the loop
                            log::info!("'q' pressed - exiting event loop");
                            self.running = false;
                        }
                    }
                }
            }

            // Held-button repeat: re-fire on_press for any position still held past
            // BUTTON_HOLD_REPEAT_INTERVAL (e.g. GNSS page's КУРС+/КУРС- continuous adjust).
            // Positions are collected up front since the trigger below needs a fresh
            // mutable borrow of self for each lookup.
            let now = Instant::now();
            let due_positions: Vec<ButtonPosition> = self.pressed_positions.iter()
                .filter(|(_, &last)| now.duration_since(last) >= BUTTON_HOLD_REPEAT_INTERVAL)
                .map(|(&pos, _)| pos)
                .collect();
            for pos in due_positions {
                if let Some(button) = self.get_current_page_mut().and_then(|p| p.button_by_position_mut(pos)) {
                    button.trigger_press();
                }
                self.pressed_positions.insert(pos, now);
            }

            // Process global UI events (PageManager events only)
            // With dual-channel system, PageManager only receives global events
            while let Ok(event) = self.global_event_receiver.try_recv() {
                self.handle_ui_event(event);
            }

            // Let the current page process its own events
            if let Some(current_page) = self.get_current_page_mut() {
                current_page.process_events();
            }
        }
        
        log::info!("Event loop finished");
        
        Ok(())
    }

    /// Handle UI events sent by buttons and other components
    fn handle_ui_event(&mut self, event: UIEvent) {
        log::info!("Processing UI event: {:?}", event);
        
        match event {
            UIEvent::BrightnessUp => {
                self.brightness_up();
            }
            UIEvent::BrightnessDown => {
                self.brightness_down();
            }
            UIEvent::SetBrightness(level) => {
                self.set_brightness(level);
            }
            UIEvent::SwitchToPage(page_id) => {
                if let Err(e) = self.switch_page(page_id) {
                    log::error!("Failed to switch to page {}: {}", page_id, e);
                }
            }
            UIEvent::Shutdown => {
                log::info!("Shutdown event received");
                self.running = false;
            }
            UIEvent::Restart => {
                log::info!("Restart event received, issuing reboot");
                match std::process::Command::new("sudo").arg("reboot").status() {
                    Ok(status) if status.success() => log::info!("Reboot issued"),
                    Ok(status) => log::error!("Reboot command exited with {}", status),
                    Err(e) => log::error!("Failed to spawn reboot: {}", e),
                }
                self.running = false;
            }
            UIEvent::SuppressAlerts => {
                self.alert_manager.suppress_alerts();
            }
            UIEvent::ButtonPressed(action) => {
                log::info!("Custom button action: {}", action);
                // Handle custom button actions here
                match action.as_str() {
                    "view_up" => log::info!("View up action"),
                    "view_down" => log::info!("View down action"),
                    "engine_data" => log::info!("Engine data diagnostic action"),
                    "clear_codes" => log::info!("Clear diagnostic codes action"),
                    _ => log::info!("Unknown action: {}", action),
                }
            }
            UIEvent::SwitchSensorSet => {
                if let Ok(new_manager) = self.sensor_config_rx.try_recv() {
                    self.sensor_manager = new_manager;
                }
                // Self-test sweep has finished handing off to real sensors — safe to start
                // raising alerts now that watchdogs read live hardware, not the synthetic sweep.
                self.alert_manager.set_enabled(true);
            }
            UIEvent::NavHeadingIncrease => {
                self.adjust_manual_heading(1.0);
            }
            UIEvent::NavHeadingDecrease => {
                self.adjust_manual_heading(-1.0);
            }
            _ => {}
        }
    }
    
    // Nudges the fused heading's manual anchor by delta_deg, based on the currently
    // displayed heading. No-op if heading fusion isn't running (e.g. BNO085 or GNSS
    // provider failed to start).
    fn adjust_manual_heading(&mut self, delta_deg: f32) {
        let current = self.sensor_manager.get_sensor_value(&HWInput::HwHeading)
            .map(|v| v.as_f32())
            .unwrap_or(0.0);
        if let Some(heading_fusion) = self.heading_fusion.as_mut() {
            heading_fusion.set_manual_heading((current + delta_deg).rem_euclid(360.0));
        }
    }

    fn get_button_position(&self, pos: &ButtonPosition, _orientation: &String) -> (f32, f32) {
        let screen_width = self.context.width as f32;
        let screen_height = self.context.height as f32 - STATUS_LINE_Y_MARGIN;
        let x_margin = 0.0;   // No horizontal margin
        let y_margin = 30.0;  // Small vertical margin from screen edges
        
        // Define fixed Y positions for each button row (1-4)
        // First button near top, last button near bottom, middle two evenly spaced
        let available_height = screen_height - 2.0 * y_margin;
        let y_positions = [
            y_margin,                                    // Row 1 - near top
            y_margin + available_height / 3.0,           // Row 2 - 1/3 down
            y_margin + 2.0 * available_height / 3.0,     // Row 3 - 2/3 down
            screen_height - y_margin,                    // Row 4 - near bottom
        ];
        
        match pos {
            ButtonPosition::Left1 => (x_margin, y_positions[0]),
            ButtonPosition::Left2 => (x_margin, y_positions[1]),
            ButtonPosition::Left3 => (x_margin, y_positions[2]),
            ButtonPosition::Left4 => (x_margin, y_positions[3]),
            ButtonPosition::Right1 => (screen_width - x_margin, y_positions[0]),
            ButtonPosition::Right2 => (screen_width - x_margin, y_positions[1]),
            ButtonPosition::Right3 => (screen_width - x_margin, y_positions[2]),
            ButtonPosition::Right4 => (screen_width - x_margin, y_positions[3]),
        }
    }
    
    fn render_button_at_position(&mut self, pos: &ButtonPosition, label: &str,
        label_font: &String, label_font_size: u32, label_color: (f32, f32, f32),
        orientation: &String, is_pressed: bool
    ) -> Result<(), String> {
        let (x, mut y) = self.get_button_position(pos, orientation);

        let (text_width, text_height) = if orientation == "horizontal" {
            (
                self.context.calculate_text_width_with_font(label, 1.0, label_font, label_font_size)?,
                self.context.calculate_text_height_with_font(label, 1.0, label_font, label_font_size)?,
            )
        } else {
            (
                self.context.calculate_text_width_with_font_vert(label, 1.0, label_font, label_font_size)?,
                self.context.calculate_text_height_with_font_vert(label, 1.0, label_font, label_font_size)?,
            )
        };

        if orientation == "vertical" {
            // Special case for vertical orientation: adjust y position
            // so that label y center point alingns with button position
            y = y - (text_height / 2.0);
            // Adjust if out of bounds
            if y < 0.0 {
                y = 0.0;
            }
        }

        let render_x = match pos {
            // Right side buttons are right-aligned
            ButtonPosition::Right1 | ButtonPosition::Right2 |
            ButtonPosition::Right3 | ButtonPosition::Right4 => {
                x - text_width - PAGE_BUTTON_X_MARGIN
            }
            // Left side buttons are left-aligned
            _ => x + PAGE_BUTTON_X_MARGIN,
        };

        if orientation == "horizontal" {
            self.context.render_text_with_font(
                label,
                render_x,
                y,
                1.0,
                label_color,
                label_font,
                label_font_size
            )?;
        } else {
            self.context.render_text_with_font_vert(
                label,
                render_x,
                y,
                1.0,
                label_color,
                label_font,
                label_font_size
            )?;
        }

        if is_pressed {
            let padding = self.ui_style.get_float(PAGE_BUTTON_PRESSED_FRAME_PADDING, 4.0);
            let thickness = self.ui_style.get_float(PAGE_BUTTON_PRESSED_FRAME_WIDTH, 2.0);
            let frame_color = self.ui_style.get_color(PAGE_BUTTON_PRESSED_FRAME_COLOR, (1.0, 1.0, 1.0));
            self.context.stroke_rect(
                render_x - padding,
                y - padding,
                text_width + 2.0 * padding,
                text_height + 2.0 * padding,
                frame_color,
                thickness,
            )?;
        }

        Ok(())
    }
    
    fn render_button_labels(&mut self) -> Result<(), String> {
        // Check if there's a current page
        if self.current_page.is_none() {
            return Ok(());
        }

        // Render settings
        let label_font = self.ui_style.get_string(PAGE_BUTTON_LABEL_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let label_font_size = self.ui_style.get_integer(PAGE_BUTTON_LABEL_FONT_SIZE, 14);
        let label_color = self.ui_style.get_color(PAGE_BUTTON_LABEL_COLOR, (1.0, 1.0, 1.0));
        let orientation = self.ui_style.get_string(PAGE_BUTTON_LABEL_ORIENTATION, "horizontal");

        // Collect button data first to avoid borrowing conflicts
        let button_data: Vec<(ButtonPosition, String, bool)> = {
            let current_page = self.get_current_page().unwrap();
            current_page.buttons()
                .iter()
                .map(|button| (*button.position(), button.label().to_string(), self.pressed_positions.contains_key(button.position())))
                .collect()
        };

        // Now render each button at its fixed position
        for (position, label, is_pressed) in button_data {
            self.render_button_at_position(&position, &label, &label_font, label_font_size, label_color, &orientation, is_pressed)?;
        }

        Ok(())
    }
    
    fn render_status_line(&mut self) -> Result<(), String> {
        let elapsed = self.start_time.elapsed();
        let fps = self.fps_counter.get_fps();
        let _frame_count = self.fps_counter.get_frame_count();
        
        // Get memory information
        let (mem_total, mem_available) = self.get_memory_info().unwrap_or((0, 0));

        // Get CPU load and temperature.
        // sample_cpu_stat snapshots /proc/stat and computes load from the delta
        // since the last frame — no blocking sleep needed.
        self.sample_cpu_stat();
        let cpu_load = self.get_cpu_load();
        let cpu_temp = self.get_cpu_temperature();
        
        // Format status information
        let total_seconds = elapsed.as_secs();
        let days = total_seconds / 86400;  // 24 * 60 * 60
        let hours = (total_seconds % 86400) / 3600;  // 60 * 60
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        let uptime_str = if days > 0 {
            format!("{}д {:02}:{:02}:{:02}", days, hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        };

        let cpu_temp_str = match cpu_temp {
            Some(t) => format!("{:.0}°C", t),
            None    => "–".to_string(),
        };

        // UPS current: positive = charging/mains present, negative = discharging/on
        // battery (see hardware::sensors::UpsCurrentSensor). "–" when the UPS I2C provider
        // isn't running or hasn't produced a fresh reading recently.
        let ups_current_str = match self.sensor_manager.get_sensor_value(&HWInput::HwUPSCurrent) {
            Some(value) => format!("{:+.0}мА", value.as_f32()),
            None => "–".to_string(),
        };

        // Battery state of charge, estimated from bus voltage (see
        // hardware::sensors::UpsChargeSensor). Omitted when no fresh reading is available.
        let ups_soc_str = match self.sensor_manager.get_sensor_value(&HWInput::HwUPSChargeState) {
            Some(value) => format!(" [{:.0}%]", value.as_f32()),
            None => String::new(),
        };

        let status_text = format!(
            "Работа: {} | К/С: {:.1} | ЦП: {:>3.0}% {} | Память: {}/{}МБ | ИБП: {}{}",
            uptime_str, fps, cpu_load, cpu_temp_str, mem_available, mem_total, ups_current_str, ups_soc_str
        );

        // Render status line at bottom of screen
        let status_y = self.context.height as f32 - STATUS_LINE_Y_MARGIN; // 25 pixels from bottom
        let status_x = STATUS_LINE_X_MARGIN; // 20 pixels from left

        let status_font = self.ui_style.get_string(PAGE_STATUS_FONT, DEFAULT_GLOBAL_FONT_PATH);
        let status_font_size = self.ui_style.get_integer(PAGE_STATUS_FONT_SIZE, 14);
        let status_color = self.ui_style.get_color(PAGE_STATUS_COLOR, (0.7, 0.7, 0.7));

        self.context.render_text_with_font(
            &status_text,
            status_x,
            status_y,
            1.0, // scale
            status_color,
            status_font.as_str(),
            status_font_size
        )?;
        
        Ok(())
    }
    
    pub fn stop(&mut self) {
        self.running = false;
    }
    
    /// Toggle bloom effect on/off
    pub fn toggle_bloom(&mut self) {
        let enabled = !self.context.is_bloom_enabled();
        self.context.set_bloom_enabled(enabled);
        log::info!("Bloom effect {}", if enabled { "enabled" } else { "disabled" });
    }
    
    /// Set bloom intensity (0.0 to 2.0)
    pub fn set_bloom_intensity(&mut self, intensity: f32) {
        self.context.set_bloom_intensity(intensity);
        log::info!("Bloom intensity set to {:.1}", intensity);
    }
    
    /// Set bloom threshold (0.0 to 1.0)
    pub fn set_bloom_threshold(&mut self, threshold: f32) {
        self.context.set_bloom_threshold(threshold);
        log::info!("Bloom threshold set to {:.1}", threshold);
    }

    // =============================================================================
    // Brightness Control for UI
    // =============================================================================

    /// Set display brightness (0.0 to 1.0)
    pub fn set_brightness(&mut self, brightness: f32) {
        self.context.set_brightness(brightness);
        log::info!("Display brightness set to: {:.1}%", brightness * 100.0);
    }

    /// Get current brightness level
    pub fn get_brightness(&self) -> f32 {
        self.context.get_brightness()
    }

    /// Increase brightness by 10%
    pub fn brightness_up(&mut self) {
        self.context.increase_brightness(0.1);
        let current = self.get_brightness();
        log::info!("Brightness increased to: {:.1}%", current * 100.0);
    }

    /// Decrease brightness by 10%
    pub fn brightness_down(&mut self) {
        self.context.decrease_brightness(0.1);
        let current = self.get_brightness();
        log::info!("Brightness decreased to: {:.1}%", current * 100.0);
    }

    // =============================================================================
    // Memory Information
    // =============================================================================

    /// Get system memory information (total and available memory in MB)
    fn get_memory_info(&self) -> Result<(u32, u32), String> {
        // Read memory information from /proc/meminfo
        let meminfo = fs::read_to_string("/proc/meminfo")
            .map_err(|e| format!("Failed to read /proc/meminfo: {}", e))?;
        
        let mut mem_total = 0;
        let mut mem_available = 0;
        
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                mem_total = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0) / 1024; // Convert KB to MB
            } else if line.starts_with("MemAvailable:") {
                mem_available = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0) / 1024; // Convert KB to MB
            }
        }
        
        Ok((mem_total, mem_available))
    }

    /// Get CPU load as a percentage, averaged across all cores.
    /// Non-blocking: returns the last 3-second average updated by `sample_cpu_stat`.
    fn get_cpu_load(&self) -> f32 {
        self.cpu_load
    }

    /// Read a fresh /proc/stat snapshot and accumulate a per-frame CPU load sample.
    /// Every CPU_LOAD_UPDATE_INTERVAL the accumulated samples are averaged and
    /// committed to `cpu_load`. Must be called once per frame.
    fn sample_cpu_stat(&mut self) {
        const CPU_LOAD_UPDATE_INTERVAL: Duration = Duration::from_secs(3);

        let stat = match fs::read_to_string("/proc/stat") {
            Ok(s) => s,
            Err(_) => return,
        };
        let first_line = match stat.lines().next() {
            Some(l) => l,
            None => return,
        };
        // Format: cpu  user nice system idle iowait irq softirq steal guest guest_nice
        let fields: Vec<u64> = first_line.split_whitespace()
            .skip(1)  // skip the "cpu" label
            .filter_map(|s| s.parse().ok())
            .collect();
        if fields.len() < 4 {
            return;
        }
        let idle = fields[3] + fields.get(4).copied().unwrap_or(0); // idle + iowait
        let total: u64 = fields.iter().sum();

        if let Some((prev_idle, prev_total)) = self.last_cpu_stat {
            let total_delta = total.saturating_sub(prev_total);
            let idle_delta  = idle.saturating_sub(prev_idle);
            if total_delta > 0 {
                let sample = (1.0 - idle_delta as f32 / total_delta as f32) * 100.0;
                self.cpu_load_samples.push(sample);
            }
        }
        self.last_cpu_stat = Some((idle, total));

        // Flush the average every CPU_LOAD_UPDATE_INTERVAL
        if self.cpu_load_last_update.elapsed() >= CPU_LOAD_UPDATE_INTERVAL {
            if !self.cpu_load_samples.is_empty() {
                let avg = self.cpu_load_samples.iter().sum::<f32>()
                    / self.cpu_load_samples.len() as f32;
                self.cpu_load = avg;
                self.cpu_load_samples.clear();
            }
            self.cpu_load_last_update = Instant::now();
        }
    }

    /// Get CPU temperature in degrees Celsius from the thermal zone sysfs interface.
    /// Refreshes the cached value at most once every 3 seconds.
    /// Returns None if the file is unavailable (e.g. not running on Raspberry Pi).
    fn get_cpu_temperature(&mut self) -> Option<f32> {
        const CPU_TEMP_UPDATE_INTERVAL: Duration = Duration::from_secs(3);

        if self.cpu_temp.is_none() || self.cpu_temp_last_update.elapsed() >= CPU_TEMP_UPDATE_INTERVAL {
            let raw = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").ok()?;
            let millidegrees: i32 = raw.trim().parse().ok()?;
            self.cpu_temp = Some(millidegrees as f32 / 1000.0);
            self.cpu_temp_last_update = Instant::now();
        }

        self.cpu_temp
    }

}

impl Drop for PageManager {
    // Safety net for settings changed on the active page without a further switch (the
    // normal switch_page path already persists on every navigation) -- e.g. a clean
    // shutdown reached while still sitting on the GNSS page after adjusting it.
    fn drop(&mut self) {
        if let Some(id) = self.current_page {
            let config = self.get_page(id).map(|p| p.get_config()).unwrap_or(serde_json::Value::Null);
            self.config.set_section(&page_config_key(id), config);
            self.config.set_section(LAST_PAGE_CONFIG_KEY, serde_json::json!(id));
        }
    }
}

#[derive(Debug)]
pub struct FpsCounter {
    frame_count: u32,
    last_time: Instant,
    current_fps: f32,
    frame_times: Vec<Duration>,
    max_samples: usize,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            last_time: Instant::now(),
            current_fps: 0.0,
            frame_times: Vec::new(),
            max_samples: 60, // Track last 60 frames for smoothing
        }
    }
    
    pub fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_time);
        
        self.frame_times.push(delta);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
        
        self.frame_count += 1;
        self.last_time = now;
        
        // Calculate FPS from average frame time
        if !self.frame_times.is_empty() {
            let avg_frame_time: Duration = self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;
            self.current_fps = 1.0 / avg_frame_time.as_secs_f32();
        }
    }
    
    pub fn get_fps(&self) -> f32 {
        self.current_fps
    }
    
    pub fn get_frame_count(&self) -> u32 {
        self.frame_count
    }
}
