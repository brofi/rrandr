use core::fmt;

use glib::{wrapper, Object};
use gtk::glib;
use x11rb::protocol::randr::{Mode as ModeId, ModeFlag, ModeInfo};

mod imp {
    use std::cell::Cell;

    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::ObjectSubclass;
    use glib::{derived_properties, object_subclass, Properties};
    use gtk::glib;
    use gtk::prelude::ObjectExt;
    use gtk::subclass::prelude::DerivedObjectProperties;
    use x11rb::protocol::randr::Mode as ModeId;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::Mode)]
    pub struct Mode {
        #[property(get, set)]
        id: Cell<ModeId>,
        #[property(get, set, maximum = u16::MAX.into())]
        width: Cell<u32>,
        #[property(get, set, maximum = u16::MAX.into())]
        height: Cell<u32>,
        #[property(get, set)]
        refresh: Cell<f64>,
    }

    #[object_subclass]
    impl ObjectSubclass for Mode {
        type Type = super::Mode;

        const NAME: &'static str = "Mode";
    }

    #[derived_properties]
    impl ObjectImpl for Mode {}
}

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
