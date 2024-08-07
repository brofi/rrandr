use std::f64::consts::PI;

use glib::Enum;
use gtk::glib;
use x11rb::protocol::randr::Rotation as RRotation;

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Enum)]
#[enum_type(name = "Rotation")]
pub enum Rotation {
    #[default]
    Normal,
    Left,
    Right,
    Inverted,
}

impl Rotation {
    pub fn rad(&self) -> f64 {
        match *self {
            Self::Normal => 0.,
            Self::Left => 1.5 * PI,
            Self::Right => PI / 2.,
            Self::Inverted => PI,
        }
    }

    pub fn xrandr(&self) -> String { format!("{self:?}").to_lowercase() }
}

impl From<Rotation> for u32 {
    fn from(value: Rotation) -> Self { value as u32 }
}

impl From<u32> for Rotation {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::Left,
            2 => Self::Right,
            3 => Self::Inverted,
            x => panic!("Not a rotation value: {x}"),
        }
    }
}

impl From<RRotation> for Rotation {
    fn from(value: RRotation) -> Self {
        if value.contains(RRotation::ROTATE90) {
            Self::Left
        } else if value.contains(RRotation::ROTATE270) {
            Self::Right
        } else if value.contains(RRotation::ROTATE180) {
            Self::Inverted
        } else {
            Self::Normal
        }
    }
}

impl From<Rotation> for RRotation {
    fn from(value: Rotation) -> Self {
        match value {
            Rotation::Normal => RRotation::ROTATE0,
            Rotation::Left => RRotation::ROTATE90,
            Rotation::Right => RRotation::ROTATE270,
            Rotation::Inverted => RRotation::ROTATE180,
        }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Enum)]
#[enum_type(name = "Reflection")]
pub enum Reflection {
    #[default]
    Normal,
    Horizontal,
    Vertical,
    Both,
}

impl Reflection {
    pub fn xrandr(&self) -> String {
        match *self {
            Self::Normal => "normal",
            Self::Horizontal => "x",
            Self::Vertical => "y",
            Self::Both => "xy",
        }
        .to_owned()
    }
}

impl From<Reflection> for u32 {
    fn from(value: Reflection) -> Self { value as u32 }
}

impl From<u32> for Reflection {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::Horizontal,
            2 => Self::Vertical,
            3 => Self::Both,
            x => panic!("Not a reflection value: {x}"),
        }
    }
}

impl From<RRotation> for Reflection {
    fn from(value: RRotation) -> Self {
        if value.contains(RRotation::REFLECT_X | RRotation::REFLECT_Y) {
            Self::Both
        } else if value.contains(RRotation::REFLECT_X) {
            Self::Horizontal
        } else if value.contains(RRotation::REFLECT_Y) {
            Self::Vertical
        } else {
            Self::Normal
        }
    }
}

impl From<Reflection> for RRotation {
    fn from(value: Reflection) -> Self {
        match value {
            Reflection::Normal => 0_u8.into(),
            Reflection::Horizontal => RRotation::REFLECT_X,
            Reflection::Vertical => RRotation::REFLECT_Y,
            Reflection::Both => RRotation::REFLECT_X | RRotation::REFLECT_Y,
        }
    }
}
