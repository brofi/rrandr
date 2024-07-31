use std::fmt;
use std::str::FromStr;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::MarkdownTable;

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

#[derive(Clone, Default)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self { Color { r, g, b } }

    pub fn to_rgba(&self, alpha: f32) -> gdk::RGBA {
        let mut rgba: gdk::RGBA = self.clone().into();
        rgba.set_alpha(alpha);
        rgba
    }
}

impl From<Color> for gdk::RGBA {
    fn from(color: Color) -> Self {
        gdk::RGBA::new(
            f32::from(color.r) / f32::from(u8::MAX),
            f32::from(color.g) / f32::from(u8::MAX),
            f32::from(color.b) / f32::from(u8::MAX),
            1.,
        )
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

pub struct ParseRgbError;

impl FromStr for Color {
    type Err = ParseRgbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix('#').ok_or(ParseRgbError)?;
        if s.len() != s.chars().filter(char::is_ascii_hexdigit).count()
            || (s.len() != 3 && s.len() != 6)
        {
            return Err(ParseRgbError);
        }

        let idx = s.len() / 3;
        let (r, gb) = s.split_at(idx);
        let (g, b) = gb.split_at(idx);
        let [r, g, b] = [r, g, b].map(|s| if idx == 1 { s.repeat(2) } else { s.to_owned() });

        #[allow(clippy::items_after_statements)] // item only and immediately used after
        fn from_hex(s: &str) -> Result<u8, ParseRgbError> {
            u8::from_str_radix(s, 16).map_err(|_| ParseRgbError)
        }
        Ok(Color::new(from_hex(&r)?, from_hex(&g)?, from_hex(&b)?))
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(RgbVisitor)
    }
}

struct RgbVisitor;

impl<'de> Visitor<'de> for RgbVisitor {
    type Value = Color;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("`#rgb` or `#rrggbb`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Color, E> {
        Color::from_str(value).map_err(|_| de::Error::invalid_value(Unexpected::Str(value), &self))
    }
}
