//! RWM - River Window Manager in Rust
//!
//! A tiling window manager for the River Wayland compositor.

mod binding;
mod config;
mod layout;
mod protocol;
mod rule;
mod rwm;

use protocol::*;
use rwm::Context;

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::{AsFd, AsRawFd};
use std::rc::Rc;

use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::signal::{SigSet, Signal};
use nix::sys::signalfd::{SfdFlags, SignalFd};

use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;

/// Application state for Wayland dispatch
struct AppState {
    context: Rc<RefCell<Context>>,
    globals: Globals,
}

/// Collected Wayland globals
#[derive(Default)]
struct Globals {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    rwm: Option<RiverWindowManagerV1>,
    rwm_xkb_bindings: Option<RiverXkbBindingsV1>,
    rwm_layer_shell: Option<RiverLayerShellV1>,
    rwm_input_manager: Option<RiverInputManagerV1>,
    rwm_libinput_config: Option<RiverLibinputConfigV1>,
    cursor_shape_manager: Option<WpCursorShapeManagerV1>,
    wl_seats: HashMap<u32, wl_seat::WlSeat>,
}

fn attach_wl_seat(
    state: &mut AppState,
    seat_ref: &Rc<RefCell<rwm::Seat>>,
    qh: &QueueHandle<AppState>,
) {
    let wl_seat_name = seat_ref.borrow().wl_seat_name;
    if wl_seat_name == 0 || seat_ref.borrow().wl_seat.is_some() {
        return;
    }
    let wl_seat = match state.globals.wl_seats.get(&wl_seat_name) {
        Some(seat) => seat.clone(),
        None => return,
    };

    {
        let mut seat = seat_ref.borrow_mut();
        seat.wl_seat = Some(wl_seat.clone());
        if seat.wl_pointer.is_none() {
            let wl_pointer = wl_seat.get_pointer(qh, seat.id);
            seat.wl_pointer = Some(wl_pointer);
        }
    }

    attach_cursor_shape_device(state, seat_ref, qh);
}

fn attach_cursor_shape_device(
    state: &mut AppState,
    seat_ref: &Rc<RefCell<rwm::Seat>>,
    qh: &QueueHandle<AppState>,
) {
    let manager = match state.globals.cursor_shape_manager.as_ref() {
        Some(manager) => manager,
        None => return,
    };
    let wl_pointer = match seat_ref.borrow().wl_pointer.as_ref() {
        Some(pointer) => pointer.clone(),
        None => return,
    };
    if seat_ref.borrow().cursor_shape_device.is_some() {
        return;
    }
    let device: WpCursorShapeDeviceV1 = manager.get_pointer(&wl_pointer, qh, ());
    seat_ref.borrow_mut().cursor_shape_device = Some(device);
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    log::info!("Binding wl_compositor v{}", version.min(4));
                    let compositor: wl_compositor::WlCompositor =
                        registry.bind(name, version.min(4), qh, ());
                    state.globals.compositor = Some(compositor);
                }
                "wl_shm" => {
                    log::info!("Binding wl_shm v{}", version.min(1));
                    let shm: wl_shm::WlShm = registry.bind(name, version.min(1), qh, ());
                    state.globals.shm = Some(shm);
                }
                "wl_seat" => {
                    log::info!("Binding wl_seat v{}", version.min(7));
                    let seat: wl_seat::WlSeat = registry.bind(name, version.min(7), qh, name);
                    state.globals.wl_seats.insert(name, seat);

                    let seats: Vec<_> = state
                        .context
                        .borrow()
                        .seats
                        .values()
                        .cloned()
                        .collect();
                    for seat_ref in seats {
                        if seat_ref.borrow().wl_seat_name == name {
                            attach_wl_seat(state, &seat_ref, qh);
                            break;
                        }
                    }
                }
                "river_window_manager_v1" => {
                    let rwm: RiverWindowManagerV1 =
                        registry.bind(name, version.min(2), qh, ());
                    state.globals.rwm = Some(rwm.clone());
                    state.context.borrow_mut().rwm = Some(rwm);
                }
                "river_xkb_bindings_v1" => {
                    let xkb: RiverXkbBindingsV1 = registry.bind(name, version.min(1), qh, ());
                    state.globals.rwm_xkb_bindings = Some(xkb.clone());
                    state.context.borrow_mut().rwm_xkb_bindings = Some(xkb);
                }
                "river_layer_shell_v1" => {
                    let ls: RiverLayerShellV1 = registry.bind(name, version.min(1), qh, ());
                    state.globals.rwm_layer_shell = Some(ls.clone());
                    state.context.borrow_mut().rwm_layer_shell = Some(ls);
                }
                "river_input_manager_v1" => {
                    let im: RiverInputManagerV1 = registry.bind(name, version.min(1), qh, ());
                    state.globals.rwm_input_manager = Some(im.clone());
                    state.context.borrow_mut().rwm_input_manager = Some(im);
                }
                "river_libinput_config_v1" => {
                    let lc: RiverLibinputConfigV1 = registry.bind(name, version.min(1), qh, ());
                    state.globals.rwm_libinput_config = Some(lc.clone());
                    state.context.borrow_mut().rwm_libinput_config = Some(lc);
                }
                "wp_cursor_shape_manager_v1" => {
                    log::info!("Binding wp_cursor_shape_manager_v1 v{}", version.min(2));
                    let manager: WpCursorShapeManagerV1 =
                        registry.bind(name, version.min(2), qh, ());
                    state.globals.cursor_shape_manager = Some(manager);
                    let seats: Vec<_> = state
                        .context
                        .borrow()
                        .seats
                        .values()
                        .cloned()
                        .collect();
                    for seat_ref in seats {
                        attach_cursor_shape_device(state, &seat_ref, qh);
                    }
                }
                _ => {}
            }
        }
    }
}

