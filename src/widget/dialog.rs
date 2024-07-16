use glib::object::{IsA, ObjectBuilder, ObjectExt};
use glib::{closure_local, wrapper, Object};
use gtk::prelude::GtkWindowExt;
use gtk::{
    glib, Accessible, ApplicationWindow, Buildable, ConstraintTarget, Native, Root,
    ShortcutManager, StringList, Widget,
};

mod imp {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use gdk::{Key, ModifierType};
    use gettextrs::gettext;
    use glib::object::{Cast, CastNone};
    use glib::subclass::types::ObjectSubclassExt;
    use glib::subclass::{InitializingObject, Signal};
    use glib::{
        clone, derived_properties, object_subclass, GString, Propagation, Properties, Type,
    };
    use gtk::prelude::{
        BoxExt, ButtonExt, EventControllerExt, GtkWindowExt, ListModelExt, ObjectExt, WidgetExt,
    };
    use gtk::subclass::prelude::{
        DerivedObjectProperties, ObjectImpl, ObjectImplExt, ObjectSubclass,
    };
    use gtk::subclass::widget::{
        CompositeTemplateClass, CompositeTemplateInitializingExt, WidgetClassExt, WidgetImpl,
    };
    use gtk::subclass::window::WindowImpl;
    use gtk::{
        glib, Align, Box, Button, CompositeTemplate, EventControllerKey, Label, StringList,
        StringObject, TemplateChild, Window,
    };

    #[derive(CompositeTemplate, Properties, Default)]
    #[template(resource = "/com/github/brofi/rrandr/dialog.ui")]
    #[properties(wrapper_type = super::Dialog)]
    pub struct Dialog {
        #[template_child]
        #[property(get = Self::heading, set = Self::set_heading, type = GString)]
        heading: TemplateChild<Label>,
        #[template_child]
        #[property(get = Self::message, set = Self::set_message, type = GString)]
        message: TemplateChild<Label>,
        #[template_child]
        buttons: TemplateChild<Box>,
        #[property(set = Self::set_actions, nullable)]
        actions: RefCell<Option<StringList>>,
        #[property(set, nullable)]
        tooltips: RefCell<Option<StringList>>,
    }

    #[object_subclass]
    impl ObjectSubclass for Dialog {
        type ParentType = Window;
        type Type = super::Dialog;

        const NAME: &'static str = "RrrDialog";

        fn class_init(klass: &mut Self::Class) { klass.bind_template(); }

        fn instance_init(obj: &InitializingObject<Self>) { obj.init_template(); }
    }

    #[derived_properties]
    impl ObjectImpl for Dialog {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("action").param_types([Type::U32]).build()])
        }

        fn constructed(&self) {
            self.parent_constructed();
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
            self.obj().add_controller(eck);
        }
    }

    impl WidgetImpl for Dialog {}
    impl WindowImpl for Dialog {}

    impl Dialog {
        fn heading(&self) -> GString { self.heading.text() }

        fn set_heading(&self, text: &str) { self.heading.set_label(text) }

        fn message(&self) -> GString { self.message.text() }

        fn set_message(&self, text: &str) { self.message.set_label(text) }

        fn set_actions(&self, actions: Option<&StringList>) {
            while let Some(button) = self.buttons.first_child() {
                button.unparent();
            }
            if let Some(actions) = actions {
                for i in 0..actions.n_items() {
                    let tooltip = self.tooltips.borrow().as_ref().map_or("".to_owned(), |t| {
                        t.item(i)
                            .and_downcast::<StringObject>()
                            .map_or("".to_owned(), |s| s.string().to_string())
                    });
                    let action = actions
                        .item(i)
                        .and_downcast::<StringObject>()
                        .unwrap()
                        .string()
                        .to_string();
                    let btn = Self::create_action_button(&action, &tooltip);
                    self.buttons.append(&btn);
                    btn.connect_clicked(clone!(@weak self as dialog => move |_| {
                        dialog.obj().emit_by_name::<()>("action", &[&i]);
                        dialog.obj().close();
                    }));
                }
            } else {
                let btn = Self::create_action_button(&gettext("_Close"), "");
                self.buttons.append(&btn);
                btn.connect_clicked(clone!(@weak self as dialog => move |_| dialog.obj().close()));
            }
            self.actions.replace(actions.cloned());
        }

        fn create_action_button(label: &str, tooltip: &str) -> Button {
            Button::builder()
                .label(label)
                .use_underline(true)
                .valign(Align::Baseline)
                .hexpand(true)
                .tooltip_text(tooltip)
                .build()
        }
    }
}

wrapper! {
    pub struct Dialog(ObjectSubclass<imp::Dialog>)
        @extends gtk::Window, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

impl Dialog {
    pub fn builder(window: &impl IsA<ApplicationWindow>) -> DialogBuilder {
        DialogBuilder::new(window)
    }

    pub fn show(&self) { self.present(); }

    pub fn connect_action(&self, callback: impl Fn(&Self, u32) + 'static) {
        self.connect_closure("action", false, closure_local!(|window, i| callback(window, i)));
    }
}

pub struct DialogBuilder {
    builder: ObjectBuilder<'static, Dialog>,
}

impl DialogBuilder {
    fn new(window: &impl IsA<ApplicationWindow>) -> Self {
        Self { builder: Object::builder().property("transient-for", window) }
    }

    pub fn title(self, title: &str) -> Self {
        Self { builder: self.builder.property("title", title) }
    }

    pub fn message(self, message: &str) -> Self {
        Self { builder: self.builder.property("message", message) }
    }

    pub fn heading(self, heading: &str) -> Self {
        Self { builder: self.builder.property("heading", heading) }
    }

    pub fn actions(self, actions: &[&str]) -> Self {
        Self { builder: self.builder.property("actions", StringList::new(actions)) }
    }

    pub fn tooltips(self, tooltips: &[&str]) -> Self {
        Self { builder: self.builder.property("tooltips", StringList::new(tooltips)) }
    }

    pub fn build(self) -> Dialog { self.builder.build() }
}
