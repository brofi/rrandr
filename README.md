# <img src="https://raw.githubusercontent.com/brofi/rrandr/master/rrandr/src/res/rrandr.svg" width="64" align="center"/> RRandR

A graphical interface to the RandR X Window System extension.

## Installation

## Configuration

[//]: # (<mark_config>)


### `[display]` Output area configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `snap_strength` | `Float or "auto"` | `auto` | Snapping strength when dragging outputs or `auto`. High values make it more "sticky", while 0 means no snapping. If left to default `snap_strength = min_size / 6` where `min_side` is the smallest side of any enabled output in px. E.g. when smallest screen resolution is Full HD => `snap_strength = 180`. |
| `pos_move_dist` | `Integer` | `10` | Move distance when moving an output via keybindings |
| `selection_line_width` | `Float` | `4` | Thickness of the selection outline in px |
| `screen_line_width` | `Float` | `2` | Thickness of the screen outline in px |

### `[display.font]` Output area font configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `family` | `String` | `monospace` | Font family |
| `size` | `Integer` | `12` | Font size in pt |
| `weight` | `Weight` | `bold` | Font weight |


### `[display.colors.light]` Output area light theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#ffffff` | Output name text color |
| `output` | `Color` | `#3c3c3c` | Output background color |
| `screen` | `Color` | `#3c3c3c` | Screen rectangle color |
| `selection` | `Color` | `#3584e4` | Output selection color |

### `[display.colors.dark]` Output area dark theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#000000` | Output name text color |
| `output` | `Color` | `#f6f5f4` | Output background color |
| `screen` | `Color` | `#f6f5f4` | Screen rectangle color |
| `selection` | `Color` | `#1b68c6` | Output selection color |

### `[popup]` Identify popup configuration

| Attribute | Type | Default | Description |
|-|-|-|-|
| `ratio` | `Float` | `0.125` | Resolution to popup size ratio |
| `padding` | `Integer` | `5` | Padding in mm |
| `spacing` | `Integer` | `10` | Margin from screen edge in mm |
| `border_width` | `Integer` | `1` | Border width in mm |
| `show_secs` | `Float` | `2.5` | Show duration in seconds |

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
| `border` | `Color` | `#3c3c3c` | Border color |

### `[popup.colors.dark]` Identify popup dark theme colors

| Attribute | Type | Default | Description |
|-|-|-|-|
| `text` | `Color` | `#ffffff` | Text color |
| `background` | `Color` | `#3c3c3c` | Background color |
| `border` | `Color` | `#f6f5f4` | Border color |

[//]: # (</mark_config>)