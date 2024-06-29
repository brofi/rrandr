use std::cell::RefCell;
use std::sync::OnceLock;

use gdk::glib::object::Cast;
use gdk::glib::subclass::object::{ObjectImpl, ObjectImplExt};
use gdk::glib::subclass::types::{ObjectSubclass, ObjectSubclassExt, ObjectSubclassIsExt};
use gdk::glib::subclass::{Signal, SignalClassHandlerToken};
use gdk::glib::{object_subclass, SignalHandlerId, Value};
use gtk::prelude::{BoxExt, ListItemExt, WidgetExt};
use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
use gtk::{
    glib, Align, BinLayout, Box, DropDown, Label, ListItem, Orientation, SignalListItemFactory,
    StringObject, Widget,
};

pub struct ModeSelector {
    pub(super) resolution: DropDown,
    pub(super) resolution_selected_handler_id: RefCell<Option<SignalHandlerId>>,
    pub(super) refresh_rate: DropDown,
    pub(super) refresh_rate_selected_handler_id: RefCell<Option<SignalHandlerId>>,
}

impl Default for ModeSelector {
    fn default() -> Self {
        Self {
            resolution: create_dropdown("Resolution"),
            resolution_selected_handler_id: RefCell::default(),
            refresh_rate: create_dropdown("Refresh rate"),
            refresh_rate_selected_handler_id: RefCell::default(),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for ModeSelector {
    type ParentType = Widget;
    type Type = super::ModeSelector;

    const NAME: &'static str = "RrrModeSelector";

    fn class_init(klass: &mut Self::Class) {
        klass.set_layout_manager_type::<BinLayout>();
        klass.set_activate_signal(Self::signals()[0].signal_id());
    }
}

impl ObjectImpl for ModeSelector {
    fn signals() -> &'static [Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![Signal::builder("activate").run_first().action().class_handler(activate).build()]
        })
    }

    fn constructed(&self) {
        self.parent_constructed();
        let linkbox =
            Box::builder().orientation(Orientation::Horizontal).css_classes(["linked"]).build();
        linkbox.append(&self.resolution);
        linkbox.append(&self.refresh_rate);
        linkbox.set_parent(&*self.obj());
    }

    fn dispose(&self) { self.obj().first_child().unwrap().unparent(); }
}

impl WidgetImpl for ModeSelector {
    fn mnemonic_activate(&self, group_cycling: bool) -> bool {
        if group_cycling {
            self.resolution.grab_focus();
        } else {
            self.resolution.activate();
        }
        true
    }
}

fn activate(_: &SignalClassHandlerToken, values: &[Value]) -> Option<Value> {
    if let Some(value) = values.get(0) {
        if let Ok(this) = value.get::<super::ModeSelector>() {
            this.imp().resolution.activate();
        }
    }
    None
}

fn create_dropdown(tooltip: &str) -> DropDown {
    DropDown::builder()
        .tooltip_text(tooltip)
        .factory(&factory())
        .list_factory(&list_factory())
        .build()
}

fn factory() -> SignalListItemFactory {
    let factory = SignalListItemFactory::new();
    factory.connect_setup(|_f, list_item| {
        list_item.set_child(Some(&Label::new(None)));
    });
    factory.connect_bind(|_f, list_item| {
        bind_label(list_item, Some(&|s| s.replace(' ', "\u{202F}").replace('x', "\u{00D7}")));
    });
    factory
}

fn list_factory() -> SignalListItemFactory {
    let factory = SignalListItemFactory::new();
    factory.connect_setup(|_f, list_item| {
        let label = Label::builder().halign(Align::End).css_classes(["monospace"]).build();
        list_item.set_child(Some(&label));
    });
    factory.connect_bind(|_f, list_item| {
        bind_label(list_item, None);
    });
    factory
}

fn bind_label(list_item: &ListItem, formatter: Option<&dyn Fn(String) -> String>) {
    if let Some(label) = list_item.child() {
        if let Ok(label) = label.downcast::<Label>() {
            if let Some(item) = list_item.item() {
                if let Ok(s) =
                    item.downcast::<StringObject>().and_then(|s| Ok(s.string().to_string()))
                {
                    label.set_label(&formatter.map_or(s.clone(), |f| f(s)));
                }
            }
        }
    }
}
