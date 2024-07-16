use gio::{ActionGroup, ActionMap};
use glib::object::IsA;
use glib::{closure_local, wrapper, Object, ValueDelegate};
use gtk::prelude::{ListModelExtManual, ObjectExt};
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{
    gio, glib, Accessible, Application, ApplicationWindow, Buildable, Button, ConstraintTarget,
    Native, Root, ShortcutManager, Widget,
};

use crate::data::output::Output;
use crate::data::outputs::Outputs;

pub const PADDING: u16 = 12;
pub const SPACING: u16 = 6;

#[derive(ValueDelegate, Clone, Copy)]
#[value_delegate(from = u8)]
pub enum Action {
    Keep,
    Revert,
}

impl From<u8> for Action {
    fn from(v: u8) -> Self {
        match v {
            0 => Action::Keep,
            1 => Action::Revert,
            x => panic!("Not an action value: {x}"),
        }
    }
}

impl<'a> From<&'a Action> for u8 {
    fn from(v: &'a Action) -> Self { *v as u8 }
}

impl From<Action> for u8 {
    fn from(v: Action) -> Self { v as u8 }
}

mod imp {
    use std::cell::Cell;
    use std::sync::OnceLock;
    use std::time::Duration;

    use gdk::{Key, ModifierType, Texture};
    use gettextrs::{gettext, ngettext};
    use glib::object::CastNone;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::{InitializingObject, Signal};
    use glib::types::StaticType;
    use glib::{
        clone, object_subclass, spawn_future_local, timeout_add, ControlFlow, Propagation, Type,
    };
    use gtk::prelude::{GtkWindowExt, ListModelExt, ObjectExt, StaticTypeExt, WidgetExt};
    use gtk::subclass::application_window::ApplicationWindowImpl;
    use gtk::subclass::widget::{
        CompositeTemplateCallbacksClass, CompositeTemplateClass, CompositeTemplateInitializingExt,
        WidgetClassExt, WidgetImpl,
    };
    use gtk::subclass::window::WindowImpl;
    use gtk::{
        glib, template_callbacks, AboutDialog, ApplicationWindow, Button, CompositeTemplate,
        EventControllerKey, FlowBox, License, Paned, TemplateChild,
    };

    use crate::data::output::Output;
    use crate::data::outputs::Outputs;
    use crate::widget::details_box::{DetailsBox, Update};
    use crate::widget::dialog::Dialog;
    use crate::widget::disabled_output_area::DisabledOutputArea;
    use crate::widget::icon_text::IconText;
    use crate::widget::output_area::OutputArea;

    const CONFIRM_DIALOG_SHOW_SECS: u8 = 15;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/github/brofi/rrandr/window.ui")]
    pub struct Window {
        #[template_child]
        paned: TemplateChild<Paned>,
        #[template_child]
        pub(super) enabled_area: TemplateChild<OutputArea>,
        #[template_child]
        pub(super) disabled_area: TemplateChild<DisabledOutputArea>,
        #[template_child]
        pub(super) details: TemplateChild<DetailsBox>,
        #[template_child]
        actions: TemplateChild<FlowBox>,
        last_handle_pos: Cell<i32>,
    }

    #[object_subclass]
    impl ObjectSubclass for Window {
        type ParentType = ApplicationWindow;
        type Type = super::Window;

        const NAME: &'static str = "MainWindow";

        fn class_init(klass: &mut Self::Class) {
            IconText::ensure_type();
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) { obj.init_template(); }
    }

    impl ObjectImpl for Window {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("apply")
                        .param_types([Button::static_type(), Outputs::static_type()])
                        .return_type_from(Type::BOOL)
                        .build(),
                    Signal::builder("confirm-action")
                        .param_types([super::Action::static_type()])
                        .build(),
                    Signal::builder("reset").param_types([Button::static_type()]).build(),
                    Signal::builder("identify").param_types([Button::static_type()]).build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let _ = &self
                .enabled_area
                .get()
                .bind_property("screen-max-width", &self.details.get(), "screen-max-width")
                .bidirectional()
                .build();
            let _ = &self
                .enabled_area
                .get()
                .bind_property("screen-max-height", &self.details.get(), "screen-max-height")
                .bidirectional()
                .build();

            // Remove focusable from automatically added FlowBoxChild
            let mut child = self.actions.first_child();
            while let Some(c) = child {
                c.set_focusable(false);
                child = c.next_sibling();
            }

