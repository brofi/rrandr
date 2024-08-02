use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::data::auto::Auto;
use crate::data::color::Color;
use crate::data::weight::Weight;
use crate::MarkdownTable;

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Identify popup configuration
pub struct Popup {
    /// Padding in mm
    pub padding: u16,
    /// Margin from screen edge in mm
    pub spacing: u16,
    /// Resolution to popup size ratio
    pub ratio: f64,
    /// Show duration in seconds
    pub show_secs: f32,
    #[table]
    pub font: Font,
    #[table]
    pub colors: Colors,
}

impl Default for Popup {
    fn default() -> Self {
        Self {
            padding: 5,
            spacing: 10,
            ratio: 1. / 8.,
            show_secs: 2.5,
            font: Font::default(),
            colors: Colors::default(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Identify popup font configuration
pub struct Font {
    /// Font family
    pub family: String,
    /// Font size in pt or "auto"
    pub size: Auto<u16>,
    /// Font weight
    pub weight: Weight,
}

impl Default for Font {
    fn default() -> Self {
        Self { family: "Sans".to_owned(), size: Auto::default(), weight: Weight::Bold }
    }
}

#[derive(Clone, Default, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Identify popup colors
pub struct Colors {
    #[table]
    pub(crate) light: LightColors,
    #[table]
    pub(crate) dark: DarkColors,
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Identify popup light theme colors
pub struct LightColors {
    /// Text color
    pub text: Color,
    /// Background color
    pub background: Color,
}

impl Default for LightColors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#000").unwrap_or_default(),
            background: Color::from_str("#f6f5f4").unwrap_or_default(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Identify popup dark theme colors
pub struct DarkColors {
    /// Text color
    pub text: Color,
    /// Background color
    pub background: Color,
}

impl Default for DarkColors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#fff").unwrap_or_default(),
            background: Color::from_str("#3c3c3c").unwrap_or_default(),
        }
    }
}
