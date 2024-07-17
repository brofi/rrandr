use gio::ListModel;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::{ListModelExt, ListModelExtManual};
use gtk::{gio, glib};
use x11rb::protocol::randr::Mode as ModeId;

use crate::data::mode::Mode;

mod imp {
    use std::cell::RefCell;

    use gio::ListModel;
    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::ObjectSubclass;
    use glib::{object_subclass, Object, Type};
    use gtk::prelude::{Cast, StaticType};
    use gtk::subclass::prelude::ListModelImpl;
    use gtk::{gio, glib};

    use crate::data::mode::Mode;

    #[derive(Default)]
    pub struct Modes(pub(super) RefCell<Vec<Mode>>);

    #[object_subclass]
    impl ObjectSubclass for Modes {
        type Interfaces = (ListModel,);
        type Type = super::Modes;

        const NAME: &'static str = "Modes";
    }

    impl ObjectImpl for Modes {}

    impl ListModelImpl for Modes {
        fn item_type(&self) -> Type { Mode::static_type() }

        fn n_items(&self) -> u32 {
            self.0.borrow().len().try_into().expect("ListModel should have less items")
        }

        fn item(&self, position: u32) -> Option<Object> {
            self.0.borrow().get(position as usize).map(|o| o.clone().upcast::<Object>())
        }
    }
}

wrapper! {
    pub struct Modes(ObjectSubclass<imp::Modes>) @implements ListModel;
}

impl Modes {
    pub fn new() -> Modes { Object::new() }

    pub fn append(&self, mode: &Mode) {
        let index = {
            let mut modes = self.imp().0.borrow_mut();
            modes.push(mode.clone());
            u32::try_from(modes.len() - 1).expect("ListModel should have less items")
        };
        self.items_changed(index, 0, 1);
    }

    pub fn find_by_id(&self, mode: ModeId) -> Option<Mode> {
        self.imp().0.borrow().iter().find(|&m| m.id() == mode).cloned()
    }

    pub fn position_by_res(&self, width: u16, height: u16) -> Option<u32> {
        self.imp()
            .0
            .borrow()
            .iter()
            .position(|m| m.width() == width && m.height() == height)
            .map(|i| i.try_into().expect("smaller position"))
    }

    pub fn contains_res(&self, width: u16, height: u16) -> bool {
        self.position_by_res(width, height).is_some()
    }

    pub fn position(&self, mode: &Mode) -> Option<u32> {
        self.imp()
            .0
            .borrow()
            .iter()
            .position(|m| m == mode)
            .map(|i| i.try_into().expect("smaller position"))
    }

    pub fn resolutions(&self) -> Self {
        let resolution_modes = Self::new();
        for mode in self.iter::<Mode>().map(Result::unwrap) {
            if !resolution_modes.contains_res(mode.width(), mode.height()) {
                resolution_modes.append(&mode);
            }
        }
        resolution_modes
    }

    pub fn refresh_rates(&self, res_mode: &Mode) -> Self {
        let refresh_rate_modes = Self::new();
        for mode in self.iter::<Mode>().map(Result::unwrap) {
            if mode.width() == res_mode.width() && mode.height() == res_mode.height() {
                refresh_rate_modes.append(&mode);
            }
        }
        refresh_rate_modes
    }
}

impl Default for Modes {
    fn default() -> Self { Object::new() }
}
