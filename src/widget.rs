use std::cell::RefCell;
use std::rc::Rc;

use gdk::gio::ListModel;
use gdk::glib::SignalHandlerId;
use gtk::prelude::*;

#[derive(Clone)]
pub struct DropDown {
    pub widget: gtk::DropDown,
    selected_item_notify_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl DropDown {
    pub fn new(widget: gtk::DropDown) -> Self {
        Self { widget, selected_item_notify_handler_id: Rc::new(RefCell::new(None)) }
    }

    pub fn connect_selected_item_notify(&mut self, f: impl Fn(&Self) + 'static) {
        *self.selected_item_notify_handler_id.borrow_mut() =
            Some(self.widget.connect_selected_item_notify({
                let widget = self.clone();
                move |_| f(&widget)
            }));
    }

    pub fn set_model(&self, model: Option<&impl IsA<ListModel>>) {
        if let Some(handler_id) = self.selected_item_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_model(model);
            self.widget.unblock_signal(handler_id);
        }
    }

    pub fn set_selected(&self, position: u32) {
        if let Some(handler_id) = self.selected_item_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_selected(position);
            self.widget.unblock_signal(handler_id);
        }
    }
}

#[derive(Clone)]
pub struct Switch {
    pub widget: gtk::Switch,
    active_notify_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl Switch {
    pub fn new(widget: gtk::Switch) -> Self {
        Self { widget, active_notify_handler_id: Rc::new(RefCell::new(None)) }
    }

    pub fn connect_active_notify(&mut self, f: impl Fn(&Self) + 'static) {
        *self.active_notify_handler_id.borrow_mut() = Some(self.widget.connect_active_notify({
            let widget = self.clone();
            move |_| f(&widget)
        }));
    }

    pub fn set_active(&self, is_active: bool) {
        if let Some(handler_id) = self.active_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_active(is_active);
            self.widget.unblock_signal(handler_id);
        }
    }
}

#[derive(Clone)]
pub struct CheckButton {
    pub widget: gtk::CheckButton,
    active_notify_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl CheckButton {
    pub fn new(widget: gtk::CheckButton) -> Self {
        Self { widget, active_notify_handler_id: Rc::new(RefCell::new(None)) }
    }

    pub fn connect_active_notify(&mut self, f: impl Fn(&Self) + 'static) {
        *self.active_notify_handler_id.borrow_mut() = Some(self.widget.connect_active_notify({
            let widget = self.clone();
            move |_| f(&widget)
        }));
    }

    pub fn set_active(&self, is_active: bool) {
        if let Some(handler_id) = self.active_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_active(is_active);
            self.widget.unblock_signal(handler_id);
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    pub widget: gtk::Entry,
    insert_text_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
    delete_text_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl Entry {
    pub fn new(widget: gtk::Entry) -> Self {
        Self {
            widget,
            insert_text_handler_id: Rc::new(RefCell::new(None)),
            delete_text_handler_id: Rc::new(RefCell::new(None)),
        }
    }

    pub fn connect_insert_text(&mut self, f: impl Fn(&Self, &str, &mut i32) + 'static) {
        if let Some(editable) = self.widget.delegate() {
            *self.insert_text_handler_id.borrow_mut() = Some(editable.connect_insert_text({
                let entry = self.clone();
                move |editable, text, position| {
                    f(&entry, text, position);
                    editable.stop_signal_emission_by_name("insert_text");
                }
            }));
        }
    }

    pub fn connect_delete_text(&mut self, f: impl Fn(&Self, i32, i32) + 'static) {
        if let Some(editable) = self.widget.delegate() {
            *self.delete_text_handler_id.borrow_mut() = Some(editable.connect_delete_text({
                let entry = self.clone();
                move |editable, start, end| {
                    f(&entry, start, end);
                    editable.stop_signal_emission_by_name("delete_text");
                }
            }));
        }
    }

    pub fn set_text(&self, text: &str) {
        if self.insert_text_handler_id.borrow().is_some()
            && self.delete_text_handler_id.borrow().is_some()
        {
            self.delete_text(0, -1);
            self.insert_text(text, &mut 0);
        }
    }

    pub fn insert_text(&self, text: &str, position: &mut i32) {
        if let Some(handler_id) = self.insert_text_handler_id.borrow().as_ref() {
            if let Some(editable) = self.widget.delegate() {
                editable.block_signal(handler_id);
                self.widget.insert_text(text, position);
                editable.unblock_signal(handler_id);
            }
        }
    }

    pub fn delete_text(&self, start_pos: i32, end_pos: i32) {
        if let Some(handler_id) = self.delete_text_handler_id.borrow().as_ref() {
            if let Some(editable) = self.widget.delegate() {
                editable.block_signal(handler_id);
                self.widget.delete_text(start_pos, end_pos);
                editable.unblock_signal(handler_id);
            }
        }
    }
}
