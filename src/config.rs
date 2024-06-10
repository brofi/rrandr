use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::color::Color;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Config {
    pub colors: Colors,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Colors {
    pub text: Color,
    pub output: Color,
    pub bounds: Color,
    pub selection: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            // light
            // text: Color::from_str("#ebdbb2").unwrap_or_default(),
            // output: Color::from_str("#282828").unwrap_or_default(),
            // bounds: Color::from_str("#282828").unwrap_or_default(),
            // selection: Color::from_str("#79740e").unwrap_or_default(),
            // dark
            text: Color::from_str("#3c3836").unwrap_or_default(),
            output: Color::from_str("#fbf1c7").unwrap_or_default(),
            bounds: Color::from_str("#fbf1c7").unwrap_or_default(),
            selection: Color::from_str("#b8bb27").unwrap_or_default(),
        }
    }
}
