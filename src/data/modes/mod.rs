mod imp;

use std::collections::HashMap;

use gio::ListModel;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::ListModelExt;
use gtk::{gio, glib};
use x11rb::protocol::randr::{Mode as ModeId, ModeInfo};

use crate::data::mode::Mode;
use crate::x11::randr::OutputInfo;

wrapper! {
    pub struct Modes(ObjectSubclass<imp::Modes>) @implements ListModel;
}

impl Modes {
    pub fn new(output_info: &OutputInfo, modes: &HashMap<ModeId, ModeInfo>) -> Modes {
        let modes_model: Modes = Object::new();
        for mode_id in &output_info.modes {
            modes_model.append(&Mode::from(modes[mode_id]));
        }
        modes_model
    }

    fn append(&self, mode: &Mode) {
        let index = {
            let mut modes = self.imp().0.borrow_mut();
            modes.push(mode.clone());
            (modes.len() - 1) as u32
        };
        self.items_changed(index, 0, 1);
    }
}

impl Default for Modes {
    fn default() -> Self { Object::new() }
}
