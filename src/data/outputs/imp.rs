use std::cell::RefCell;

use gio::ListModel;
use glib::subclass::object::ObjectImpl;
use glib::subclass::types::ObjectSubclass;
use glib::{object_subclass, Object, Type};
use gtk::prelude::{Cast, StaticType};
use gtk::subclass::prelude::ListModelImpl;
use gtk::{gio, glib};

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
