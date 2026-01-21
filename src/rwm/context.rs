//! Core context - central state management

use crate::binding::{Action, Direction, Edge, PointerBinding, XkbBinding};
use crate::config::{Config, MUTABLE_CONFIG};
use crate::layout::{self, LayoutArea, LayoutWindow};
use crate::protocol::river_window_management_v1::client::river_window_v1::Edges;
use crate::protocol::*;
use crate::rule;

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::rc::Rc;

use super::{Output, OutputId, Seat, SeatId, Window, WindowId};

/// The central window manager context
pub struct Context {
    // Wayland globals
    pub rwm: Option<RiverWindowManagerV1>,
    pub rwm_xkb_bindings: Option<RiverXkbBindingsV1>,
    pub rwm_layer_shell: Option<RiverLayerShellV1>,
    pub rwm_input_manager: Option<RiverInputManagerV1>,
    pub rwm_libinput_config: Option<RiverLibinputConfigV1>,

    // Managed objects
    pub windows: HashMap<WindowId, Rc<RefCell<Window>>>,
    pub outputs: HashMap<OutputId, Rc<RefCell<Output>>>,
    pub seats: HashMap<SeatId, Rc<RefCell<Seat>>>,

    // Focus management
    pub focus_stack: Vec<WindowId>,
    pub focused_window: Option<WindowId>,

    // Current state
    pub current_output: Option<OutputId>,
    pub current_seat: Option<SeatId>,

    // ID generators
    next_window_id: WindowId,
    next_output_id: OutputId,
    next_seat_id: SeatId,

    // Configuration
    pub config: Config,

    // Runtime state
    pub running: bool,
    pub session_locked: bool,

    // Terminal windows for swallowing
    pub terminal_windows: HashMap<i32, WindowId>, // pid -> window_id
}

impl Context {
    /// Create a new context with default configuration
    pub fn new() -> Self {
        Self {
            rwm: None,
            rwm_xkb_bindings: None,
            rwm_layer_shell: None,
            rwm_input_manager: None,
            rwm_libinput_config: None,

            windows: HashMap::new(),
            outputs: HashMap::new(),
            seats: HashMap::new(),

            focus_stack: Vec::new(),
            focused_window: None,

            current_output: None,
            current_seat: None,

            next_window_id: 0,
            next_output_id: 0,
            next_seat_id: 0,

            config: Config::default(),

            running: true,
            session_locked: false,

            terminal_windows: HashMap::new(),
        }
    }

    /// Create a new window and add to context
    pub fn create_window(&mut self, rwm_window: RiverWindowV1) -> Rc<RefCell<Window>> {
        let id = self.next_window_id;
        self.next_window_id += 1;

        let mut window = Window::new(id);
        window.rwm_window = Some(rwm_window);

        // Set default decoration
        window.decoration = Some(self.config.default_window_decoration);

        // Assign to current output if available
        if let Some(output_id) = self.current_output {
            if let Some(output) = self.outputs.get(&output_id) {
                window.output = Some(Rc::downgrade(output));
                window.tag = output.borrow().main_tag;
            }
        }

        // Stacking WM: all windows are floating by default
        window.floating = true;

        let window = Rc::new(RefCell::new(window));
        self.windows.insert(id, window.clone());
        self.focus_stack.push(id);

        window
    }

    /// Remove a window from context
    pub fn destroy_window(&mut self, window_id: WindowId) {
        // Remove from focus stack
        self.focus_stack.retain(|&id| id != window_id);

        // Update focused window if necessary
        if self.focused_window == Some(window_id) {
            self.focused_window = self.focus_stack.first().copied();
        }

        // Remove from terminal windows
        self.terminal_windows.retain(|_, &mut wid| wid != window_id);

        self.windows.remove(&window_id);
    }

