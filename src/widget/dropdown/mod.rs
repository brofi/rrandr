mod imp;

use gdk::gio::ListModel;
use gdk::glib::object::IsA;
use gdk::glib::subclass::types::ObjectSubclassIsExt;
use gdk::glib::wrapper;
use gdk::prelude::ObjectExt;
use glib::Object;
use gtk::{glib, Widget};

wrapper! {
    pub struct DropDown(ObjectSubclass<imp::DropDown>) @extends Widget;
}

impl DropDown {
    pub fn new(tooltip: &str) -> Self {
        Object::builder().property("tooltip-text", tooltip).build()
    }

    pub fn connect_selected_item_notify(&self, f: impl Fn(&gtk::DropDown) + 'static) {
        let imp = self.imp();
        *imp.selected_item_notify_handler_id.borrow_mut() =
            Some(imp.widget.connect_selected_item_notify(f));
    }

    pub fn set_model(&self, model: Option<&impl IsA<ListModel>>) {
        let imp = self.imp();
        if let Some(handler_id) = imp.selected_item_notify_handler_id.borrow().as_ref() {
            imp.widget.block_signal(handler_id);
            imp.widget.set_model(model);
            imp.widget.unblock_signal(handler_id);
        }
    }

    pub fn selected(&self) -> u32 { self.imp().widget.selected() }

    pub fn set_selected(&self, position: u32) {
        let imp = self.imp();
        if let Some(handler_id) = imp.selected_item_notify_handler_id.borrow().as_ref() {
            imp.widget.block_signal(handler_id);
            imp.widget.set_selected(position);
            imp.widget.unblock_signal(handler_id);
        }
    }
}
