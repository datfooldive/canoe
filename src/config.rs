//! Configuration for the River window manager

#![allow(dead_code)]

use crate::rule::Rule;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

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
            focus: 0xffff00ff,   // Bright yellow, fully opaque
            unfocus: 0x888800ff, // Dark yellow/olive for unfocused
            urgent: 0xff0000ff,
        }
    }
}

/// Layered border colors (outer, mid, inner)
#[derive(Debug, Clone, Copy)]
pub struct BorderLayers {
    pub outer: u32,
    pub mid: u32,
    pub inner: u32,
}

impl Default for BorderLayers {
    fn default() -> Self {
        Self {
            outer: 0x000000FF,
            mid: 0xC0C0C0FF,
            inner: 0x000000FF,
        }
    }
}

/// XCursor theme configuration
#[derive(Debug, Clone)]
pub struct XCursorTheme {
    pub name: String,
    pub size: u32,
}

/// UI theme configuration
#[derive(Debug, Clone)]
pub struct UiConfig {
    pub border_width: i32,
    pub border_active: BorderLayers,
    pub border_inactive: BorderLayers,
    pub titlebar_text_active: u32,
    pub titlebar_text_inactive: u32,
    pub titlebar_bg_active: u32,
    pub titlebar_bg_inactive: u32,
    pub menu_bg: u32,
    pub menu_text: u32,
    pub menu_highlight_bg: u32,
    pub menu_highlight_text: u32,
    pub button_bg: u32,
    pub button_highlight: u32,
    pub button_shadow: u32,
    pub font_name: Option<String>,
    pub font_size: f32,
    pub desktop_background: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            border_width: 4,
            border_active: BorderLayers::default(),
            border_inactive: BorderLayers::default(),
            titlebar_text_active: 0xFFFFFFFF,
            titlebar_text_inactive: 0xFFFFFFFF,
            titlebar_bg_active: 0x000080FF,
            titlebar_bg_inactive: 0xFFFFFFFF,
            menu_bg: 0xC0C0C0FF,
            menu_text: 0x000000FF,
            menu_highlight_bg: 0x000080FF,
            menu_highlight_text: 0xFFFFFFFF,
            button_bg: 0xC0C0C0FF,
            button_highlight: 0xFFFFFFFF,
            button_shadow: 0x808080FF,
            font_name: None,
            font_size: 12.0,
            desktop_background: 0x008080FF,
        }
    }
}

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

    pub default_window_decoration: WindowDecoration,
    pub border_color: BorderColor,
    pub ui: UiConfig,

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

            default_window_decoration: WindowDecoration::Ssd,
            border_color: BorderColor::default(),
            ui: UiConfig::default(),

            rules: default_rules(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    ui: Option<UiConfigFile>,
}

#[derive(Debug, Deserialize)]
struct UiConfigFile {
    border_width: Option<i32>,
    border_active: Option<BorderLayersFile>,
    border_inactive: Option<BorderLayersFile>,
    titlebar_text_active: Option<u32>,
    titlebar_text_inactive: Option<u32>,
    titlebar_bg_active: Option<u32>,
    titlebar_bg_inactive: Option<u32>,
    menu_bg: Option<u32>,
    menu_text: Option<u32>,
    menu_highlight_bg: Option<u32>,
    menu_highlight_text: Option<u32>,
    button_bg: Option<u32>,
    button_highlight: Option<u32>,
    button_shadow: Option<u32>,
    font_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_f32")]
    font_size: Option<f32>,
    desktop_background: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct BorderLayersFile {
    outer: Option<u32>,
    mid: Option<u32>,
    inner: Option<u32>,
}

impl BorderLayers {
    fn apply(&mut self, overrides: BorderLayersFile) {
        if let Some(outer) = overrides.outer {
            self.outer = outer;
        }
        if let Some(mid) = overrides.mid {
            self.mid = mid;
        }
        if let Some(inner) = overrides.inner {
            self.inner = inner;
        }
    }
}

impl UiConfig {
    fn apply(&mut self, overrides: UiConfigFile) {
        if let Some(border_width) = overrides.border_width {
            self.border_width = border_width;
        }
        if let Some(border_active) = overrides.border_active {
            self.border_active.apply(border_active);
        }
        if let Some(border_inactive) = overrides.border_inactive {
            self.border_inactive.apply(border_inactive);
        }
        if let Some(color) = overrides.titlebar_text_active {
            self.titlebar_text_active = color;
        }
        if let Some(color) = overrides.titlebar_text_inactive {
            self.titlebar_text_inactive = color;
        }
        if let Some(color) = overrides.titlebar_bg_active {
            self.titlebar_bg_active = color;
        }
        if let Some(color) = overrides.titlebar_bg_inactive {
            self.titlebar_bg_inactive = color;
        }
        if let Some(color) = overrides.menu_bg {
            self.menu_bg = color;
        }
        if let Some(color) = overrides.menu_text {
            self.menu_text = color;
        }
        if let Some(color) = overrides.menu_highlight_bg {
            self.menu_highlight_bg = color;
        }
        if let Some(color) = overrides.menu_highlight_text {
            self.menu_highlight_text = color;
        }
        if let Some(color) = overrides.button_bg {
            self.button_bg = color;
        }
        if let Some(color) = overrides.button_highlight {
            self.button_highlight = color;
        }
        if let Some(color) = overrides.button_shadow {
            self.button_shadow = color;
        }
        if let Some(font_name) = overrides.font_name {
            let trimmed = font_name.trim().to_string();
            self.font_name = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }
        if let Some(font_size) = overrides.font_size {
            self.font_size = font_size;
        }
        if let Some(color) = overrides.desktop_background {
            self.desktop_background = color;
        }
    }
}

fn deserialize_opt_f32<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<toml::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(toml::Value::Float(value)) => Ok(Some(value as f32)),
        Some(toml::Value::Integer(value)) => Ok(Some(value as f32)),
        Some(other) => Err(de::Error::custom(format!(
            "expected float or integer, got {}",
            other.type_str()
        ))),
    }
}

fn config_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".config").join("rwm").join("rwm.toml"))
}

/// Load config from ~/.config/rwm/rwm.toml and apply overrides to defaults.
pub fn load_config() -> Config {
    let mut config = Config::default();
    if let Some(path) = config_path() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            match toml::from_str::<FileConfig>(&contents) {
                Ok(file_config) => {
                    if let Some(ui) = file_config.ui {
                        config.ui.apply(ui);
                    }
                }
                Err(err) => {
                    log::warn!("Failed to parse config {}: {}", path.display(), err);
                }
            }
        }
    }

    config
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
            ..Default::default()
        },
    ]
}

/// Helper module for home directory
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
