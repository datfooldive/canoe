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

## Features

- Stacking window management
- Forced server-side decorations with classic window borders & titlebars
- Titlebar/edge window movement and resizing (Super+Drag anywhere)
- Window focus follows click
- Optional "swallowing" of client-side decoration via per-window rules

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Super+Shift+Return` | Open terminal (foot) |
| `Super+Space` | Open application launcher (fuzzel) |
| `Super+w` | Close focused window |
| `Super+Tab` | Focus next window |
| `Super+Shift+Tab` | Focus previous window |
| ``Super+` `` | Focus next window of the same application |
| `Super+Enter` | Toggle fullscreen |
| `Super+Down` | Unfullscreen/unmaximize, otherwise minimize focused window |
| `Super+Up` | Maximize focused window |
| `Super+h` | Minimize focused window |
| `Super+m` | Minimize focused window |
| `Super+Left` | Snap focused window to left half; restore if snapped right |
| `Super+Right` | Snap focused window to right half; restore if snapped left |
| `Super+Alt+Left` | Send focused window to previous output |
| `Super+Alt+Right` | Send focused window to next output |
| `Super+Alt+Up` | Send focused window to previous output |
| `Super+Alt+Down` | Send focused window to next output |

## Mouse Actions

| Action | Result |
|--------|--------|
| Click on window | Focus window |
| Drag titlebar | Move window |
| Drag window edges | Resize window |
| `Super+Left Drag` | Move window (anywhere) |
| `Super+Right Drag` | Resize window (anywhere) |

## Configuration

Canoe reads `~/.config/canoe/canoe.toml`.
The main modifier defaults to `super`, but you can change it:

```toml
main_modifier = "alt"
```

The launcher defaults to `fuzzel`. You can override it with a command or argv:

```toml
launcher_cmd = "fuzzel"
# Or with arguments:
launcher_cmd = ["fuzzel", "--dmenu"]
```

### Rule Matching

Rules live under `[[rules]]` in `canoe.toml`. App ID matching uses OR across the
app-id fields, and property matching uses AND across the listed properties.

```toml
[[rules]]
match_app_id = ["foot", "kitty"]   # exact match, any value matches
match_app_id_prefix = "mate-"      # prefix match (e.g. mate-calc)
match_props = ["toplevel", "csd_only"] # all props must match
```

Supported `match_props` values:
- `toplevel` (window has no parent)
- `csd_only` (client does not support SSD)

The matching windows can have parts of their content "swallowed". This removes
the client-side decoration on my Firefox, for instance:

```toml
match_app_id = "firefox-esr"
match_props = "toplevel"
swallow_top = 38
```

## Requirements

- [River](https://codeberg.org/river/river) Wayland compositor
- foot terminal (for Super+Shift+Return)
- fuzzel (for Super+Space launcher)

## License

MIT
