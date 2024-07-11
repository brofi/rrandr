use gio::ListModel;
use glib::object::{IsA, ObjectExt};
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object, SignalHandlerId};
use gtk::{gio, glib, DropDown, Widget};

mod imp {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use glib::object::Cast;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt, ObjectSubclassIsExt};
    use glib::subclass::{Signal, SignalClassHandlerToken};
    use glib::{object_subclass, SignalHandlerId, Value};
    use gtk::prelude::{BoxExt, ListItemExt, WidgetExt};
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{
        glib, Align, BinLayout, Box, DropDown, Label, ListItem, Orientation, SignalListItemFactory,
        StringObject, Widget,
    };

    pub struct ModeSelector {
        pub(super) resolution: DropDown,
        pub(super) resolution_selected_handler_id: RefCell<Option<SignalHandlerId>>,
        pub(super) refresh_rate: DropDown,
        pub(super) refresh_rate_selected_handler_id: RefCell<Option<SignalHandlerId>>,
    }

    impl Default for ModeSelector {
        fn default() -> Self {
            Self {
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

    pub fn set_resolutions(&self, model: Option<&impl IsA<ListModel>>) {
        let imp = self.imp();
        if let Some(handler_id) = imp.resolution_selected_handler_id.borrow().as_ref() {
            Self::set_model(&imp.resolution, handler_id, model);
        }
    }

    pub fn set_refresh_rates(&self, model: Option<&impl IsA<ListModel>>) {
        let imp = self.imp();
        if let Some(handler_id) = imp.refresh_rate_selected_handler_id.borrow().as_ref() {
            Self::set_model(&imp.refresh_rate, handler_id, model);
        }
    }

    pub fn get_resolution(&self) -> u32 { self.imp().resolution.selected() }

    pub fn get_refresh_rate(&self) -> u32 { self.imp().refresh_rate.selected() }

    pub fn set_resolution(&self, position: u32) {
        let imp = self.imp();
        if let Some(handler_id) = imp.resolution_selected_handler_id.borrow().as_ref() {
            Self::set_selected(&imp.resolution, handler_id, position);
        }
    }

    pub fn set_refresh_rate(&self, position: u32) {
        let imp = self.imp();
        if let Some(handler_id) = imp.refresh_rate_selected_handler_id.borrow().as_ref() {
            Self::set_selected(&imp.refresh_rate, handler_id, position);
        }
    }

    fn set_model(dd: &DropDown, handler_id: &SignalHandlerId, model: Option<&impl IsA<ListModel>>) {
        dd.block_signal(handler_id);
        dd.set_model(model);
        dd.unblock_signal(handler_id);
    }

    fn set_selected(dd: &DropDown, handler_id: &SignalHandlerId, position: u32) {
        dd.block_signal(handler_id);
        dd.set_selected(position);
        dd.unblock_signal(handler_id);
    }
}
