# <img src="https://raw.githubusercontent.com/brofi/rrandr/master/rrandr/src/res/rrandr.svg" width="64" align="center"/> RRandR

A graphical interface to the RandR X Window System extension.

## Installation

For Arch Linux users an upstream version of _RRandR_ is available in the _Arch Linux User Repository ([AUR](https://aur.archlinux.org/))_: [rrandr-git](https://aur.archlinux.org/packages/rrandr-git).

### Manual Build & Installation

#### Build

* Install build dependencies:
    * [`rustup`](https://rustup.rs/), [`gettext`](https://www.gnu.org/software/gettext/)
    * Arch Linux: `pacman -S rustup gettext`
* Install dependencies:
    * `gtk4`, `pango`, `cairo`, `libxcb`, `glib2`, `glibc`, `gcc-libs`
    * Dependency names might differ depending on your distribution
    * Arch Linux: `pacman -S gtk4`
* Get, build & run:
    * `git clone https://github.com/brofi/rrandr`
    * `cd rrandr`
    * `cargo build --release`
    * `target/release/rrandr`

#### Install (optional)

```sh
#!/bin/bash
# Run as superuser from project root

# Install binary
install -Dm755 target/release/rrandr -t /usr/bin
# Install license
install -Dm644 COPYING -t /usr/share/licenses/rrandr
# Install logo
install -Dm644 rrandr/src/res/rrandr.svg -t /usr/share/pixmaps
# Install desktop file
desktop-file-install rrandr/src/res/rrandr.desktop
update-desktop-database
# Install translations
mapfile -t linguas < rrandr/po/LINGUAS
for lang in "${linguas[@]}"; do
    install -Dm644 "target/po/$lang/LC_MESSAGES/rrandr.mo" -t \
        "/usr/share/locale/$lang/LC_MESSAGES"
done
```

## Configuration

_RRandR_ is configured via a [TOML](https://toml.io/en/) configuration file. A configuration can be put in the following locations:

1. `$XDG_CONFIG_HOME/rrandr/rrandr.toml`
2. `$XDG_CONFIG_HOME/rrandr.toml`
3. `$HOME/.rrandr.toml`

Where `$XDG_CONFIG_HOME` if unset defaults to `$HOME/.config`.

The following sections describe all available configuration attributes grouped by TOML table.

[//]: # (<mark_config>)

### `[]` Root level configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `show_xrandr` | `Boolean` | `false` | Show an additional xrandr command for the current configuration |
| `revert_timeout` | `Integer` | `15` | Time in seconds until applied changes are being reverted |
| `apply_hook` | `String` | `` | Execute this child program when the screen configuration has been applied successfully. Useful for example to reset a wallpaper when not using a desktop environment. |
| `revert_hook` | `String` | `` | Execute this child program when the screen configuration has been reverted. |

### `[display]` Output area configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `snap_strength` | `Float or "auto"` | `auto` | Snapping strength when dragging outputs or `auto`. High values make it more "sticky", while 0 means no snapping. If left to default `snap_strength = min_size / 6` where `min_side` is the smallest side of any enabled output in px. E.g. when smallest screen resolution is Full HD => `snap_strength = 180`. |
| `pos_move_dist` | `Integer` | `10` | Move distance when moving an output via keybindings |
| `output_line_width` | `Float` | `3.5` | Thickness of the output outline in px |
| `output_line_style` | `BorderStyle` | `solid` | Style of the output outline |
| `selection_line_width` | `Float` | `3.5` | Thickness of the selection outline in px |
| `selection_line_style` | `BorderStyle` | `solid` | Style of the selection outline |
| `screen_line_width` | `Float` | `2.5` | Thickness of the screen outline in px |
| `screen_line_style` | `BorderStyle` | `dashed` | Style of the screen outline |

### `[display.font]` Output area font configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `family` | `String` | `monospace` | Font family |
| `size` | `Integer` | `12` | Font size in pt |
| `weight` | `Weight` | `bold` | Font weight |


### `[display.colors.light]` Output area light theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#000000` | Output name text color |
| `output` | `Color` | `#e8e6e3` | Output background color |
| `border` | `Color` | `#d8d4d0` | Output border color |
| `screen` | `Color` | `#cdc7c2` | Screen rectangle color |
| `selection` | `Color` | `#3584e4` | Output selection color |

### `[display.colors.dark]` Output area dark theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#ffffff` | Output name text color |
| `output` | `Color` | `#202020` | Output background color |
| `border` | `Color` | `#282828` | Output border color |
| `screen` | `Color` | `#1b1b1b` | Screen rectangle color |
| `selection` | `Color` | `#1b68c6` | Output selection color |

### `[popup]` Identify popup configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `ratio` | `Float` | `0.125` | Resolution to popup size ratio |
| `padding` | `Integer` | `5` | Padding in mm |
| `spacing` | `Integer` | `10` | Margin from screen edge in mm |
| `border_width` | `Integer` | `1` | Border width in mm |
| `timeout` | `Float` | `2.5` | Time in seconds the identify popup stays on screen |

### `[popup.font]` Identify popup font configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `family` | `String` | `Sans` | Font family |
| `size` | `Integer or "auto"` | `auto` | Font size in pt or "auto" |
| `weight` | `Weight` | `bold` | Font weight |


### `[popup.colors.light]` Identify popup light theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#000000` | Text color |
| `background` | `Color` | `#f6f5f4` | Background color |
| `border` | `Color` | `#cdc7c2` | Border color |

### `[popup.colors.dark]` Identify popup dark theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#ffffff` | Text color |
| `background` | `Color` | `#353535` | Background color |
| `border` | `Color` | `#1b1b1b` | Border color |

[//]: # (</mark_config>)