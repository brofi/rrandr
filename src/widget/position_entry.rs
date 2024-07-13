use glib::object::ObjectExt;
use glib::{closure_local, wrapper, Object, ValueDelegate};
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{glib, Widget};

#[derive(ValueDelegate, Clone, Copy)]
#[value_delegate(from = u8)]
pub enum Axis {
    X,
    Y,
}

impl From<u8> for Axis {
    fn from(v: u8) -> Self {
        match v {
            0 => Axis::X,
            1 => Axis::Y,
            x => panic!("Not an axis value: {x}"),
        }
    }
}

impl<'a> From<&'a Axis> for u8 {
    fn from(v: &'a Axis) -> Self { *v as u8 }
}

impl From<Axis> for u8 {
    fn from(v: Axis) -> Self { v as u8 }
}

impl From<usize> for Axis {
    fn from(v: usize) -> Self {
        match v {
            0 => Axis::X,
            1 => Axis::Y,
            x => panic!("Not an axis value: {x}"),
        }
    }
}

impl From<Axis> for usize {
    fn from(v: Axis) -> Self { v as usize }
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::num::IntErrorKind;
    use std::sync::OnceLock;

    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::Signal;
    use glib::types::StaticType;
    use glib::{clone, derived_properties, object_subclass, Properties, SignalHandlerId, Type};
    use gtk::prelude::{BoxExt, EditableExt, EditableExtManual, EntryExt, ObjectExt, WidgetExt};
    use gtk::subclass::prelude::DerivedObjectProperties;
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{glib, BinLayout, Box, Entry, InputPurpose, Orientation, Widget};

    use super::Axis;

    #[derive(Properties)]
    #[properties(wrapper_type = super::PositionEntry)]
    pub struct PositionEntry {
        #[property(set, maximum = i16::MAX.into())]
        max_x: Cell<i32>,
        #[property(set, maximum = i16::MAX.into())]
        max_y: Cell<i32>,
        entries: [Entry; 2],
        insert_handler_ids: RefCell<[Option<SignalHandlerId>; 2]>,
        delete_handler_ids: RefCell<[Option<SignalHandlerId>; 2]>,
    }

