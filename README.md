# RWM - River Window Manager

A stacking window manager for the River Wayland compositor, written in Rust.

## Building

```bash
cargo build --release
```

## Running

```bash
river -c ./target/release/rwm
```

For debug output:
```bash
RUST_LOG=info river -c ./target/release/rwm
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Alt+Shift+Return` | Open terminal (foot) |
| `Alt+Space` | Open application launcher (fuzzel) |
| `Alt+Shift+c` | Close focused window |
| `Alt+Shift+q` | Quit RWM |
| `Alt+Tab` | Focus next window |
| `Alt+Shift+Tab` | Focus previous window |
| `Alt+f` | Toggle fullscreen |

## Mouse Actions

| Action | Result |
|--------|--------|
| Click on window | Focus window |
| Drag titlebar | Move window |
| Drag window edges | Resize window |
| `Alt+Left Drag` | Move window (anywhere) |
| `Alt+Right Drag` | Resize window (anywhere) |

## Features

- Stacking/floating window management
- Yellow window borders (10px)
- Yellow titlebars (24px)
- Server-side decorations
- Titlebar/edge window movement and resizing (Alt+Drag anywhere)
- Window focus follows click

## Requirements

- River compositor
- foot terminal (for Alt+Shift+Return)
- fuzzel (for Alt+Space launcher)

## License

MIT