// Implement dispatch for River Window Manager protocol
impl Dispatch<RiverWindowManagerV1, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverWindowManagerV1,
        event: river_window_management_v1::client::river_window_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use river_window_management_v1::client::river_window_manager_v1::Event;

        match event {
            Event::Unavailable => {
                log::error!("Window management unavailable - another WM may be running");
                state.context.borrow_mut().running = false;
            }
            Event::Finished => {
                log::info!("Window manager finished");
                state.context.borrow_mut().running = false;
            }
            Event::ManageStart => {
                state.context.borrow_mut().handle_manage_start();
                state.context.borrow().finish_manage();
            }
            Event::RenderStart => {
                state.context.borrow_mut().handle_render_start();

                // Update titlebars
                if let Some(ref shm) = state.globals.shm {
                    let context = state.context.borrow();
                    let focused_window = context.focused_window;

                    for (&window_id, window) in &context.windows {
                        let mut w = window.borrow_mut();

                        // Skip hidden windows
                        if w.hidden {
                            continue;
                        }

                        // Extract window data before borrowing titlebar
                        let width = w.width;
                        let title = w.title.clone();
                        let is_focused = focused_window == Some(window_id);
                        let height = w.height;

                        // Update titlebar if it exists and window has valid dimensions
                        if let Some(ref mut titlebar) = w.titlebar {
                            log::info!("Window {} titlebar: width={}, has_buffer={}",
                                window_id, width, titlebar.buffer.is_some());
                            if width > 0 && height > 0 {
                                // Ensure buffer is allocated
                                titlebar.ensure_buffer(width, height, shm, qh);
                                if let Some(ref compositor) = state.globals.compositor {
                                    titlebar.update_input_region(compositor, qh);
                                }

                                // Render titlebar content
                                titlebar.render(title.as_deref(), is_focused);
                                log::info!("Window {} titlebar rendered, focused={}", window_id, is_focused);

                                // Position decoration so it sits above content with borders
                                let border_width = rwm::titlebar::BORDER_WIDTH;
                                let titlebar_height = rwm::titlebar::TITLEBAR_HEIGHT;
                                titlebar.set_offset(-border_width, -border_width - titlebar_height);

                                // Sync and commit (only if we have a buffer)
                                if titlebar.buffer.is_some() {
                                    titlebar.sync_next_commit();
                                    titlebar.commit();
                                    log::info!("Window {} titlebar committed (width={})", window_id, width);
                                }
                            } else {
                                log::info!("Window {} titlebar skipped: width=0", window_id);
                            }
                        } else {
                            log::warn!("Window {} has no titlebar!", window_id);
                        }
                    }
                }

                state.context.borrow().finish_render();
            }
            Event::Window { id } => {
                let window = state.context.borrow_mut().create_window(id.clone());
                let window_id = window.borrow().id;

                // Get the node for this window
                let node: RiverNodeV1 = id.get_node(qh, window_id);
                window.borrow_mut().rwm_node = Some(node);

                // Create titlebar if compositor is available
                if let Some(ref compositor) = state.globals.compositor {
                    log::info!("Creating titlebar surface for window {}", window_id);

                    // Create surface for titlebar
                    let surface = compositor.create_surface(qh, TitlebarSurfaceData { window_id });

                    // Create decoration for titlebar (above window content)
                    let decoration: RiverDecorationV1 = id.get_decoration_above(&surface, qh, window_id);

                    // Create titlebar
                    let titlebar = rwm::Titlebar::new(surface, decoration);
                    window.borrow_mut().titlebar = Some(titlebar);

                    log::info!("Created titlebar for window {}", window_id);
                } else {
                    log::warn!("No compositor available, cannot create titlebar for window {}", window_id);
                }

                // Queue init event
                window.borrow_mut().queue_event(rwm::WindowEvent::Init);
            }
            Event::Output { id } => {
                let output = state.context.borrow_mut().create_output(id.clone());

                // Get layer shell output if available
                if let Some(ref layer_shell) = state.globals.rwm_layer_shell {
                    let ls_output: RiverLayerShellOutputV1 =
                        layer_shell.get_output(&id, qh, output.borrow().id);
                    output.borrow_mut().layer_shell_output = Some(ls_output);
                }
            }
            Event::Seat { id } => {
                let seat = state.context.borrow_mut().create_seat(id.clone());
                let seat_id = seat.borrow().id;

                // Get layer shell seat if available
                if let Some(ref layer_shell) = state.globals.rwm_layer_shell {
                    let ls_seat: RiverLayerShellSeatV1 =
                        layer_shell.get_seat(&id, qh, seat_id);
                    seat.borrow_mut().layer_shell_seat = Some(ls_seat);
                }

                // Register XKB bindings with the compositor
                if let Some(ref xkb_bindings_global) = state.globals.rwm_xkb_bindings {
                    let mut seat_ref = seat.borrow_mut();
                    log::info!("Registering {} XKB bindings for seat {}", seat_ref.xkb_bindings.len(), seat_id);

                    for (idx, (binding, rwm_binding_slot)) in seat_ref.xkb_bindings.iter_mut().enumerate() {
                        // Protocol: get_xkb_binding(seat, keysym, modifiers) -> new_id
                        // wayland-client adds qh and user_data at the end
                        use river_window_management_v1::client::river_seat_v1::Modifiers;
                        let mods = Modifiers::from_bits_truncate(binding.modifiers);
                        let rwm_binding: RiverXkbBindingV1 = xkb_bindings_global.get_xkb_binding(
                            &id,
                            binding.keysym,
                            mods,
                            qh,
                            (seat_id, idx),
                        );

                        // Enable binding if it's for the current mode
                        if binding.enabled {
                            rwm_binding.enable();
                            log::debug!("Enabled binding {} (keysym: {:#x}, mods: {:#x})", idx, binding.keysym, binding.modifiers);
                        }

                        *rwm_binding_slot = Some(rwm_binding);
                    }
                }

                // Register pointer bindings with the compositor
                {
                    let mut seat_ref = seat.borrow_mut();
                    let rwm_seat = seat_ref.rwm_seat.clone();
                    if let Some(rwm_seat) = rwm_seat {
                        log::info!("Registering {} pointer bindings for seat {}", seat_ref.pointer_bindings.len(), seat_id);

                        for (idx, (binding, rwm_binding_slot)) in seat_ref.pointer_bindings.iter_mut().enumerate() {
                            // Protocol: get_pointer_binding(button, modifiers) -> new_id on river_seat_v1
                            use river_window_management_v1::client::river_seat_v1::Modifiers;
                            let mods = Modifiers::from_bits_truncate(binding.modifiers);
                            let rwm_binding: RiverPointerBindingV1 = rwm_seat.get_pointer_binding(
                                binding.button,
                                mods,
                                qh,
                                (seat_id, idx),
                            );

                            // Enable binding if it's for the current mode
                            if binding.enabled {
                                rwm_binding.enable();
                            }

                            *rwm_binding_slot = Some(rwm_binding);
                        }
                    }
                }
            }
            Event::SessionLocked => {
                state.context.borrow_mut().session_locked = true;
                // Switch to lock mode
                if let Some(seat_id) = state.context.borrow().current_seat {
                    if let Some(seat) = state.context.borrow().seats.get(&seat_id) {
                        seat.borrow_mut().switch_mode(config::Mode::Lock);
                    }
                }
            }
            Event::SessionUnlocked => {
                state.context.borrow_mut().session_locked = false;
                // Switch back to default mode
                if let Some(seat_id) = state.context.borrow().current_seat {
                    if let Some(seat) = state.context.borrow().seats.get(&seat_id) {
                        seat.borrow_mut().switch_mode(config::Mode::Default);
                    }
                }
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            // window event (opcode 6) creates a river_window_v1
            6 => qhandle.make_data::<RiverWindowV1, _>(0usize), // placeholder window id
            // output event (opcode 7) creates a river_output_v1
            7 => qhandle.make_data::<RiverOutputV1, _>(0usize), // placeholder output id
            // seat event (opcode 8) creates a river_seat_v1
            8 => qhandle.make_data::<RiverSeatV1, _>(0usize), // placeholder seat id
            _ => unreachable!("unknown event opcode {}", opcode),
        }
    }
}

