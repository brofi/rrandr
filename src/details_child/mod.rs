mod imp;

use gdk::glib::object::IsA;
use gdk::glib::wrapper;
use glib::Object;
use gtk::{glib, Accessible, Buildable, ConstraintTarget, FlowBoxChild, Widget};

wrapper! {
    pub struct DetailsChild(ObjectSubclass<imp::DetailsChild>)
        @extends FlowBoxChild, Widget,
        @implements Accessible, Buildable, ConstraintTarget;
}

impl DetailsChild {
    pub fn new<W: IsA<Widget>>(label: &str, control: &W) -> Self {
        Object::builder().property("label", label).property("control", control).build()
    }
}

impl Default for DetailsChild {
    fn default() -> Self { Object::new() }
}
