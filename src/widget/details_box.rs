use glib::{closure_local, wrapper, Object, ValueDelegate};
use gtk::prelude::ObjectExt;
use gtk::{glib, Widget};

use crate::data::output::Output;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::OnceLock;
    use std::time::Duration;

    use gettextrs::gettext;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::Signal;
    use glib::{
        clone, derived_properties, object_subclass, timeout_add_local_once, Properties,
        SignalHandlerId, SourceId,
    };
    use gtk::prelude::{CheckButtonExt, ObjectExt, StaticType, WidgetExt};
    use gtk::subclass::prelude::DerivedObjectProperties;
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{glib, Align, BinLayout, FlowBox, Orientation, SelectionMode, Widget};

    use super::Update;
    use crate::data::mode::Mode;
    use crate::data::modes::Modes;
    use crate::data::output::Output;
    use crate::utils::nearly_eq;
    use crate::widget::checkbutton::CheckButton;
    use crate::widget::details_child::DetailsChild;
    use crate::widget::mode_selector::ModeSelector;
    use crate::widget::position_entry::{Axis, PositionEntry};
    use crate::widget::switch::Switch;
    use crate::window::SPACING;

    const POS_UPDATE_DELAY: u64 = 500;

    #[derive(Properties)]
    #[properties(wrapper_type = super::DetailsBox)]
    pub struct DetailsBox {
        #[property(get, set = Self::set_output, nullable)]
        output: RefCell<Option<Output>>,
        enabled_changed_handler: RefCell<Option<SignalHandlerId>>,
        mode_changed_handler: RefCell<Option<SignalHandlerId>>,
        pos_changed_handlers: RefCell<[Option<SignalHandlerId>; 2]>,
        pos_modify_sids: RefCell<[Option<SourceId>; 2]>,
        primary_changed_handler: RefCell<Option<SignalHandlerId>>,
        #[property(get, set, construct, default = i16::MAX.try_into().unwrap(), maximum = u16::MAX.into())]
        screen_max_width: Cell<u32>,
        #[property(get, set, construct, default = i16::MAX.try_into().unwrap(), maximum = u16::MAX.into())]
        screen_max_height: Cell<u32>,
        root: FlowBox,
        sw_enabled: Switch,
        mode_selector: ModeSelector,
        position_entry: PositionEntry,
        cb_primary: CheckButton,
    }

    impl Default for DetailsBox {
        fn default() -> Self {
            Self {
                output: RefCell::default(),
                enabled_changed_handler: RefCell::default(),
                mode_changed_handler: RefCell::default(),
                pos_changed_handlers: RefCell::default(),
                pos_modify_sids: RefCell::default(),
                primary_changed_handler: RefCell::default(),
                screen_max_width: Cell::default(),
                screen_max_height: Cell::default(),
                root: FlowBox::builder()
                    .row_spacing(SPACING.into())
                    .column_spacing(SPACING.into())
                    .orientation(Orientation::Horizontal)
                    .selection_mode(SelectionMode::None)
                    .max_children_per_line(u32::MAX)
                    .build(),
                sw_enabled: Switch::new(&gettext("Enable/disable")),
                mode_selector: ModeSelector::new(),
                position_entry: PositionEntry::new(),
                cb_primary: CheckButton::new(&gettext("Set as primary")),
            }
        }
    }

    #[object_subclass]
    impl ObjectSubclass for DetailsBox {
        type ParentType = Widget;
        type Type = super::DetailsBox;

        const NAME: &'static str = "DetailsBox";

        fn class_init(klass: &mut Self::Class) { klass.set_layout_manager_type::<BinLayout>(); }
    }

    #[derived_properties]
    impl ObjectImpl for DetailsBox {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("output-changed")
                        .param_types([Output::static_type(), Update::static_type()])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_halign(Align::Fill);
            obj.set_hexpand(true);

            self.root.append(&DetailsChild::new(
                // Output status
                &gettext("Enabled"),
                &self.sw_enabled,
            ));
            self.root.append(&DetailsChild::new(
                // Output mode consisting of resolution and refresh rate
                &gettext("Mode"),
                &self.mode_selector,
            ));
            self.root.append(&DetailsChild::new(
                // Output position consisting of X and Y coordinate
                &gettext("Position"),
                &self.position_entry,
            ));
            self.root.append(&DetailsChild::new(
                // Primary output status
                &gettext("Primary"),
                &self.cb_primary,
            ));

            self.sw_enabled.connect_active_notify(clone!(
                @weak self as this => move |sw| this.on_enabled_switched(sw)
            ));
            self.mode_selector.connect_selected_mode_notify(clone!(
                @weak self as this => move |mode_selector| this.on_mode_selected(mode_selector)
            ));
            self.position_entry.connect_coordinate_changed(clone!(
                @weak self as this => move |_, axis, coord| this.update_position(axis, coord);
            ));
            self.cb_primary.connect_active_notify(clone!(
                @weak self as this => move |cb| this.on_primary_checked(cb)
            ));

            self.root.set_parent(&*obj);
        }

        fn dispose(&self) { self.root.unparent(); }
    }

    impl WidgetImpl for DetailsBox {}

    impl DetailsBox {
        fn set_output(&self, output: Option<&Output>) {
            self.disconnect_output_property_handlers();

            for sid in self.pos_modify_sids.take().into_iter().flatten() {
                sid.remove();
            }
            if let Some(output) = output {
                self.connect_output_property_handlers(output);

                self.sw_enabled.set_active(output.enabled());
                self.cb_primary.set_active(output.primary());

                self.position_entry.set_x(&output.pos_x().to_string());
                self.position_entry.set_y(&output.pos_y().to_string());
                if let Some(mode) = output.mode() {
                    self.position_entry.set_max_x(i32::from(
                        self.screen_max_width
                            .get()
                            .saturating_sub(mode.width())
                            .try_into()
                            .unwrap_or(i16::MAX),
                    ));
                    self.position_entry.set_max_y(i32::from(
                        self.screen_max_height
                            .get()
                            .saturating_sub(mode.height())
                            .try_into()
                            .unwrap_or(i16::MAX),
                    ));
                } else {
                    self.position_entry.set_max_x(0);
                    self.position_entry.set_max_y(0);
                }

                self.mode_selector.set_modes(Some(output.modes()));
                self.mode_selector.set_selected_mode(output.mode());
            } else {
                self.mode_selector.set_selected_mode(None::<Mode>);
                self.mode_selector.set_modes(None::<Modes>);
            }
            self.output.replace(output.cloned());
            self.update_visibility();
        }

        fn connect_output_property_handlers(&self, output: &Output) {
            self.enabled_changed_handler.replace(Some(output.connect_enabled_notify(clone!(
                @weak self as this => move |o| {
                    this.sw_enabled.set_active(o.enabled());
                    this.update_visibility();
                }
            ))));
            self.mode_changed_handler.replace(Some(output.connect_mode_notify(clone!(
                @weak self as this => move |o| this.mode_selector.set_selected_mode(o.mode())
            ))));
            self.primary_changed_handler.replace(Some(output.connect_primary_notify(clone!(
                @weak self as this => move |o| this.cb_primary.set_active(o.primary())
            ))));
            self.pos_changed_handlers.replace([
                Some(output.connect_pos_x_notify(clone!(
                    @weak self as this => move |o| {
                        if let Some(sid) = this.pos_modify_sids.borrow_mut()[usize::from(Axis::X)].take() {
                            sid.remove();
                        }
                        this.position_entry.set_x(&o.pos_x().to_string());
                    }
                ))),
                Some(output.connect_pos_y_notify(clone!(
                    @weak self as this => move |o| {
                        if let Some(sid) = this.pos_modify_sids.borrow_mut()[usize::from(Axis::Y)].take() {
                            sid.remove();
                        }
                        this.position_entry.set_y(&o.pos_y().to_string());
                    }
                )))
            ]);
        }

        fn disconnect_output_property_handlers(&self) {
            for handler in [
                self.enabled_changed_handler.take(),
                self.mode_changed_handler.take(),
                self.primary_changed_handler.take(),
            ] {
                if let (Some(output), Some(handler_id)) = (self.output.borrow().as_ref(), handler) {
                    output.disconnect(handler_id);
                }
            }
            for handler in self.pos_changed_handlers.take() {
                if let (Some(output), Some(handler_id)) = (self.output.borrow().as_ref(), handler) {
                    output.disconnect(handler_id);
                }
            }
        }

        fn update_visibility(&self) {
            let mut child = self.root.first_child();
            while let Some(c) = child {
                let visible = self
                    .output
                    .borrow()
                    .as_ref()
                    .is_some_and(|o| o.enabled() || c.widget_name() == "fbc_enabled");
                c.set_visible(visible);
                child = c.next_sibling();
            }
        }

        fn on_enabled_switched(&self, sw: &gtk::Switch) {
            if let Some(output) = self.output.borrow().as_ref() {
                // Update output
                if sw.is_active() {
                    output.enable();
                    self.notify_updated(output, Update::Enabled);
                } else {
                    output.disable();
                    self.notify_updated(output, Update::Disabled);
                }
            }
        }

        fn on_mode_selected(&self, mode_selector: &ModeSelector) {
            if let Some(output) = self.output.borrow().as_ref() {
                let new_mode = mode_selector.selected_mode();
                let old_mode = output.mode();
                output.set_mode(new_mode.clone());
                if let (Some(old_mode), Some(new_mode)) = (old_mode, new_mode) {
                    if old_mode.width() != new_mode.width()
                        || old_mode.height() != new_mode.height()
                    {
                        self.notify_updated(output, Update::Resolution);
                    } else if nearly_eq(old_mode.refresh(), new_mode.refresh()) {
                        self.notify_updated(output, Update::Refresh);
                    }
                }
            }
        }

        fn update_position(&self, axis: Axis, coord: i32) {
            if let Some(output) = self.output.borrow().as_ref() {
                if let Some(sid) = self.pos_modify_sids.borrow_mut()[usize::from(axis)].take() {
                    sid.remove();
                }

                let sid = timeout_add_local_once(
                    Duration::from_millis(POS_UPDATE_DELAY),
                    clone!(
                        @weak self as this, @weak output => move || {
                            this.pos_modify_sids.borrow_mut()[usize::from(axis)].take();
                            let cur_pos = match axis {
                                Axis::X => output.pos_x(),
                                Axis::Y => output.pos_y(),
                            };
                            if cur_pos != coord {
                                let set_coord = || {
                                    match axis {
                                        Axis::X => output.set_pos_x(coord),
                                        Axis::Y => output.set_pos_y(coord),
                                    };
                                };
                                if let Some(handler_id) = &this.pos_changed_handlers.borrow()[usize::from(axis)] {
                                    output.block_signal(handler_id);
                                    set_coord();
                                    output.unblock_signal(handler_id);
                                } else {
                                    set_coord();
                                }
                                this.notify_updated(&output, Update::Position);
                            }
                        }
                    ),
                );
                self.pos_modify_sids.borrow_mut()[usize::from(axis)].replace(sid);
            }
        }

        fn on_primary_checked(&self, cb: &gtk::CheckButton) {
            if let Some(output) = self.output.borrow().as_ref() {
                output.set_primary(output.enabled() && cb.is_active());
                self.notify_updated(output, Update::Primary);
            }
        }

        fn notify_updated(&self, output: &Output, update: Update) {
            self.obj().emit_by_name::<()>("output-changed", &[output, &update]);
        }
    }
}

