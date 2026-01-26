//! Generated Wayland protocol bindings for River window manager

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_non_exhaustive)]
#![allow(clippy::single_component_path_imports)]
#![allow(non_camel_case_types)]
#![allow(missing_docs)]

use wayland_client;

/// River Window Management Protocol
pub mod river_window_management_v1 {
    use wayland_client;
    use wayland_client::protocol::__interfaces::*;
    use wayland_client::protocol::wl_output;
    use wayland_client::protocol::wl_seat;
    use wayland_client::protocol::wl_surface;

    wayland_scanner::generate_interfaces!("protocol/river-window-management-v1.xml");

    pub mod client {
        use super::*;
        use wayland_client;
        use wayland_client::protocol::wl_output;
        use wayland_client::protocol::wl_seat;
        use wayland_client::protocol::wl_surface;

        wayland_scanner::generate_client_code!("protocol/river-window-management-v1.xml");
    }
}

/// River XKB Bindings Protocol
pub mod river_xkb_bindings_v1 {
    use super::river_window_management_v1::*;
    use wayland_client;

    wayland_scanner::generate_interfaces!("protocol/river-xkb-bindings-v1.xml");

    pub mod client {
        use super::*;
        use crate::protocol::river_window_management_v1::client::*;
        use wayland_client;

        wayland_scanner::generate_client_code!("protocol/river-xkb-bindings-v1.xml");
    }
}

/// River Layer Shell Protocol
pub mod river_layer_shell_v1 {
    use super::river_window_management_v1::*;
    use wayland_client;

    wayland_scanner::generate_interfaces!("protocol/river-layer-shell-v1.xml");

    pub mod client {
        use super::*;
        use crate::protocol::river_window_management_v1::client::*;
        use wayland_client;

        wayland_scanner::generate_client_code!("protocol/river-layer-shell-v1.xml");
    }
}

/// River Input Management Protocol
pub mod river_input_management_v1 {
    use wayland_client;
    use wayland_client::protocol::__interfaces::*;
    use wayland_client::protocol::wl_output;

    wayland_scanner::generate_interfaces!("protocol/river-input-management-v1.xml");

    pub mod client {
        use super::*;
        use wayland_client;
        use wayland_client::protocol::wl_output;

        wayland_scanner::generate_client_code!("protocol/river-input-management-v1.xml");
    }
}

/// River Libinput Config Protocol
pub mod river_libinput_config_v1 {
    use super::river_input_management_v1::*;
    use wayland_client;

    wayland_scanner::generate_interfaces!("protocol/river-libinput-config-v1.xml");

    pub mod client {
        use super::*;
        use crate::protocol::river_input_management_v1::client::*;
        use wayland_client;

        wayland_scanner::generate_client_code!("protocol/river-libinput-config-v1.xml");
    }
}

// Re-export commonly used types
pub use river_window_management_v1::client::river_decoration_v1::RiverDecorationV1;
pub use river_window_management_v1::client::river_node_v1::RiverNodeV1;
pub use river_window_management_v1::client::river_output_v1::RiverOutputV1;
pub use river_window_management_v1::client::river_pointer_binding_v1::RiverPointerBindingV1;
pub use river_window_management_v1::client::river_seat_v1::RiverSeatV1;
pub use river_window_management_v1::client::river_shell_surface_v1::RiverShellSurfaceV1;
pub use river_window_management_v1::client::river_window_manager_v1::RiverWindowManagerV1;
pub use river_window_management_v1::client::river_window_v1::RiverWindowV1;

pub use river_xkb_bindings_v1::client::river_xkb_binding_v1::RiverXkbBindingV1;
pub use river_xkb_bindings_v1::client::river_xkb_bindings_v1::RiverXkbBindingsV1;

pub use river_layer_shell_v1::client::river_layer_shell_output_v1::RiverLayerShellOutputV1;
pub use river_layer_shell_v1::client::river_layer_shell_seat_v1::RiverLayerShellSeatV1;
pub use river_layer_shell_v1::client::river_layer_shell_v1::RiverLayerShellV1;

pub use river_input_management_v1::client::river_input_device_v1::RiverInputDeviceV1;
pub use river_input_management_v1::client::river_input_manager_v1::RiverInputManagerV1;

pub use river_libinput_config_v1::client::river_libinput_config_v1::RiverLibinputConfigV1;
pub use river_libinput_config_v1::client::river_libinput_device_v1::RiverLibinputDeviceV1;
pub use river_libinput_config_v1::client::river_libinput_result_v1::RiverLibinputResultV1;
