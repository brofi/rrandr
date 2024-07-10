mod imp;

use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::ObjectExt;
use gtk::{glib, Widget};

wrapper! {
    pub struct Switch(ObjectSubclass<imp::Switch>) @extends Widget;
}

impl Switch {
    pub fn new(tooltip: &str) -> Self {
        Object::builder().property("tooltip-text", tooltip).build()
    }

    pub fn connect_active_notify(&self, f: impl Fn(&gtk::Switch) + 'static) {
        let imp = self.imp();
        *imp.active_notify_handler_id.borrow_mut() = Some(imp.widget.connect_active_notify(f));
    }

    pub fn set_active(&self, is_active: bool) {
        let imp = self.imp();
        if let Some(handler_id) = imp.active_notify_handler_id.borrow().as_ref() {
            imp.widget.block_signal(handler_id);
            imp.widget.set_active(is_active);
            imp.widget.unblock_signal(handler_id);
        }
    }

    pub fn is_active(&self) -> bool { self.imp().widget.is_active() }
}