            let event_controller_key = EventControllerKey::new();
            event_controller_key.connect_key_pressed(clone!(
                @weak self as this => @default-panic, move |eck, keyval, keycode, state| this.on_key_pressed(eck, keyval, keycode, state)
            ));
            obj.add_controller(event_controller_key);

            self.details.connect_output_changed(clone!(
                @weak self as this => move |_, output, update| {
                    this.enabled_area.update(output, update);
                    this.disabled_area.update(output, update);
                }
            ));
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}

    #[template_callbacks]
    impl Window {
        fn on_key_pressed(
            &self,
            _eck: &EventControllerKey,
            keyval: Key,
            _keycode: u32,
            _state: ModifierType,
        ) -> Propagation {
            match keyval {
                Key::Delete => {
                    if let Some(output) = self.enabled_area.selected_output() {
                        output.disable();
                        self.enabled_area.update(&output, Update::Disabled);
                        self.disabled_area.update(&output, Update::Disabled);
                        return Propagation::Stop;
                    }
                    if let Some(output) = self.disabled_area.selected_output() {
                        output.enable();
                        self.enabled_area.update(&output, Update::Enabled);
                        self.disabled_area.update(&output, Update::Enabled);
                        return Propagation::Stop;
                    }
                }
                Key::F9 => {
                    if let Some(handle) =
                        self.paned.start_child().and_then(|start| start.next_sibling())
                    {
                        let cur_pos = self.paned.position();
                        let max_pos = self.paned.width() - handle.width();
                        self.paned.set_position(if cur_pos == max_pos {
                            self.last_handle_pos.get()
                        } else {
                            self.last_handle_pos.set(cur_pos);
                            max_pos
                        });
                    }
                }
                _ => (),
            };
            Propagation::Proceed
        }

        #[template_callback]
        fn on_enabled_selected(&self, output: &Output) {
            self.disabled_area.deselect();
            self.disabled_area.queue_draw();
            self.details.set_output(Some(output));
        }

        #[template_callback]
        fn on_enabled_deselected(&self) {
            self.disabled_area.deselect();
            self.disabled_area.queue_draw();
            self.details.set_output(None::<Output>);
        }

        #[template_callback]
        fn on_disabled_selected(&self, output: &Output) {
            self.enabled_area.deselect();
            self.enabled_area.queue_draw();
            self.details.set_output(Some(output));
        }

        #[template_callback]
        fn on_disabled_deselected(&self) {
            self.enabled_area.deselect();
            self.enabled_area.queue_draw();
            self.details.set_output(None::<Output>);
        }

        #[template_callback]
        fn on_apply_clicked(&self, btn: &Button) {
            let obj = self.obj();
            if obj.emit_by_name::<bool>("apply", &[&btn, &self.get_outputs()]) {
                let mut secs = CONFIRM_DIALOG_SHOW_SECS.saturating_sub(1);

                let dialog = Dialog::builder(&*obj)
                    .title(&gettext("Confirm changes"))
                    .heading(&gettext("Keep changes?"))
                    .message(&ngettext!(
                        "Reverting in {} second",
                        "Reverting in {} seconds",
                        secs.into(),
                        secs
                    ))
                    .actions(&[&gettext("_Keep"), &gettext("_Revert")])
                    .tooltips(&[&gettext("Keep changes"), &gettext("Revert changes")])
                    .build();

                let (sender, receiver) = async_channel::bounded(1);
                timeout_add(Duration::from_secs(1), move || {
                    secs = secs.saturating_sub(1);
                    if sender.send_blocking(secs).is_ok() && secs > 0 {
                        ControlFlow::Continue
                    } else {
                        ControlFlow::Break
                    }
                });
                spawn_future_local(clone!(
                    @strong receiver, @strong dialog, @weak self as window => async move {
                        while let Ok(secs) = receiver.recv().await {
                            // Translators: '{}' gets replaced with the number of seconds left.
                            let msg = ngettext!(
                                "Reverting in {} second",
                                "Reverting in {} seconds",
                                secs.into(),
                                secs
                            );
                            dialog.set_message(msg);
                            if secs == 0 {
                                dialog.close();
                                window.obj().emit_by_name::<()>("confirm-action", &[&super::Action::Revert]);
                            }
                        }
                    }
                ));

                dialog.connect_action(clone!(
                @weak self as window => move |_, i| {
                    let i = u8::try_from(i).expect("two actions");
                    window.obj().emit_by_name::<()>("confirm-action", &[&super::Action::from(i)]);
                }
            ));
                dialog.connect_close_request(move |_| {
                    receiver.close();
                    Propagation::Proceed
                });

                dialog.show();
            } else {
                Dialog::builder(&*obj)
                    .title(&gettext("Failure"))
                    .heading(&gettext("Failure"))
                    .message(&gettext("Changes have been reverted."))
                    .build()
                    .show();
            }
        }

