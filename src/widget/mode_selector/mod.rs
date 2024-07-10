mod imp;

use gio::ListModel;
use glib::object::{IsA, ObjectExt};
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object, SignalHandlerId};
use gtk::{gio, glib, DropDown, Widget};

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
