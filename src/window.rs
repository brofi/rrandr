use gio::{ActionEntry, ActionGroup, ActionMap};
use glib::object::IsA;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{closure_local, wrapper, Object};
use gtk::prelude::{ActionMapExtManual, ObjectExt};
use gtk::{
    gio, glib, Accessible, Application, ApplicationWindow, Buildable, Button, ConstraintTarget,
    Native, Root, ShortcutManager, Widget,
};

pub const PADDING: u16 = 12;
pub const SPACING: u16 = 6;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::OnceLock;

    use gdk::{Key, ModifierType, Texture};
    use gettextrs::{gettext, ngettext};
    use glib::object::CastNone;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::{InitializingObject, Signal};
    use glib::types::StaticType;
    use glib::{
        clone, object_subclass, spawn_future_local, timeout_future_seconds, MainContext, Priority,
        Propagation,
    };
    use gtk::prelude::{
        GtkWindowExt, ListModelExt, ListModelExtManual, ObjectExt, StaticTypeExt, WidgetExt,
    };
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
    use log::warn;

    use crate::data::output::Output;
    use crate::data::outputs::Outputs;
    use crate::widget::details_box::{DetailsBox, Update};
    use crate::widget::dialog::Dialog;
    use crate::widget::disabled_output_area::DisabledOutputArea;
    use crate::widget::icon_text::IconText;
    use crate::widget::output_area::OutputArea;
    use crate::x11::randr::{self, Randr, ScreenSizeRange, Snapshot};

    const CONFIRM_DIALOG_SHOW_SECS: u8 = 15;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/github/brofi/rrandr/window.ui")]
    pub struct Window {
        randr: Rc<Randr>,
        snapshot: RefCell<Option<Snapshot>>,
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
                vec![Signal::builder("identify").param_types([Button::static_type()]).build()]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_actions();

            self.set_screen_max_size();
            self.set_outputs();

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

            self.setup_randr_notify();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}

    #[template_callbacks]
    impl Window {
        fn set_screen_max_size(&self) {
            let ScreenSizeRange { max_width, max_height, .. } = self.randr.screen_size_range();
            self.enabled_area.set_screen_max_width(max_width);
            self.enabled_area.set_screen_max_height(max_height);
            self.details.set_screen_max_width(max_width);
            self.details.set_screen_max_height(max_height);
        }

        fn set_outputs(&self) {
            let outputs = self.randr.output_model();
            let enabled = Outputs::new();
            let disabled = Outputs::new();
            for output in outputs.iter::<Output>().map(Result::unwrap) {
                if output.enabled() { enabled.append(&output) } else { disabled.append(&output) }
            }
            // Keep selection when outputs move from enabled to disabled and vice versa
            if let Some(selected) =
                self.enabled_area.selected_output().or(self.disabled_area.selected_output())
            {
                if let Some(o) = outputs.find_by_id(selected.id()) {
                    if selected.enabled() && !o.enabled() {
                        self.disabled_area.select(&o);
                    } else if !selected.enabled() && o.enabled() {
                        self.enabled_area.select(&o);
                    }
                    self.details.set_output(Some(o));
                }
            }
            self.enabled_area.set_outputs(&enabled);
            self.disabled_area.set_outputs(&disabled);
        }

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

        pub(super) fn apply(&self) {
            let obj = self.obj();
            self.snapshot.replace(Some(self.randr.snapshot()));
            if self.randr.apply(&self.get_outputs()) {
                let dialog = Dialog::builder(&*obj)
                    .title(&gettext("Confirm changes"))
                    .heading(&gettext("Keep changes?"))
                    .message(&ngettext!(
                        "Reverting in {} second",
                        "Reverting in {} seconds",
                        CONFIRM_DIALOG_SHOW_SECS.into(),
                        CONFIRM_DIALOG_SHOW_SECS
                    ))
                    .actions(&[&gettext("_Keep"), &gettext("_Revert")])
                    .tooltips(&[&gettext("Keep changes"), &gettext("Revert changes")])
                    .build();

                let countdown = spawn_future_local(
                    clone!(@strong dialog, @weak self as window => async move {
                        for i in (1..=CONFIRM_DIALOG_SHOW_SECS).rev() {
                            // Translators: '{}' gets replaced with the number of seconds left.
                            let msg = ngettext!("Reverting in {} second","Reverting in {} seconds", i.into(), i);
                            dialog.set_message(msg);
                            timeout_future_seconds(1).await;
                        }
                        dialog.close();
                        window.revert();
                    }),
                );

                dialog.connect_action(clone!(
                    @weak self as window => move |_, i| if i == 1 {
                        window.revert();
                    }
                ));
                dialog.connect_close_request(move |_| {
                    countdown.abort();
                    Propagation::Proceed
                });

                dialog.show();
            } else {
                self.revert();
                Dialog::builder(&*obj)
                    .title(&gettext("Failure"))
                    .heading(&gettext("Failure"))
                    .message(&gettext("Changes have been reverted."))
                    .build()
                    .show();
            }
        }

        pub(super) fn reset(&self) { self.set_outputs(); }

        pub(super) fn redraw(&self) {
            self.enabled_area.queue_draw();
            self.disabled_area.queue_draw();
        }

        #[template_callback]
        fn on_identify_clicked(&self, btn: &Button) {
            self.obj().emit_by_name::<()>("identify", &[&btn]);
        }

        #[template_callback]
        fn on_about_clicked(&self, _btn: &Button) {
            let about = AboutDialog::builder()
                .transient_for(&*self.obj())
                .modal(true)
                .logo(&Texture::from_resource("/com/github/brofi/rrandr/rrandr.svg"))
                .program_name(env!("CARGO_PKG_NAME"))
                .version(env!("CARGO_PKG_VERSION"))
                .comments(gettext("A graphical interface to the RandR X Window System extension."))
                .website_label(gettext("Repository"))
                .website(env!("CARGO_PKG_REPOSITORY"))
                .copyright(env!("RRANDR_COPYRIGHT_NOTICE"))
                .license_type(License::Gpl30)
                .authors(env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>())
                // Translators: Add your name and email address to the translation (one translator
                // per line).
                .translator_credits(gettext("translator-credits"))
                .build();
            if about.comments().unwrap() != env!("CARGO_PKG_DESCRIPTION") {
                warn!("About dialog description differs from crate description");
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

        fn revert(&self) {
            if let Some(snapshot) = self.snapshot.take() {
                self.randr.revert(snapshot);
                self.set_outputs();
            }
        }

        fn setup_randr_notify(&self) {
            let (sender, receiver) = async_channel::unbounded();
            if randr::run_event_loop(sender).is_ok() {
                let ctx = MainContext::ref_thread_default();
                ctx.spawn_local_with_priority(
                    Priority::DEFAULT_IDLE,
                    clone!(@weak self as this => async move {
                        while let Ok(event) = receiver.recv().await {
                            this.obj().set_sensitive(false);
                            this.randr.handle_event(&event);
                            this.set_outputs();
                            this.obj().set_sensitive(true);
                        }
                    }),
                );
            }
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

    pub fn connect_identify(&self, callback: impl Fn(&Self, &Button) + 'static) {
        self.connect_closure(
            "identify",
            false,
            closure_local!(|window, btn| callback(window, btn)),
        );
    }

    fn setup_actions(&self) {
        self.add_action_entries([
            ActionEntry::builder("reset")
                .activate(|window: &Self, _, _| window.imp().reset())
                .build(),
            ActionEntry::builder("apply")
                .activate(|window: &Self, _, _| window.imp().apply())
                .build(),
            ActionEntry::builder("redraw")
                .activate(|window: &Self, _, _| window.imp().redraw())
                .build(),
        ]);
    }
}

impl Default for Window {
    fn default() -> Self { Object::new() }
}
