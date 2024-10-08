use gio::ListModel;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{wrapper, Object};
use gtk::prelude::ListModelExt;
use gtk::{gio, glib};
use x11rb::protocol::randr::Output as OutputId;

use crate::data::output::Output;

mod imp {
    use std::cell::RefCell;

    use gio::ListModel;
    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::ObjectSubclass;
    use glib::{object_subclass, Object, Type};
    use gtk::prelude::{Cast, StaticType};
    use gtk::subclass::prelude::ListModelImpl;
    use gtk::{gio, glib};

    use crate::data::output::Output;

    #[derive(Default)]
    pub struct Outputs(pub(super) RefCell<Vec<Output>>);

    #[object_subclass]
    impl ObjectSubclass for Outputs {
        type Interfaces = (ListModel,);
        type Type = super::Outputs;

        const NAME: &'static str = "Outputs";
    }

    impl ObjectImpl for Outputs {}

    impl ListModelImpl for Outputs {
        fn item_type(&self) -> Type { Output::static_type() }

        fn n_items(&self) -> u32 {
            self.0.borrow().len().try_into().expect("ListModel should have less items")
        }

        fn item(&self, position: u32) -> Option<Object> {
            self.0.borrow().get(position as usize).map(|o| o.clone().upcast::<Object>())
        }
    }
}

wrapper! {
    pub struct Outputs(ObjectSubclass<imp::Outputs>) @implements ListModel;
}

impl Outputs {
    pub fn new() -> Outputs { Object::new() }

    pub fn first(&self) -> Option<Output> { self.imp().0.borrow().first().cloned() }

    pub fn append(&self, output: &Output) {
        let index = {
            let mut outputs = self.imp().0.borrow_mut();
            outputs.push(output.clone());
            u32::try_from(outputs.len() - 1).expect("ListModel should have less items")
        };
        self.items_changed(index, 0, 1);
    }

    pub fn remove(&self, output_id: OutputId) -> Output {
        let mut outputs = self.imp().0.borrow_mut();
        let index = outputs
            .iter()
            .position(|other| other.id() == output_id)
            .unwrap_or_else(|| panic!("no output {output_id}"));
        let removed = outputs.remove(index);
        self.items_changed(index.try_into().expect("ListModel should have less items"), 1, 0);
        removed
    }

    pub fn push_back(&self, output: &Output) { self.append(&self.remove(output.id())); }

    pub fn index(&self, index: usize) -> Output { self.imp().0.borrow()[index].clone() }

    pub fn position(&self, output: &Output) -> Option<u32> {
        self.imp()
            .0
            .borrow()
            .iter()
            .position(|o| o == output)
            .map(|i| i.try_into().expect("smaller position"))
    }

    pub fn find_by_id(&self, output: OutputId) -> Option<Output> {
        self.imp().0.borrow().iter().find(|&o| o.id() == output).cloned()
    }

    pub fn to_vec(&self) -> Vec<Output> { self.imp().0.borrow().to_vec() }
}

impl Default for Outputs {
    fn default() -> Self { Object::new() }
}
