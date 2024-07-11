use std::cell::{Cell, RefCell};
use std::num::IntErrorKind;
use std::sync::OnceLock;

use glib::object::CastNone;
use glib::subclass::object::{ObjectImpl, ObjectImplExt};
use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use glib::subclass::Signal;
use glib::{clone, derived_properties, object_subclass, Properties, SignalHandlerId};
use gtk::prelude::{CheckButtonExt, ListModelExt, ObjectExt, StaticType, WidgetExt};
use gtk::subclass::prelude::DerivedObjectProperties;
use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
use gtk::{glib, Align, BinLayout, FlowBox, Orientation, SelectionMode, StringList, Widget};

use super::Update;
use crate::data::mode::Mode;
use crate::widget::checkbutton::CheckButton;
use crate::widget::details_child::DetailsChild;
use crate::widget::mode_selector::ModeSelector;
use crate::widget::position_entry::PositionEntry;
use crate::widget::switch::Switch;
use crate::widget::window::{Axis, SPACING};
use crate::Output;

#[derive(Properties)]
#[properties(wrapper_type = super::DetailsBox)]
pub struct DetailsBox {
    #[property(get, set = Self::set_output, nullable)]
    output: RefCell<Option<Output>>,
    enabled_changed_handler: RefCell<Option<SignalHandlerId>>,
    pos_x_changed_handler: RefCell<Option<SignalHandlerId>>,
    pos_y_changed_handler: RefCell<Option<SignalHandlerId>>,
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
            output: Default::default(),
            enabled_changed_handler: Default::default(),
            pos_x_changed_handler: Default::default(),
            pos_y_changed_handler: Default::default(),
            screen_max_width: Default::default(),
            screen_max_height: Default::default(),
            root: FlowBox::builder()
                .row_spacing(SPACING.into())
                .column_spacing(SPACING.into())
                .orientation(Orientation::Horizontal)
                .selection_mode(SelectionMode::None)
                .max_children_per_line(u32::MAX)
                .build(),
            sw_enabled: Switch::new("Enable/Disable"),
            mode_selector: ModeSelector::new(),
            position_entry: PositionEntry::new(),
            cb_primary: CheckButton::new("Set as primary"),
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

        self.root.append(&DetailsChild::new("Enabled", &self.sw_enabled));
        self.root.append(&DetailsChild::new("Mode", &self.mode_selector));
        self.root.append(&DetailsChild::new("Position", &self.position_entry));
        self.root.append(&DetailsChild::new("Primary", &self.cb_primary));

