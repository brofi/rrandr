use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::ObjectExt;
use gtk::{glib, Widget};

mod imp {
    use std::cell::RefCell;
    use std::sync::OnceLock;

    use glib::object::ObjectExt;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt, ObjectSubclassIsExt};
    use glib::subclass::{Signal, SignalClassHandlerToken};
    use glib::{object_subclass, SignalHandlerId, Value};
    use gtk::prelude::WidgetExt;
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{glib, BinLayout, Widget};

    #[derive(Default)]
    pub struct Switch {
        pub(super) widget: gtk::Switch,
        pub(super) active_notify_handler_id: RefCell<Option<SignalHandlerId>>,
    }

    #[object_subclass]
    impl ObjectSubclass for Switch {
        type ParentType = Widget;
        type Type = super::Switch;

        const NAME: &'static str = "RrrSwitch";

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<BinLayout>();
            klass.set_activate_signal(Self::signals()[0].signal_id());
        }
    }

    impl ObjectImpl for Switch {
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
            let obj = self.obj();
            obj.bind_property("tooltip-text", &self.widget, "tooltip-text").sync_create().build();
            self.widget.set_parent(&*obj);
        }

        fn dispose(&self) { self.widget.unparent(); }
    }

    impl WidgetImpl for Switch {
        fn mnemonic_activate(&self, group_cycling: bool) -> bool {
            self.widget.mnemonic_activate(group_cycling)
        }
    }

    fn activate(_: &SignalClassHandlerToken, values: &[Value]) -> Option<Value> {
        if let Some(value) = values.get(0) {
            if let Ok(this) = value.get::<super::Switch>() {
                this.imp().widget.activate();
            }
        }
        None
    }
}

wrapper! {
    pub struct Switch(ObjectSubclass<imp::Switch>) @extends Widget;
}

impl Switch {
    pub fn new(tooltip: &str) -> Self {
        Object::builder().property("tooltip-text", tooltip).build()
    }

    pub fn connect_active_notify(&self, f: impl Fn(&gtk::Switch) + 'static) {
        let imp = self.imp();
        *imp.active_notify_handler_id.borrow_mut() = Some(imp.widget.connect_active_notify(f));
    }

    pub fn set_active(&self, is_active: bool) {
        let imp = self.imp();
        if let Some(handler_id) = imp.active_notify_handler_id.borrow().as_ref() {
            imp.widget.block_signal(handler_id);
            imp.widget.set_active(is_active);
            imp.widget.unblock_signal(handler_id);
        }
    }

    pub fn is_active(&self) -> bool { self.imp().widget.is_active() }
}
