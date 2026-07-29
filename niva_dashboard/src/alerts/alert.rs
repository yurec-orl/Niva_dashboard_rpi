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
    /// An inactive alert is kept around until `remove_timeout` elapses *since it went
    /// inactive* so its watchdog cannot immediately raise a duplicate (see the
    /// module-level lifecycle docs). `remove_timeout: None` expires the alert
    /// immediately once inactive.
    pub fn is_expired(&self) -> bool {
        match self.remove_timeout {
            None => true,                                                   // Same as 0 timeout - expires immediately
            Some(_) if self.is_active() => false,                           // Never remove while active
            Some(timeout) => {
                // Alert stays active for creation_time + display_timeout duration, then
                // becomes expired after creation_time + display_timeout + remove_timeout,
                // so to compare remove_timeout properly, creation_time.elapsed() needs
                // to have display_timeout subtracted.
                let since_inactive = self.creation_time.elapsed()
                    .saturating_sub(self.display_timeout.unwrap_or_default());
                since_inactive >= timeout
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new_alert_is_active_when_no_display_timeout() {
        let alert = Alert::new("test".to_string(), Severity::Warning, None, None);
        assert!(alert.is_active());
    }

    #[test]
    fn test_alert_becomes_inactive_after_display_timeout_elapses() {
        let alert = Alert::new("test".to_string(), Severity::Warning, Some(Duration::from_millis(10)), None);
        assert!(alert.is_active(), "should still be active immediately after creation");

        std::thread::sleep(Duration::from_millis(30));
        assert!(!alert.is_active(), "should be inactive once display_timeout has elapsed");
    }

    #[test]
    fn test_suppress_forces_alert_inactive_immediately() {
        let mut alert = Alert::new("test".to_string(), Severity::Critical, None, Some(Duration::from_millis(50)));
        assert!(alert.is_active());

        alert.suppress();
        assert!(!alert.is_active(), "suppress() should force the alert inactive right away");
    }

    #[test]
    fn test_no_remove_timeout_expires_immediately_once_inactive() {
        let mut alert = Alert::new("test".to_string(), Severity::Warning, None, None);
        alert.suppress();
        assert!(alert.is_expired(), "remove_timeout: None means expire as soon as inactive");
    }

    #[test]
    fn test_active_alert_never_expires_even_with_remove_timeout_set() {
        let alert = Alert::new("test".to_string(), Severity::Warning, None, Some(Duration::from_millis(10)));
        std::thread::sleep(Duration::from_millis(20));
        assert!(!alert.is_expired(), "an alert with display_timeout: None is always active, so never expires");
    }

    #[test]
    fn test_alert_expires_only_after_remove_timeout_since_going_inactive() {
        let mut alert = Alert::new("test".to_string(), Severity::Warning, None, Some(Duration::from_millis(30)));
        alert.suppress(); // goes inactive now, remove_timeout countdown starts

        assert!(!alert.is_expired(), "remove_timeout hasn't elapsed yet");

        std::thread::sleep(Duration::from_millis(50));
        assert!(alert.is_expired(), "remove_timeout has now elapsed since the alert went inactive");
    }

    #[test]
    fn test_message_and_severity_accessors() {
        let alert = Alert::new("engine hot".to_string(), Severity::Critical, None, None);
        assert_eq!(alert.message(), "engine hot");
        assert!(matches!(alert.severity(), Severity::Critical));
    }
}