        self.sw_enabled.connect_active_notify(clone!(
            @weak self as this => move |sw| this.on_enabled_switched(sw)
        ));
        self.mode_selector.connect_resolution_selected(clone!(
            @weak self as this => move |dd| this.on_resolution_selected(dd)
        ));
        self.mode_selector.connect_refresh_rate_selected(clone!(
            @weak self as this => move |dd| this.on_refresh_rate_selected(dd)
        ));
        self.position_entry.connect_insert_x(clone!(
            @weak self as this => move |entry, text, position| this.on_position_insert(entry, text, position, Axis::X)
        ));
        self.position_entry.connect_delete_x(clone!(
            @weak self as this => move |entry, start, end| this.on_position_delete(entry, start, end, Axis::X)
        ));
        self.position_entry.connect_insert_y(clone!(
            @weak self as this => move |entry, text, position| this.on_position_insert(entry, text, position, Axis::Y)
        ));
        self.position_entry.connect_delete_y(clone!(
            @weak self as this => move |entry, start, end| this.on_position_delete(entry, start, end, Axis::Y)
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
        if let Some(handler_id) = self.enabled_changed_handler.take() {
            if let Some(output) = self.output.borrow().as_ref() {
                output.disconnect(handler_id);
            }
        }
        if let (Some(x_handler), Some(y_handler)) =
            (self.pos_x_changed_handler.take(), self.pos_y_changed_handler.take())
        {
            if let Some(output) = self.output.borrow().as_ref() {
                output.disconnect(x_handler);
                output.disconnect(y_handler);
            }
        }
        if let Some(output) = output {
            self.sw_enabled.set_active(output.enabled());
            self.enabled_changed_handler.replace(Some(output.connect_enabled_notify(clone!(
                @weak self as this => move |o| {
                    if o.enabled() != this.sw_enabled.is_active() {
                        this.sw_enabled.set_active(o.enabled());
                    }
                    this.update_visibility();
                }
            ))));
            self.cb_primary.set_active(output.primary());

            self.position_entry.set_x(&output.pos_x().to_string());
            self.position_entry.set_y(&output.pos_y().to_string());
            self.pos_x_changed_handler.replace(Some(output.connect_pos_x_notify(clone!(
                @weak self.position_entry as pos => move |o| pos.set_x(&o.pos_x().to_string())
            ))));
            self.pos_y_changed_handler.replace(Some(output.connect_pos_y_notify(clone!(
                @weak self.position_entry as pos => move |o| pos.set_y(&o.pos_y().to_string())
            ))));

            let resolutions = output.get_resolutions_dropdown();
            self.mode_selector.set_resolutions(Some(&into_string_list(&resolutions)));
            if let Some(res_idx) = output.get_current_resolution_dropdown_index() {
                self.mode_selector
                    .set_resolution(u32::try_from(res_idx).expect("less resolutions"));
                let refresh_rates = output.get_refresh_rates_dropdown(res_idx);
                self.mode_selector.set_refresh_rates(Some(&into_string_list(&refresh_rates)));
                if let Some(ref_idx) = output.get_current_refresh_rate_dropdown_index(res_idx) {
                    self.mode_selector
                        .set_refresh_rate(u32::try_from(ref_idx).expect("less refresh rates"));
                }
            }
        }
        self.output.replace(output.cloned());
        self.update_visibility();
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
        let mut updated = None;
        let mut update = None;
        if let Some(output) = self.output.borrow().as_ref() {
            let active = sw.is_active();
            // Update output
            if active {
                output.enable();
                update = Some(Update::Enabled);
            } else {
                output.disable();
                update = Some(Update::Disabled);
            }
            updated = Some(output.clone());
        }
        if let (Some(updated), Some(update)) = (updated, update) {
            self.notify_updated(&updated, &update);
        }
    }

    fn on_resolution_selected(&self, dd: &gtk::DropDown) {
        let mut updated = None;
        if let Some(output) = self.output.borrow().as_ref() {
            if !output.enabled() {
                return;
            }

            let dd_selected = dd.selected() as usize;

            // Update current mode
            let mode = output
                .modes()
                .item(output.resolution_dropdown_mode_index(dd_selected) as u32)
                .and_downcast::<Mode>()
                .unwrap();
            if output.mode().is_some_and(|m| m.id() != mode.id()) || output.mode().is_none() {
                output.set_mode(Some(mode));
                updated = Some(output.clone());
            }

            // Update refresh rate dropdown
            self.mode_selector.set_refresh_rates(Some(&into_string_list(
                &output.get_refresh_rates_dropdown(dd_selected),
            )));
            if let Some(idx) = output.get_current_refresh_rate_dropdown_index(dd_selected) {
                self.mode_selector
                    .set_refresh_rate(u32::try_from(idx).expect("less refresh rates"));
            }
        }
        if let Some(updated) = updated {
            self.notify_updated(&updated, &Update::Resolution);
        }
    }

    fn on_refresh_rate_selected(&self, dd: &gtk::DropDown) {
        if let Some(output) = self.output.borrow().as_ref() {
            if !output.enabled() {
                return;
            }

            // Update current mode
            let mode = &output
                .modes()
                .item(output.refresh_rate_dropdown_mode_index(
                self.mode_selector.get_resolution() as usize,
                dd.selected() as usize,
                ) as u32)
                .and_downcast::<Mode>()
            .unwrap();
            if output.mode().is_some_and(|m| m.id() != mode.id()) || output.mode().is_none() {
                output.set_mode(Some(mode));
                self.notify_updated(output, &Update::Refresh);
            }
        }
    }

    fn on_position_insert(
        &self,
        entry: &PositionEntry,
        text: &str,
        position: &mut i32,
        axis: Axis,
    ) {
        let idx = usize::try_from(*position).expect("smaller position");
        let mut new_text = entry.text(axis).to_string();
        new_text.insert_str(idx, text);
        if let Some(coord) = self.parse_coord(&new_text, axis) {
            if coord.to_string() == new_text {
                entry.insert_text(text, position, axis);
            } else if coord.to_string() != entry.text(axis) {
                entry.set_text(&coord.to_string(), axis);
            }
            self.update_position(axis, coord);
        } else if entry.text(axis).is_empty() {
            entry.insert_text("0", &mut 0, axis);
        }
    }

    fn on_position_delete(&self, entry: &PositionEntry, start_pos: i32, end_pos: i32, axis: Axis) {
        let start_idx = usize::try_from(start_pos).expect("smaller start position");
        let end_idx = usize::try_from(end_pos).expect("smaller end position");
        let mut new_text = entry.text(axis).to_string();
        new_text.replace_range(start_idx..end_idx, "");
        if let Some(coord) = self.parse_coord(&new_text, axis) {
            if coord.to_string() == new_text {
                entry.delete_text(start_pos, end_pos, axis);
            } else {
                entry.set_text(&coord.to_string(), axis);
            }
            self.update_position(axis, coord);
        } else {
            entry.delete_text(start_pos, end_pos, axis);
            self.update_position(axis, 0);
        }
    }

    fn parse_coord(&self, text: &str, axis: Axis) -> Option<i16> {
        if let Some(output) = self.output.borrow().as_ref() {
            if let Some(mode) = output.mode() {
                let max = match axis {
                    Axis::X => {
                        i16::try_from(self.screen_max_width.get().saturating_sub(mode.width()))
                            .unwrap_or(i16::MAX)
                    }
                    Axis::Y => {
                        i16::try_from(self.screen_max_height.get().saturating_sub(mode.height()))
                            .unwrap_or(i16::MAX)
                    }
                };
                return match text
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<i16>()
                {
                    Ok(c) => Some(c.min(max)),
                    Err(e) => match e.kind() {
                        IntErrorKind::PosOverflow => Some(max),
                        _ => None,
                    },
                };
            }
        }
        None
    }

    fn update_position(&self, axis: Axis, coord: i16) {
        if let Some(output) = self.output.borrow().as_ref() {
            let (new_x, new_y) = match axis {
                Axis::X => (coord, output.pos_y() as i16),
                Axis::Y => (output.pos_x() as i16, coord),
            };
            if new_x != output.pos_x() as i16 || new_y != output.pos_y() as i16 {
                output.set_pos_x(new_x as i32);
                output.set_pos_y(new_y as i32);
                self.notify_updated(output, &Update::Position);
            }
        }
    }

    fn on_primary_checked(&self, cb: &gtk::CheckButton) {
        if let Some(output) = self.output.borrow().as_ref() {
            output.set_primary(output.enabled() && cb.is_active());
            self.notify_updated(output, &Update::Primary);
        }
    }

    fn notify_updated(&self, output: &Output, update: &Update) {
        self.obj().emit_by_name::<()>("output-changed", &[output, update]);
    }
}

fn into_string_list(list: &[String]) -> StringList {
    let list = list.iter().map(String::as_str).collect::<Vec<&str>>();
    StringList::new(list.as_slice())
}
