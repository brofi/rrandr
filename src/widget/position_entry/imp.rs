use std::cell::RefCell;

use glib::subclass::object::{ObjectImpl, ObjectImplExt};
use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use glib::{object_subclass, SignalHandlerId};
use gtk::prelude::{BoxExt, EditableExt, EntryExt, WidgetExt};
use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
use gtk::{glib, BinLayout, Box, Entry, InputPurpose, Orientation, Widget};

pub struct PositionEntry {
    pub(super) position_x: Entry,
    pub(super) insert_x_handler_id: RefCell<Option<SignalHandlerId>>,
    pub(super) delete_x_handler_id: RefCell<Option<SignalHandlerId>>,
    pub(super) position_y: Entry,
    pub(super) insert_y_handler_id: RefCell<Option<SignalHandlerId>>,
    pub(super) delete_y_handler_id: RefCell<Option<SignalHandlerId>>,
}

impl Default for PositionEntry {
    fn default() -> Self {
        Self {
            position_x: create_entry("Horizontal position", "x"),
            insert_x_handler_id: RefCell::default(),
            delete_x_handler_id: RefCell::default(),
            position_y: create_entry("Vertical position", "y"),
            insert_y_handler_id: RefCell::default(),
            delete_y_handler_id: RefCell::default(),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for PositionEntry {
    type ParentType = Widget;
    type Type = super::PositionEntry;

    const NAME: &'static str = "RrrPositionEntry";

    fn class_init(klass: &mut Self::Class) { klass.set_layout_manager_type::<BinLayout>(); }
}

impl ObjectImpl for PositionEntry {
    fn constructed(&self) {
        self.parent_constructed();
        let linkbox =
            Box::builder().orientation(Orientation::Horizontal).css_classes(["linked"]).build();
        linkbox.append(&self.position_x);
        linkbox.append(&self.position_y);
        linkbox.set_parent(&*self.obj());
    }

    fn dispose(&self) { self.obj().first_child().unwrap().unparent(); }
}

impl WidgetImpl for PositionEntry {
    fn mnemonic_activate(&self, _: bool) -> bool {
        let (Some(pos_x_editable), Some(pos_y_editable)) =
            (self.position_x.delegate(), self.position_y.delegate())
        else {
            return false;
        };
        if pos_x_editable.has_focus() {
            self.position_y.grab_focus();
        } else if pos_y_editable.has_focus() {
            self.position_x.grab_focus();
        } else {
            self.position_x.grab_focus();
        }
        true
    }
}

fn create_entry(tooltip: &str, placeholder: &str) -> Entry {
    let entry = Entry::builder()
        .tooltip_text(tooltip)
        .placeholder_text(placeholder)
        .input_purpose(InputPurpose::Digits)
        .text("0")
        .max_length(6)
        .width_chars(5)
        .max_width_chars(5)
        .build();
    EntryExt::set_alignment(&entry, 1.);
    entry
}
