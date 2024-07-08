mod imp;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::types::ObjectSubclassIsExt;
use gdk::glib::{self, closure_local, wrapper, Object};
use gtk::prelude::WidgetExt;
use gtk::subclass::drawing_area::DrawingAreaImpl;
use gtk::{Accessible, Buildable, ConstraintTarget, DrawingArea, Widget};

use super::details_box::Update;
use crate::data::output::Output;
use crate::data::outputs::Outputs;

wrapper! {
    pub struct OutputArea(ObjectSubclass<imp::OutputArea>)
        @extends DrawingArea, Widget,
        @implements Accessible, Buildable, ConstraintTarget;
}

impl OutputArea {
    pub fn new(outputs: &Vec<Output>, screen_max_width: u16, screen_max_height: u16) -> Self {
        Object::builder()
            .property("outputs", Outputs::new(outputs))
            .property("screen-max-width", u32::from(screen_max_width))
            .property("screen-max-height", u32::from(screen_max_height))
            .build()
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
            Update::Enabled => self.imp().add_output(output),
            Update::Disabled => self.imp().remove_output(output),
            _ => (),
        }
        // Mind the gap
        match update {
            Update::Enabled | Update::Disabled | Update::Resolution => {
                imp::OutputArea::mind_the_gap_and_overlap(&self.outputs());
            }
            _ => (),
        }
        // Resize
        match update {
            Update::Enabled | Update::Disabled | Update::Resolution | Update::Position => {
                self.imp().resize(self.width(), self.height())
            }
            _ => (),
        }
        // Redraw
        match update {
            Update::Refresh => (),
            _ => self.queue_draw(),
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
