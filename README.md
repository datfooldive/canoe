# Canoe 🛶 - River Window Manager

A stacking window manager for the River Wayland compositor, written in Rust.

## Building

```bash
cargo build --release
```

## Running

```bash
river -c ./target/release/canoe
```

For debug output:
```bash
RUST_LOG=info river -c ./target/release/canoe
```

## Configuration

Canoe reads `~/.config/canoe/canoe.toml`.
The main modifier defaults to `super`, but you can change it:

```toml
main_modifier = "alt"
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Super+Shift+Return` | Open terminal (foot) |
| `Super+Space` | Open application launcher (fuzzel) |
| `Super+w` | Close focused window |
| `Super+Tab` | Focus next window |
| `Super+Shift+Tab` | Focus previous window |
| `Super+Enter` | Toggle fullscreen |

## Mouse Actions

| Action | Result |
|--------|--------|
| Click on window | Focus window |
| Drag titlebar | Move window |
| Drag window edges | Resize window |
| `Super+Left Drag` | Move window (anywhere) |
| `Super+Right Drag` | Resize window (anywhere) |

## Features

- Stacking window management
- Yellow window borders (10px)
- Yellow titlebars (24px)
- Server-side decorations
- Titlebar/edge window movement and resizing (Super+Drag anywhere)
- Window focus follows click

## Requirements

- River compositor
- foot terminal (for Super+Shift+Return)
- fuzzel (for Super+Space launcher)

## License

MIT
