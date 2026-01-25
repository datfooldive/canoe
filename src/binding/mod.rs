//! Key and pointer binding handling

#![allow(dead_code)]

pub mod action;
mod pointer;
mod xkb;

pub use action::{Action, Direction, State};
pub use pointer::PointerBinding;
pub use xkb::XkbBinding;

use crate::config::Mode;

/// Binding event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingEvent {
    Pressed,
    Released,
}

/// Common binding trait
pub trait Binding {
    fn mode(&self) -> Mode;
    fn action(&self) -> &Action;
    fn event(&self) -> BindingEvent;
}