    /// Create a new output and add to context
    pub fn create_output(&mut self, rwm_output: RiverOutputV1) -> Rc<RefCell<Output>> {
        let id = self.next_output_id;
        self.next_output_id += 1;

        let mut output = Output::new(id, self.config.default_layout);
        output.rwm_output = Some(rwm_output);

        let output = Rc::new(RefCell::new(output));
        self.outputs.insert(id, output.clone());

        // Set as current output if first
        if self.current_output.is_none() {
            self.current_output = Some(id);
        }

        output
    }

    /// Remove an output from context
    pub fn destroy_output(&mut self, output_id: OutputId) {
        // Update current output if necessary
        if self.current_output == Some(output_id) {
            self.current_output = self.outputs.keys().find(|&&id| id != output_id).copied();
        }

        self.outputs.remove(&output_id);
    }

    /// Create a new seat and add to context
    pub fn create_seat(&mut self, rwm_seat: RiverSeatV1) -> Rc<RefCell<Seat>> {
        let id = self.next_seat_id;
        self.next_seat_id += 1;

        let mut seat = Seat::new(id);
        seat.rwm_seat = Some(rwm_seat);

        // Add default bindings
        self.setup_seat_bindings(&mut seat);

        let seat = Rc::new(RefCell::new(seat));
        self.seats.insert(id, seat.clone());

        // Set as current seat if first
        if self.current_seat.is_none() {
            self.current_seat = Some(id);
        }

        seat
    }

    /// Set up bindings for a seat
    fn setup_seat_bindings(&self, seat: &mut Seat) {
        use crate::binding::action::{default_pointer_bindings, default_tag_bindings, default_xkb_bindings};

        // Add XKB bindings
        for (mode, keysym, modifiers, action) in default_xkb_bindings() {
            seat.add_xkb_binding(XkbBinding::new(mode, keysym, modifiers, action));
        }

        // Add tag bindings
        for (mode, keysym, modifiers, action) in default_tag_bindings() {
            seat.add_xkb_binding(XkbBinding::new(mode, keysym, modifiers, action));
        }

        // Add pointer bindings
        for (mode, button, modifiers, action) in default_pointer_bindings() {
            seat.add_pointer_binding(PointerBinding::new(mode, button, modifiers, action));
        }

        seat.initialize_bindings();
    }

    /// Remove a seat from context
    pub fn destroy_seat(&mut self, seat_id: SeatId) {
        // Update current seat if necessary
        if self.current_seat == Some(seat_id) {
            self.current_seat = self.seats.keys().find(|&&id| id != seat_id).copied();
        }

        self.seats.remove(&seat_id);
    }

    /// Apply rules to a newly created window
    pub fn apply_rules_to_window(&self, window: &mut Window) {
        let applied = rule::apply_rules(
            &self.config.rules,
            window.app_id.as_deref(),
            window.title.as_deref(),
        );

        if let Some(tag) = applied.tag {
            window.tag = tag;
        }
        if let Some(floating) = applied.floating {
            window.floating = floating;
        }
        if let Some(decoration) = applied.decoration {
            window.decoration = Some(decoration);
        }
        if let Some(is_terminal) = applied.is_terminal {
            window.is_terminal = is_terminal;
        }
        if let Some(disable_swallow) = applied.disable_swallow {
            window.disable_swallow = disable_swallow;
        }
        if let Some(mfact) = applied.scroller_mfact {
            window.scroller_mfact = Some(mfact);
        }
    }

    /// Focus a window
    pub fn focus(&mut self, window_id: WindowId) {
        // Move to front of focus stack
        self.focus_stack.retain(|&id| id != window_id);
        self.focus_stack.insert(0, window_id);
        self.focused_window = Some(window_id);

        // Actually focus the window via seat
        if let (Some(window), Some(seat_id)) = (self.windows.get(&window_id), self.current_seat) {
            if let Some(seat) = self.seats.get(&seat_id) {
                seat.borrow().focus_window(&window.borrow());
            }
        }
    }

