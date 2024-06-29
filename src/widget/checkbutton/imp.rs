use std::cell::RefCell;
use std::sync::OnceLock;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::object::{ObjectImpl, ObjectImplExt};
use gdk::glib::subclass::types::{ObjectSubclass, ObjectSubclassExt, ObjectSubclassIsExt};
use gdk::glib::subclass::{Signal, SignalClassHandlerToken};
use gdk::glib::{object_subclass, SignalHandlerId, Value};
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

    fn class_init(klass: &mut Self::Class) {
        klass.set_layout_manager_type::<BinLayout>();
        klass.set_activate_signal(Self::signals()[0].signal_id());
    }
}

impl ObjectImpl for CheckButton {
    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![Signal::builder("activate").run_first().action().class_handler(activate).build()]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        obj.bind_property("tooltip-text", &self.widget, "tooltip-text").sync_create().build();
        self.widget.set_parent(&*obj);
    }

    fn dispose(&self) { self.widget.unparent(); }
}

impl WidgetImpl for CheckButton {
    fn mnemonic_activate(&self, group_cycling: bool) -> bool {
        self.widget.mnemonic_activate(group_cycling)
    }
}

fn activate(_: &SignalClassHandlerToken, values: &[Value]) -> Option<Value> {
    if let Some(value) = values.get(0) {
        if let Ok(this) = value.get::<super::CheckButton>() {
            this.imp().widget.activate();
        }
    }
    None
}
