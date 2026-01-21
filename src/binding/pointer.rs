//! Pointer (mouse) bindings

use super::{Action, Binding, BindingEvent};
use crate::config::Mode;

/// A pointer (mouse button) binding
#[derive(Debug, Clone)]
pub struct PointerBinding {
    /// Mode in which this binding is active
    pub mode: Mode,
    /// Mouse button code
    pub button: u32,
    /// Required modifiers
    pub modifiers: u32,
    /// When to trigger (pressed/released)
    pub event: BindingEvent,
    /// Action to perform
    pub action: Action,
    /// Whether this binding is currently enabled
    pub enabled: bool,
}

impl PointerBinding {
    pub fn new(mode: Mode, button: u32, modifiers: u32, action: Action) -> Self {
        Self {
            mode,
            button,
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

impl Binding for PointerBinding {
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
