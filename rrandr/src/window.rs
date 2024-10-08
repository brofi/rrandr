use gio::{ActionEntry, ActionGroup, ActionMap};
use glib::object::IsA;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::ActionMapExtManual;
use gtk::{
    gio, glib, Accessible, Application, ApplicationWindow, Buildable, ConstraintTarget, Native,
    Root, ShortcutManager, Widget,
};

pub const PADDING: u16 = 12;
pub const SPACING: u16 = 6;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use config::Config;
    use gdk::{Key, ModifierType, Texture};
    use gettextrs::{gettext, ngettext};
    use glib::object::CastNone;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::InitializingObject;
    use glib::{
        clone, object_subclass, spawn_future_local, timeout_future, timeout_future_seconds,
        Propagation,
    };
    use gtk::prelude::{
        GtkWindowExt, ListModelExt, ListModelExtManual, ObjectExt, StaticTypeExt, ToggleButtonExt,
        WidgetExt,
    };
    use gtk::subclass::application_window::ApplicationWindowImpl;
    use gtk::subclass::widget::{
        CompositeTemplateCallbacksClass, CompositeTemplateClass, CompositeTemplateInitializingExt,
        WidgetClassExt, WidgetImpl,
    };
    use gtk::subclass::window::WindowImpl;
    use gtk::{
        glib, template_callbacks, AboutDialog, ApplicationWindow, Box, Button, CompositeTemplate,
        EventControllerKey, GestureClick, Label, License, Paned, Separator, TemplateChild,
        ToggleButton,
    };
    use log::warn;

    use crate::app::{APP_NAME, APP_NAME_LOC};
    use crate::data::output::Output;
    use crate::data::outputs::Outputs;
    use crate::hook::{self};
    use crate::widget::details_box::{DetailsBox, Update};
    use crate::widget::dialog::Dialog;
    use crate::widget::disabled_output_area::DisabledOutputArea;
    use crate::widget::icon_text::IconText;
    use crate::widget::output_area::OutputArea;
    use crate::x11::popup::show_popup_windows;
    use crate::x11::randr::{self, Randr, ScreenSizeRange, Snapshot};

    const COPY_OVERLAY_SHOW_SECS: f64 = 1.5;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/github/brofi/rrandr/window.ui")]
    pub struct Window {
        config: RefCell<Config>,
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
        hsep: TemplateChild<Separator>,
        #[template_child]
        actions: TemplateChild<Box>,
        #[template_child]
        xrandr_container: TemplateChild<Box>,
        #[template_child]
        overlay: TemplateChild<Label>,
        #[template_child]
        xrandr: TemplateChild<Label>,
        #[template_child]
        tb_show_xrandr: TemplateChild<ToggleButton>,
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
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_title(Some(APP_NAME_LOC));
            obj.setup_actions();

            self.set_config();
            self.set_screen_max_size();
            self.set_outputs();

            self.hsep.set_visible(!self.config.borrow().show_xrandr);

            // Remove focusable from automatically added FlowBoxChild
            let mut child = self.actions.first_child();
            while let Some(c) = child {
                c.set_focusable(false);
                child = c.next_sibling();
            }

            let event_controller_key = EventControllerKey::new();
            event_controller_key.connect_key_pressed(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |eck, keyval, keycode, state| this.on_key_pressed(eck, keyval, keycode, state)
            ));
            obj.add_controller(event_controller_key);

            self.details.connect_output_changed(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, output, update| {
                    this.enabled_area.update(output, update);
                    this.disabled_area.update(output, update);
                }
            ));

            let gc = GestureClick::new();
            gc.connect_pressed(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, n_press, _, _| this.on_xrandr_clicked(n_press)
            ));
            self.xrandr.add_controller(gc);
            self.tb_show_xrandr.set_active(self.config.borrow().show_xrandr);
            self.tb_show_xrandr.set_tooltip_text(Some(
                &(if self.config.borrow().show_xrandr {
                    gettext("Hide xrandr command")
                } else {
                    gettext("Show xrandr command")
                } + "\u{2026}"),
            ));

            self.setup_randr_notify();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}

    #[template_callbacks]
    impl Window {
        pub fn set_config(&self) {
            let cfg = Config::new(APP_NAME, Some(self.obj().settings()));
            self.enabled_area.set_config(&cfg);
            self.disabled_area.set_config(&cfg);
            self.config.replace(cfg);
        }

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
                output.connect_notify_local(
                    None,
                    clone!(
                        #[strong]
                        outputs,
                        #[weak(rename_to = view)]
                        self.xrandr,
                        move |_, _| view.set_text(&randr::gen_xrandr_command(&outputs))
                    ),
                );
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

            self.xrandr.set_text(&randr::gen_xrandr_command(&outputs));
            enabled.connect_items_changed(clone!(
                #[strong]
                outputs,
                #[weak(rename_to = view)]
                self.xrandr,
                move |_, _, _, _| view.set_text(&randr::gen_xrandr_command(&outputs))
            ));
            disabled.connect_items_changed(clone!(
                #[strong]
                outputs,
                #[weak(rename_to = view)]
                self.xrandr,
                move |_, _, _, _| view.set_text(&randr::gen_xrandr_command(&outputs))
            ));
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
            self.hsep.set_visible(true);
        }

        #[template_callback]
        fn on_enabled_deselected(&self) {
            self.disabled_area.deselect();
            self.disabled_area.queue_draw();
            self.details.set_output(None::<Output>);
            self.hsep.set_visible(!self.tb_show_xrandr.is_active());
        }

        #[template_callback]
        fn on_disabled_selected(&self, output: &Output) {
            self.enabled_area.deselect();
            self.enabled_area.queue_draw();
            self.details.set_output(Some(output));
            self.hsep.set_visible(true);
        }

        #[template_callback]
        fn on_disabled_deselected(&self) {
            self.enabled_area.deselect();
            self.enabled_area.queue_draw();
            self.details.set_output(None::<Output>);
            self.hsep.set_visible(!self.tb_show_xrandr.is_active());
        }

        pub(super) fn apply(&self) {
            let obj = self.obj();
            self.snapshot.replace(Some(self.randr.snapshot()));
            if self.randr.apply(&self.get_outputs()) {
                let cfg = self.config.borrow();
                if let Err(e) = hook::spawn(&cfg.apply_hook) {
                    warn!("{e}");
                }
                let dialog = Dialog::builder(&*obj)
                    .title(&gettext("Confirm changes"))
                    .heading(&gettext("Keep changes?"))
                    .actions(&[&gettext("_Keep"), &gettext("_Revert")])
                    .tooltips(&[&gettext("Keep changes"), &gettext("Revert changes")])
                    .build();

                let timeout = cfg.revert_timeout;
                if timeout > 0 {
                    dialog.set_message(ngettext!(
                        "Reverting in {} second",
                        "Reverting in {} seconds",
                        timeout.into(),
                        timeout
                    ));
                    let countdown = spawn_future_local(clone!(
                        #[weak(rename_to = window)]
                        self,
                        #[strong]
                        dialog,
                        async move {
                            for i in (1..=timeout).rev() {
                                // Translators: '{}' gets replaced with the number of seconds left.
                                let msg = ngettext!(
                                    "Reverting in {} second",
                                    "Reverting in {} seconds",
                                    i.into(),
                                    i
                                );
                                dialog.set_message(msg);
                                timeout_future_seconds(1).await;
                            }
                            dialog.close();
                            window.revert();
                        }
                    ));

                    dialog.connect_close_request(move |_| {
                        countdown.abort();
                        Propagation::Proceed
                    });
                }

                dialog.connect_action(clone!(
                    #[weak(rename_to = window)]
                    self,
                    move |_, i| if i == 1 {
                        window.revert();
                    }
                ));

                dialog.show();
            } else {
                self.revert();
                if let Err(e) = hook::spawn(&self.config.borrow().revert_hook) {
                    warn!("{e}");
                }
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
            show_popup_windows(&self.config.borrow(), btn);
        }

        #[template_callback]
        fn on_about_clicked(&self, _btn: &Button) {
            let about = AboutDialog::builder()
                .transient_for(&*self.obj())
                .modal(true)
                .logo(&Texture::from_resource("/com/github/brofi/rrandr/rrandr.svg"))
                .program_name(env!("CARGO_PKG_NAME"))
                .version(env!("CARGO_PKG_VERSION"))
                .comments(gettext("A graphical interface to the RandR X Window System extension"))
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

        #[template_callback]
        fn on_show_xrandr_toggled(&self, tb: &ToggleButton) {
            self.xrandr_container.set_visible(tb.is_active());
            self.hsep.set_visible(self.details.output().is_some());
            self.tb_show_xrandr.set_tooltip_text(Some(
                &(if tb.is_active() {
                    gettext("Hide xrandr command")
                } else {
                    gettext("Show xrandr command")
                } + "\u{2026}"),
            ));
        }

        fn on_xrandr_clicked(&self, n_press: i32) {
            if n_press == 2 {
                self.copy_xrandr_command();
            }
        }

        #[template_callback]
        fn on_copy_clicked(&self) { self.copy_xrandr_command(); }

        fn copy_xrandr_command(&self) {
            self.xrandr.clipboard().set_text(&self.xrandr.text());
            spawn_future_local(clone!(
                #[weak(rename_to = overlay)]
                self.overlay,
                async move {
                    overlay.set_visible(true);
                    timeout_future(Duration::from_secs_f64(COPY_OVERLAY_SHOW_SECS)).await;
                    overlay.set_visible(false);
                }
            ));
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
                spawn_future_local(clone!(
                    #[weak(rename_to = this)]
                    self,
                    async move {
                        while let Ok(event) = receiver.recv().await {
                            this.randr.handle_event(&event);
                            if receiver.is_empty() {
                                this.set_outputs();
                            }
                        }
                    }
                ));
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
