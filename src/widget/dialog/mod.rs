mod imp;

use glib::object::{IsA, ObjectBuilder, ObjectExt};
use glib::{closure_local, wrapper, Object};
use gtk::prelude::GtkWindowExt;
use gtk::{
    glib, Accessible, ApplicationWindow, Buildable, ConstraintTarget, Native, Root,
    ShortcutManager, StringList, Widget,
};

wrapper! {
    pub struct Dialog(ObjectSubclass<imp::Dialog>)
        @extends gtk::Window, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

impl Dialog {
    pub fn builder(window: &impl IsA<ApplicationWindow>) -> DialogBuilder {
        DialogBuilder::new(window)
    }

    pub fn show(&self) { self.present(); }

    pub fn connect_action(&self, callback: impl Fn(&Self, u32) + 'static) {
        self.connect_closure("action", false, closure_local!(|window, i| callback(window, i)));
    }
}

pub struct DialogBuilder {
    builder: ObjectBuilder<'static, Dialog>,
}

impl DialogBuilder {
    fn new(window: &impl IsA<ApplicationWindow>) -> Self {
        Self { builder: Object::builder().property("transient-for", window) }
    }

    pub fn title(self, title: &str) -> Self {
        Self { builder: self.builder.property("title", title) }
    }

    pub fn message(self, message: &str) -> Self {
        Self { builder: self.builder.property("message", message) }
    }

    pub fn heading(self, heading: &str) -> Self {
        Self { builder: self.builder.property("heading", heading) }
    }

    pub fn actions(self, actions: &[&str]) -> Self {
        Self { builder: self.builder.property("actions", StringList::new(actions)) }
    }

    pub fn tooltips(self, tooltips: &[&str]) -> Self {
        Self { builder: self.builder.property("tooltips", StringList::new(tooltips)) }
    }

    pub fn build(self) -> Dialog { self.builder.build() }
}
