mod imp;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::types::ObjectSubclassIsExt;
use gdk::glib::{wrapper, GString};
use glib::Object;
use gtk::prelude::{EditableExt, EditableExtManual};
use gtk::{glib, Widget};

wrapper! {
    pub struct Entry(ObjectSubclass<imp::Entry>) @extends Widget;
}

impl Entry {
    pub fn new(tooltip: &str, placeholder: &str) -> Self {
        Object::builder()
            .property("tooltip-text", tooltip)
            .property("placeholder-text", placeholder)
            .build()
    }

    pub fn connect_insert_text(&self, f: impl Fn(&Self, &str, &mut i32) + 'static) {
        let imp = self.imp();
        if let Some(editable) = imp.widget.borrow().as_ref().and_then(|w| w.delegate()) {
            *imp.insert_text_handler_id.borrow_mut() = Some(editable.connect_insert_text({
                let entry = self.clone();
                move |editable, text, position| {
                    f(&entry, text, position);
                    editable.stop_signal_emission_by_name("insert_text");
                }
            }));
        }
    }

    pub fn connect_delete_text(&mut self, f: impl Fn(&Self, i32, i32) + 'static) {
        let imp = self.imp();
        if let Some(editable) = imp.widget.borrow().as_ref().and_then(|w| w.delegate()) {
            *imp.delete_text_handler_id.borrow_mut() = Some(editable.connect_delete_text({
                let entry = self.clone();
                move |editable, start, end| {
                    f(&entry, start, end);
                    editable.stop_signal_emission_by_name("delete_text");
                }
            }));
        }
    }

    pub fn text(&self) -> GString {
        self.imp().widget.borrow().as_ref().map_or("".into(), |entry| entry.text())
    }

    pub fn set_text(&self, text: &str) {
        let imp = self.imp();
        if imp.insert_text_handler_id.borrow().is_some()
            && imp.delete_text_handler_id.borrow().is_some()
        {
            self.delete_text(0, -1);
            self.insert_text(text, &mut 0);
        }
    }

    pub fn insert_text(&self, text: &str, position: &mut i32) {
        let imp = self.imp();
        if let Some(handler_id) = imp.insert_text_handler_id.borrow().as_ref() {
            if let Some(entry) = imp.widget.borrow().as_ref() {
                if let Some(editable) = entry.delegate() {
                    editable.block_signal(handler_id);
                    entry.insert_text(text, position);
                    editable.unblock_signal(handler_id);
                }
            }
        }
    }

    pub fn delete_text(&self, start_pos: i32, end_pos: i32) {
        let imp = self.imp();
        if let Some(handler_id) = imp.delete_text_handler_id.borrow().as_ref() {
            if let Some(entry) = imp.widget.borrow().as_ref() {
                if let Some(editable) = entry.delegate() {
                    editable.block_signal(handler_id);
                    entry.delete_text(start_pos, end_pos);
                    editable.unblock_signal(handler_id);
                }
            }
        }
    }
}
