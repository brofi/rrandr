use std::fmt;

use gtk::pango;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weight {
    Thin,
    Ultralight,
    Light,
    Semilight,
    Book,
    Normal,
    Medium,
    Semibold,
    Bold,
    Ultrabold,
    Heavy,
    Ultraheavy,
}

impl From<Weight> for pango::Weight {
    fn from(value: Weight) -> Self {
        match value {
            Weight::Thin => Self::Thin,
            Weight::Ultralight => Self::Ultralight,
            Weight::Light => Self::Light,
            Weight::Semilight => Self::Semilight,
            Weight::Book => Self::Book,
            Weight::Normal => Self::Normal,
            Weight::Medium => Self::Medium,
            Weight::Semibold => Self::Semibold,
            Weight::Bold => Self::Bold,
            Weight::Ultrabold => Self::Ultrabold,
            Weight::Heavy => Self::Heavy,
            Weight::Ultraheavy => Self::Ultraheavy,
        }
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    Solid,
    Dotted,
    Dashed,
    None,
}

impl fmt::Display for BorderStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}
