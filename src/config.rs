use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use glib::{home_dir, user_config_dir};
use gtk::{glib, Settings};
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::app::APP_NAME;
use crate::color::Color;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub snap_strength: Option<f64>,
    pub pos_move_dist: i16,
    pub font: Font,
    pub colors: Colors,
    pub popup: Popup,
    #[serde(skip)]
    settings: Option<Settings>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            snap_strength: None,
            pos_move_dist: 10,
            font: Font::default(),
            colors: Colors::default(),
            popup: Popup::default(),
            settings: None,
        }
    }
}

macro_rules! color {
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
    color!(text_color, text);

    color!(output_color, output);

    color!(bounds_color, bounds);

    color!(selection_color, selection);

    pub fn new(settings: Option<Settings>) -> Self {
        let mut config = Config::default();
        if let Some(cfg) = Self::find_config() {
            if let Ok(cfg) = Self::parse_config(cfg) {
                config = cfg;
            } else {
                warn!("Failed to parse config - using default");
            }
        } else {
            info!("No config found - using default");
        }
        config.settings = settings;
        config
    }

    fn parse_config(cfg: PathBuf) -> Result<Config, Box<dyn Error>> {
        Ok(toml::from_str(&fs::read_to_string(cfg)?)?)
    }

    fn find_config() -> Option<PathBuf> {
        let cfg = format!("{APP_NAME}.toml");
        let cfgs = [
            user_config_dir().join(APP_NAME).join(&cfg),
            user_config_dir().join(&cfg),
            home_dir().join(&cfg),
        ];
        cfgs.iter().find(|&cfg| cfg.exists()).cloned()
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Font {
    pub family: String,
    pub size: u16,
}

impl Default for Font {
    fn default() -> Self { Self { family: "monospace".to_owned(), size: 12 } }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Colors {
    light: LightColors,
    dark: DarkColors,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LightColors {
    pub text: Color,
    pub output: Color,
    pub bounds: Color,
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DarkColors {
    pub text: Color,
    pub output: Color,
    pub bounds: Color,
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Popup {
    pub padding: u16,
    pub spacing: u16,
    pub ratio: f64,
    pub show_secs: f32,
    pub font: PopupFont,
}

impl Default for Popup {
    fn default() -> Self {
        Self { padding: 5, spacing: 10, ratio: 1. / 8., show_secs: 2.5, font: PopupFont::default() }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PopupFont {
    pub family: String,
    pub size: Option<u16>,
}

impl Default for PopupFont {
    fn default() -> Self { Self { family: "Sans".to_owned(), size: None } }
}