        #[template_callback]
        fn on_reset_clicked(&self, btn: &Button) {
            self.obj().emit_by_name::<()>("reset", &[&btn]);
        }

        #[template_callback]
        fn on_identify_clicked(&self, btn: &Button) {
            self.obj().emit_by_name::<()>("identify", &[&btn]);
        }

        #[template_callback]
        fn on_about_clicked(&self, _btn: &Button) {
            let about = AboutDialog::builder()
                .transient_for(&*self.obj())
                .program_name(env!("CARGO_PKG_NAME"))
                .version(env!("CARGO_PKG_VERSION"))
                .comments(env!("CARGO_PKG_DESCRIPTION"))
                .website_label(gettext("Repository"))
                .website(env!("CARGO_PKG_REPOSITORY"))
                .copyright(env!("RRANDR_COPYRIGHT_NOTICE"))
                .license_type(License::Gpl30)
                .authors(env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>())
                .build();
            if let Ok(logo) = Texture::from_filename("src/res/logo.svg") {
                about.set_logo(Some(&logo));
            }
            about.show();
        }

        fn get_outputs(&self) -> Outputs {
            let outputs = Outputs::default();
            let enabled = self.enabled_area.outputs();
            let disabled = self.disabled_area.outputs();
            for i in 0..enabled.n_items() {
                outputs.append(&enabled.item(i).and_downcast::<Output>().unwrap());
            }
            for i in 0..disabled.n_items() {
                outputs.append(&disabled.item(i).and_downcast::<Output>().unwrap());
            }
            outputs
        }
    }
}

wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends ApplicationWindow, gtk::Window, Widget,
        @implements ActionGroup, ActionMap, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

impl Window {
    pub fn new(app: &impl IsA<Application>) -> Self {
        Object::builder().property("application", app).build()
    }

    pub fn set_screen_max_size(&self, width: u16, height: u16) {
        self.imp().enabled_area.set_screen_max_width(u32::from(width));
        self.imp().enabled_area.set_screen_max_height(u32::from(height));
        self.imp().details.set_screen_max_width(u32::from(width));
        self.imp().details.set_screen_max_height(u32::from(height));
    }

    pub fn set_outputs(&self, outputs: &Outputs) {
        let imp = self.imp();
        let enabled = Outputs::new();
        let disabled = Outputs::new();
        for output in outputs.iter::<Output>().map(Result::unwrap) {
            if output.enabled() { enabled.append(&output) } else { disabled.append(&output) }
        }
        // Keep selection when outputs move from enabled to disabled and vice versa
        if let Some(selected) =
            imp.enabled_area.selected_output().or(imp.disabled_area.selected_output())
        {
            if let Some(o) = outputs.find_by_id(selected.id()) {
                if selected.enabled() && !o.enabled() {
                    imp.disabled_area.select(&o);
                } else if !selected.enabled() && o.enabled() {
                    imp.enabled_area.select(&o);
                }
                imp.details.set_output(Some(o));
            }
        }
        imp.enabled_area.set_outputs(&enabled);
        imp.disabled_area.set_outputs(&disabled);
    }

    pub fn connect_apply(&self, callback: impl Fn(&Self, &Button, &Outputs) -> bool + 'static) {
        self.connect_closure(
            "apply",
            false,
            closure_local!(|window, btn, outputs| callback(window, btn, outputs)),
        );
    }

    pub fn connect_confirm_action(&self, callback: impl Fn(&Self, Action) + 'static) {
        self.connect_closure(
            "confirm-action",
            false,
            closure_local!(|window, action| callback(window, action)),
        );
    }

    pub fn connect_reset(&self, callback: impl Fn(&Self, &Button) + 'static) {
        self.connect_closure("reset", false, closure_local!(|window, btn| callback(window, btn)));
    }

    pub fn connect_identify(&self, callback: impl Fn(&Self, &Button) + 'static) {
        self.connect_closure(
            "identify",
            false,
            closure_local!(|window, btn| callback(window, btn)),
        );
    }
}

impl Default for Window {
    fn default() -> Self { Object::new() }
}