    /// Focus the next/previous window
    pub fn focus_iter(&mut self, direction: Direction, skip_floating: bool) {
        let current_output = match self.current_output.and_then(|id| self.outputs.get(&id)) {
            Some(o) => o.clone(),
            None => return,
        };
        let output = current_output.borrow();

        // Get visible windows on current output
        let visible: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(_, w)| {
                let w = w.borrow();
                w.is_visible_on(&output) && (!skip_floating || !w.floating)
            })
            .map(|(&id, _)| id)
            .collect();

        if visible.is_empty() {
            return;
        }

        // Find current focused index
        let current_idx = self
            .focused_window
            .and_then(|id| visible.iter().position(|&wid| wid == id))
            .unwrap_or(0);

        // Calculate next index
        let next_idx = match direction {
            Direction::Forward => (current_idx + 1) % visible.len(),
            Direction::Reverse => {
                if current_idx == 0 {
                    visible.len() - 1
                } else {
                    current_idx - 1
                }
            }
        };

        self.focus(visible[next_idx]);
    }

    /// Arrange windows - stacking WM: do nothing, windows keep their positions
    pub fn arrange_output(&mut self, _output_id: OutputId) {
        // Stacking WM doesn't auto-arrange windows
        // New window positioning is handled in handle_window_event for Init
    }

    /// Execute an action
    pub fn execute_action(&mut self, action: Action, seat_id: SeatId) {
        match action {
            Action::Quit => {
                self.running = false;
            }
            Action::Close => {
                if let Some(window_id) = self.focused_window {
                    if let Some(window) = self.windows.get(&window_id) {
                        window.borrow().close();
                    }
                }
            }
            Action::Spawn { argv } => {
                self.spawn(&argv);
            }
            Action::SpawnShell { cmd } => {
                self.spawn_shell(&cmd);
            }
            Action::FocusIter {
                direction,
                skip_floating,
            } => {
                self.focus_iter(direction, skip_floating);
            }
            Action::FocusOutputIter { direction } => {
                self.focus_output_iter(direction);
            }
            Action::SendToOutput { direction } => {
                self.send_to_output(direction);
            }
            Action::Swap { direction } => {
                self.swap_windows(direction);
            }
            Action::Move { step } => {
                self.move_focused_window(step.horizontal, step.vertical);
            }
            Action::Resize { step } => {
                self.resize_focused_window(step.horizontal, step.vertical);
            }
            Action::PointerMove => {
                self.start_pointer_move(seat_id);
            }
            Action::PointerResize => {
                self.start_pointer_resize(seat_id);
            }
            Action::Snap { edge } => {
                self.snap_focused_window(edge);
            }
            Action::SwitchMode { mode } => {
                if let Some(seat) = self.seats.get(&seat_id) {
                    seat.borrow_mut().switch_mode(mode);
                }
            }
            Action::SwitchLayout { layout } => {
                if let Some(output_id) = self.current_output {
                    if let Some(output) = self.outputs.get(&output_id) {
                        output.borrow_mut().set_layout(layout);
                        self.arrange_output(output_id);
                    }
                }
            }
            Action::ToggleFullscreen { in_window } => {
                self.toggle_fullscreen(in_window);
            }
            Action::ToggleFloating => {
                self.toggle_floating();
            }
            Action::ToggleSwallow => {
                self.toggle_swallow();
            }
            Action::ToggleBar => {
                // Bar support would go here
            }
            Action::Zoom => {
                self.zoom();
            }
            Action::SetOutputTag { tag } => {
                if let Some(output_id) = self.current_output {
                    if let Some(output) = self.outputs.get(&output_id) {
                        output.borrow_mut().set_tag(tag);
                        self.arrange_output(output_id);
                    }
                }
            }
            Action::SetWindowTag { tag } => {
                if let Some(window_id) = self.focused_window {
                    if let Some(window) = self.windows.get(&window_id) {
                        window.borrow_mut().set_tag(tag);
                        if let Some(output_id) = self.current_output {
                            self.arrange_output(output_id);
                        }
                    }
                }
            }
            Action::ToggleOutputTag { mask } => {
                if let Some(output_id) = self.current_output {
                    if let Some(output) = self.outputs.get(&output_id) {
                        output.borrow_mut().toggle_tag(mask);
                        self.arrange_output(output_id);
                    }
                }
            }
            Action::ToggleWindowTag { mask } => {
                if let Some(window_id) = self.focused_window {
                    if let Some(window) = self.windows.get(&window_id) {
                        window.borrow_mut().toggle_tag(mask);
                        if let Some(output_id) = self.current_output {
                            self.arrange_output(output_id);
                        }
                    }
                }
            }
            Action::SwitchToPreviousTag => {
                if let Some(output_id) = self.current_output {
                    if let Some(output) = self.outputs.get(&output_id) {
                        output.borrow_mut().switch_to_previous_tag();
                        self.arrange_output(output_id);
                    }
                }
            }
            Action::CustomFn { func, ref arg } => {
                let state = self.get_state();
                func(&state, arg);
            }
        }
    }

    /// Get current state for custom actions
    pub fn get_state(&self) -> crate::binding::State {
        let layout = self
            .current_output
            .and_then(|id| self.outputs.get(&id))
            .map(|o| o.borrow().current_layout());

        let output_tag = self
            .current_output
            .and_then(|id| self.outputs.get(&id))
            .map(|o| o.borrow().tag)
            .unwrap_or(1);

        let focused_window_tag = self
            .focused_window
            .and_then(|id| self.windows.get(&id))
            .map(|w| w.borrow().tag);

        crate::binding::State {
            layout,
            output_tag,
            focused_window_tag,
        }
    }

    /// Spawn a command
    pub fn spawn(&self, argv: &[String]) {
        if argv.is_empty() {
            return;
        }

        log::info!("Spawning command: {:?}", argv);

        match unsafe { nix::unistd::fork() } {
            Ok(nix::unistd::ForkResult::Parent { .. }) => {
                // Parent continues
            }
            Ok(nix::unistd::ForkResult::Child) => {
                // Child process - create new session and exec
                let _ = nix::unistd::setsid();

                let mut cmd = Command::new(&argv[0]);
                cmd.args(&argv[1..]);
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::null());
                cmd.stderr(Stdio::null());

                if let Some(ref dir) = self.config.working_directory {
                    cmd.current_dir(dir);
                }

                for (key, value) in &self.config.env {
                    cmd.env(key, value);
                }

                // Make sure child inherits display environment
                // (WAYLAND_DISPLAY should already be in env)

                match cmd.exec() {
                    // exec() only returns on error
                    err => {
                        eprintln!("Failed to exec {:?}: {:?}", argv, err);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to fork for spawn {:?}: {}", argv, e);
            }
        }
    }

    /// Spawn a shell command
    pub fn spawn_shell(&self, cmd: &str) {
        self.spawn(&["sh".to_string(), "-c".to_string(), cmd.to_string()]);
    }

    /// Focus next/previous output
    fn focus_output_iter(&mut self, direction: Direction) {
        let output_ids: Vec<OutputId> = self.outputs.keys().copied().collect();
        if output_ids.len() <= 1 {
            return;
        }

        let current_idx = self
            .current_output
            .and_then(|id| output_ids.iter().position(|&oid| oid == id))
            .unwrap_or(0);

        let next_idx = match direction {
            Direction::Forward => (current_idx + 1) % output_ids.len(),
            Direction::Reverse => {
                if current_idx == 0 {
                    output_ids.len() - 1
                } else {
                    current_idx - 1
                }
            }
        };

        self.current_output = Some(output_ids[next_idx]);

        // Focus top window on new output
        if let Some(output) = self.outputs.get(&output_ids[next_idx]) {
            let output_ref = output.borrow();
            if let Some(window_id) = self.windows.iter().find_map(|(&id, w)| {
                if w.borrow().is_visible_on(&output_ref) {
                    Some(id)
                } else {
                    None
                }
            }) {
                drop(output_ref);
                self.focus(window_id);
            }
        }
    }

    /// Send focused window to next/previous output
    fn send_to_output(&mut self, direction: Direction) {
        let window_id = match self.focused_window {
            Some(id) => id,
            None => return,
        };

        let output_ids: Vec<OutputId> = self.outputs.keys().copied().collect();
        if output_ids.len() <= 1 {
            return;
        }

        let current_idx = self
            .current_output
            .and_then(|id| output_ids.iter().position(|&oid| oid == id))
            .unwrap_or(0);

        let next_idx = match direction {
            Direction::Forward => (current_idx + 1) % output_ids.len(),
            Direction::Reverse => {
                if current_idx == 0 {
                    output_ids.len() - 1
                } else {
                    current_idx - 1
                }
            }
        };

        let target_output_id = output_ids[next_idx];
        if let Some(target_output) = self.outputs.get(&target_output_id) {
            if let Some(window) = self.windows.get(&window_id) {
                let mut w = window.borrow_mut();
                w.output = Some(Rc::downgrade(target_output));
                w.tag = target_output.borrow().main_tag;
            }
        }

        // Rearrange both outputs
        if let Some(current_id) = self.current_output {
            self.arrange_output(current_id);
        }
        self.arrange_output(target_output_id);
    }

    /// Swap focused window with next/previous
    fn swap_windows(&mut self, direction: Direction) {
        let current_output = match self.current_output.and_then(|id| self.outputs.get(&id)) {
            Some(o) => o.clone(),
            None => return,
        };

        let focused_id = match self.focused_window {
            Some(id) => id,
            None => return,
        };

        // Get tiled windows on current output
        let output_ref = current_output.borrow();
        let mut tiled: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(_, w)| {
                let w = w.borrow();
                w.is_visible_on(&output_ref) && w.is_tiled()
            })
            .map(|(&id, _)| id)
            .collect();
        drop(output_ref);

        if tiled.len() <= 1 {
            return;
        }

        // Sort by position in focus stack
        tiled.sort_by_key(|id| {
            self.focus_stack
                .iter()
                .position(|&fid| fid == *id)
                .unwrap_or(usize::MAX)
        });

        let focused_idx = match tiled.iter().position(|&id| id == focused_id) {
            Some(idx) => idx,
            None => return,
        };

        let swap_idx = match direction {
            Direction::Forward => (focused_idx + 1) % tiled.len(),
            Direction::Reverse => {
                if focused_idx == 0 {
                    tiled.len() - 1
                } else {
                    focused_idx - 1
                }
            }
        };

        // Swap in focus stack
        let focused_pos = self.focus_stack.iter().position(|&id| id == focused_id);
        let swap_pos = self.focus_stack.iter().position(|&id| id == tiled[swap_idx]);

        if let (Some(fp), Some(sp)) = (focused_pos, swap_pos) {
            self.focus_stack.swap(fp, sp);
        }

        // Rearrange
        if let Some(output_id) = self.current_output {
            self.arrange_output(output_id);
        }
    }

    /// Move focused floating window
    fn move_focused_window(&mut self, dx: i32, dy: i32) {
        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                let mut w = window.borrow_mut();
                if w.floating {
                    let new_x = w.x + dx;
                    let new_y = w.y + dy;
                    w.set_position(new_x, new_y);
                }
            }
        }
    }

    /// Resize focused window
    fn resize_focused_window(&mut self, dw: i32, dh: i32) {
        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                let mut w = window.borrow_mut();
                if w.floating {
                    let new_width = (w.width + dw).max(w.min_width);
                    let new_height = (w.height + dh).max(w.min_height);
                    w.propose_dimensions(new_width, new_height);
                }
            }
        }
    }

    /// Start pointer move operation
    fn start_pointer_move(&mut self, seat_id: SeatId) {
        // First, focus the window under the pointer
        if let Some(seat) = self.seats.get(&seat_id) {
            let window_below = seat.borrow().window_below_pointer.clone();
            if let Some(weak) = window_below {
                if let Some(window) = weak.upgrade() {
                    let wid = window.borrow().id;
                    self.focus(wid);
                }
            }
        }

        // Now move the focused window
        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                if let Some(seat) = self.seats.get(&seat_id) {
                    let mut w = window.borrow_mut();
                    w.floating = true; // Make floating if not already
                    w.start_move(Rc::downgrade(seat));
                    seat.borrow().start_pointer_op();
                }
            }
        }
    }

    /// Start pointer resize operation
    fn start_pointer_resize(&mut self, seat_id: SeatId) {
        // First, focus the window under the pointer
        if let Some(seat) = self.seats.get(&seat_id) {
            let window_below = seat.borrow().window_below_pointer.clone();
            if let Some(weak) = window_below {
                if let Some(window) = weak.upgrade() {
                    let wid = window.borrow().id;
                    self.focus(wid);
                }
            }
        }

        // Now resize the focused window
        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                if let Some(seat) = self.seats.get(&seat_id) {
                    let mut w = window.borrow_mut();
                    w.floating = true; // Make floating if not already

                    // Determine edges based on pointer position relative to window
                    let seat_ref = seat.borrow();
                    let px = seat_ref.pointer_x;
                    let py = seat_ref.pointer_y;
                    drop(seat_ref);

                    let edges = calculate_resize_edges(&w, px, py);
                    w.start_resize(Rc::downgrade(seat), edges);
                    seat.borrow().start_pointer_op();
                }
            }
        }
    }

    /// Start move/resize based on pointer location within the window frame
    pub fn handle_window_interaction(&mut self, seat_id: SeatId, window_id: WindowId) {
        let seat = match self.seats.get(&seat_id) {
            Some(seat) => seat.clone(),
            None => return,
        };
        let window = match self.windows.get(&window_id) {
            Some(window) => window.clone(),
            None => return,
        };

        let (px, py) = {
            let seat_ref = seat.borrow();
            (seat_ref.pointer_x, seat_ref.pointer_y)
        };

        let (x, y, width, height, has_titlebar) = {
            let w = window.borrow();
            (w.x, w.y, w.width, w.height, w.titlebar.is_some())
        };

        let edges = calculate_resize_edges_near_border(
            x,
            y,
            width,
            height,
            super::titlebar::BORDER_WIDTH,
            px,
            py,
        );

        if edges != 0 {
            let mut w = window.borrow_mut();
            w.floating = true;
            w.start_resize(Rc::downgrade(&seat), edges);
            seat.borrow().start_pointer_op();
            return;
        }

        if has_titlebar
            && point_in_titlebar(x, y, width, super::titlebar::TITLEBAR_HEIGHT, px, py)
        {
            let mut w = window.borrow_mut();
            w.floating = true;
            w.start_move(Rc::downgrade(&seat));
            seat.borrow().start_pointer_op();
        }
    }

    /// Snap focused window to edge
    fn snap_focused_window(&mut self, edge: Edge) {
        let (output_x, output_y, output_w, output_h) = match self
            .current_output
            .and_then(|id| self.outputs.get(&id))
        {
            Some(o) => o.borrow().usable_area(),
            None => return,
        };

        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                let mut w = window.borrow_mut();
                if w.floating {
                    let (new_x, new_y) = match edge {
                        Edge::Left => (output_x, w.y),
                        Edge::Right => (output_x + output_w - w.width, w.y),
                        Edge::Top => (w.x, output_y),
                        Edge::Bottom => (w.x, output_y + output_h - w.height),
                    };
                    w.set_position(new_x, new_y);
                }
            }
        }
    }

    /// Toggle fullscreen for focused window
    fn toggle_fullscreen(&mut self, _in_window: bool) {
        let window_id = match self.focused_window {
            Some(id) => id,
            None => return,
        };

        if let Some(window) = self.windows.get(&window_id) {
            let mut w = window.borrow_mut();
            match &w.fullscreen {
                super::window::FullscreenState::None => {
                    // Enter fullscreen
                    if let Some(output_id) = self.current_output {
                        if let Some(output) = self.outputs.get(&output_id) {
                            let output_ref = output.borrow();
                            if let Some(ref rwm_output) = output_ref.rwm_output {
                                w.fullscreen_on(rwm_output);
                                w.fullscreen = super::window::FullscreenState::Output(Rc::downgrade(output));
                            }
                        }
                    }
                }
                _ => {
                    // Exit fullscreen
                    w.exit_fullscreen();
                }
            }
        }

        if let Some(output_id) = self.current_output {
            self.arrange_output(output_id);
        }
    }

    /// Toggle floating for focused window
    fn toggle_floating(&mut self) {
        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                let mut w = window.borrow_mut();
                w.floating = !w.floating;
            }
        }

        if let Some(output_id) = self.current_output {
            self.arrange_output(output_id);
        }
    }

    /// Toggle swallow for focused window
    fn toggle_swallow(&mut self) {
        if let Some(window_id) = self.focused_window {
            if let Some(window) = self.windows.get(&window_id) {
                let mut w = window.borrow_mut();
                w.disable_swallow = !w.disable_swallow;
            }
        }
    }

    /// Zoom (swap with master)
    fn zoom(&mut self) {
        let focused_id = match self.focused_window {
            Some(id) => id,
            None => return,
        };

        // Move focused window to front of focus stack
        self.focus_stack.retain(|&id| id != focused_id);
        self.focus_stack.insert(0, focused_id);

        if let Some(output_id) = self.current_output {
            self.arrange_output(output_id);
        }
    }

    /// Handle manage_start event - process pending window events and arrange layout
    pub fn handle_manage_start(&mut self) {
        // Process seat actions
        let seat_ids: Vec<SeatId> = self.seats.keys().copied().collect();
        for seat_id in seat_ids {
            let actions = if let Some(seat) = self.seats.get(&seat_id) {
                seat.borrow_mut().drain_actions()
            } else {
                continue;
            };

            for action in actions {
                self.execute_action(action, seat_id);
            }
        }

        // Process window events
        let window_ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for window_id in window_ids {
            let events = if let Some(window) = self.windows.get(&window_id) {
                window.borrow_mut().handle_events()
            } else {
                continue;
            };

            for event in events {
                self.handle_window_event(window_id, event);
            }
        }

        // Arrange all outputs
        let output_ids: Vec<OutputId> = self.outputs.keys().copied().collect();
        for output_id in output_ids {
            self.arrange_output(output_id);
        }
    }

    /// Handle a window event
    fn handle_window_event(&mut self, window_id: WindowId, event: super::window::WindowEvent) {
        use super::window::WindowEvent;

        match event {
            WindowEvent::Init => {
                // Apply rules
                if let Some(window) = self.windows.get(&window_id) {
                    let mut w = window.borrow_mut();
                    self.apply_rules_to_window(&mut w);

                    // Set decoration to SSD (server-side)
                    if let Some(decoration) = w.decoration {
                        log::info!("Window {} setting decoration: {:?}", window_id, decoration);
                        w.set_decoration(decoration);
                    }

                    // We must call propose_dimensions for windows to be displayed.
                    // The protocol says (0,0) means "let window decide" but that
                    // gives us the window's minimum size (often tiny).
                    // Use a reasonable default that most windows will fit into.
                    // The window will respond with its actual dimensions.
                    let default_width = 800;
                    let default_height = 600;
                    log::info!("Window {} init - proposing default dimensions {}x{}",
                        window_id, default_width, default_height);
                    w.propose_dimensions(default_width, default_height);
                }

                // Focus the new window
                self.focus(window_id);
                log::info!("Window {} initialized and focused", window_id);
            }
            WindowEvent::Fullscreen(output) => {
                if let Some(window) = self.windows.get(&window_id) {
                    let mut w = window.borrow_mut();
                    // Handle fullscreen request
                    if let Some(output) = output.and_then(|o| o.upgrade()) {
                        if let Some(ref rwm_output) = output.borrow().rwm_output {
                            w.fullscreen_on(rwm_output);
                        }
                    }
                }
            }
            WindowEvent::Unfullscreen => {
                if let Some(window) = self.windows.get(&window_id) {
                    window.borrow_mut().exit_fullscreen();
                }
            }
            WindowEvent::Move(seat) => {
                if let Some(seat) = seat.upgrade() {
                    self.start_pointer_move(seat.borrow().id);
                }
            }
            WindowEvent::Resize(seat, edges) => {
                if let Some(window) = self.windows.get(&window_id) {
                    if let Some(seat) = seat.upgrade() {
                        let mut w = window.borrow_mut();
                        w.start_resize(Rc::downgrade(&seat), edges);
                        seat.borrow().start_pointer_op();
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle render_start event - position windows and set borders
    pub fn handle_render_start(&mut self) {
        let border_width = MUTABLE_CONFIG.read().unwrap().border_width;

        log::info!("render_start: {} windows, {} outputs, current_output={:?}, border_width={}",
            self.windows.len(), self.outputs.len(), self.current_output, border_width);

        // Process each window
        for (window_id, window) in &self.windows {
            let mut w = window.borrow_mut();

            // Check if window should be visible
            let visible = self.current_output.and_then(|oid| {
                self.outputs.get(&oid).map(|o| w.is_visible_on(&o.borrow()))
            }).unwrap_or(false);

            log::info!("Window {} visible={} hidden={} tag={}", window_id, visible, w.hidden, w.tag);

            if visible && !w.hidden {
                w.show();

                // Disable compositor borders; custom decoration handles borders.
                let is_focused = self.focused_window == Some(*window_id);
                let edges = Edges::all();
                w.set_borders(edges, 0, 0, 0, 0, 0);

                // Raise focused window
                if is_focused {
                    w.place_top();
                }
            } else {
                w.hide();
            }
        }
    }

    /// Finish manage sequence
    pub fn finish_manage(&self) {
        if let Some(ref rwm) = self.rwm {
            rwm.manage_finish();
        }
    }

    /// Finish render sequence
    pub fn finish_render(&self) {
        if let Some(ref rwm) = self.rwm {
            rwm.render_finish();
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate resize edges based on pointer position
fn calculate_resize_edges(window: &Window, px: i32, py: i32) -> u32 {
    let cx = window.x + window.width / 2;
    let cy = window.y + window.height / 2;

    let mut edges = 0u32;

    if px < cx {
        edges |= 4; // Left
    } else {
        edges |= 8; // Right
    }

    if py < cy {
        edges |= 1; // Top
    } else {
        edges |= 2; // Bottom
    }

    edges
}

fn calculate_resize_edges_near_border(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    border: i32,
    px: i32,
    py: i32,
) -> u32 {
    if width <= 0 || height <= 0 || border <= 0 {
        return 0;
    }

    let left_edge = x;
    let right_edge = x + width;
    let top_edge = y;
    let bottom_edge = y + height;

    let within_vert = py >= top_edge - border && py <= bottom_edge + border;
    let within_horiz = px >= left_edge - border && px <= right_edge + border;

    let dist_left = (px - left_edge).abs();
    let dist_right = (px - right_edge).abs();
    let dist_top = (py - top_edge).abs();
    let dist_bottom = (py - bottom_edge).abs();

    let mut edges = 0u32;

    if within_vert {
        let mut horiz = 0u32;
        if dist_left <= border {
            horiz = 4;
        }
        if dist_right <= border && (horiz == 0 || dist_right < dist_left) {
            horiz = 8;
        }
        edges |= horiz;
    }

    if within_horiz {
        let mut vert = 0u32;
        if dist_top <= border {
            vert = 1;
        }
        if dist_bottom <= border && (vert == 0 || dist_bottom < dist_top) {
            vert = 2;
        }
        edges |= vert;
    }

    edges
}

fn point_in_titlebar(
    x: i32,
    y: i32,
    width: i32,
    titlebar_height: i32,
    px: i32,
    py: i32,
) -> bool {
    if width <= 0 || titlebar_height <= 0 {
        return false;
    }

    px >= x && px <= x + width && py >= y && py <= y + titlebar_height
}
