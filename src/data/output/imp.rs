use std::cell::{Cell, RefCell};

use gdk::glib::subclass::object::ObjectImpl;
use gdk::glib::subclass::types::ObjectSubclass;
use gdk::glib::{derived_properties, object_subclass, Properties, ValueArray};
use gdk::prelude::ObjectExt;
use gdk::subclass::prelude::DerivedObjectProperties;
use gtk::glib;
use x11rb::protocol::randr::Output as OutputId;

use crate::data::mode::Mode;

#[derive(Properties)]
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
    modes: RefCell<ValueArray>,
    #[property(get, set)]
    width: Cell<u32>,
    #[property(get, set)]
    height: Cell<u32>,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            id: Default::default(),
            name: Default::default(),
            product_name: Default::default(),
            enabled: Default::default(),
            primary: Default::default(),
            pos_y: Default::default(),
            pos_x: Default::default(),
            mode: Default::default(),
            modes: RefCell::new(ValueArray::new(0)),
            width: Default::default(),
            height: Default::default(),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for Output {
    type Type = super::Output;

    const NAME: &'static str = "Output";
}

#[derived_properties]
impl ObjectImpl for Output {}
