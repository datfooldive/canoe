//! Window management

use super::titlebar::TitlebarButton;
use crate::config::WindowDecoration;
use crate::protocol::river_window_management_v1::client::river_window_v1::Edges;
use crate::protocol::{RiverNodeV1, RiverOutputV1, RiverWindowV1};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Weak;

/// Window identifier
pub type WindowId = usize;

/// Fullscreen state
#[derive(Debug, Clone)]
pub enum FullscreenState {
    None,
    /// Fullscreen within window bounds
    Window,
    /// Fullscreen on a specific output
    Output(Weak<RefCell<super::Output>>),
}

/// Window clip state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipState {
    #[default]
    Unknown,
    Normal,
    Clipped,
}

/// Saved geometry for restoring after maximize
#[derive(Debug, Clone, Copy)]
pub struct SavedGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub floating: bool,
}

/// Operator state for move/resize operations
#[derive(Debug, Clone)]
pub enum Operator {
    None,
    Move {
        start_x: i32,
        start_y: i32,
        seat: Option<Weak<RefCell<super::Seat>>>,
    },
    Resize {
        start_x: i32,
        start_y: i32,
        start_width: i32,
        start_height: i32,
        edges: u32,
        seat: Option<Weak<RefCell<super::Seat>>>,
    },
}

impl Default for Operator {
    fn default() -> Self {
        Operator::None
    }
}

/// Window event types
#[derive(Debug, Clone)]
pub enum WindowEvent {
    Init,
    Close,
    Fullscreen(Option<Weak<RefCell<super::Output>>>),
    Unfullscreen,
    Maximize,
    Unmaximize,
    Minimize,
    Move(Weak<RefCell<super::Seat>>),
    Resize(Weak<RefCell<super::Seat>>, u32), // seat, edges
}

/// A managed window
pub struct Window {
    /// Unique window ID
    pub id: WindowId,
    /// River window protocol object
    pub rwm_window: Option<RiverWindowV1>,
    /// River node protocol object for rendering
    pub rwm_node: Option<RiverNodeV1>,

    /// Associated output
    pub output: Option<Weak<RefCell<super::Output>>>,
    /// Window tags (bitmask)
    pub tag: u32,
    /// Process ID
    pub pid: i32,
    /// Application ID
    pub app_id: Option<String>,
    /// Window title
    pub title: Option<String>,

    /// Position X
    pub x: i32,
    /// Position Y
    pub y: i32,
    /// Width
    pub width: i32,
    /// Height
    pub height: i32,
    /// Minimum width
    pub min_width: i32,
    /// Minimum height
    pub min_height: i32,

    /// Fullscreen state
    pub fullscreen: FullscreenState,
    /// Maximized state
    pub maximized: bool,
    /// Geometry to restore when unmaximizing
    pub pre_maximize: Option<SavedGeometry>,
    /// Floating state
    pub floating: bool,
    /// Hidden state
    pub hidden: bool,
    /// Clip state
    pub clip_state: ClipState,

    /// Decoration mode
    pub decoration: Option<WindowDecoration>,
    /// Decoration hint from client
    pub decoration_hint: u32,

    /// Is this a terminal window (for swallowing)
    pub is_terminal: bool,
    /// Parent window (for swallowing)
    pub parent: Option<Weak<RefCell<Window>>>,
    /// Window being swallowed
    pub swallowing: Option<Weak<RefCell<Window>>>,
    /// Window that swallowed this one
    pub swallowed_by: Option<Weak<RefCell<Window>>>,
    /// Disable swallowing for this window
    pub disable_swallow: bool,

    /// Current operator (move/resize)
    pub operator: Operator,
    /// Pending events queue
    pub unhandled_events: VecDeque<WindowEvent>,
    /// Position is undefined (newly created)
    pub position_undefined: bool,
    /// Titlebar for server-side decoration
    pub titlebar: Option<super::Titlebar>,
    /// Hovered titlebar button (if any)
    pub titlebar_hovered: Option<TitlebarButton>,
    /// Pressed titlebar button (if any)
    pub titlebar_pressed: Option<TitlebarButton>,
    /// Whether left mouse is currently held on the titlebar surface
    pub titlebar_left_down: bool,

    /// Window needs to be configured (propose_dimensions)
    pub needs_configure: bool,

    /// Proposed dimensions (for sizing)
    proposed_width: i32,
    proposed_height: i32,
}

