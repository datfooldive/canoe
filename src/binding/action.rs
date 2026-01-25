//! Binding actions - what happens when a binding is triggered

#![allow(dead_code)]

use crate::config::Mode;

/// Direction for iteration/movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// Edge for snapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Step for movement/resize
#[derive(Debug, Clone, Copy, Default)]
pub struct Step {
    pub horizontal: i32,
    pub vertical: i32,
}

/// Window manager state for custom actions
#[derive(Debug, Clone)]
pub struct State {
    pub output_tag: u32,
    pub focused_window_tag: Option<u32>,
}

impl State {
    pub fn refresh_current_bar(&self) {
        // Will be implemented with bar support
    }
}

/// Argument types for custom functions
#[derive(Debug, Clone)]
pub enum Arg {
    None,
    Int(i32),
    Float(f32),
    Uint(u32),
    Char(char),
}

/// Type alias for custom action functions
pub type CustomFn = fn(&State, &Arg);

/// All possible binding actions
#[derive(Debug, Clone, Default)]
pub enum Action {
    /// Quit the window manager
    #[default]
    Quit,
    /// Close the focused window
    Close,

    /// Spawn a command
    Spawn { argv: Vec<String> },
    /// Spawn a shell command
    SpawnShell { cmd: String },

    /// Cycle focus through windows
    FocusIter {
        direction: Direction,
        skip_floating: bool,
    },
    /// Cycle focus through outputs
    FocusOutputIter { direction: Direction },

    /// Send focused window to another output
    SendToOutput { direction: Direction },
    /// Swap focused window with another
    Swap { direction: Direction },

    /// Move floating window
    Move { step: Step },
    /// Resize window
    Resize { step: Step },
    /// Start pointer move operation
    PointerMove,
    /// Start pointer resize operation
    PointerResize,

    /// Snap window to edge
    Snap { edge: Edge },

    /// Switch input mode
    SwitchMode { mode: Mode },

    /// Toggle fullscreen
    ToggleFullscreen { in_window: bool },
    /// Toggle floating
    ToggleFloating,
    /// Toggle swallow for focused window
    ToggleSwallow,
    /// Toggle bar visibility
    ToggleBar,

    /// Zoom (swap with master)
    Zoom,

    /// Hide (minimize) the focused window
    HideFocused,
    /// Maximize the focused window to the output
    MaximizeFocused,

    /// Set output tags
    SetOutputTag { tag: u32 },
    /// Set window tags
    SetWindowTag { tag: u32 },
    /// Toggle output tags
    ToggleOutputTag { mask: u32 },
    /// Toggle window tags
    ToggleWindowTag { mask: u32 },
    /// Switch to previous tag
    SwitchToPreviousTag,

    /// Activate selected window menu item
    ActivateMenuHovered,
    /// Cycle window menu for Alt-Tab
    WindowMenuCycle,
    /// Activate selected window menu item for Alt-Tab
    WindowMenuCommit,

    /// Custom function action
    CustomFn { func: CustomFn, arg: Arg },
}

/// Default keybindings configuration for stacking WM
pub fn default_xkb_bindings() -> Vec<(Mode, u32, u32, Action, super::BindingEvent)> {
    use crate::config::modifiers::*;
    use xkbcommon::xkb::Keysym;

    let alt = ALT;
    let shift = SHIFT;

    vec![
        // Essential window management
        (
            Mode::Default,
            Keysym::q.raw(),
            alt | shift,
            Action::Quit,
            super::BindingEvent::Pressed,
        ),
        (
            Mode::Default,
            Keysym::c.raw(),
            alt | shift,
            Action::Close,
            super::BindingEvent::Pressed,
        ),
        (
            Mode::Default,
            Keysym::Down.raw(),
            alt,
            Action::HideFocused,
            super::BindingEvent::Pressed,
        ),
        (
            Mode::Default,
            Keysym::Up.raw(),
            alt,
            Action::MaximizeFocused,
            super::BindingEvent::Pressed,
        ),
        // Focus navigation (cycle through windows)
        (
            Mode::Default,
            Keysym::Tab.raw(),
            alt,
            Action::WindowMenuCycle,
            super::BindingEvent::Pressed,
        ),
        (
            Mode::Default,
            Keysym::Tab.raw(),
            alt | shift,
            Action::WindowMenuCycle,
            super::BindingEvent::Pressed,
        ),
        (
            Mode::Default,
            Keysym::Alt_L.raw(),
            0,
            Action::WindowMenuCommit,
            super::BindingEvent::Released,
        ),
        (
            Mode::Default,
            Keysym::Alt_R.raw(),
            0,
            Action::WindowMenuCommit,
            super::BindingEvent::Released,
        ),
        // Fullscreen toggle
        (
            Mode::Default,
            Keysym::f.raw(),
            alt,
            Action::ToggleFullscreen { in_window: false },
            super::BindingEvent::Pressed,
        ),
        // Spawn terminal
        (
            Mode::Default,
            Keysym::Return.raw(),
            alt | shift,
            Action::Spawn {
                argv: vec!["foot".to_string()],
            },
            super::BindingEvent::Pressed,
        ),
        // Spawn launcher
        (
            Mode::Default,
            Keysym::space.raw(),
            alt,
            Action::SpawnShell {
                cmd: "fuzzel".to_string(),
            },
            super::BindingEvent::Pressed,
        ),
    ]
}

/// Generate tag bindings - empty for stacking WM
pub fn default_tag_bindings() -> Vec<(Mode, u32, u32, Action)> {
    // Stacking WM doesn't use tags/workspaces
    Vec::new()
}

/// Default pointer bindings
pub fn default_pointer_bindings() -> Vec<(Mode, u32, u32, Action)> {
    use crate::config::button;
    use crate::config::modifiers::*;

    // Alt+Drag to move, Alt+Right-Drag to resize
    vec![
        (Mode::Default, button::LEFT, ALT, Action::PointerMove),
        (Mode::Default, button::RIGHT, ALT, Action::PointerResize),
    ]
}
