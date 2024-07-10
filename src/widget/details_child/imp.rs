use std::cell::RefCell;

use glib::subclass::object::{ObjectImpl, ObjectImplExt};
use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use glib::{derived_properties, object_subclass, GString, Properties, Value};
use gtk::prelude::{BoxExt, FlowBoxChildExt, ObjectExt, WidgetExt};
use gtk::subclass::flow_box_child::FlowBoxChildImpl;
use gtk::subclass::prelude::DerivedObjectProperties;
use gtk::subclass::widget::WidgetImpl;
use gtk::{glib, Align, Box, FlowBoxChild, GestureClick, Label, Orientation, Widget};

use crate::view::SPACING;

#[derive(Properties)]
#[properties(wrapper_type = super::DetailsChild)]
pub struct DetailsChild {
    hbox: Box,
    #[property(get = Self::label, set = Self::set_label, type = GString)]
    label: Label,
    #[property(get, set = Self::set_control, nullable)]
    control: RefCell<Option<Widget>>,
}

impl Default for DetailsChild {
    fn default() -> Self {
        Self {
            hbox: Box::builder()
                .orientation(Orientation::Horizontal)
                .valign(Align::Center)
                .spacing(SPACING.into())
                .build(),
            label: Label::default(),
            control: RefCell::default(),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for DetailsChild {
    type ParentType = FlowBoxChild;
    type Type = super::DetailsChild;

    const NAME: &'static str = "DetailsChild";
}

#[derived_properties]
impl ObjectImpl for DetailsChild {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        obj.set_halign(Align::Start);
        obj.set_valign(Align::Center);
        obj.set_hexpand(false);
        obj.set_vexpand(false);
        obj.set_focusable(false);
        obj.set_visible(false);
        obj.bind_property("label", &*obj, "name")
            .transform_to(|_, label: &str| {
                Some(Value::from(&("fbc_".to_owned() + &label.replace('_', "").to_lowercase())))
            })
            .build();
        self.hbox.append(&self.label);
        obj.set_child(Some(&self.hbox));
    }
}
impl WidgetImpl for DetailsChild {}
impl FlowBoxChildImpl for DetailsChild {}

impl DetailsChild {
    fn label(&self) -> GString { self.label.label() }

    fn set_label(&self, label: &str) {
        self.label.set_text_with_mnemonic(&if label.contains('_') {
            label.to_owned()
        } else {
            format!("_{label}")
        });
    }

    fn set_control(&self, widget: Option<&Widget>) {
        if widget == self.control.borrow().as_ref() {
            return;
        }

        if let Some(control) = self.control.borrow_mut().take() {
            control.unparent();
        }

        if let Some(w) = widget {
            let mut control = Some(w.clone());
            if w.is::<Box>() {
                control = w.first_child().or(control);
            }
            self.label.set_mnemonic_widget(control.as_ref());
            let gesture_click = GestureClick::new();
            gesture_click.connect_released({
                let control = control.clone().expect("has control widget");
                move |_, _, _, _| _ = control.activate()
            });
            self.label.add_controller(gesture_click);
            self.hbox.append(w);
            self.control.replace(control);
        }
    }
}
