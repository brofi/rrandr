use std::collections::HashMap;

use gio::ListModel;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::ListModelExt;
use gtk::{gio, glib};
use x11rb::protocol::randr::{Mode as ModeId, ModeInfo};

use crate::data::mode::Mode;
use crate::x11::randr::OutputInfo;

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

        fn n_items(&self) -> u32 { self.0.borrow().len() as u32 }

        fn item(&self, position: u32) -> Option<Object> {
            self.0.borrow().get(position as usize).map(|o| o.clone().upcast::<Object>())
        }
    }
}

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
