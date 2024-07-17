use core::fmt;

use gettextrs::gettext;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::glib;
use x11rb::protocol::randr::{Mode as ModeId, ModeFlag, ModeInfo};

use super::values::U16;

mod imp {
    use std::cell::Cell;

    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::ObjectSubclass;
    use glib::{derived_properties, object_subclass, Properties};
    use gtk::glib;
    use gtk::prelude::ObjectExt;
    use gtk::subclass::prelude::DerivedObjectProperties;
    use x11rb::protocol::randr::Mode as ModeId;

    use crate::data::values::U16;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::Mode)]
    pub struct Mode {
        #[property(get, set, construct_only)]
        id: Cell<ModeId>,
        #[property(set, construct_only)]
        pub(super) width: Cell<U16>,
        #[property(set, construct_only)]
        pub(super) height: Cell<U16>,
        #[property(get, set, construct_only)]
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
            .property("width", U16::new(width))
            .property("height", U16::new(height))
            .property("refresh", refresh)
            .build()
    }

    pub fn width(&self) -> u16 { self.imp().width.get().get() }

    pub fn height(&self) -> u16 { self.imp().height.get().get() }

    pub fn as_resolution_str(&self, format_width: Option<usize>) -> String {
        let fw = format_width.unwrap_or_default();
        format!("{}\u{202F}x\u{202F}{:<fw$}", self.width(), self.height())
    }

    pub fn as_refresh_rate_str(self) -> String {
        format!("{:.2}\u{202F}{}", self.refresh(), gettext("Hz"))
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
