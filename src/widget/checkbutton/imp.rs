use std::cell::RefCell;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::object::{ObjectImpl, ObjectImplExt};
use gdk::glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use gdk::glib::{object_subclass, SignalHandlerId};
use gtk::prelude::WidgetExt;
use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
use gtk::{glib, BinLayout, Widget};

#[derive(Default)]
pub struct CheckButton {
    pub(super) widget: gtk::CheckButton,
    pub(super) active_notify_handler_id: RefCell<Option<SignalHandlerId>>,
}

#[object_subclass]
impl ObjectSubclass for CheckButton {
    type ParentType = Widget;
    type Type = super::CheckButton;

    const NAME: &'static str = "RrrCheckButton";

    fn class_init(klass: &mut Self::Class) { klass.set_layout_manager_type::<BinLayout>(); }
}

impl ObjectImpl for CheckButton {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        obj.bind_property("tooltip-text", &self.widget, "tooltip-text").sync_create().build();
        self.widget.set_parent(&*obj);
    }

    fn dispose(&self) { self.widget.unparent(); }
}

impl WidgetImpl for CheckButton {}
