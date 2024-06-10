use std::cell::RefCell;
use std::rc::Rc;

use gdk::gio::ListModel;
use gdk::glib::{clone, Propagation, SignalHandlerId};
use gdk::Key;
use gtk::prelude::*;
use gtk::{Align, ApplicationWindow, Button, EventControllerKey, Label, Orientation, Window};

use crate::view::PADDING;

#[derive(Clone)]
pub struct DropDown {
    pub widget: gtk::DropDown,
    selected_item_notify_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl DropDown {
    pub fn new(widget: gtk::DropDown) -> Self {
        Self { widget, selected_item_notify_handler_id: Rc::new(RefCell::new(None)) }
    }

    pub fn connect_selected_item_notify(&mut self, f: impl Fn(&Self) + 'static) {
        *self.selected_item_notify_handler_id.borrow_mut() =
            Some(self.widget.connect_selected_item_notify({
                let widget = self.clone();
                move |_| f(&widget)
            }));
    }

    pub fn set_model(&self, model: Option<&impl IsA<ListModel>>) {
        if let Some(handler_id) = self.selected_item_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_model(model);
            self.widget.unblock_signal(handler_id);
        }
    }

    pub fn set_selected(&self, position: u32) {
        if let Some(handler_id) = self.selected_item_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_selected(position);
            self.widget.unblock_signal(handler_id);
        }
    }
}

#[derive(Clone)]
pub struct Switch {
    pub widget: gtk::Switch,
    active_notify_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl Switch {
    pub fn new(widget: gtk::Switch) -> Self {
        Self { widget, active_notify_handler_id: Rc::new(RefCell::new(None)) }
    }

    pub fn connect_active_notify(&mut self, f: impl Fn(&Self) + 'static) {
        *self.active_notify_handler_id.borrow_mut() = Some(self.widget.connect_active_notify({
            let widget = self.clone();
            move |_| f(&widget)
        }));
    }

    pub fn set_active(&self, is_active: bool) {
        if let Some(handler_id) = self.active_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_active(is_active);
            self.widget.unblock_signal(handler_id);
        }
    }
}

#[derive(Clone)]
pub struct CheckButton {
    pub widget: gtk::CheckButton,
    active_notify_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl CheckButton {
    pub fn new(widget: gtk::CheckButton) -> Self {
        Self { widget, active_notify_handler_id: Rc::new(RefCell::new(None)) }
    }

    pub fn connect_active_notify(&mut self, f: impl Fn(&Self) + 'static) {
        *self.active_notify_handler_id.borrow_mut() = Some(self.widget.connect_active_notify({
            let widget = self.clone();
            move |_| f(&widget)
        }));
    }

    pub fn set_active(&self, is_active: bool) {
        if let Some(handler_id) = self.active_notify_handler_id.borrow().as_ref() {
            self.widget.block_signal(handler_id);
            self.widget.set_active(is_active);
            self.widget.unblock_signal(handler_id);
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    pub widget: gtk::Entry,
    insert_text_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
    delete_text_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
}

impl Entry {
    pub fn new(widget: gtk::Entry) -> Self {
        Self {
            widget,
            insert_text_handler_id: Rc::new(RefCell::new(None)),
            delete_text_handler_id: Rc::new(RefCell::new(None)),
        }
    }

    pub fn connect_insert_text(&mut self, f: impl Fn(&Self, &str, &mut i32) + 'static) {
        if let Some(editable) = self.widget.delegate() {
            *self.insert_text_handler_id.borrow_mut() = Some(editable.connect_insert_text({
                let entry = self.clone();
                move |editable, text, position| {
                    f(&entry, text, position);
                    editable.stop_signal_emission_by_name("insert_text");
                }
            }));
        }
    }

    pub fn connect_delete_text(&mut self, f: impl Fn(&Self, i32, i32) + 'static) {
        if let Some(editable) = self.widget.delegate() {
            *self.delete_text_handler_id.borrow_mut() = Some(editable.connect_delete_text({
                let entry = self.clone();
                move |editable, start, end| {
                    f(&entry, start, end);
                    editable.stop_signal_emission_by_name("delete_text");
                }
            }));
        }
    }

    pub fn set_text(&self, text: &str) {
        if self.insert_text_handler_id.borrow().is_some()
            && self.delete_text_handler_id.borrow().is_some()
        {
            self.delete_text(0, -1);
            self.insert_text(text, &mut 0);
        }
    }

    pub fn insert_text(&self, text: &str, position: &mut i32) {
        if let Some(handler_id) = self.insert_text_handler_id.borrow().as_ref() {
            if let Some(editable) = self.widget.delegate() {
                editable.block_signal(handler_id);
                self.widget.insert_text(text, position);
                editable.unblock_signal(handler_id);
            }
        }
    }

    pub fn delete_text(&self, start_pos: i32, end_pos: i32) {
        if let Some(handler_id) = self.delete_text_handler_id.borrow().as_ref() {
            if let Some(editable) = self.widget.delegate() {
                editable.block_signal(handler_id);
                self.widget.delete_text(start_pos, end_pos);
                editable.unblock_signal(handler_id);
            }
        }
    }
}

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

    pub fn on_result(mut self, callback: impl Fn(usize) + 'static) -> Self {
        self.on_result = Some(Rc::new(callback));
        self
    }

    pub fn build(self) -> Dialog {
        let window = gtk::Window::builder()
            .width_request(300 - 2 * i32::from(PADDING))
            .height_request(150 - 2 * i32::from(PADDING))
            .transient_for(&self.parent)
            .destroy_with_parent(true)
            .modal(true)
            .hide_on_close(true)
            .build();
        window.set_title(self.title.as_deref());
        let eck = EventControllerKey::new();
        eck.connect_key_pressed(|eck, keyval, _keycode, _state| match keyval {
            Key::Escape => {
                eck.widget().downcast::<Window>().unwrap().close();
                Propagation::Stop
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
                    let btn = Self::create_action_button(name);
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
                let btn = Self::create_action_button("_Close");
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

    fn create_action_button(name: &str) -> Button {
        Button::builder()
            .label(name)
            .use_underline(true)
            .valign(Align::Baseline)
            .hexpand(true)
            .tooltip_text(name)
            .build()
    }
}
