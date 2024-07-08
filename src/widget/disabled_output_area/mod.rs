mod imp;

use gdk::glib::subclass::types::ObjectSubclassIsExt;
use gdk::glib::{self, closure_local, wrapper, Object};
use gdk::prelude::{ListModelExt, ObjectExt};
use gtk::prelude::WidgetExt;
use gtk::{Accessible, Buildable, ConstraintTarget, DrawingArea, Widget};
use x11rb::protocol::randr::Output as OutputId;

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

    pub fn update_view(&self) { self.queue_draw(); }

    pub fn selected_output(&self) -> Option<OutputId> { self.imp().selected_output() }

    pub fn deselect(&self) { self.imp().deselect(); }

    pub fn add_output(&self, output: &Output) {
        self.outputs().append(&output);
        self.imp().select((self.outputs().n_items() - 1) as usize);
    }

    pub fn remove_output(&self, output: OutputId) -> Output {
        self.imp().deselect();
        self.outputs().remove(output)
    }
}
