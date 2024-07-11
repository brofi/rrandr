mod imp;

use gio::{ActionGroup, ActionMap};
use glib::object::IsA;
use glib::{closure_local, wrapper, Object, ValueDelegate};
use gtk::prelude::ObjectExt;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{
    gio, glib, Accessible, Application, ApplicationWindow, Buildable, Button, ConstraintTarget,
    Native, Root, ShortcutManager, Widget,
};

use crate::data::outputs::Outputs;

pub const PADDING: u16 = 12;
pub const SPACING: u16 = 6;

#[derive(Clone, Copy)]
pub enum Axis {
    X,
    Y,
}

#[derive(ValueDelegate, Clone, Copy)]
#[value_delegate(from = u8)]
pub enum Action {
    Keep,
    Revert,
}

impl From<u8> for Action {
    fn from(v: u8) -> Self {
        match v {
            0 => Action::Keep,
            1 => Action::Revert,
            x => panic!("Not an action value: {x}"),
        }
    }
}

impl<'a> From<&'a Action> for u8 {
    fn from(v: &'a Action) -> Self { *v as u8 }
}

impl From<Action> for u8 {
    fn from(v: Action) -> Self { v as u8 }
}

wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends ApplicationWindow, gtk::Window, Widget,
        @implements ActionGroup, ActionMap, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

impl Window {
    pub fn new(app: &impl IsA<Application>) -> Self {
        Object::builder().property("application", app).build()
    }

    pub fn set_screen_max_size(&self, width: u16, height: u16) {
        self.imp().enabled_area.set_screen_max_width(u32::from(width));
        self.imp().enabled_area.set_screen_max_height(u32::from(height));
        self.imp().details.set_screen_max_width(u32::from(width));
        self.imp().details.set_screen_max_height(u32::from(height));
    }

    pub fn set_outputs(&self, outputs: &Outputs) {
        self.imp().enabled_area.set_outputs(Outputs::new_from(outputs, true));
        self.imp().disabled_area.set_outputs(Outputs::new_from(outputs, false));
    }

    pub fn connect_apply(&self, callback: impl Fn(&Self, &Button, &Outputs) -> bool + 'static) {
        self.connect_closure(
            "apply",
            false,
            closure_local!(|window, btn, outputs| callback(window, btn, outputs)),
        );
    }

    pub fn connect_confirm_action(&self, callback: impl Fn(&Self, Action) + 'static) {
        self.connect_closure(
            "confirm-action",
            false,
            closure_local!(|window, action| callback(window, action)),
        );
    }

    pub fn connect_reset(&self, callback: impl Fn(&Self, &Button) + 'static) {
        self.connect_closure("reset", false, closure_local!(|window, btn| callback(window, btn)));
    }

    pub fn connect_identify(&self, callback: impl Fn(&Self, &Button) + 'static) {
        self.connect_closure(
            "identify",
            false,
            closure_local!(|window, btn| callback(window, btn)),
        );
    }
}

impl Default for Window {
    fn default() -> Self { Object::new() }
}
