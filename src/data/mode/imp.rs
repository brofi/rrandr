use std::cell::Cell;

use gdk::glib::subclass::object::ObjectImpl;
use gdk::glib::subclass::types::ObjectSubclass;
use gdk::glib::{derived_properties, object_subclass, Properties};
use gdk::prelude::ObjectExt;
use gdk::subclass::prelude::DerivedObjectProperties;
use gtk::glib;
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