impl Window {
    /// Create a new window
    pub fn new(id: WindowId) -> Self {
        Self {
            id,
            rwm_window: None,
            rwm_node: None,
            output: None,
            tag: 1, // Default to tag 1
            pid: 0,
            app_id: None,
            title: None,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            min_width: 0,
            min_height: 0,
            fullscreen: FullscreenState::None,
            maximized: false,
            pre_maximize: None,
            floating: false,
            hidden: false,
            clip_state: ClipState::Unknown,
            decoration: None,
            decoration_hint: 0,
            is_terminal: false,
            parent: None,
            swallowing: None,
            swallowed_by: None,
            disable_swallow: false,
            operator: Operator::None,
            unhandled_events: VecDeque::new(),
            position_undefined: true,
            titlebar: None,
            titlebar_hovered: None,
            titlebar_pressed: None,
            titlebar_left_down: false,
            needs_configure: true,
            proposed_width: 0,
            proposed_height: 0,
        }
    }

    /// Check if window is visible on the given output
    pub fn is_visible_on(&self, output: &super::Output) -> bool {
        if self.hidden {
            return false;
        }

        // Check if window has the same output
        if let Some(ref win_output) = self.output {
            if let Some(win_output) = win_output.upgrade() {
                if win_output.borrow().id != output.id {
                    return false;
                }
            }
        }

        // Check tag visibility
        (self.tag & output.tag) != 0
    }

    /// Check if window should be treated as tiled
    pub fn is_tiled(&self) -> bool {
        !self.floating && matches!(self.fullscreen, FullscreenState::None)
    }

    /// Set window tag
    pub fn set_tag(&mut self, tag: u32) {
        self.tag = tag;
    }

    /// Toggle window tag
    pub fn toggle_tag(&mut self, mask: u32) {
        let new_tag = self.tag ^ mask;
        if new_tag != 0 {
            self.tag = new_tag;
        }
    }

