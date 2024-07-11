use glib::object::ObjectExt;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, GString, Object, SignalHandlerId};
use gtk::prelude::{EditableExt, EditableExtManual};
use gtk::{glib, Editable, Widget};

use crate::window::Axis;

mod imp {
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
}

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