// Implement dispatch for River Window
impl Dispatch<RiverWindowV1, rwm::WindowId> for AppState {
    fn event(
        state: &mut Self,
        proxy: &RiverWindowV1,
        event: river_window_management_v1::client::river_window_v1::Event,
        _window_id: &rwm::WindowId, // Don't use this - it's always 0 from event_created_child
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use river_window_management_v1::client::river_window_v1::Event;

        // Find window by matching the RiverWindowV1 object, not by user data
        let context = state.context.borrow();
        let found = context.windows.iter().find_map(|(&id, w)| {
            if w.borrow().rwm_window.as_ref().map(|rw| rw == proxy).unwrap_or(false) {
                Some((id, w.clone()))
            } else {
                None
            }
        });
        let (window_id, window) = match found {
            Some(f) => f,
            None => return,
        };
        drop(context);

        match event {
            Event::Closed => {
                state.context.borrow_mut().destroy_window(window_id);
            }
            Event::DimensionsHint {
                min_width,
                min_height,
                max_width,
                max_height,
            } => {
                log::info!("Window {} DimensionsHint: min={}x{}, max={}x{}",
                    window_id, min_width, min_height, max_width, max_height);
                let mut w = window.borrow_mut();
                w.min_width = min_width;
                w.min_height = min_height;
            }
            Event::Dimensions { width, height } => {
                log::info!("Window {} received Dimensions event: {}x{}", window_id, width, height);
                window.borrow_mut().update_dimensions(width, height);
            }
            Event::AppId { app_id } => {
                window.borrow_mut().app_id = app_id;
            }
            Event::Title { title } => {
                window.borrow_mut().title = title;
            }
            Event::DecorationHint { hint } => {
                // Convert WEnum to u32
                if let wayland_client::WEnum::Value(h) = hint {
                    window.borrow_mut().decoration_hint = h as u32;
                }
            }
            Event::UnreliablePid { unreliable_pid } => {
                let mut w = window.borrow_mut();
                w.pid = unreliable_pid;

                // Track terminal windows for swallowing
                if w.is_terminal {
                    state.context.borrow_mut().terminal_windows.insert(unreliable_pid, window_id);
                }
            }
            Event::PointerMoveRequested { seat } => {
                // Find the seat and queue move action
                let context = state.context.borrow();
                if let Some((seat_id, seat_rc)) = context.seats.iter().find(|(_, s)| {
                    s.borrow().rwm_seat.as_ref().map(|rs| rs == &seat).unwrap_or(false)
                }) {
                    window.borrow_mut().queue_event(
                        rwm::WindowEvent::Move(Rc::downgrade(seat_rc))
                    );
                }
            }
            Event::PointerResizeRequested { seat, edges } => {
                let context = state.context.borrow();
                if let Some((_, seat_rc)) = context.seats.iter().find(|(_, s)| {
                    s.borrow().rwm_seat.as_ref().map(|rs| rs == &seat).unwrap_or(false)
                }) {
                    // Convert WEnum<Edges> to u32
                    let edges_u32 = if let wayland_client::WEnum::Value(e) = edges {
                        e.bits()
                    } else {
                        0
                    };
                    window.borrow_mut().queue_event(
                        rwm::WindowEvent::Resize(Rc::downgrade(seat_rc), edges_u32)
                    );
                }
            }
            Event::FullscreenRequested { output } => {
                let output_weak = output.and_then(|o| {
                    let context = state.context.borrow();
                    context.outputs.iter().find_map(|(_, out)| {
                        if out.borrow().rwm_output.as_ref().map(|ro| ro == &o).unwrap_or(false) {
                            Some(Rc::downgrade(out))
                        } else {
                            None
                        }
                    })
                });
                window.borrow_mut().queue_event(
                    rwm::WindowEvent::Fullscreen(output_weak)
                );
            }
            Event::ExitFullscreenRequested => {
                window.borrow_mut().queue_event(rwm::WindowEvent::Unfullscreen);
            }
            Event::MaximizeRequested => {
                window.borrow_mut().queue_event(rwm::WindowEvent::Maximize);
            }
            Event::UnmaximizeRequested => {
                window.borrow_mut().queue_event(rwm::WindowEvent::Unmaximize);
            }
            Event::MinimizeRequested => {
                window.borrow_mut().queue_event(rwm::WindowEvent::Minimize);
            }
            _ => {}
        }
    }
}

