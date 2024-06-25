use std::cell::RefCell;

use gdk::glib::object::ObjectExt;
use gdk::glib::subclass::object::{ObjectImpl, ObjectImplExt};
use gdk::glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use gdk::glib::{object_subclass, SignalHandlerId};
use gdk::prelude::Cast;
use gtk::prelude::{ListItemExt, WidgetExt};
use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
use gtk::{glib, Align, BinLayout, Label, ListItem, SignalListItemFactory, StringObject, Widget};

#[derive(Default)]
pub struct DropDown {
    pub(super) widget: gtk::DropDown,
    pub(super) selected_item_notify_handler_id: RefCell<Option<SignalHandlerId>>,
}

#[object_subclass]
impl ObjectSubclass for DropDown {
    type ParentType = Widget;
    type Type = super::DropDown;

    const NAME: &'static str = "RrrDropDown";

    fn class_init(klass: &mut Self::Class) { klass.set_layout_manager_type::<BinLayout>(); }
}

impl ObjectImpl for DropDown {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        obj.bind_property("tooltip-text", &self.widget, "tooltip-text").sync_create().build();
        self.widget.set_factory(Some(&factory()));
        self.widget.set_list_factory(Some(&list_factory()));
        self.widget.set_parent(&*obj);
    }

    fn dispose(&self) { self.widget.unparent() }
}

impl WidgetImpl for DropDown {}

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
