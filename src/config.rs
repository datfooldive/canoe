//! Configuration for the River window manager

use crate::layout::LayoutType;
use crate::rule::Rule;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

/// Mouse button codes (Linux input event codes)
pub mod button {
    pub const LEFT: u32 = 0x110;
    pub const RIGHT: u32 = 0x111;
    pub const MIDDLE: u32 = 0x112;
}

/// Modifier key masks
pub mod modifiers {
    pub const NONE: u32 = 0;
    pub const SHIFT: u32 = 1;
    pub const CTRL: u32 = 4;
    pub const ALT: u32 = 8; // mod1
    pub const MOD3: u32 = 32;
    pub const SUPER: u32 = 64; // mod4
    pub const MOD5: u32 = 128;
}

/// Input mode for bindings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    Lock,
    #[default]
    Default,
    Floating,
    Passthrough,
}

impl Mode {
    pub fn tag(&self) -> &'static str {
        match self {
            Mode::Lock => "",
            Mode::Default => "",
            Mode::Floating => "F",
            Mode::Passthrough => "P",
        }
    }
}

/// Window decoration style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowDecoration {
    Csd,
    #[default]
    Ssd,
}

/// Border colors
#[derive(Debug, Clone, Copy)]
pub struct BorderColor {
    pub focus: u32,
    pub unfocus: u32,
    pub urgent: u32,
}

impl Default for BorderColor {
    fn default() -> Self {
        Self {
            focus: 0xffff00ff,    // Bright yellow, fully opaque
            unfocus: 0x888800ff,  // Dark yellow/olive for unfocused
            urgent: 0xff0000ff,
        }
    }
}

/// Bar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarPosition {
    #[default]
    Top,
    Bottom,
}

/// Bar color configuration
#[derive(Debug, Clone, Copy)]
pub struct BarColor {
    pub fg: u32,
    pub bg: u32,
}

/// Status source for bar
#[derive(Debug, Clone)]
pub enum StatusSource {
    Text(String),
    Stdin,
    Fifo(String),
}

impl Default for StatusSource {
    fn default() -> Self {
        Self::Text("rwm".to_string())
    }
}

/// Bar configuration
#[derive(Debug, Clone)]
pub struct BarConfig {
    pub show_default: bool,
    pub position: BarPosition,
    pub font: String,
    pub color_normal: BarColor,
    pub color_select: BarColor,
    pub status: StatusSource,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            show_default: true,
            position: BarPosition::Top,
            font: "monospace:size=10".to_string(),
            color_normal: BarColor {
                fg: 0x828bb8ff,
                bg: 0x1b1d2bd0,
            },
            color_select: BarColor {
                fg: 0x444a73ff,
                bg: 0xc8d3f5d0,
            },
            status: StatusSource::default(),
        }
    }
}

/// XCursor theme configuration
#[derive(Debug, Clone)]
pub struct XCursorTheme {
    pub name: String,
    pub size: u32,
}

/// Tile layout configuration
#[derive(Debug, Clone, Copy)]
pub struct TileConfig {
    pub nmaster: u32,
    pub mfact: f32,
    pub inner_gap: i32,
    pub outer_gap: i32,
    pub master_location: MasterLocation,
}

impl Default for TileConfig {
    fn default() -> Self {
        Self {
            nmaster: 1,
            mfact: 0.55,
            inner_gap: 12,
            outer_gap: 9,
            master_location: MasterLocation::Left,
        }
    }
}

/// Master area location for tile layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MasterLocation {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

/// Grid layout configuration
#[derive(Debug, Clone, Copy)]
pub struct GridConfig {
    pub outer_gap: i32,
    pub inner_gap: i32,
    pub direction: GridDirection,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            outer_gap: 9,
            inner_gap: 12,
            direction: GridDirection::Horizontal,
        }
    }
}

/// Grid layout direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridDirection {
    #[default]
    Horizontal,
    Vertical,
}

/// Monocle layout configuration
#[derive(Debug, Clone, Copy)]
pub struct MonocleConfig {
    pub gap: i32,
}

impl Default for MonocleConfig {
    fn default() -> Self {
        Self { gap: 9 }
    }
}