// Implement dispatch for River Node
impl Dispatch<RiverNodeV1, rwm::WindowId> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverNodeV1,
        _event: river_window_management_v1::client::river_node_v1::Event,
        _data: &rwm::WindowId,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Node events would be handled here if needed
    }
}

// Implement dispatch for River Output
impl Dispatch<RiverOutputV1, rwm::OutputId> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverOutputV1,
        event: river_window_management_v1::client::river_output_v1::Event,
        output_id: &rwm::OutputId,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use river_window_management_v1::client::river_output_v1::Event;

        let context = state.context.borrow();
        let output = match context.outputs.get(output_id) {
            Some(o) => o.clone(),
            None => return,
        };
        drop(context);

        match event {
            Event::Removed => {
                state.context.borrow_mut().destroy_output(*output_id);
            }
            Event::WlOutput { name } => {
                output.borrow_mut().wl_output_name = name;
            }
            Event::Position { x, y } => {
                output.borrow_mut().update_position(x, y);
            }
            Event::Dimensions { width, height } => {
                output.borrow_mut().update_dimensions(width, height);
            }
            _ => {}
        }
    }
}

// Implement dispatch for River Seat
impl Dispatch<RiverSeatV1, rwm::SeatId> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverSeatV1,
        event: river_window_management_v1::client::river_seat_v1::Event,
        seat_id: &rwm::SeatId,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use river_window_management_v1::client::river_seat_v1::Event;

        let context = state.context.borrow();
        let seat = match context.seats.get(seat_id) {
            Some(s) => s.clone(),
            None => return,
        };
        drop(context);

        match event {
            Event::Removed => {
                state.context.borrow_mut().destroy_seat(*seat_id);
            }
            Event::WlSeat { name } => {
                seat.borrow_mut().wl_seat_name = name;
                attach_wl_seat(state, &seat, qh);
            }
            Event::PointerEnter { window } => {
                // Find the window
                let context = state.context.borrow();
                let found = context.windows.iter().find_map(|(id, w)| {
                    if w.borrow().rwm_window.as_ref().map(|rw| rw == &window).unwrap_or(false) {
                        Some((*id, Rc::downgrade(w)))
                    } else {
                        None
                    }
                });
                let sloppy_focus = context.config.sloppy_focus;
                drop(context);

                if let Some((wid, weak)) = found {
                    seat.borrow_mut().window_below_pointer = Some(weak);

                    // Focus on hover if sloppy focus is enabled
                    if sloppy_focus {
                        state.context.borrow_mut().focus(wid);
                    }
                    state.context.borrow_mut().update_cursor_for_seat(*seat_id);
                }
            }
            Event::PointerLeave => {
                seat.borrow_mut().window_below_pointer = None;
                state.context.borrow_mut().update_cursor_for_seat(*seat_id);
            }
            Event::WindowInteraction { window } => {
                // Always focus the window on click/interaction
                let context = state.context.borrow();
                if let Some((&wid, _)) = context.windows.iter().find(|(_, w)| {
                    w.borrow().rwm_window.as_ref().map(|rw| rw == &window).unwrap_or(false)
                }) {
                    drop(context);
                    log::debug!("Window interaction - focusing window {}", wid);
                    let mut context = state.context.borrow_mut();
                    context.focus(wid);
                    context.handle_window_interaction(*seat_id, wid);
                }
            }
            Event::OpDelta { dx, dy } => {
                // Apply delta to window being operated on
                let context = state.context.borrow();
                if let Some(wid) = context.focused_window {
                    if let Some(window) = context.windows.get(&wid) {
                        window.borrow_mut().apply_op_delta(dx, dy);
                    }
                }
            }
            Event::OpRelease => {
                // End operation
                let context = state.context.borrow();
                if let Some(wid) = context.focused_window {
                    if let Some(window) = context.windows.get(&wid) {
                        window.borrow_mut().end_operation();
                    }
                }
                seat.borrow().end_pointer_op();
                drop(context);
                state.context.borrow_mut().update_cursor_for_seat(*seat_id);
            }
            Event::PointerPosition { x, y } => {
                seat.borrow_mut().update_pointer_position(x, y);
                state.context.borrow_mut().update_cursor_for_seat(*seat_id);
            }
            _ => {}
        }
    }
}

