use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::{glib, DropDown, Widget};

mod imp {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use gio::ListModel;
    use glib::object::{Cast, CastNone, IsA};
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt, ObjectSubclassIsExt};
    use glib::subclass::{Signal, SignalClassHandlerToken};
    use glib::{clone, derived_properties, object_subclass, Properties, SignalHandlerId, Value};
    use gtk::prelude::{BoxExt, ListItemExt, ListModelExt, ObjectExt, WidgetExt};
    use gtk::subclass::prelude::DerivedObjectProperties;
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{
        gio, glib, Align, BinLayout, Box, DropDown, Label, ListItem, Orientation,
        SignalListItemFactory, StringList, StringObject, Widget,
    };

    use crate::data::mode::Mode;
    use crate::data::modes::Modes;
    use crate::utils::nearly_eq;

    type Resolution = [u16; 2];

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
                resolution: create_dropdown("Resolution"),
                resolution_selected_handler_id: RefCell::default(),
                refresh_rate: create_dropdown("Refresh rate"),
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

            *self.resolution_selected_handler_id.borrow_mut() =
                Some(self.resolution.connect_selected_notify(clone!(
                    @weak self as this => move |dd| this.on_resolution_selected(dd)
                )));
            *self.refresh_rate_selected_handler_id.borrow_mut() =
                Some(self.refresh_rate.connect_selected_item_notify(clone!(
                    @weak self as this => move |dd| this.on_refresh_rate_selected(dd)
                )));
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
        fn set_modes(&self, modes: &Modes) {
            self.modes.replace(modes.clone());
            let resolutions = self.get_resolutions_dropdown();
            self.set_resolutions(Some(&into_string_list(&resolutions)));
            // TODO set selection should be moved into set_selected_mode
            if let Some(res_idx) = self.get_current_resolution_dropdown_index() {
                self.set_resolution(u32::try_from(res_idx).expect("less resolutions"));
                let refresh_rates = self.get_refresh_rates_dropdown(res_idx);
                self.set_refresh_rates(Some(&into_string_list(&refresh_rates)));
                if let Some(ref_idx) = self.get_current_refresh_rate_dropdown_index(res_idx) {
                    self.set_refresh_rate(u32::try_from(ref_idx).expect("less refresh rates"));
                }
            }
        }

        fn set_selected_mode(&self, selected_mode: Option<&Mode>) {
            self.selected_mode.replace(selected_mode.cloned());
        }

        fn on_resolution_selected(&self, dd: &gtk::DropDown) {
            let dd_selected = dd.selected() as usize;
            let obj = self.obj();

            // Update current mode
            let mode = obj
                .modes()
                .item(self.resolution_dropdown_mode_index(dd_selected) as u32)
                .and_downcast::<Mode>()
                .unwrap();
            if obj.selected_mode().is_some_and(|m| m.id() != mode.id())
                || obj.selected_mode().is_none()
            {
                self.selected_mode.replace(Some(mode));
                self.obj().notify_selected_mode();
            }

            // Update refresh rate dropdown
            self.set_refresh_rates(Some(&into_string_list(
                &self.get_refresh_rates_dropdown(dd_selected),
            )));
            if let Some(idx) = self.get_current_refresh_rate_dropdown_index(dd_selected) {
                self.set_refresh_rate(u32::try_from(idx).expect("less refresh rates"));
            }
        }

        fn on_refresh_rate_selected(&self, dd: &gtk::DropDown) {
            let obj = self.obj();
            // Update current mode
            let mode = obj
                .modes()
                .item(self.refresh_rate_dropdown_mode_index(
                    self.resolution.selected() as usize,
                    dd.selected() as usize,
                ) as u32)
                .and_downcast::<Mode>()
                .unwrap();
            if obj.selected_mode().is_some_and(|m| m.id() != mode.id())
                || obj.selected_mode().is_none()
            {
                self.selected_mode.replace(Some(mode));
                self.obj().notify_selected_mode();
            }
        }

        fn get_resolutions_dropdown(&self) -> Vec<String> {
            let resolutions = self.get_resolutions();
            let format_width =
                resolutions.iter().map(|r| r[1].to_string().len()).max().unwrap_or_default();
            resolutions
                .iter()
                .map(|&r| Self::resolution_str(r, format_width))
                .collect::<Vec<String>>()
        }

        fn get_current_resolution_dropdown_index(&self) -> Option<usize> {
            if let Some(mode) = self.obj().selected_mode() {
                return self
                    .get_resolutions()
                    .iter()
                    .position(|res: &Resolution| {
                        u32::from(res[0]) == mode.width() && u32::from(res[1]) == mode.height()
                    })?
                    .into();
            }
            None
        }

        fn resolution_dropdown_mode_index(&self, index: usize) -> usize {
            let res = self.get_resolutions()[index];
            self.modes_vec()
                .iter()
                .position(|m| m.width() == u32::from(res[0]) && m.height() == u32::from(res[1]))
                .unwrap()
        }

        fn refresh_rate_dropdown_mode_index(&self, resolution_index: usize, index: usize) -> usize {
            let res = self.get_resolutions()[resolution_index];
            let refresh = self.get_refresh_rates(resolution_index)[index];
            self.modes_vec()
                .iter()
                .position(|m| {
                    m.width() == u32::from(res[0])
                        && m.height() == u32::from(res[1])
                        && nearly_eq(m.refresh(), refresh)
                })
                .unwrap()
        }

        fn get_current_refresh_rate_dropdown_index(
            &self,
            resolution_index: usize,
        ) -> Option<usize> {
            if let Some(mode) = self.obj().selected_mode() {
                return self
                    .get_refresh_rates(resolution_index)
                    .iter()
                    .position(|&refresh| nearly_eq(refresh, mode.refresh()))?
                    .into();
            }
            None
        }

        fn get_refresh_rates_dropdown(&self, resolution_index: usize) -> Vec<String> {
            self.get_refresh_rates(resolution_index)
                .iter()
                .map(|&r| Self::refresh_str(r))
                .collect::<Vec<String>>()
        }

        fn get_resolutions(&self) -> Vec<Resolution> {
            let mut dd_list = Vec::new();
            for mode in self.modes_vec() {
                let r = [mode.width() as u16, mode.height() as u16];
                if !dd_list.contains(&r) {
                    dd_list.push(r);
                }
            }
            dd_list
        }

        fn get_refresh_rates(&self, resolution_index: usize) -> Vec<f64> {
            let res = self.get_resolutions()[resolution_index];
            self.modes_vec()
                .iter()
                .filter(|m| m.width() == u32::from(res[0]) && m.height() == u32::from(res[1]))
                .map(|m| m.refresh())
                .collect::<Vec<f64>>()
        }

        fn resolution_str(res: Resolution, format_width: usize) -> String {
            let [w, h] = res;
            format!("{w} x {h:<format_width$}")
        }

        fn refresh_str(refresh: f64) -> String { format!("{refresh:.2} Hz") }

        fn set_resolutions(&self, model: Option<&impl IsA<ListModel>>) {
            if let Some(handler_id) = self.resolution_selected_handler_id.borrow().as_ref() {
                Self::set_model(&self.resolution, handler_id, model);
            }
        }

        fn set_refresh_rates(&self, model: Option<&impl IsA<ListModel>>) {
            if let Some(handler_id) = self.refresh_rate_selected_handler_id.borrow().as_ref() {
                Self::set_model(&self.refresh_rate, handler_id, model);
            }
        }

        fn set_resolution(&self, position: u32) {
            if let Some(handler_id) = self.resolution_selected_handler_id.borrow().as_ref() {
                Self::set_selected(&self.resolution, handler_id, position);
            }
        }

        fn set_refresh_rate(&self, position: u32) {
            if let Some(handler_id) = self.refresh_rate_selected_handler_id.borrow().as_ref() {
                Self::set_selected(&self.refresh_rate, handler_id, position);
            }
        }

        fn set_model(
            dd: &DropDown,
            handler_id: &SignalHandlerId,
            model: Option<&impl IsA<ListModel>>,
        ) {
            dd.block_signal(handler_id);
            dd.set_model(model);
            dd.unblock_signal(handler_id);
        }

        fn set_selected(dd: &DropDown, handler_id: &SignalHandlerId, position: u32) {
            dd.block_signal(handler_id);
            dd.set_selected(position);
            dd.unblock_signal(handler_id);
        }

        fn modes_vec(&self) -> Vec<Mode> {
            let obj = self.obj();
            let mut modes = Vec::new();
            for i in 0..obj.modes().n_items() {
                modes.push(obj.modes().item(i).and_downcast::<Mode>().unwrap())
            }
            modes
        }
    }

    fn activate(_: &SignalClassHandlerToken, values: &[Value]) -> Option<Value> {
        if let Some(value) = values.get(0) {
            if let Ok(this) = value.get::<super::ModeSelector>() {
                this.imp().resolution.activate();
            }
        }
        None
    }

    fn create_dropdown(tooltip: &str) -> DropDown {
        DropDown::builder()
            .tooltip_text(tooltip)
            .factory(&factory())
            .list_factory(&list_factory())
            .build()
    }

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

    fn into_string_list(list: &[String]) -> StringList {
        let list = list.iter().map(String::as_str).collect::<Vec<&str>>();
        StringList::new(list.as_slice())
    }
}

wrapper! {
    pub struct ModeSelector(ObjectSubclass<imp::ModeSelector>) @extends Widget;
}

impl ModeSelector {
    pub fn new() -> Self { Object::new() }

    pub fn connect_resolution_selected(&self, f: impl Fn(&DropDown) + 'static) {
        let imp = self.imp();
        *imp.resolution_selected_handler_id.borrow_mut() =
            Some(imp.resolution.connect_selected_item_notify(f));
    }

    pub fn connect_refresh_rate_selected(&self, f: impl Fn(&DropDown) + 'static) {
        let imp = self.imp();
        *imp.refresh_rate_selected_handler_id.borrow_mut() =
            Some(imp.refresh_rate.connect_selected_item_notify(f));
    }
}