    /// Propose dimensions for the window
    pub fn propose_dimensions(&mut self, width: i32, height: i32) {
        self.proposed_width = width.max(self.min_width);
        self.proposed_height = height.max(self.min_height);

        // Send to compositor via protocol
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.propose_dimensions(self.proposed_width, self.proposed_height);
        }
    }

    /// Set window position
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        self.position_undefined = false;

        if let Some(ref rwm_node) = self.rwm_node {
            rwm_node.set_position(x, y);
        }
    }

    /// Update window dimensions (called when compositor confirms dimensions)
    pub fn update_dimensions(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
    }

    /// Hide the window
    pub fn hide(&mut self) {
        if !self.hidden {
            self.hidden = true;
            if let Some(ref rwm_window) = self.rwm_window {
                rwm_window.hide();
            }
        }
    }

    /// Show the window
    pub fn show(&mut self) {
        if self.hidden {
            self.hidden = false;
            if let Some(ref rwm_window) = self.rwm_window {
                rwm_window.show();
            }
        }
    }

    /// Request window close
    pub fn close(&self) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.close();
        }
    }

    /// Set window borders
    pub fn set_borders(&self, edges: Edges, width: i32, r: u32, g: u32, b: u32, a: u32) {
        if let Some(ref rwm_window) = self.rwm_window {
            log::debug!(
                "set_borders: edges={:?} width={} rgba=({},{},{},{})",
                edges,
                width,
                r,
                g,
                b,
                a
            );
            rwm_window.set_borders(edges, width, r, g, b, a);
        }
    }

    /// Set window tiled edges
    pub fn set_tiled(&self, edges: Edges) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.set_tiled(edges);
        }
    }

    /// Request fullscreen on output
    pub fn fullscreen_on(&mut self, output: &RiverOutputV1) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.fullscreen(output);
            rwm_window.inform_fullscreen();
        }
    }

    /// Exit fullscreen
    pub fn exit_fullscreen(&mut self) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.exit_fullscreen();
            rwm_window.inform_not_fullscreen();
        }
        self.fullscreen = FullscreenState::None;
    }

    /// Inform window it's maximized
    pub fn inform_maximized(&self) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.inform_maximized();
        }
    }

    /// Inform window it's unmaximized
    pub fn inform_unmaximized(&self) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.inform_unmaximized();
        }
    }

    pub fn clear_maximized_without_restore(&mut self) {
        if self.maximized {
            self.maximized = false;
            self.pre_maximize = None;
            self.inform_unmaximized();
        }
    }

    pub fn unmaximize_restore_size_only(&mut self) {
        if self.maximized {
            self.maximized = false;
            if let Some(saved) = self.pre_maximize.take() {
                self.propose_dimensions(saved.width, saved.height);
            }
            self.inform_unmaximized();
        }
    }

    /// Set decoration mode
    pub fn set_decoration(&self, decoration: WindowDecoration) {
        if let Some(ref rwm_window) = self.rwm_window {
            match decoration {
                WindowDecoration::Csd => rwm_window.use_csd(),
                WindowDecoration::Ssd => rwm_window.use_ssd(),
            }
        }
    }

    /// Set clip box for rendering
    pub fn set_clip_box(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.set_clip_box(x, y, width, height);
        }
        self.clip_state = if width > 0 && height > 0 {
            ClipState::Clipped
        } else {
            ClipState::Normal
        };
    }

    /// Clear clip box
    pub fn clear_clip_box(&mut self) {
        self.set_clip_box(0, 0, 0, 0);
    }

    /// Place this window's node above another
    pub fn place_above(&self, other: &Window) {
        if let (Some(ref node), Some(ref other_node)) = (&self.rwm_node, &other.rwm_node) {
            node.place_above(other_node);
        }
    }

    /// Place this window's node below another
    pub fn place_below(&self, other: &Window) {
        if let (Some(ref node), Some(ref other_node)) = (&self.rwm_node, &other.rwm_node) {
            node.place_below(other_node);
        }
    }

    /// Place this window at top of render list
    pub fn place_top(&self) {
        if let Some(ref node) = self.rwm_node {
            node.place_top();
        }
    }

    /// Place this window at bottom of render list
    pub fn place_bottom(&self) {
        if let Some(ref node) = self.rwm_node {
            node.place_bottom();
        }
    }

    /// Queue an event for processing during manage phase
    pub fn queue_event(&mut self, event: WindowEvent) {
        self.unhandled_events.push_back(event);
    }

    /// Process queued events
    pub fn handle_events(&mut self) -> Vec<WindowEvent> {
        self.unhandled_events.drain(..).collect()
    }

    /// Start a move operation
    pub fn start_move(&mut self, seat: Weak<RefCell<super::Seat>>) {
        self.operator = Operator::Move {
            start_x: self.x,
            start_y: self.y,
            seat: Some(seat),
        };
    }

    /// Start a resize operation
    pub fn start_resize(&mut self, seat: Weak<RefCell<super::Seat>>, edges: u32) {
        self.operator = Operator::Resize {
            start_x: self.x,
            start_y: self.y,
            start_width: self.width,
            start_height: self.height,
            edges,
            seat: Some(seat),
        };

        if let Some(ref rwm_window) = self.rwm_window {
            rwm_window.inform_resize_start();
        }
    }

    /// End current operation
    pub fn end_operation(&mut self) {
        if matches!(self.operator, Operator::Resize { .. }) {
            if let Some(ref rwm_window) = self.rwm_window {
                rwm_window.inform_resize_end();
            }
        }
        self.operator = Operator::None;
    }

    /// Apply operation delta
    pub fn apply_op_delta(&mut self, dx: i32, dy: i32) {
        match &self.operator {
            Operator::Move {
                start_x, start_y, ..
            } => {
                self.set_position(*start_x + dx, *start_y + dy);
            }
            Operator::Resize {
                start_x,
                start_y,
                start_width,
                start_height,
                edges,
                ..
            } => {
                let edges = *edges;
                let mut new_width = *start_width;
                let mut new_height = *start_height;
                let mut new_x = *start_x;
                let mut new_y = *start_y;

                // Apply resize based on edges
                if edges & 4 != 0 {
                    // Left edge
                    new_width = (*start_width - dx).max(self.min_width);
                    new_x = *start_x + (*start_width - new_width);
                }
                if edges & 8 != 0 {
                    // Right edge
                    new_width = (*start_width + dx).max(self.min_width);
                }
                if edges & 1 != 0 {
                    // Top edge
                    new_height = (*start_height - dy).max(self.min_height);
                    new_y = *start_y + (*start_height - new_height);
                }
                if edges & 2 != 0 {
                    // Bottom edge
                    new_height = (*start_height + dy).max(self.min_height);
                }

                if new_x != self.x || new_y != self.y {
                    self.set_position(new_x, new_y);
                }
                self.propose_dimensions(new_width, new_height);
            }
            Operator::None => {}
        }
    }
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("id", &self.id)
            .field("app_id", &self.app_id)
            .field("title", &self.title)
            .field("tag", &self.tag)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("floating", &self.floating)
            .field("hidden", &self.hidden)
            .finish()
    }
}