// Implement dispatch for Layer Shell
impl Dispatch<RiverLayerShellV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverLayerShellV1,
        _event: river_layer_shell_v1::client::river_layer_shell_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Layer shell global events
    }
}

impl Dispatch<RiverLayerShellOutputV1, rwm::OutputId> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverLayerShellOutputV1,
        event: river_layer_shell_v1::client::river_layer_shell_output_v1::Event,
        output_id: &rwm::OutputId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river_layer_shell_v1::client::river_layer_shell_output_v1::Event;

        if let Event::NonExclusiveArea { x, y, width, height } = event {
            if let Some(output) = state.context.borrow().outputs.get(output_id) {
                output.borrow_mut().update_exclusive_area(x, y, width, height);
            }
        }
    }
}

impl Dispatch<RiverLayerShellSeatV1, rwm::SeatId> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverLayerShellSeatV1,
        event: river_layer_shell_v1::client::river_layer_shell_seat_v1::Event,
        seat_id: &rwm::SeatId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river_layer_shell_v1::client::river_layer_shell_seat_v1::Event;

        if let Some(seat) = state.context.borrow().seats.get(seat_id) {
            match event {
                Event::FocusExclusive => {
                    seat.borrow_mut().focus_exclusive = true;
                }
                Event::FocusNonExclusive | Event::FocusNone => {
                    seat.borrow_mut().focus_exclusive = false;
                }
                _ => {}
            }
        }
    }
}

