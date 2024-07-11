mod imp;

use core::fmt;

use glib::{wrapper, Object};
use gtk::glib;
use x11rb::protocol::randr::{Mode as ModeId, ModeFlag, ModeInfo};

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

    pub fn new_from(mode: &Mode) -> Mode {
        Self::new(mode.id(), mode.width() as u16, mode.height() as u16, mode.refresh())
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

fn get_refresh_rate(mode_info: &ModeInfo) -> f64 {
    let mut vtotal = mode_info.vtotal;

    if mode_info.mode_flags.contains(ModeFlag::DOUBLE_SCAN) {
        vtotal *= 2;
    }
    if mode_info.mode_flags.contains(ModeFlag::INTERLACE) {
        vtotal /= 2;
    }

    if mode_info.htotal > 0 && vtotal > 0 {
        f64::from(mode_info.dot_clock) / (f64::from(mode_info.htotal) * f64::from(vtotal))
    } else {
        0.0
    }
}
