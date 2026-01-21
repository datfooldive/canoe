//! XKB keyboard bindings

use super::{Action, Binding, BindingEvent};
use crate::config::Mode;

/// An XKB keyboard binding
#[derive(Debug, Clone)]
pub struct XkbBinding {
    /// Mode in which this binding is active
    pub mode: Mode,
    /// XKB keysym to bind
    pub keysym: u32,
    /// Required modifiers
    pub modifiers: u32,
    /// When to trigger (pressed/released)
    pub event: BindingEvent,
    /// Action to perform
    pub action: Action,
    /// Whether this binding is currently enabled
    pub enabled: bool,
}

impl XkbBinding {
    pub fn new(mode: Mode, keysym: u32, modifiers: u32, action: Action) -> Self {
        Self {
            mode,
            keysym,
            modifiers,
            event: BindingEvent::Pressed,
            action,
            enabled: false,
        }
    }

    pub fn with_event(mut self, event: BindingEvent) -> Self {
        self.event = event;
        self
    }
}

impl Binding for XkbBinding {
    fn mode(&self) -> Mode {
        self.mode
    }

    fn action(&self) -> &Action {
        &self.action
    }

    fn event(&self) -> BindingEvent {
        self.event
    }
}
