//! A single alert overlay raised by a `Watchdog` and shown on screen by `AlertManager`.
//!
//! ## Alert lifecycle
//! An `Alert` is created by `AlertManager::check_watchdogs` the moment a `Watchdog`'s
//! condition fires; it is never constructed directly elsewhere. From there:
//! 1. **Active** — rendered every frame while `is_active()` is true. With
//!    `display_timeout: None` it stays active indefinitely; otherwise it becomes
//!    inactive once `display_timeout` has elapsed since creation (or since the last
//!    `suppress()` call).
//! 2. **Suppressed** — `suppress()` forces the alert inactive immediately (used by the
//!    master-warning clear action) and restarts the remove-timeout countdown from that
//!    moment.
//! 3. **Inactive but queued** — once inactive (by timeout or suppression), the alert
//!    stays in `AlertManager`'s queue for `remove_timeout`, during which its watchdog is
//!    blocked from raising a duplicate (see `AlertManager::check_watchdogs`'s
//!    `already_active` check) — this is what prevents alert flooding from a flapping
//!    sensor.
//! 4. **Expired** — once `remove_timeout` has elapsed (or immediately if `None`),
//!    `is_expired()` returns true and `AlertManager::render_alerts` drops the alert from
//!    the queue, freeing its watchdog to raise a fresh one.
#![allow(dead_code)]
use crate::alerts::alert_manager::{Severity, AlertStyle};
use crate::graphics::context::GraphicsContext;
use crate::indicators::indicator::IndicatorBounds;

pub struct Alert {
    message: String,
    severity: Severity,
    display_timeout: Option<std::time::Duration>,
    remove_timeout: Option<std::time::Duration>,
    creation_time: std::time::Instant,
}

impl Alert {
    /// Creates a new alert with the given message, severity, and timeouts.
    /// `display_timeout` and `remove_timeout` follow the semantics described in the
    /// module docs above and in `Watchdog`'s field docs. Only
    /// `AlertManager::check_watchdogs` should construct alerts.
    pub fn new(message: String, severity: Severity, display_timeout: Option<std::time::Duration>, remove_timeout: Option<std::time::Duration>) -> Self {
        Self {
            message,
            severity,
            display_timeout,
            remove_timeout,
            creation_time: std::time::Instant::now(),
        }
    }

    /// Draws the alert's message text within `bounds`, colored per its severity.
    /// Called once per active alert, per frame, by `AlertManager::render_alerts`.
    pub fn render(&self, bounds: IndicatorBounds, context: &mut GraphicsContext,
                  alert_style: &AlertStyle) -> Result<(), String> {

        let text_color = match self.severity {
            Severity::Warning => alert_style.warning_color,
            Severity::Critical => alert_style.critical_color,
        };

        context.render_text_with_font(
            &self.message,
            bounds.x,
            bounds.y,
            1.0,
            text_color,
            &alert_style.font_path,
            alert_style.font_size as u32,
        )?;
        Ok(())
    }

    /// Returns the alert's display message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the alert's severity.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Forces the alert into the inactive state immediately, regardless of
    /// `display_timeout`, and restarts the remove-timeout countdown from now. Used to
    /// dismiss an alert manually (e.g. a master-warning clear button) without waiting
    /// for its natural display timeout to elapse.
    pub fn suppress(&mut self) {
        self.display_timeout = Some(std::time::Duration::ZERO);
        self.creation_time = std::time::Instant::now();                     // Reset creation time for remove_timeout (countdown starts when alert is suppressed)
    }

    /// Returns whether the alert should currently be rendered on screen.
    /// `display_timeout: None` means always active; otherwise active until
    /// `display_timeout` has elapsed since creation (or since the last `suppress()`).
    pub fn is_active(&self) -> bool {
        match self.display_timeout {
            None => true,                                                   // Always active if no timeout set
            Some(timeout) => self.creation_time.elapsed() < timeout,
        }
    }

    /// Returns whether the alert should be dropped from `AlertManager`'s queue.
    /// An inactive alert is kept around until `remove_timeout` elapses so its watchdog
    /// cannot immediately raise a duplicate (see the module-level lifecycle docs).
    /// `remove_timeout: None` expires the alert immediately once inactive.
    pub fn is_expired(&self) -> bool {
        match self.remove_timeout {
            None => true,                                                   // Same as 0 timeout - expires immediately
            Some(_) if self.is_active() => false,                           // Never remove while active
            Some(timeout) => self.creation_time.elapsed() >= timeout,       // If not active, remove after timeout
        }
    }
}