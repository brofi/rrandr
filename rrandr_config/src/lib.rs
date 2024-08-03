pub mod data;
pub mod display;
pub mod popup;

use std::fs;
use std::path::PathBuf;

use display::Display;
use glib::{home_dir, user_config_dir};
use gtk::{glib, Settings};
use log::{info, warn};
use popup::Popup;
use rrandr_config_derive::MarkdownTable;
use serde::{Deserialize, Serialize};

use crate::data::color::Color;

#[derive(Clone, Deserialize, Serialize, MarkdownTable)]
#[serde(default)]
/// Root level configuration
pub struct Config {
    /// Show an additional xrandr command for the current configuration
    pub show_xrandr: bool,
    /// Time in seconds until applied changes are being reverted
    pub revert_timeout: u8,
    #[table]
    pub display: Display,
    #[table]
    pub popup: Popup,
    #[serde(skip)]
    settings: Option<Settings>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_xrandr: false,
            revert_timeout: 15,
            display: Display::default(),
            popup: Popup::default(),
            settings: None,
        }
    }
}

macro_rules! impl_color {
    ($table:ident, $fn:ident, $attr:ident) => {
        pub fn $fn(&self) -> Color {
            self.settings.as_ref().map_or(self.$table.colors.dark.$attr.clone(), |s| {
                if s.is_gtk_application_prefer_dark_theme() {
                    self.$table.colors.dark.$attr.clone()
                } else {
                    self.$table.colors.light.$attr.clone()
                }
            })
        }
    };
}

impl Config {
    impl_color!(display, display_text_color, text);

    impl_color!(display, display_output_color, output);

    impl_color!(display, display_screen_color, screen);

    impl_color!(display, display_selection_color, selection);

    impl_color!(popup, popup_text_color, text);

    impl_color!(popup, popup_background_color, background);

    impl_color!(popup, popup_border_color, border);

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