    impl Default for PositionEntry {
        fn default() -> Self {
            Self {
                max_x: Default::default(),
                max_y: Default::default(),
                entries: [
                    create_entry("Horizontal position", "x"),
                    create_entry("Vertical position", "y"),
                ],
                insert_handler_ids: Default::default(),
                delete_handler_ids: Default::default(),
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

    #[derived_properties]
    impl ObjectImpl for PositionEntry {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("coordinate-changed")
                        .param_types([Axis::static_type(), Type::I32])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let linkbox =
                Box::builder().orientation(Orientation::Horizontal).css_classes(["linked"]).build();
            linkbox.append(&self.entries[usize::from(Axis::X)]);
            linkbox.append(&self.entries[usize::from(Axis::Y)]);
            linkbox.set_parent(&*self.obj());

            for (i, entry) in self.entries.iter().enumerate() {
                if let Some(editable) = entry.delegate() {
                    let id = editable.connect_insert_text(clone!(
                        @weak self as this => move |editable, text, position| {
                            this.insert_coord(text, position, Axis::from(i));
                            editable.stop_signal_emission_by_name("insert_text");
                        }
                    ));
                    self.insert_handler_ids.borrow_mut()[i] = Some(id);
                    let id = editable.connect_delete_text(clone!(
                        @weak self as this => move |editable, start_pos, end_pos| {
                            this.delete_coord(start_pos, end_pos, Axis::from(i));
                            editable.stop_signal_emission_by_name("delete_text");
                        }
                    ));
                    self.delete_handler_ids.borrow_mut()[i] = Some(id);
                }
            }
        }

        fn dispose(&self) { self.obj().first_child().unwrap().unparent(); }
    }

    impl WidgetImpl for PositionEntry {
        fn mnemonic_activate(&self, _: bool) -> bool {
            let (Some(editable_x), Some(editable_y)) = (
                self.entries[usize::from(Axis::X)].delegate(),
                self.entries[usize::from(Axis::Y)].delegate(),
            ) else {
                return false;
            };
            if editable_x.has_focus() {
                self.entries[usize::from(Axis::Y)].grab_focus();
            } else if editable_y.has_focus() {
                self.entries[usize::from(Axis::X)].grab_focus();
            } else {
                self.entries[usize::from(Axis::X)].grab_focus();
            }
            true
        }
    }

    impl PositionEntry {
        pub(super) fn set_text(&self, text: &str, axis: Axis) {
            if self.insert_handler_ids.borrow()[usize::from(axis)].is_some()
                && self.delete_handler_ids.borrow()[usize::from(axis)].is_some()
            {
                self.delete_text(0, -1, axis);
                self.insert_text(text, &mut 0, axis);
            }
        }

        fn insert_text(&self, text: &str, position: &mut i32, axis: Axis) {
            if let Some(handler_id) = &self.insert_handler_ids.borrow()[usize::from(axis)] {
                if let Some(editable) = self.entries[usize::from(axis)].delegate() {
                    editable.block_signal(handler_id);
                    self.entries[usize::from(axis)].insert_text(text, position);
                    editable.unblock_signal(handler_id);
                }
            }
        }

        fn delete_text(&self, start_pos: i32, end_pos: i32, axis: Axis) {
            if let Some(handler_id) = &self.delete_handler_ids.borrow()[usize::from(axis)] {
                if let Some(editable) = self.entries[usize::from(axis)].delegate() {
                    editable.block_signal(handler_id);
                    self.entries[usize::from(axis)].delete_text(start_pos, end_pos);
                    editable.unblock_signal(handler_id);
                }
            }
        }

        fn insert_coord(&self, text: &str, position: &mut i32, axis: Axis) {
            let idx = usize::try_from(*position).expect("smaller position");
            let mut new_text = self.entries[usize::from(axis)].text().to_string();
            new_text.insert_str(idx, text);

            if let Some(coord) = self.parse_coord(&new_text, axis) {
                if coord.to_string() == new_text {
                    self.insert_text(text, position, axis);
                } else if coord.to_string() != self.entries[usize::from(axis)].text() {
                    self.entries[usize::from(axis)].set_text(&coord.to_string());
                }
                self.obj().emit_by_name::<()>("coordinate-changed", &[&axis, &i32::from(coord)]);
            } else if self.entries[usize::from(axis)].text().is_empty() {
                self.insert_text("0", &mut 0, axis);
            }
        }

        fn delete_coord(&self, start_pos: i32, end_pos: i32, axis: Axis) {
            let mut new_text = self.entries[usize::from(axis)].text().to_string();
            let start_idx = usize::try_from(start_pos).expect("smaller start position");
            let end_idx = if end_pos < 0 {
                new_text.len()
            } else {
                usize::try_from(end_pos).expect("smaller end position")
            };
            new_text.replace_range(start_idx..end_idx, "");

            if let Some(coord) = self.parse_coord(&new_text, axis) {
                if coord.to_string() == new_text {
                    self.delete_text(start_pos, end_pos, axis);
                } else {
                    self.entries[usize::from(axis)].set_text(&coord.to_string());
                }
                self.obj().emit_by_name::<()>("coordinate-changed", &[&axis, &i32::from(coord)]);
            } else {
                self.delete_text(start_pos, end_pos, axis);
                self.obj().emit_by_name::<()>("coordinate-changed", &[&axis, &0_i32]);
            }
        }

        fn parse_coord(&self, text: &str, axis: Axis) -> Option<i16> {
            let max: i16 = match axis {
                Axis::X => i16::try_from(self.max_x.get()).unwrap_or(i16::MAX),
                Axis::Y => i16::try_from(self.max_y.get()).unwrap_or(i16::MAX),
            };
            match text.chars().filter(char::is_ascii_digit).collect::<String>().parse::<i16>() {
                Ok(c) => Some(c.min(max)),
                Err(e) => match e.kind() {
                    IntErrorKind::PosOverflow => Some(max),
                    _ => None,
                },
            }
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

    pub fn set_x(&self, text: &str) { self.imp().set_text(text, Axis::X); }

    pub fn set_y(&self, text: &str) { self.imp().set_text(text, Axis::Y); }

    pub fn connect_coordinate_changed(&self, callback: impl Fn(&Self, Axis, i32) + 'static) {
        self.connect_closure(
            "coordinate-changed",
            false,
            closure_local!(|position_entry, axis, coord| callback(position_entry, axis, coord)),
        );
    }
}
