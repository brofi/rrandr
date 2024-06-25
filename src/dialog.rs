use std::rc::Rc;

use gdk::glib::{clone, Propagation};
use gdk::{Key, ModifierType};
use gtk::prelude::*;
use gtk::{Align, ApplicationWindow, Button, EventControllerKey, Label, Orientation, Window};

use crate::view::PADDING;

pub struct Dialog {
    window: Window,
    message: Label,
}

impl Dialog {
    pub fn builder(parent: &ApplicationWindow) -> DialogBuilder { DialogBuilder::new(parent) }

    pub fn set_message(&self, message: &str) { self.message.set_label(message); }

    pub fn show(&self) { self.window.present(); }

    pub fn close(&self) { self.window.close(); }

    pub fn set_on_close(&self, callback: impl Fn(&Window) -> Propagation + 'static) {
        self.window.connect_close_request(callback);
    }
}

pub struct DialogBuilder {
    parent: ApplicationWindow,
    title: Option<String>,
    heading: Option<String>,
    message: Option<String>,
    actions: Option<[String; 2]>,
    tooltips: Option<[String; 2]>,
    on_result: Option<Rc<dyn Fn(usize)>>,
}

impl DialogBuilder {
    fn new(parent: &ApplicationWindow) -> Self {
        Self {
            parent: parent.clone(),
            title: None,
            heading: None,
            message: None,
            actions: None,
            tooltips: None,
            on_result: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn heading(mut self, heading: &str) -> Self {
        self.heading = Some(heading.to_string());
        self
    }

    pub fn message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }

    pub fn actions(mut self, actions: [&str; 2]) -> Self {
        self.actions = Some(actions.map(str::to_string));
        self
    }

    pub fn tooltips(mut self, tooltips: [&str; 2]) -> Self {
        self.tooltips = Some(tooltips.map(str::to_string));
        self
    }

    pub fn on_result(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_result = Some(Rc::new(callback));
        self
    }

    pub fn build(self) -> Dialog {
        let window = gtk::Window::builder()
            .default_width(300 - 2 * i32::from(PADDING))
            .default_height(150 - 2 * i32::from(PADDING))
            .resizable(false)
            .transient_for(&self.parent)
            .destroy_with_parent(true)
            .modal(true)
            .hide_on_close(true)
            .build();
        window.set_title(self.title.as_deref());
        let eck = EventControllerKey::new();
        eck.connect_key_pressed(|eck, keyval, _keycode, state| match keyval {
            Key::Escape => {
                eck.widget().downcast::<Window>().unwrap().close();
                Propagation::Stop
            }
            Key::w => {
                if state.contains(ModifierType::CONTROL_MASK) {
                    eck.widget().downcast::<Window>().unwrap().close();
                    Propagation::Stop
                } else {
                    Propagation::Proceed
                }
            }
            _ => Propagation::Proceed,
        });
        window.add_controller(eck);

        let root = gtk::Box::builder().orientation(Orientation::Vertical).build();
        let text = gtk::Box::builder()
            .margin_start(PADDING.into())
            .margin_end(PADDING.into())
            .margin_top(PADDING.into())
            .margin_bottom(PADDING.into())
            .orientation(Orientation::Vertical)
            .spacing(PADDING.into())
            .hexpand(true)
            .build();
        let heading =
            Label::builder().halign(Align::Center).css_classes(["title", "title-2"]).build();
        heading.set_label(&self.heading.unwrap_or_default());
        let message = Label::builder()
            .halign(Align::Center)
            .valign(Align::Start)
            .vexpand(true)
            .css_classes(["body"])
            .build();
        message.set_label(&self.message.unwrap_or_default());
        let box_actions = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["linked"])
            .build();
        match self.actions {
            Some(actions) => {
                for (i, name) in actions.iter().enumerate() {
                    let tooltip = self.tooltips.clone().map_or("".to_owned(), |t| t[i].clone());
                    let btn = Self::create_action_button(name, &tooltip);
                    box_actions.append(&btn);
                    btn.connect_clicked(
                        clone!(@strong window, @strong self.on_result as on_result => move |_| {
                            if let Some(on_result) = &on_result {
                                on_result(i);
                            }
                            window.close();
                        }),
                    );
                }
            }
            _ => {
                let btn = Self::create_action_button("_Close", "");
                box_actions.append(&btn);
                btn.connect_clicked(clone!(@strong window => move |_| window.close()));
            }
        }
        text.append(&heading);
        text.append(&message);
        root.append(&text);
        root.append(&box_actions);
        window.set_child(Some(&root));

        Dialog { window, message }
    }

    fn create_action_button(name: &str, tooltip: &str) -> Button {
        Button::builder()
            .label(name)
            .use_underline(true)
            .valign(Align::Baseline)
            .hexpand(true)
            .tooltip_text(tooltip)
            .build()
    }
}
