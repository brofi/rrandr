mod imp;

use glib::subclass::types::ObjectSubclassIsExt;
use glib::{closure_local, wrapper, Object};
use gtk::prelude::{ListModelExt, ObjectExt, WidgetExt};
use gtk::{glib, Accessible, Buildable, ConstraintTarget, DrawingArea, Widget};

use super::details_box::Update;
use crate::data::output::Output;
use crate::data::outputs::Outputs;

wrapper! {
    pub struct DisabledOutputArea(ObjectSubclass<imp::DisabledOutputArea>)
        @extends DrawingArea, Widget,
        @implements Accessible, Buildable, ConstraintTarget;
}

impl DisabledOutputArea {
    pub fn new(outputs: &Vec<Output>) -> Self {
        Object::builder().property("outputs", Outputs::new(outputs)).build()
    }

    pub fn connect_output_selected(&self, callback: impl Fn(&Self, &Output) + 'static) {
        self.connect_closure(
            "output-selected",
            false,
            closure_local!(|details, output| callback(details, output)),
        );
    }

    pub fn connect_output_deselected(&self, callback: impl Fn(&Self) + 'static) {
        self.connect_closure(
            "output-deselected",
            false,
            closure_local!(|details| callback(details)),
        );
    }

    pub fn update(&self, output: &Output, update: Update) {
        // Add/Remove
        match update {
            Update::Enabled => {
                self.imp().deselect();
                self.outputs().remove(output.id());
            }
            Update::Disabled => {
                self.outputs().append(&output);
                self.imp().select((self.outputs().n_items() - 1) as usize);
            }
            _ => (),
        }
        // Redraw
        match update {
            Update::Enabled | Update::Disabled => self.queue_draw(),
            _ => (),
        }
    }

    pub fn selected_output(&self) -> Option<Output> {
        if let Some(i) = self.imp().selected_output.get() {
            return Some(self.outputs().index(i));
        }
        None
    }

    pub fn deselect(&self) { self.imp().deselect(); }
}
