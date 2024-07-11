use std::cell::{Cell, RefCell};

use glib::subclass::object::ObjectImpl;
use glib::subclass::types::ObjectSubclass;
use glib::{derived_properties, object_subclass, Properties};
use gtk::glib;
use gtk::prelude::ObjectExt;
use gtk::subclass::prelude::DerivedObjectProperties;
use x11rb::protocol::randr::Output as OutputId;

use crate::data::mode::Mode;
use crate::data::modes::Modes;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::Output)]
pub struct Output {
    #[property(get, set)]
    id: Cell<OutputId>,
    #[property(get, set)]
    name: RefCell<String>,
    #[property(get, set, nullable)]
    product_name: RefCell<Option<String>>,
    #[property(get, set)]
    enabled: Cell<bool>,
    #[property(get, set)]
    primary: Cell<bool>,
    #[property(get, set, maximum = i16::MAX.into())]
    pos_y: Cell<i32>,
    #[property(get, set, maximum = i16::MAX.into())]
    pos_x: Cell<i32>,
    #[property(get, set, nullable)]
    mode: RefCell<Option<Mode>>,
    #[property(get, set)]
    modes: RefCell<Modes>,
    #[property(get, set)]
    width: Cell<u32>,
    #[property(get, set)]
    height: Cell<u32>,
}

#[object_subclass]
impl ObjectSubclass for Output {
    type Type = super::Output;

    const NAME: &'static str = "Output";
}

#[derived_properties]
impl ObjectImpl for Output {}
