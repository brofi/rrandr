use std::cell::RefCell;

use gdk::gio::ListModel;
use gdk::glib::subclass::object::ObjectImpl;
use gdk::glib::subclass::types::ObjectSubclass;
use gdk::glib::{object_subclass, Object, Type};
use gdk::prelude::{Cast, StaticType};
use gdk::subclass::prelude::ListModelImpl;
use gtk::glib;

use crate::data::output::Output;

#[derive(Default)]
pub struct Outputs(pub(super) RefCell<Vec<Output>>);

#[object_subclass]
impl ObjectSubclass for Outputs {
    type Interfaces = (ListModel,);
    type Type = super::Outputs;

    const NAME: &'static str = "Outputs";
}

impl ObjectImpl for Outputs {}

impl ListModelImpl for Outputs {
    fn item_type(&self) -> Type { Output::static_type() }

    fn n_items(&self) -> u32 { self.0.borrow().len() as u32 }

    fn item(&self, position: u32) -> Option<Object> {
        self.0.borrow().get(position as usize).map(|o| o.clone().upcast::<Object>())
    }
}
