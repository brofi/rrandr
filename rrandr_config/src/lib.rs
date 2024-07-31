pub mod auto;
pub mod color;

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use auto::Auto;
use glib::{home_dir, user_config_dir};
use gtk::{glib, Settings};
use log::{info, warn};
use rrandr_config_derive::MarkdownTable;
use serde::{Deserialize, Serialize};

use crate::color::Color;

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Root level configuration
pub struct Config {
    /// Snapping strength when dragging outputs or `auto`. High values make it
    /// more "sticky", while 0 means no snapping. If left to default
    /// `snap_strength = min_size / 6` where `min_side` is the smallest side of
    /// any enabled output in px. E.g. when smallest screen resolution is Full
    /// HD => `snap_strength = 180`.
    pub snap_strength: Auto<f64>,
    /// Move distance when moving an output via keybindings
    pub pos_move_dist: i16,
    #[table]
    pub font: Font,
    #[table]
    pub colors: Colors,
    #[table]
    pub popup: Popup,
    #[serde(skip)]
    settings: Option<Settings>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            snap_strength: Auto::default(),
            pos_move_dist: 10,
            font: Font::default(),
            colors: Colors::default(),
            popup: Popup::default(),
            settings: None,
        }
    }
}

macro_rules! impl_color {
    ($name:ident, $attr:ident) => {
        pub fn $name(&self) -> Color {
            self.settings.as_ref().map_or(self.colors.dark.$attr.clone(), |s| {
                if s.is_gtk_application_prefer_dark_theme() {
                    self.colors.dark.$attr.clone()
                } else {
                    self.colors.light.$attr.clone()
                }
            })
        }
    };
}

impl Config {
    impl_color!(text_color, text);

    impl_color!(output_color, output);

    impl_color!(bounds_color, bounds);

    impl_color!(selection_color, selection);

    pub fn new(app_name: &str, settings: Option<Settings>) -> Self {
        let mut config = Config::default();
        if let Some(cfg) = Self::find_config(app_name) {
            if let Ok(cfg) = fs::read_to_string(cfg) {
                match toml::from_str(&cfg) {
                    Ok(cfg) => config = cfg,
                    Err(e) => warn!("Failed to parse config\n{e}"),
                }
            } else {
                warn!("Failed to read config");
            }
        } else {
            info!("No config found - using default");
        }
        config.settings = settings;
        config
    }

    fn find_config(app_name: &str) -> Option<PathBuf> {
        let cfg = format!("{app_name}.toml");
        let cfgs = [
            user_config_dir().join(app_name).join(&cfg),
            user_config_dir().join(&cfg),
            home_dir().join(&cfg),
        ];
        cfgs.iter().find(|&cfg| cfg.exists()).cloned()
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
}

impl Default for Font {
    fn default() -> Self { Self { family: "monospace".to_owned(), size: 12 } }
}

#[derive(Clone, Default, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Output area colors
pub struct Colors {
    #[table]
    light: LightColors,
    #[table]
    dark: DarkColors,
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
    pub bounds: Color,
    /// Output selection color
    pub selection: Color,
}

impl Default for LightColors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#fff").unwrap_or_default(),
            output: Color::from_str("#3c3c3c").unwrap_or_default(),
            bounds: Color::from_str("#3c3c3c").unwrap_or_default(),
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
    pub bounds: Color,
    /// Output selection color
    pub selection: Color,
}

impl Default for DarkColors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#000").unwrap_or_default(),
            output: Color::from_str("#f6f5f4").unwrap_or_default(),
            bounds: Color::from_str("#f6f5f4").unwrap_or_default(),
            selection: Color::from_str("#1b68c6").unwrap_or_default(),
        }
    }
}

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
}

impl Default for PopupFont {
    fn default() -> Self { Self { family: "Sans".to_owned(), size: Auto::default() } }
}

pub trait MarkdownTable {
    fn to_markdown_table(key: &str, lvl: u8) -> String;
}
