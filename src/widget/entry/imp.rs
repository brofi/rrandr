use std::cell::RefCell;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::object::{ObjectImpl, ObjectImplExt};
use gdk::glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use gdk::glib::{derived_properties, object_subclass, GString, Properties, SignalHandlerId};
use gdk::subclass::prelude::DerivedObjectProperties;
use gtk::prelude::{EditableExt, EntryExt, WidgetExt};
use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
use gtk::{glib, BinLayout, InputPurpose, Widget};

#[derive(Default, Properties)]
#[properties(wrapper_type = super::Entry)]
pub struct Entry {
    #[property(name = "placeholder-text", get = Self::placeholder_text, set = Self::set_placeholder_text, type = GString)]
    pub(super) widget: RefCell<Option<gtk::Entry>>,
    pub(super) insert_text_handler_id: RefCell<Option<SignalHandlerId>>,
    pub(super) delete_text_handler_id: RefCell<Option<SignalHandlerId>>,
}

#[object_subclass]
impl ObjectSubclass for Entry {
    type ParentType = Widget;
    type Type = super::Entry;

    const NAME: &'static str = "RrrEntry";

    fn class_init(klass: &mut Self::Class) { klass.set_layout_manager_type::<BinLayout>(); }
}

#[derived_properties]
impl ObjectImpl for Entry {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();

        let entry = gtk::Entry::builder()
            .input_purpose(InputPurpose::Digits)
            .text("0")
            .max_length(6)
            .width_chars(5)
            .max_width_chars(5)
            .build();
        EntryExt::set_alignment(&entry, 1.);

        obj.bind_property("tooltip-text", &entry, "tooltip-text").sync_create().build();
        obj.bind_property("placeholder-text", &entry, "placeholder-text").build();

        entry.set_parent(&*obj);
        *self.widget.borrow_mut() = Some(entry);
    }

    fn dispose(&self) {
        if let Some(entry) = self.widget.borrow_mut().take() {
            entry.unparent();
        }
    }
}

impl WidgetImpl for Entry {}

impl Entry {
    fn placeholder_text(&self) -> GString {
        self.widget.borrow().as_ref().map_or("".into(), |entry| entry.text())
    }

    fn set_placeholder_text(&self, text: &str) {
        if let Some(entry) = self.widget.borrow().as_ref() {
            entry.set_text(text);
        }
    }
}
