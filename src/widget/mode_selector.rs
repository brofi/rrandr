use glib::{wrapper, Object};
use gtk::{glib, Widget};

mod imp {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use gettextrs::gettext;
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
        gio, glib, Align, BinLayout, Box, DropDown, Image, Label, ListItem, Orientation,
        SignalListItemFactory, Widget,
    };
    use x11rb::protocol::randr::ModeFlag;

    use crate::data::mode::Mode;
    use crate::data::modes::Modes;

    #[derive(Properties)]
    #[properties(wrapper_type = super::ModeSelector)]
    pub struct ModeSelector {
        #[property(get, set = Self::set_modes, nullable)]
        modes: RefCell<Option<Modes>>,
        #[property(get, set = Self::set_selected_mode, nullable, explicit_notify)]
        selected_mode: RefCell<Option<Mode>>,
        pub(super) resolution: DropDown,
        pub(super) resolution_selected_handler_id: RefCell<Option<SignalHandlerId>>,
        pub(super) refresh_rate: DropDown,
        pub(super) refresh_rate_selected_handler_id: RefCell<Option<SignalHandlerId>>,
        selected_handlers_list_item: RefCell<HashMap<ListItem, SignalHandlerId>>,
    }

    impl Default for ModeSelector {
        fn default() -> Self {
            Self {
                modes: RefCell::default(),
                selected_mode: RefCell::default(),
                resolution: DropDown::builder()
                    .tooltip_text(gettext("Resolution"))
                    .factory(&factory(bind_res_mode))
                    .build(),
                resolution_selected_handler_id: RefCell::default(),
                refresh_rate: DropDown::builder()
                    .tooltip_text(gettext("Refresh rate"))
                    .factory(&factory(bind_rr_mode))
                    .build(),
                refresh_rate_selected_handler_id: RefCell::default(),
                selected_handlers_list_item: RefCell::default(),
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

            self.refresh_rate
                .set_list_factory(Some(&self.list_factory(&self.refresh_rate, bind_list_rr_mode)));

            self.resolution_selected_handler_id.replace(Some(
                self.resolution.connect_selected_item_notify(clone!(
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
        fn set_modes(&self, modes: Option<&Modes>) {
            let res_hid = self.resolution_selected_handler_id.borrow();
            let rr_hid = self.refresh_rate_selected_handler_id.borrow();
            Self::select_pos(&self.refresh_rate, rr_hid.as_ref(), gtk::INVALID_LIST_POSITION);
            Self::select_pos(&self.resolution, res_hid.as_ref(), gtk::INVALID_LIST_POSITION);
            self.modes.replace(modes.cloned());

            let mut rr_model = None;
            let mut res_model = None;
            if let Some(modes) = modes {
                let resolutions = modes.resolutions();

                if resolutions.n_items() > 0 {
                    rr_model = Some(
                        modes.refresh_rates(&resolutions.item(0).and_downcast::<Mode>().unwrap()),
                    );
                }

                let format_width = resolutions
                    .iter::<Mode>()
                    .map(Result::unwrap)
                    .map(|r| r.height().to_string().len())
                    .max()
                    .unwrap_or_default();
                self.resolution.set_list_factory(Some(
                    &self.list_factory(&self.resolution, move |label, mode| {
                        bind_list_res_mode(label, mode, Some(format_width));
                    }),
                ));

                res_model = Some(resolutions);
            }
            Self::set_model(&self.resolution, res_hid.as_ref(), res_model.as_ref());
            Self::set_model(&self.refresh_rate, rr_hid.as_ref(), rr_model.as_ref());
        }

        fn set_selected_mode(&self, selected_mode: Option<&Mode>) {
            self.selected_mode.replace(selected_mode.cloned());
            let res_hid = self.resolution_selected_handler_id.borrow();
            let rr_hid = self.refresh_rate_selected_handler_id.borrow();

            if let (Some(modes), Some(selected_mode)) =
                (self.modes.borrow().as_ref(), selected_mode)
            {
                if let Some(res_pos) =
                    self.resolution.model().and_downcast::<Modes>().and_then(|modes| {
                        modes.position_by_res(selected_mode.width(), selected_mode.height())
                    })
                {
                    Self::select_pos(&self.resolution, res_hid.as_ref(), res_pos);
                    Self::set_model(
                        &self.refresh_rate,
                        rr_hid.as_ref(),
                        Some(&modes.refresh_rates(selected_mode)),
                    );
                    if let Some(rr_pos) = self
                        .refresh_rate
                        .model()
                        .and_downcast::<Modes>()
                        .and_then(|modes| modes.position(selected_mode))
                    {
                        Self::select_pos(&self.refresh_rate, rr_hid.as_ref(), rr_pos);
                    }
                }
            } else {
                Self::select_pos(&self.refresh_rate, rr_hid.as_ref(), gtk::INVALID_LIST_POSITION);
                Self::select_pos(&self.resolution, res_hid.as_ref(), gtk::INVALID_LIST_POSITION);
            }
        }

        // TODO bind selected-item to this selected-item ?
        fn on_resolution_selected(&self, dd: &gtk::DropDown) {
            let selected_mode = dd.selected_item().and_downcast::<Mode>();
            if selected_mode != *self.selected_mode.borrow() {
                let mut rr_model = None;
                if let (Some(modes), Some(mode)) = (self.modes.borrow().as_ref(), &selected_mode) {
                    rr_model = Some(modes.refresh_rates(mode));
                }
                Self::set_model(
                    &self.refresh_rate,
                    self.refresh_rate_selected_handler_id.borrow().as_ref(),
                    rr_model.as_ref(),
                );
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
            if let Some(hid) = hid {
                dd.block_signal(hid);
            }
            dd.set_model(model);
            if let Some(hid) = hid {
                dd.unblock_signal(hid);
            }
        }

        fn select_pos(dd: &DropDown, hid: Option<&SignalHandlerId>, pos: u32) {
            if let Some(hid) = hid {
                dd.block_signal(hid);
            }
            dd.set_selected(pos);
            if let Some(hid) = hid {
                dd.unblock_signal(hid);
            }
        }

        fn list_factory(
            &self,
            dd: &DropDown,
            bind_mode: impl Fn(&Label, &Mode) + 'static,
        ) -> SignalListItemFactory {
            let factory = SignalListItemFactory::new();
            factory.connect_setup(|_f, list_item| {
                let hbox = Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(4)
                    .halign(Align::End)
                    .build();
                hbox.append(&Image::from_icon_name("object-select-symbolic"));
                hbox.append(&Label::builder().css_classes(["monospace"]).build());
                list_item.set_child(Some(&hbox));
            });
            factory.connect_bind(clone!(
                @weak self as this, @weak dd => move |_f, list_item| {
                    if let (Some(label), Some(mode)) = (
                        list_item
                            .child()
                            .and_then(|hbox| hbox.last_child())
                            .and_downcast::<Label>(),
                        list_item.item().and_downcast::<Mode>(),
                    ) {
                        this.selected_handlers_list_item.borrow_mut().insert(
                            list_item.clone(), dd.connect_selected_item_notify(clone!(
                                @strong list_item => move |dd| update_list_item_selected_icon(dd, &list_item)
                            ))
                        );
                        update_list_item_selected_icon(&dd, list_item);
                        bind_mode(&label, &mode);
                    }
                }
            ));
            factory.connect_unbind(clone!(
                @weak self as this, @weak dd => move |_f, list_item|
                    if let Some(handler) = this.selected_handlers_list_item.borrow_mut().remove(list_item) {
                        dd.disconnect(handler);
                    };
            ));
            factory
        }
    }

    fn factory(bind_mode: impl Fn(&Label, &Mode) + 'static) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_f, list_item| {
            list_item.set_child(Some(&Label::new(None)));
        });
        factory.connect_bind(move |_f, list_item| {
            if let (Some(label), Some(mode)) =
                (list_item.child().and_downcast::<Label>(), list_item.item().and_downcast::<Mode>())
            {
                bind_mode(&label, &mode);
            }
        });
        factory
    }

    fn bind_res_mode(label: &Label, mode: &Mode) {
        label.set_text(&mode.as_resolution_str(None).replace('x', "\u{00D7}"));
    }

    fn bind_list_res_mode(label: &Label, mode: &Mode, format_width: Option<usize>) {
        label.set_text(&mode.as_resolution_str(format_width));
    }

    fn bind_rr_mode(label: &Label, mode: &Mode) { label.set_text(&mode.as_refresh_rate_str()); }

    fn bind_list_rr_mode(label: &Label, mode: &Mode) {
        let text = mode.as_refresh_rate_str();
        let mark_flags = ModeFlag::INTERLACE | ModeFlag::DOUBLE_SCAN;
        if mode.flags().intersects(mark_flags) {
            label.set_markup(&format!("<i>{text}</i>"));
            label.set_tooltip_text(Some(&format!("{:#?}", mode.flags() & mark_flags)));
        } else {
            label.set_text(&text);
        }
    }

    fn update_list_item_selected_icon(dd: &DropDown, list_item: &ListItem) {
        if let Some(icon) =
            list_item.child().and_then(|hbox| hbox.first_child()).and_downcast::<Image>()
        {
            icon.set_opacity(if dd.selected_item() == list_item.item() { 1. } else { 0. });
        }
    }

    fn activate(_: &SignalClassHandlerToken, values: &[Value]) -> Option<Value> {
        if let Some(value) = values.first() {
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

impl Default for ModeSelector {
    fn default() -> Self { Self::new() }
}
