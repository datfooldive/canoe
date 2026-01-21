//! RWM - River Window Manager core modules

mod context;
mod desktop;
mod output;
mod seat;
mod menu;
pub mod titlebar;
pub mod window;

pub use context::Context;
pub use desktop::DesktopSurface;
pub use menu::{MenuItem, WindowMenu};
pub use output::{Output, OutputId};
pub use seat::{PointerTarget, Seat, SeatId};
pub use titlebar::Titlebar;
pub use window::{Window, WindowId, WindowEvent};

/// User data for layer shell surfaces owned by the WM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerSurfaceKind {
    Desktop(OutputId),
    Menu,
}