wrapper! {
    pub struct DetailsBox(ObjectSubclass<imp::DetailsBox>) @extends Widget;
}

impl DetailsBox {
    pub fn new(screen_max_width: u16, screen_max_height: u16) -> Self {
        Object::builder()
            .property("screen-max-width", u32::from(screen_max_width))
            .property("screen-max-height", u32::from(screen_max_height))
            .build()
    }

    // TODO connect to Output properties notify signals instead of passing Update
    // enum
    pub fn connect_output_changed(&self, callback: impl Fn(&Self, &Output, Update) + 'static) {
        self.connect_closure(
            "output-changed",
            false,
            closure_local!(|details, output, update| callback(details, output, update)),
        );
    }
}

#[derive(ValueDelegate, Clone, Copy)]
#[value_delegate(from = u8)]
pub enum Update {
    Enabled,
    Disabled,
    Resolution,
    Refresh,
    Position,
    Primary,
}

impl From<u8> for Update {
    fn from(v: u8) -> Self {
        match v {
            0 => Update::Enabled,
            1 => Update::Disabled,
            2 => Update::Resolution,
            3 => Update::Refresh,
            4 => Update::Position,
            5 => Update::Primary,
            x => panic!("Not an update value: {x}"),
        }
    }
}

impl<'a> From<&'a Update> for u8 {
    fn from(v: &'a Update) -> Self { *v as u8 }
}

impl From<Update> for u8 {
    fn from(v: Update) -> Self { v as u8 }
}
