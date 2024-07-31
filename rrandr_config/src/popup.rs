use serde::{Deserialize, Serialize};

use crate::auto::Auto;
use crate::font::Weight;
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
    pub font: PopupFont,
}

impl Default for Popup {
    fn default() -> Self {
        Self { padding: 5, spacing: 10, ratio: 1. / 8., show_secs: 2.5, font: PopupFont::default() }
    }
}

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Identify popup font configuration
pub struct PopupFont {
    /// Font family
    pub family: String,
    /// Font size in pt or "auto"
    pub size: Auto<u16>,
    /// Font weight
    pub weight: Weight,
}

impl Default for PopupFont {
    fn default() -> Self {
        Self { family: "Sans".to_owned(), size: Auto::default(), weight: Weight::Bold }
    }
}
