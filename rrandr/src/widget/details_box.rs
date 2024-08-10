use glib::subclass::types::ObjectSubclassIsExt;
use glib::{closure_local, wrapper, Object, ValueDelegate};
use gtk::prelude::ObjectExt;
use gtk::{glib, Widget};

use crate::data::output::Output;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::OnceLock;
    use std::time::Duration;

    use gettextrs::{gettext, pgettext};
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::Signal;
    use glib::{
        clone, derived_properties, object_subclass, timeout_add_local_once, Properties,
        SignalHandlerId, SourceId,
    };
    use gtk::prelude::{
        BoxExt, CheckButtonExt, ObjectExt, RangeExt, ScaleExt, StaticType, ToggleButtonExt,
        WidgetExt,
    };
    use gtk::subclass::prelude::DerivedObjectProperties;
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{
        glib, Align, BinLayout, Box, DropDown, FlowBox, Orientation, PositionType, Scale,
        SelectionMode, Separator, ToggleButton, Widget, INVALID_LIST_POSITION,
    };

    use super::Update;
    use crate::data::enums::{Reflection, Rotation};
    use crate::data::mode::Mode;
    use crate::data::modes::Modes;
    use crate::data::output::Output;
    use crate::data::values::I16;
    use crate::utils::nearly_eq;
    use crate::widget::checkbutton::CheckButton;
    use crate::widget::details_child::DetailsChild;
    use crate::widget::icon_text::IconText;
    use crate::widget::mode_selector::ModeSelector;
    use crate::widget::position_entry::{Axis, PositionEntry};
    use crate::widget::switch::Switch;
    use crate::window::{PADDING, SPACING};

    const POS_UPDATE_DELAY: u64 = 500;

    const SW_ENABLED_NAME: &str = "sw_enabled";
    const DD_ROTATION_NAME: &str = "dd_rotation";
    const DD_REFLECTION_NAME: &str = "dd_reflection";
    const SC_SCALE_NAME: &str = "sc_scale";

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
        dd_rotation_selected_handler: RefCell<Option<SignalHandlerId>>,
        dd_reflection_selected_handler: RefCell<Option<SignalHandlerId>>,
        scale_value_changed_handler: RefCell<Option<SignalHandlerId>>,

        pub(super) screen_max_width: Cell<u16>,
        pub(super) screen_max_height: Cell<u16>,

        root: Box,
        fb_details: FlowBox,
        sw_enabled: Switch,
        mode_selector: ModeSelector,
        position_entry: PositionEntry,
        cb_primary: CheckButton,
        dd_rotation: DropDown,
        dd_reflection: DropDown,
        scale: Scale,
        vsep: Separator,
        tb_advanced: ToggleButton,
    }

    impl Default for DetailsBox {
        fn default() -> Self {
            let scale = Scale::with_range(Orientation::Horizontal, 1., 200., 1.);
            scale.set_slider_size_fixed(true);
            scale.set_draw_value(true);
            scale.set_value_pos(PositionType::Right);
            scale.set_value(100.);
            scale.add_mark(100., PositionType::Bottom, None);
            scale.set_flippable(true);
            scale.set_format_value_func(|_, v| format!("{:.2}", v / 100.));

            let tb_child = IconText::new(true);
            tb_child.set_icon_name("view-more-symbolic");
            tb_child.set_label(gettext("Advanced") + "\u{2026}");

            Self {
                output: RefCell::default(),

                enabled_changed_handler: RefCell::default(),
                mode_changed_handler: RefCell::default(),
                pos_changed_handlers: RefCell::default(),
                pos_modify_sids: RefCell::default(),
                primary_changed_handler: RefCell::default(),
                dd_rotation_selected_handler: RefCell::default(),
                dd_reflection_selected_handler: RefCell::default(),
                scale_value_changed_handler: RefCell::default(),

                screen_max_width: Cell::default(),
                screen_max_height: Cell::default(),

                root: Box::new(Orientation::Horizontal, PADDING.into()),
                fb_details: FlowBox::builder()
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
                dd_rotation: DropDown::from_strings(&[
                    &pgettext("Rotation", "Normal"),
                    // Translators: Rotate 270 degrees
                    &gettext("Left"),
                    // Translators: Rotate 90 degrees
                    &gettext("Right"),
                    // Translators: Rotate 180 degrees
                    &gettext("Inverted"),
                ]),
                dd_reflection: DropDown::from_strings(&[
                    &pgettext("Reflection", "Normal"),
                    // Translators: Horizontal reflection
                    &gettext("Horizontal"),
                    // Translators: Vertical reflection
                    &gettext("Vertical"),
                    // Translators: Horizontal and vertical reflection
                    &gettext("Both"),
                ]),
                scale,
                vsep: Separator::new(Orientation::Vertical),
                tb_advanced: ToggleButton::builder()
                    .visible(false)
                    .valign(Align::Center)
                    .child(&tb_child)
                    .tooltip_text(gettext("Advanced") + "\u{2026}")
                    .build(),
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

            self.fb_details.set_halign(Align::Fill);
            self.fb_details.set_hexpand(true);

            self.fb_details.append(&DetailsChild::new(
                // Output status
                &gettext("Enabled"),
                &self.sw_enabled,
                SW_ENABLED_NAME,
            ));
            self.fb_details.append(&DetailsChild::new(
                // Output mode consisting of resolution and refresh rate
                &gettext("Mode"),
                &self.mode_selector,
                "",
            ));
            self.fb_details.append(&DetailsChild::new(
                // Output position consisting of X and Y coordinate
                &gettext("Position"),
                &self.position_entry,
                "",
            ));
            self.fb_details.append(&DetailsChild::new(
                // Primary output status
                &gettext("Primary"),
                &self.cb_primary,
                "",
            ));
            self.fb_details.append(&DetailsChild::new(
                &gettext("Rotate"),
                &self.dd_rotation,
                DD_ROTATION_NAME,
            ));
            self.fb_details.append(&DetailsChild::new(
                &gettext("Reflect"),
                &self.dd_reflection,
                DD_REFLECTION_NAME,
            ));
            self.fb_details.append(&DetailsChild::new(
                &gettext("Scale"),
                &self.scale,
                SC_SCALE_NAME,
            ));
            self.scale.set_width_request(200);

            self.sw_enabled.connect_active_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |sw| this.on_enabled_switched(sw)
            ));
            self.mode_selector.connect_selected_mode_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |mode_selector| this.on_mode_selected(mode_selector)
            ));
            self.position_entry.connect_coordinate_changed(clone!(
                #[weak(rename_to = this)]
                self,
                move |_, axis, coord| this.update_position(axis, coord)
            ));
            self.cb_primary.connect_active_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |cb| this.on_primary_checked(cb)
            ));
            self.dd_rotation_selected_handler.replace(Some(
                self.dd_rotation.connect_selected_item_notify(clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |dd| this.on_rotation_selected(dd)
                )),
            ));
            self.dd_reflection_selected_handler.replace(Some(
                self.dd_reflection.connect_selected_item_notify(clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |dd| this.on_reflection_selected(dd)
                )),
            ));
            self.scale_value_changed_handler.replace(Some(self.scale.connect_value_changed(
                clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |s| this.on_scale_changed(s)
                ),
            )));
            self.tb_advanced.connect_toggled(clone!(
                #[weak(rename_to = this)]
                self,
                move |tb| this.on_advanced_toggle(tb)
            ));

            self.root.append(&self.tb_advanced);
            self.root.append(&self.vsep);
            self.root.append(&self.fb_details);
            self.root.set_parent(&*self.obj());
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

                self.position_entry.set_x(&output.x().to_string());
                self.position_entry.set_y(&output.y().to_string());

                self.position_entry.set_max_x(
                    self.screen_max_width
                        .get()
                        .saturating_sub(output.width())
                        .try_into()
                        .unwrap_or(i16::MAX),
                );
                self.position_entry.set_max_y(
                    self.screen_max_height
                        .get()
                        .saturating_sub(output.height())
                        .try_into()
                        .unwrap_or(i16::MAX),
                );

                self.mode_selector.set_modes(Some(output.modes()));
                self.mode_selector.set_selected_mode(output.mode());

                Self::select_pos(
                    &self.dd_rotation,
                    self.dd_rotation_selected_handler.borrow().as_ref(),
                    output.rotation().into(),
                );
                Self::select_pos(
                    &self.dd_reflection,
                    self.dd_reflection_selected_handler.borrow().as_ref(),
                    output.reflection().into(),
                );
                Self::set_scale(
                    &self.scale,
                    self.scale_value_changed_handler.borrow().as_ref(),
                    output.scale_x() * 100.,
                );
            } else {
                self.mode_selector.set_selected_mode(None::<Mode>);
                self.mode_selector.set_modes(None::<Modes>);

                Self::select_pos(
                    &self.dd_rotation,
                    self.dd_rotation_selected_handler.borrow().as_ref(),
                    INVALID_LIST_POSITION,
                );
                Self::select_pos(
                    &self.dd_reflection,
                    self.dd_reflection_selected_handler.borrow().as_ref(),
                    INVALID_LIST_POSITION,
                );
                Self::set_scale(
                    &self.scale,
                    self.scale_value_changed_handler.borrow().as_ref(),
                    100.,
                );
            }
            self.output.replace(output.cloned());
            self.update_visibility();
        }

        fn connect_output_property_handlers(&self, output: &Output) {
            self.enabled_changed_handler.replace(Some(output.connect_enabled_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |o| {
                    this.sw_enabled.set_active(o.enabled());
                    this.update_visibility();
                }
            ))));
            self.mode_changed_handler.replace(Some(output.connect_mode_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |o| this.mode_selector.set_selected_mode(o.mode())
            ))));
            self.primary_changed_handler.replace(Some(output.connect_primary_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |o| this.cb_primary.set_active(o.primary())
            ))));
            self.pos_changed_handlers.replace([
                Some(output.connect_pos_x_notify(clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |o| {
                        if let Some(sid) =
                            this.pos_modify_sids.borrow_mut()[usize::from(Axis::X)].take()
                        {
                            sid.remove();
                        }
                        this.position_entry.set_x(&o.x().to_string());
                    }
                ))),
                Some(output.connect_pos_y_notify(clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |o| {
                        if let Some(sid) =
                            this.pos_modify_sids.borrow_mut()[usize::from(Axis::Y)].take()
                        {
                            sid.remove();
                        }
                        this.position_entry.set_y(&o.y().to_string());
                    }
                ))),
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
            let mut child = self.fb_details.first_child();
            let output = self.output.borrow();
            while let Some(c) = child {
                c.set_visible(output.as_ref().is_some_and(|o| match c.widget_name().as_str() {
                    SW_ENABLED_NAME => true,
                    DD_ROTATION_NAME | DD_REFLECTION_NAME | SC_SCALE_NAME => {
                        o.enabled() && self.tb_advanced.is_active()
                    }
                    _ => o.enabled(),
                }));
                child = c.next_sibling();
            }
            let visible = output.as_ref().is_some_and(|o| o.enabled());
            self.vsep.set_visible(visible);
            self.tb_advanced.set_visible(visible);
        }

        fn on_advanced_toggle(&self, tb: &ToggleButton) {
            tb.set_tooltip_text(Some(
                &(if tb.is_active() { gettext("Basic") } else { gettext("Advanced") } + "\u{2026}"),
            ));
            let mut child = self.fb_details.first_child();
            while let Some(c) = child {
                match c.widget_name().as_str() {
                    DD_ROTATION_NAME | DD_REFLECTION_NAME | SC_SCALE_NAME => {
                        c.set_visible(tb.is_active())
                    }
                    _ => (),
                }
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

        fn on_rotation_selected(&self, dd: &DropDown) {
            if let Some(output) = self.output.borrow().as_ref() {
                let rotation = Rotation::from(dd.selected());
                if rotation != output.rotation() {
                    output.set_rotation(rotation);
                    self.notify_updated(output, Update::Rotation);
                }
            }
        }

        fn on_reflection_selected(&self, dd: &DropDown) {
            if let Some(output) = self.output.borrow().as_ref() {
                let reflection = Reflection::from(dd.selected());
                if reflection != output.reflection() {
                    output.set_reflection(reflection);
                    self.notify_updated(output, Update::Reflection);
                }
            }
        }

        fn on_scale_changed(&self, s: &Scale) {
            if let Some(output) = self.output.borrow().as_ref() {
                output.set_scale(s.value() / 100.);
                self.notify_updated(output, Update::Scale);
            }
        }

        fn update_position(&self, axis: Axis, coord: I16) {
            let coord = coord.get();
            if let Some(output) = self.output.borrow().as_ref() {
                if let Some(sid) = self.pos_modify_sids.borrow_mut()[usize::from(axis)].take() {
                    sid.remove();
                }

                let sid = timeout_add_local_once(
                    Duration::from_millis(POS_UPDATE_DELAY),
                    clone!(
                        #[weak(rename_to = this)]
                        self,
                        #[weak]
                        output,
                        move || {
                            this.pos_modify_sids.borrow_mut()[usize::from(axis)].take();
                            let cur_pos = match axis {
                                Axis::X => output.x(),
                                Axis::Y => output.y(),
                            };
                            if cur_pos != coord {
                                let set_coord = || {
                                    match axis {
                                        Axis::X => output.set_x(coord),
                                        Axis::Y => output.set_y(coord),
                                    };
                                };
                                if let Some(handler_id) =
                                    &this.pos_changed_handlers.borrow()[usize::from(axis)]
                                {
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

        fn select_pos(dd: &DropDown, hid: Option<&SignalHandlerId>, pos: u32) {
            if let Some(hid) = hid {
                dd.block_signal(hid);
            }
            dd.set_selected(pos);
            if let Some(hid) = hid {
                dd.unblock_signal(hid);
            }
        }

        fn set_scale(scale: &Scale, hid: Option<&SignalHandlerId>, value: f64) {
            if let Some(hid) = hid {
                scale.block_signal(hid);
            }
            scale.set_value(value);
            if let Some(hid) = hid {
                scale.unblock_signal(hid);
            }
        }
    }
}

wrapper! {
    pub struct DetailsBox(ObjectSubclass<imp::DetailsBox>) @extends Widget;
}

impl DetailsBox {
    pub fn new() -> Self { Object::new() }

    pub fn set_screen_max_width(&self, screen_max_width: u16) {
        self.imp().screen_max_width.set(screen_max_width);
    }

    pub fn set_screen_max_height(&self, screen_max_height: u16) {
        self.imp().screen_max_height.set(screen_max_height);
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

impl Default for DetailsBox {
    fn default() -> Self { Self::new() }
}

#[derive(ValueDelegate, Clone, Copy)]
#[value_delegate(from = u8)]
pub enum Update {
    Enabled,
    Disabled,
    Resolution,
    Refresh,
    Rotation,
    Reflection,
    Scale,
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
            4 => Update::Rotation,
            5 => Update::Reflection,
            6 => Update::Scale,
            7 => Update::Position,
            8 => Update::Primary,
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
