mod imp;

use gdk::gio::ListModel;
use gdk::glib::subclass::types::ObjectSubclassIsExt;
use gdk::glib::{wrapper, Object};
use gdk::prelude::ListModelExt;
use x11rb::protocol::randr::Output as OutputId;

use crate::data::output::Output;

wrapper! {
    pub struct Outputs(ObjectSubclass<imp::Outputs>) @implements ListModel;
}

impl Outputs {
    pub fn new(outputs: &Vec<Output>) -> Outputs {
        let obj: Outputs = Object::new();
        for output in outputs {
            obj.append(output);
        }
        obj
    }

    pub fn append(&self, output: &Output) {
        let index = {
            let mut outputs = self.imp().0.borrow_mut();
            outputs.push(output.clone());
            (outputs.len() - 1) as u32
        };
        self.items_changed(index, 0, 1);
    }

    pub fn remove(&self, output_id: OutputId) -> Output {
        let mut outputs = self.imp().0.borrow_mut();
        let index = outputs
            .iter()
            .position(|other| other.id() == output_id)
            .unwrap_or_else(|| panic!("no output {}", output_id));
        let removed = outputs.remove(index);
        self.items_changed(index as u32, 1, 0);
        removed
    }

    pub fn push_back(&self, output: &Output) { self.append(&self.remove(output.id())); }

    pub fn index(&self, index: usize) -> Output { self.imp().0.borrow()[index].clone() }

    pub fn to_vec(&self) -> Vec<Output> { self.imp().0.borrow().to_vec() }
}

impl Default for Outputs {
    fn default() -> Self { Object::new() }
}
