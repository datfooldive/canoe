//! RWM - River Window Manager core modules

mod context;
mod output;
mod seat;
pub mod titlebar;
pub mod window;

pub use context::Context;
pub use output::{Output, OutputId};
pub use seat::{Seat, SeatId};
pub use titlebar::Titlebar;
pub use window::{Window, WindowId, WindowEvent};
