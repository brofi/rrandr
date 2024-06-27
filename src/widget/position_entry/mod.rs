mod imp;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::types::ObjectSubclassIsExt;
use gdk::glib::{wrapper, GString, SignalHandlerId};
use glib::Object;
use gtk::prelude::{EditableExt, EditableExtManual};
use gtk::{glib, Editable, Widget};

use crate::view::Axis;

wrapper! {
    pub struct PositionEntry(ObjectSubclass<imp::PositionEntry>) @extends Widget;
}

impl PositionEntry {
    pub fn new() -> Self { Object::new() }

    pub fn connect_insert_x(&self, f: impl Fn(&Self, &str, &mut i32) + 'static) {
        let imp = self.imp();
        if let Some(editable) = imp.position_x.delegate() {
            *imp.insert_x_handler_id.borrow_mut() = Some(self.connect_insert_text(&editable, f));
        }
    }

    pub fn connect_insert_y(&self, f: impl Fn(&Self, &str, &mut i32) + 'static) {
        let imp = self.imp();
        if let Some(editable) = imp.position_y.delegate() {
            *imp.insert_y_handler_id.borrow_mut() = Some(self.connect_insert_text(&editable, f));
        }
    }

    pub fn connect_delete_x(&self, f: impl Fn(&Self, i32, i32) + 'static) {
        let imp = self.imp();
        if let Some(editable) = imp.position_x.delegate() {
            *imp.delete_x_handler_id.borrow_mut() = Some(self.connect_delete_text(&editable, f));
        }
    }

    pub fn connect_delete_y(&self, f: impl Fn(&Self, i32, i32) + 'static) {
        let imp = self.imp();
        if let Some(editable) = imp.position_y.delegate() {
            *imp.delete_y_handler_id.borrow_mut() = Some(self.connect_delete_text(&editable, f));
        }
    }

    fn connect_insert_text(
        &self,
        editable: &Editable,
        f: impl Fn(&Self, &str, &mut i32) + 'static,
    ) -> SignalHandlerId {
        editable.connect_insert_text({
            let entry = self.clone();
            move |editable, text, position| {
                f(&entry, text, position);
                editable.stop_signal_emission_by_name("insert_text");
            }
        })
    }

    fn connect_delete_text(
        &self,
        editable: &Editable,
        f: impl Fn(&Self, i32, i32) + 'static,
    ) -> SignalHandlerId {
        editable.connect_delete_text({
            let entry = self.clone();
            move |editable, start, end| {
                f(&entry, start, end);
                editable.stop_signal_emission_by_name("delete_text");
            }
        })
    }

    pub fn text(&self, axis: Axis) -> GString {
        let imp = self.imp();
        match axis {
            Axis::X => imp.position_x.text(),
            Axis::Y => imp.position_y.text(),
        }
    }

    pub fn set_x(&self, text: &str) { self.set_text(text, Axis::X); }

    pub fn set_y(&self, text: &str) { self.set_text(text, Axis::Y); }

    pub fn set_text(&self, text: &str, axis: Axis) {
        let imp = self.imp();
        if match axis {
            Axis::X => {
                imp.insert_x_handler_id.borrow().is_some()
                    && imp.delete_x_handler_id.borrow().is_some()
            }
            Axis::Y => {
                imp.insert_y_handler_id.borrow().is_some()
                    && imp.delete_y_handler_id.borrow().is_some()
            }
        } {
            self.delete_text(0, -1, axis);
            self.insert_text(text, &mut 0, axis);
        };
    }

    pub fn insert_text(&self, text: &str, position: &mut i32, axis: Axis) {
        let imp = self.imp();
        if let Some(handler_id) = match axis {
            Axis::X => imp.insert_x_handler_id.borrow(),
            Axis::Y => imp.insert_y_handler_id.borrow(),
        }
        .as_ref()
        {
            let entry = match axis {
                Axis::X => &imp.position_x,
                Axis::Y => &imp.position_y,
            };
            if let Some(editable) = entry.delegate() {
                editable.block_signal(handler_id);
                entry.insert_text(text, position);
                editable.unblock_signal(handler_id);
            }
        }
    }

    pub fn delete_text(&self, start_pos: i32, end_pos: i32, axis: Axis) {
        let imp = self.imp();
        if let Some(handler_id) = match axis {
            Axis::X => imp.delete_x_handler_id.borrow(),
            Axis::Y => imp.delete_y_handler_id.borrow(),
        }
        .as_ref()
        {
            let entry = match axis {
                Axis::X => &imp.position_x,
                Axis::Y => &imp.position_y,
            };
            if let Some(editable) = entry.delegate() {
                editable.block_signal(handler_id);
                entry.delete_text(start_pos, end_pos);
                editable.unblock_signal(handler_id);
            }
        }
    }
}