/// Scroller layout configuration
#[derive(Debug, Clone, Copy)]
pub struct ScrollerConfig {
    pub mfact: f32,
    pub inner_gap: i32,
    pub outer_gap: i32,
    pub snap_to_left: bool,
}

impl Default for ScrollerConfig {
    fn default() -> Self {
        Self {
            mfact: 0.5,
            inner_gap: 16,
            outer_gap: 9,
            snap_to_left: false,
        }
    }
}

/// Runtime-mutable configuration values
pub struct MutableConfig {
    pub tile: TileConfig,
    pub grid: GridConfig,
    pub monocle: MonocleConfig,
    pub scroller: ScrollerConfig,
    pub border_width: i32,
    pub auto_swallow: bool,
}

impl Default for MutableConfig {
    fn default() -> Self {
        Self {
            tile: TileConfig::default(),
            grid: GridConfig::default(),
            monocle: MonocleConfig::default(),
            scroller: ScrollerConfig::default(),
            border_width: 10,  // Fat yellow border
            auto_swallow: true,
        }
    }
}

/// Global mutable configuration
pub static MUTABLE_CONFIG: LazyLock<RwLock<MutableConfig>> =
    LazyLock::new(|| RwLock::new(MutableConfig::default()));

/// Main configuration structure
#[derive(Debug, Clone)]
pub struct Config {
    pub env: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub startup_cmds: Vec<Vec<String>>,
    pub xcursor_theme: Option<XCursorTheme>,

    pub repeat_rate: i32,
    pub repeat_delay: i32,
    pub scroll_factor: f64,
    pub sloppy_focus: bool,

    pub bar: BarConfig,
    pub default_window_decoration: WindowDecoration,
    pub border_color: BorderColor,

    pub default_layout: LayoutType,
    pub tags: Vec<String>,
    pub rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            env: HashMap::new(),
            working_directory: dirs::home_dir().map(|p| p.to_string_lossy().to_string()),
            startup_cmds: Vec::new(),
            xcursor_theme: None,

            repeat_rate: 50,
            repeat_delay: 300,
            scroll_factor: 1.0,
            sloppy_focus: false,

            bar: BarConfig::default(),
            default_window_decoration: WindowDecoration::Ssd,
            border_color: BorderColor::default(),

            default_layout: LayoutType::Tile,
            tags: (1..=9).map(|i| i.to_string()).collect(),
            rules: default_rules(),
        }
    }
}

/// Get default window rules
fn default_rules() -> Vec<Rule> {
    vec![
        Rule {
            app_id: None,
            title: None,
            floating: Some(true),
            ..Default::default()
        },
        Rule {
            app_id: Some("zenity".to_string()),
            floating: Some(true),
            ..Default::default()
        },
        Rule {
            app_id: Some("DesktopEditors".to_string()),
            floating: Some(true),
            ..Default::default()
        },
        Rule {
            app_id: Some("xdg-desktop-portal-gtk".to_string()),
            floating: Some(true),
            ..Default::default()
        },
        Rule {
            app_id: Some("chromium".to_string()),
            tag: Some(1 << 1),
            scroller_mfact: Some(0.9),
            ..Default::default()
        },
        Rule {
            app_id: Some("foot".to_string()),
            is_terminal: Some(true),
            scroller_mfact: Some(0.8),
            ..Default::default()
        },
    ]
}

/// Get layout tag string for display
pub fn layout_tag(layout: LayoutType) -> &'static str {
    let config = MUTABLE_CONFIG.read().unwrap();
    match layout {
        LayoutType::Tile => match config.tile.master_location {
            MasterLocation::Left => "[]=",
            MasterLocation::Right => "=[]",
            MasterLocation::Top => "[^]",
            MasterLocation::Bottom => "[_]",
        },
        LayoutType::Grid => match config.grid.direction {
            GridDirection::Horizontal => "|+|",
            GridDirection::Vertical => "|||",
        },
        LayoutType::Monocle => "[=]",
        LayoutType::Scroller => {
            if config.scroller.snap_to_left {
                "[<-]"
            } else {
                "[==]"
            }
        }
        LayoutType::Float => "><>",
    }
}

/// Helper module for home directory
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
