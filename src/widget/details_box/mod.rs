pub(crate) mod imp;

use glib::{closure_local, wrapper, Object, ValueDelegate};
use gtk::prelude::ObjectExt;
use gtk::{glib, Widget};

use crate::Output;

wrapper! {
    pub struct DetailsBox(ObjectSubclass<imp::DetailsBox>) @extends Widget;
}

impl DetailsBox {
    pub fn new(screen_max_width: u16, screen_max_height: u16) -> Self {
        Object::builder()
            .property("screen-max-width", u32::from(screen_max_width))
            .property("screen-max-height", u32::from(screen_max_height))
            .build()
    }

    pub fn connect_output_changed(&self, callback: impl Fn(&Self, &Output, Update) + 'static) {
        self.connect_closure(
            "output-changed",
            false,
            closure_local!(|details, output, update| callback(details, output, update)),
        );
    }
}

#[derive(ValueDelegate, Clone, Copy)]
#[value_delegate(from = u8)]
pub enum Update {
    Enabled,
    Disabled,
    Resolution,
    Refresh,
    Position,
    Primary,
}

impl From<u8> for Update {
    fn from(v: u8) -> Self {
        match v {
            0 => Update::Enabled,
            1 => Update::Disabled,
            2 => Update::Resolution,
            3 => Update::Refresh,
            4 => Update::Position,
            5 => Update::Primary,
            x => panic!("Not an update value: {x}"),
        }
    }
}

impl<'a> From<&'a Update> for u8 {
    fn from(v: &'a Update) -> Self { *v as u8 }
}

impl From<Update> for u8 {
    fn from(v: Update) -> Self { v as u8 }
}