// Implement dispatch for XKB bindings
impl Dispatch<RiverXkbBindingsV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverXkbBindingsV1,
        _event: river_xkb_bindings_v1::client::river_xkb_bindings_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // XKB bindings global events
    }
}

impl Dispatch<RiverXkbBindingV1, (rwm::SeatId, usize)> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverXkbBindingV1,
        event: river_xkb_bindings_v1::client::river_xkb_binding_v1::Event,
        (seat_id, binding_idx): &(rwm::SeatId, usize),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river_xkb_bindings_v1::client::river_xkb_binding_v1::Event;

        log::debug!("XKB binding event: seat={}, binding_idx={}, event={:?}", seat_id, binding_idx, event);

        if let Some(seat) = state.context.borrow().seats.get(seat_id) {
            let seat = seat.clone();
            let mut seat_ref = seat.borrow_mut();

            if let Some((binding, _)) = seat_ref.xkb_bindings.get(*binding_idx) {
                let action = binding.action.clone();
                log::info!("Binding triggered: keysym={:#x}, mods={:#x}, enabled={}, action={:?}",
                    binding.keysym, binding.modifiers, binding.enabled, action);

                match event {
                    Event::Pressed => {
                        if binding.enabled && binding.event == binding::BindingEvent::Pressed {
                            log::info!("Executing action: {:?}", action);
                            seat_ref.queue_action(action);
                        }
                    }
                    Event::Released => {
                        if binding.enabled && binding.event == binding::BindingEvent::Released {
                            log::info!("Executing action (on release): {:?}", action);
                            seat_ref.queue_action(action);
                        }
                    }
                }
            }
        }
    }
}

