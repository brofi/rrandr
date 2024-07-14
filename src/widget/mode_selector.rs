use glib::{wrapper, Object};
use gtk::{glib, Widget};

mod imp {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use gio::ListModel;
    use glib::object::{CastNone, IsA};
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt, ObjectSubclassIsExt};
    use glib::subclass::{Signal, SignalClassHandlerToken};
    use glib::{clone, derived_properties, object_subclass, Properties, SignalHandlerId, Value};
    use gtk::prelude::{
        BoxExt, ListItemExt, ListModelExt, ListModelExtManual, ObjectExt, WidgetExt,
    };
    use gtk::subclass::prelude::DerivedObjectProperties;
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{
        gio, glib, Align, BinLayout, Box, DropDown, Label, Orientation, SignalListItemFactory,
        Widget,
    };

    use crate::data::mode::Mode;
    use crate::data::modes::Modes;

    #[derive(Clone, Copy)]
    enum ModeDropDown {
        Resolution,
        RefreshRate,
    }

    #[derive(Properties)]
    #[properties(wrapper_type = super::ModeSelector)]
    pub struct ModeSelector {
        #[property(get, set = Self::set_modes)]
        modes: RefCell<Modes>,
        #[property(get, set = Self::set_selected_mode, nullable, explicit_notify)]
        selected_mode: RefCell<Option<Mode>>,
        pub(super) resolution: DropDown,
        pub(super) resolution_selected_handler_id: RefCell<Option<SignalHandlerId>>,
        pub(super) refresh_rate: DropDown,
        pub(super) refresh_rate_selected_handler_id: RefCell<Option<SignalHandlerId>>,
    }

    impl Default for ModeSelector {
        fn default() -> Self {
            Self {
                modes: Default::default(),
                selected_mode: Default::default(),
                resolution: DropDown::builder()
                    .tooltip_text("Resolution")
                    .factory(&factory(ModeDropDown::Resolution))
                    .build(),
                resolution_selected_handler_id: RefCell::default(),
                refresh_rate: DropDown::builder()
                    .tooltip_text("Refresh rate")
                    .factory(&factory(ModeDropDown::RefreshRate))
                    .list_factory(&list_factory(ModeDropDown::RefreshRate, None))
                    .build(),
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

    #[derived_properties]
    impl ObjectImpl for ModeSelector {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("activate")
                        .run_first()
                        .action()
                        .class_handler(activate)
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let linkbox =
                Box::builder().orientation(Orientation::Horizontal).css_classes(["linked"]).build();
            linkbox.append(&self.resolution);
            linkbox.append(&self.refresh_rate);
            linkbox.set_parent(&*self.obj());

            self.resolution_selected_handler_id.replace(Some(
                self.resolution.connect_selected_notify(clone!(
                    @weak self as this => move |dd| this.on_resolution_selected(dd)
                )),
            ));
            self.refresh_rate_selected_handler_id.replace(Some(
                self.refresh_rate.connect_selected_item_notify(clone!(
                    @weak self as this => move |dd| this.on_refresh_rate_selected(dd)
                )),
            ));
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

    impl ModeSelector {
        fn resolutions_model(&self) -> Modes {
            let mut cur_width = 0;
            let mut cur_height = 0;
            let resolution_modes = Modes::new();
            for mode in self.modes.borrow().iter::<Mode>().map(Result::unwrap) {
                if mode.width() != cur_width || mode.height() != cur_height {
                    resolution_modes.append(&mode);
                    cur_width = mode.width();
                    cur_height = mode.height();
                }
            }
            resolution_modes
        }

        fn refresh_rates_model(&self, res_mode: &Mode) -> Modes {
            let refresh_rate_modes = Modes::new();
            for mode in self.modes.borrow().iter::<Mode>().map(Result::unwrap) {
                if mode.width() == res_mode.width() && mode.height() == res_mode.height() {
                    refresh_rate_modes.append(&mode);
                }
            }
            refresh_rate_modes
        }

        fn set_modes(&self, modes: &Modes) {
            self.modes.replace(modes.clone());
            let res_model = self.resolutions_model();
            Self::set_model(
                &self.resolution,
                self.resolution_selected_handler_id.borrow().as_ref(),
                Some(&res_model),
            );
            Self::set_model(
                &self.refresh_rate,
                self.refresh_rate_selected_handler_id.borrow().as_ref(),
                Some(&self.refresh_rates_model(&res_model.item(0).and_downcast::<Mode>().unwrap())),
            );
            let format_width = res_model
                .iter::<Mode>()
                .map(Result::unwrap)
                .map(|r| r.height().to_string().len())
                .max()
                .unwrap_or_default();
            self.resolution.set_list_factory(Some(&list_factory(
                ModeDropDown::Resolution,
                Some(format_width),
            )));
        }

        fn set_selected_mode(&self, selected_mode: Option<&Mode>) {
            self.selected_mode.replace(selected_mode.cloned());
            if let Some(mode) = selected_mode {
                Self::set_selected(
                    &self.resolution,
                    self.resolution_selected_handler_id.borrow().as_ref(),
                    mode,
                );
                Self::set_selected(
                    &self.refresh_rate,
                    self.refresh_rate_selected_handler_id.borrow().as_ref(),
                    mode,
                );
            }
        }

        // TODO bind selected-item to this selected-item ?
        fn on_resolution_selected(&self, dd: &gtk::DropDown) {
            let selected_mode = dd.selected_item().and_downcast::<Mode>();
            if selected_mode != *self.selected_mode.borrow() {
                if let Some(mode) = &selected_mode {
                    Self::set_model(
                        &self.refresh_rate,
                        self.refresh_rate_selected_handler_id.borrow().as_ref(),
                        Some(&self.refresh_rates_model(mode)),
                    );
                }
                self.selected_mode.replace(selected_mode);
                self.obj().notify_selected_mode();
            }
        }

        fn on_refresh_rate_selected(&self, dd: &gtk::DropDown) {
            let selected_mode = dd.selected_item().and_downcast::<Mode>();
            if selected_mode != *self.selected_mode.borrow() {
                self.selected_mode.replace(selected_mode);
                self.obj().notify_selected_mode();
            }
        }

        fn set_model(
            dd: &DropDown,
            hid: Option<&SignalHandlerId>,
            model: Option<&impl IsA<ListModel>>,
        ) {
            hid.map(|hid| dd.block_signal(&hid));
            dd.set_model(model);
            hid.map(|hid| dd.unblock_signal(&hid));
        }

        fn set_selected(dd: &DropDown, hid: Option<&SignalHandlerId>, selected_mode: &Mode) {
            if let Some(pos) =
                dd.model().and_downcast::<Modes>().and_then(|modes| modes.position(&selected_mode))
            {
                hid.map(|hid| dd.block_signal(&hid));
                dd.set_selected(pos);
                hid.map(|hid| dd.unblock_signal(&hid));
            }
        }
    }

    fn factory(mdd: ModeDropDown) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_f, list_item| {
            list_item.set_child(Some(&Label::new(None)));
        });
        factory.connect_bind(move |_f, list_item| {
            if let (Some(label), Some(mode)) =
                (list_item.child().and_downcast::<Label>(), list_item.item().and_downcast::<Mode>())
            {
                label.set_label(&match mdd {
                    ModeDropDown::Resolution => {
                        mode.as_resolution_str(None).replace('x', "\u{00D7}")
                    }
                    ModeDropDown::RefreshRate => mode.as_refresh_rate_str(),
                });
            }
        });
        factory
    }

    fn list_factory(mdd: ModeDropDown, res_format_width: Option<usize>) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_f, list_item| {
            let label = Label::builder().halign(Align::End).css_classes(["monospace"]).build();
            list_item.set_child(Some(&label));
        });
        factory.connect_bind(move |_f, list_item| {
            if let (Some(label), Some(mode)) =
                (list_item.child().and_downcast::<Label>(), list_item.item().and_downcast::<Mode>())
            {
                label.set_label(&match mdd {
                    ModeDropDown::Resolution => mode.as_resolution_str(res_format_width),
                    ModeDropDown::RefreshRate => mode.as_refresh_rate_str(),
                });
            }
        });
        factory
    }

    fn activate(_: &SignalClassHandlerToken, values: &[Value]) -> Option<Value> {
        if let Some(value) = values.get(0) {
            if let Ok(this) = value.get::<super::ModeSelector>() {
                this.imp().resolution.activate();
            }
        }
        None
    }
}

wrapper! {
    pub struct ModeSelector(ObjectSubclass<imp::ModeSelector>) @extends Widget;
}

impl ModeSelector {
    pub fn new() -> Self { Object::new() }
}
