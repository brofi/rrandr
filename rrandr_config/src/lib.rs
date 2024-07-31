pub mod auto;
pub mod color;
pub mod font;
pub mod popup;

use std::fs;
use std::path::PathBuf;

use auto::Auto;
use color::Colors;
use font::Font;
use glib::{home_dir, user_config_dir};
use gtk::{glib, Settings};
use log::{info, warn};
use popup::Popup;
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

pub trait MarkdownTable {
    fn to_markdown_table(key: &str, lvl: u8) -> String;
}