// Implement dispatch for Pointer bindings
impl Dispatch<RiverPointerBindingV1, (rwm::SeatId, usize)> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverPointerBindingV1,
        event: river_window_management_v1::client::river_pointer_binding_v1::Event,
        (seat_id, binding_idx): &(rwm::SeatId, usize),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river_window_management_v1::client::river_pointer_binding_v1::Event;

        if let Some(seat) = state.context.borrow().seats.get(seat_id) {
            let seat = seat.clone();
            let mut seat_ref = seat.borrow_mut();

            if let Some((binding, _)) = seat_ref.pointer_bindings.get(*binding_idx) {
                let action = binding.action.clone();

                match event {
                    Event::Pressed => {
                        if binding.enabled && binding.event == binding::BindingEvent::Pressed {
                            seat_ref.queue_action(action);
                        }
                    }
                    Event::Released => {
                        if binding.enabled && binding.event == binding::BindingEvent::Released {
                            seat_ref.queue_action(action);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

// Implement dispatch for Input Manager
impl Dispatch<RiverInputManagerV1, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &RiverInputManagerV1,
        event: river_input_management_v1::client::river_input_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use river_input_management_v1::client::river_input_manager_v1::Event;

        match event {
            Event::InputDevice { id } => {
                // Input device created - configure it
                let config = &state.context.borrow().config;
                id.set_repeat_info(config.repeat_rate, config.repeat_delay);
                id.set_scroll_factor(config.scroll_factor);
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            // input_device event (opcode 1) creates a river_input_device_v1
            1 => qhandle.make_data::<RiverInputDeviceV1, _>(()),
            _ => unreachable!("unknown event opcode {}", opcode),
        }
    }
}

impl Dispatch<RiverInputDeviceV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverInputDeviceV1,
        _event: river_input_management_v1::client::river_input_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Input device events
    }
}

// Implement dispatch for Libinput Config
impl Dispatch<RiverLibinputConfigV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverLibinputConfigV1,
        _event: river_libinput_config_v1::client::river_libinput_config_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Libinput config events
    }
}

impl Dispatch<RiverLibinputDeviceV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverLibinputDeviceV1,
        _event: river_libinput_config_v1::client::river_libinput_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Libinput device events
    }
}

// Standard Wayland protocol dispatches for titlebar surfaces
impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wl_compositor has no events
    }
}

impl Dispatch<wl_region::WlRegion, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_region::WlRegion,
        _event: wl_region::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wl_region has no events
    }
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_shm::Event::Format { format: _ } = event {
            // We only need ARGB8888 which is always available
        }
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wl_shm_pool has no events
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            // Buffer is no longer in use by compositor
        }
    }
}

impl Dispatch<wl_seat::WlSeat, u32> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wl_seat events are not needed; river provides seat information.
    }
}

impl Dispatch<wl_pointer::WlPointer, rwm::SeatId> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        seat_id: &rwm::SeatId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let seat = {
            let context = state.context.borrow();
            context.seats.get(seat_id).cloned()
        };
        if let Some(seat) = seat {
            match event {
                wl_pointer::Event::Enter { serial, .. } => {
                    seat.borrow_mut().pointer_enter_serial = serial;
                    state.context.borrow_mut().update_cursor_for_seat(*seat_id);
                }
                wl_pointer::Event::Leave { serial, .. } => {
                    let mut seat = seat.borrow_mut();
                    seat.pointer_enter_serial = serial;
                    seat.cursor_shape = None;
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WpCursorShapeManagerV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WpCursorShapeManagerV1,
        _event: wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Cursor shape manager has no events.
    }
}

impl Dispatch<WpCursorShapeDeviceV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &WpCursorShapeDeviceV1,
        _event: wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Cursor shape device has no events.
    }
}

