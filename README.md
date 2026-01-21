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
| `Alt+p` | Open application launcher (wmenu-run) |
| `Alt+Shift+c` | Close focused window |
| `Alt+Shift+q` | Quit RWM |
| `Alt+Tab` | Focus next window |
| `Alt+Shift+Tab` | Focus previous window |
| `Alt+f` | Toggle fullscreen |

## Mouse Actions

| Action | Result |
|--------|--------|
| Click on window | Focus window |
| `Alt+Left Click` | Focus window |
| `Alt+Left Drag` | Move window |
| `Alt+Right Drag` | Resize window |

## Features

- Stacking/floating window management
- Yellow window borders (10px)
- Server-side decorations
- Alt+Drag window movement and resizing
- Window focus follows click

## Requirements

- River compositor
- foot terminal (for Alt+Shift+Return)
- wmenu (for Alt+p launcher)

## License

MIT
