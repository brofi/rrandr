use std::cell::RefCell;

use gio::ListModel;
use glib::subclass::object::ObjectImpl;
use glib::subclass::types::ObjectSubclass;
use glib::{object_subclass, Object, Type};
use gtk::prelude::{Cast, StaticType};
use gtk::subclass::prelude::ListModelImpl;
use gtk::{gio, glib};

use crate::data::mode::Mode;

#[derive(Default)]
pub struct Modes(pub(super) RefCell<Vec<Mode>>);

#[object_subclass]
impl ObjectSubclass for Modes {
    type Interfaces = (ListModel,);
    type Type = super::Modes;

    const NAME: &'static str = "Modes";
}

impl ObjectImpl for Modes {}

impl ListModelImpl for Modes {
    fn item_type(&self) -> Type { Mode::static_type() }

    fn n_items(&self) -> u32 { self.0.borrow().len() as u32 }

    fn item(&self, position: u32) -> Option<Object> {
        self.0.borrow().get(position as usize).map(|o| o.clone().upcast::<Object>())
    }
}
