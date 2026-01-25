//! Output (display) management

#![allow(dead_code)]

use crate::protocol::{RiverLayerShellOutputV1, RiverOutputV1};
use std::cell::RefCell;
use std::rc::Weak;
use wayland_client::protocol::wl_output::WlOutput;

use super::DesktopSurface;

/// Output identifier
pub type OutputId = usize;

/// A managed output (display)
pub struct Output {
    /// Unique output ID
    pub id: OutputId,
    /// River output protocol object
    pub rwm_output: Option<RiverOutputV1>,
    /// Layer shell output for bar
    pub layer_shell_output: Option<RiverLayerShellOutputV1>,
    /// Wayland output object
    pub wl_output: Option<WlOutput>,
    /// Global name of the wl_output
    pub wl_output_name: u32,

    /// Output name (from wl_output)
    pub name: Option<String>,

    /// Position X in global coordinates
    pub x: i32,
    /// Position Y in global coordinates
    pub y: i32,
    /// Width
    pub width: i32,
    /// Height
    pub height: i32,
    /// Output scale factor (integer)
    pub scale: i32,

    /// Current visible tags
    pub tag: u32,
    /// Main active tag
    pub main_tag: u32,
    /// Previous tag state (for switching back)
    pub prev_tag: u32,
    /// Previous main tag
    pub prev_main_tag: u32,

    /// Currently fullscreen window on this output
    pub fullscreen_window: Option<Weak<RefCell<super::Window>>>,

    /// Exclusive area (for layer shell surfaces like bars)
    pub exclusive_x: i32,
    pub exclusive_y: i32,
    pub exclusive_width: i32,
    pub exclusive_height: i32,

    /// Desktop background surface for pointer input
    pub desktop_surface: Option<DesktopSurface>,

    /// Whether this output has been removed
    pub removed: bool,
}

impl Output {
    /// Create a new output
    pub fn new(id: OutputId) -> Self {
        Self {
            id,
            rwm_output: None,
            layer_shell_output: None,
            wl_output: None,
            wl_output_name: 0,
            name: None,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            scale: 1,
            tag: 1, // Default to tag 1
            main_tag: 1,
            prev_tag: 1,
            prev_main_tag: 1,
            fullscreen_window: None,
            exclusive_x: 0,
            exclusive_y: 0,
            exclusive_width: 0,
            exclusive_height: 0,
            desktop_surface: None,
            removed: false,
        }
    }

    /// Set current tag
    pub fn set_tag(&mut self, tag: u32) {
        if tag == 0 {
            return;
        }

        // Save previous state
        self.prev_tag = self.tag;
        self.prev_main_tag = self.main_tag;

        self.tag = tag;
        // Main tag is the lowest set bit
        self.main_tag = tag & tag.wrapping_neg();
    }

    /// Toggle tag visibility
    pub fn toggle_tag(&mut self, mask: u32) {
        let new_tag = self.tag ^ mask;
        if new_tag != 0 {
            self.prev_tag = self.tag;
            self.prev_main_tag = self.main_tag;
            self.tag = new_tag;
            // Update main tag if it was toggled off
            if (self.main_tag & new_tag) == 0 {
                self.main_tag = new_tag & new_tag.wrapping_neg();
            }
        }
    }

    /// Switch to previous tag
    pub fn switch_to_previous_tag(&mut self) {
        std::mem::swap(&mut self.tag, &mut self.prev_tag);
        std::mem::swap(&mut self.main_tag, &mut self.prev_main_tag);
    }

    /// Get usable area (after subtracting exclusive zones)
    pub fn usable_area(&self) -> (i32, i32, i32, i32) {
        if self.exclusive_width > 0 && self.exclusive_height > 0 {
            (
                self.exclusive_x,
                self.exclusive_y,
                self.exclusive_width,
                self.exclusive_height,
            )
        } else {
            (self.x, self.y, self.width, self.height)
        }
    }

    /// Update position from protocol event
    pub fn update_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        // Reset exclusive area to match new position
        if self.exclusive_width == 0 {
            self.exclusive_x = x;
            self.exclusive_y = y;
        }
    }

    /// Update dimensions from protocol event
    pub fn update_dimensions(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        // Reset exclusive area to match new dimensions
        if self.exclusive_width == 0 {
            self.exclusive_width = width;
            self.exclusive_height = height;
        }
    }

    /// Update exclusive area from layer shell
    pub fn update_exclusive_area(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.exclusive_x = x;
        self.exclusive_y = y;
        self.exclusive_width = width;
        self.exclusive_height = height;
    }

    /// Mark as the default output for layer shell
    pub fn set_as_default(&self) {
        if let Some(ref layer_shell_output) = self.layer_shell_output {
            layer_shell_output.set_default();
        }
    }

    /// Check if a point is within this output
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("tag", &self.tag)
            .finish()
    }
}