// Titlebar surface user data
struct TitlebarSurfaceData {
    window_id: rwm::WindowId,
}

impl Dispatch<wl_surface::WlSurface, TitlebarSurfaceData> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &TitlebarSurfaceData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Surface events (enter/leave output) - not needed for titlebars
    }
}

impl Dispatch<RiverDecorationV1, rwm::WindowId> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &RiverDecorationV1,
        _event: river_window_management_v1::client::river_decoration_v1::Event,
        _data: &rwm::WindowId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // river_decoration_v1 has no events
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("RWM - River Window Manager starting");

    // Connect to Wayland display
    let conn = Connection::connect_to_env()?;
    let display = conn.display();

    // Create event queue
    let mut event_queue: EventQueue<AppState> = conn.new_event_queue();
    let qh = event_queue.handle();

    // Create app state
    let context = Rc::new(RefCell::new(Context::new()));
    let mut state = AppState {
        context: context.clone(),
        globals: Globals::default(),
    };

    // Get registry and collect globals
    let _registry = display.get_registry(&qh, ());

    // Roundtrip to receive globals
    event_queue.roundtrip(&mut state)?;

    // Check required globals
    if state.globals.rwm.is_none() {
        log::error!("river_window_manager_v1 not available - is River running?");
        return Err("River window manager protocol not available".into());
    }

    // Set up signal handling
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGINT);
    mask.add(Signal::SIGTERM);
    mask.add(Signal::SIGQUIT);
    mask.add(Signal::SIGCHLD);
    mask.thread_block()?;

    let signal_fd = SignalFd::with_flags(&mask, SfdFlags::SFD_NONBLOCK)?;

    // Run startup commands
    for cmd in &state.context.borrow().config.startup_cmds.clone() {
        state.context.borrow().spawn(cmd);
    }

    log::info!("RWM initialized, entering main loop");

    // Main event loop
    while state.context.borrow().running {
        // Flush outgoing requests
        conn.flush()?;

        // Prepare poll file descriptors
        let wayland_fd = conn.as_fd();
        let signal_raw_fd = signal_fd.as_raw_fd();

        let mut poll_fds = [
            PollFd::new(wayland_fd, PollFlags::POLLIN),
            PollFd::new(signal_fd.as_fd(), PollFlags::POLLIN),
        ];

        // Poll for events (None = infinite timeout)
        match poll(&mut poll_fds, None::<u16>) {
            Ok(_) => {}
            Err(nix::errno::Errno::EINTR) => continue,
            Err(e) => return Err(e.into()),
        }

        // Handle Wayland events
        if poll_fds[0].revents().map(|r| r.contains(PollFlags::POLLIN)).unwrap_or(false) {
            event_queue.dispatch_pending(&mut state)?;

            // Read and dispatch new events
            if let Some(guard) = conn.prepare_read() {
                match guard.read() {
                    Ok(_) => {}
                    Err(wayland_client::backend::WaylandError::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e.into()),
                }
            }
            event_queue.dispatch_pending(&mut state)?;
        }

        // Handle signals
        if poll_fds[1].revents().map(|r| r.contains(PollFlags::POLLIN)).unwrap_or(false) {
            if let Ok(Some(sig_info)) = signal_fd.read_signal() {
                match Signal::try_from(sig_info.ssi_signo as i32) {
                    Ok(Signal::SIGINT) | Ok(Signal::SIGTERM) | Ok(Signal::SIGQUIT) => {
                        log::info!("Received termination signal, shutting down");
                        state.context.borrow_mut().running = false;
                    }
                    Ok(Signal::SIGCHLD) => {
                        // Reap child processes
                        loop {
                            match nix::sys::wait::waitpid(
                                None,
                                Some(nix::sys::wait::WaitPidFlag::WNOHANG),
                            ) {
                                Ok(nix::sys::wait::WaitStatus::StillAlive) => break,
                                Ok(_) => continue,
                                Err(_) => break,
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    log::info!("RWM shutting down");
    Ok(())
}
