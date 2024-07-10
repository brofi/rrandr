mod imp;

use core::fmt;

use glib::{wrapper, Object};
use gtk::glib;
use x11rb::protocol::randr::{Mode as ModeId, ModeInfo};

use crate::get_refresh_rate;

wrapper! {
    pub struct Mode(ObjectSubclass<imp::Mode>);
}

impl Mode {
    pub fn new(id: ModeId, width: u16, height: u16, refresh: f64) -> Mode {
        Object::builder()
            .property("id", id)
            .property("width", u32::from(width))
            .property("height", u32::from(height))
            .property("refresh", refresh)
            .build()
    }
}

impl From<ModeInfo> for Mode {
    fn from(mode_info: ModeInfo) -> Self {
        Self::new(mode_info.id, mode_info.width, mode_info.height, get_refresh_rate(&mode_info))
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}_{:.2}", self.width(), self.height(), self.refresh())
    }
}
