use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use glib::{home_dir, user_config_dir};
use gtk::glib;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::app::APP_NAME;
use crate::color::Color;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub colors: Colors,
}

impl Config {
    pub fn new() -> Self {
        if let Some(cfg) = Self::find_config() {
            if let Ok(cfg) = Self::parse_config(cfg) {
                return cfg;
            }
            warn!("Failed to parse config");
        }
        info!("Using default config");
        Config::default()
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
pub struct Colors {
    pub text: Color,
    pub output: Color,
    pub bounds: Color,
    pub selection: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            text: Color::from_str("#000").unwrap_or_default(),
            // text: Color::from_str("#fff").unwrap_or_default(),
            output: Color::from_str("#f6f5f4").unwrap_or_default(),
            // output: Color::from_str("#3c3c3c").unwrap_or_default(),
            bounds: Color::from_str("#f6f5f4").unwrap_or_default(),
            // bounds: Color::from_str("#3c3c3c").unwrap_or_default(),
            selection: Color::from_str("#1b68c6").unwrap_or_default(),
            // selection: Color::from_str("#3584e4").unwrap_or_default(),
        }
    }
}
