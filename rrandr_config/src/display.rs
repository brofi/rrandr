use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::data::auto::Auto;
use crate::data::color::Color;
use crate::data::weight::Weight;
use crate::MarkdownTable;

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Output area configuration
pub struct Display {
    /// Snapping strength when dragging outputs or `auto`. High values make it
    /// more "sticky", while 0 means no snapping. If left to default
    /// `snap_strength = min_size / 6` where `min_side` is the smallest side of
    /// any enabled output in px. E.g. when smallest screen resolution is Full
    /// HD => `snap_strength = 180`.
    pub snap_strength: Auto<f64>,
    /// Move distance when moving an output via keybindings
    pub pos_move_dist: i16,
    /// Thickness of the selection outline in px
    pub selection_line_width: f64,
    /// Thickness of the screen outline in px
    pub screen_line_width: f64,
    #[table]
    pub font: Font,
    #[table]
    pub colors: Colors,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            snap_strength: Auto::default(),
            pos_move_dist: 10,
            selection_line_width: 4.,
            screen_line_width: 2.,
            font: Default::default(),
            colors: Default::default(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Output area font configuration
pub struct Font {
    /// Font family
    pub family: String,
    /// Font size in pt
    pub size: u16,
    /// Font weight
    pub weight: Weight,
}

impl Default for Font {
    fn default() -> Self { Self { family: "monospace".to_owned(), size: 12, weight: Weight::Bold } }
}

#[derive(Clone, Default, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Output area colors
pub struct Colors {
    #[table]
    pub(crate) light: LightColors,
    #[table]
    pub(crate) dark: DarkColors,
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Output area light theme colors
pub struct LightColors {
    /// Output name text color
    pub text: Color,
    /// Output background color
    pub output: Color,
    /// Screen rectangle color
    pub screen: Color,
    /// Output selection color
    pub selection: Color,
}

impl Default for LightColors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#fff").unwrap_or_default(),
            output: Color::from_str("#3c3c3c").unwrap_or_default(),
            screen: Color::from_str("#3c3c3c").unwrap_or_default(),
            selection: Color::from_str("#3584e4").unwrap_or_default(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Output area dark theme colors
pub struct DarkColors {
    /// Output name text color
    pub text: Color,
    /// Output background color
    pub output: Color,
    /// Screen rectangle color
    pub screen: Color,
    /// Output selection color
    pub selection: Color,
}

impl Default for DarkColors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#000").unwrap_or_default(),
            output: Color::from_str("#f6f5f4").unwrap_or_default(),
            screen: Color::from_str("#f6f5f4").unwrap_or_default(),
            selection: Color::from_str("#1b68c6").unwrap_or_default(),
        }
    }
}
