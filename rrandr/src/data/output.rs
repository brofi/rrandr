use glib::{wrapper, Object};
use gtk::glib;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use x11rb::protocol::randr::{Output as OutputId, Rotation as RRotation};

use super::enums::{Reflection, Rotation};
use super::values::U16;
use crate::data::mode::Mode;
use crate::data::modes::Modes;
use crate::data::values::I16;
use crate::math::{Rect, MM_PER_INCH};
use crate::utils::nearly_eq;

pub const PPI_DEFAULT: [f64; 2] = [96., 96.];
pub const PPMM_DEFAULT: [f64; 2] = [PPI_DEFAULT[0] / MM_PER_INCH, PPI_DEFAULT[1] / MM_PER_INCH];

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::ObjectSubclass;
    use glib::{derived_properties, object_subclass, Properties};
    use gtk::glib;
    use gtk::prelude::ObjectExt;
    use gtk::subclass::prelude::{DerivedObjectProperties, ObjectSubclassExt};
    use x11rb::protocol::randr::Output as OutputId;

    use crate::data::enums::{Reflection, Rotation};
    use crate::data::mode::Mode;
    use crate::data::modes::Modes;
    use crate::data::values::{I16, U16};

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::Output)]
    pub struct Output {
        #[property(get, set, construct_only)]
        id: Cell<OutputId>,
        #[property(get, set, construct_only)]
        name: RefCell<String>,
        #[property(get, set, nullable)]
        product_name: RefCell<Option<String>>,
        #[property(get, set)]
        enabled: Cell<bool>,
        #[property(get, set)]
        primary: Cell<bool>,
        #[property(get, set)]
        pos_y: Cell<I16>,
        #[property(get, set)]
        pos_x: Cell<I16>,
        #[property(get, set, construct_only)]
        modes: RefCell<Modes>,
        #[property(get, set = Self::set_mode, nullable)]
        mode: RefCell<Option<Mode>>,
        #[property(get, set = Self::set_rotation, builder(Rotation::default()))]
        rotation: Cell<Rotation>,
        #[property(get, set, builder(Reflection::default()))]
        reflection: Cell<Reflection>,
        #[property(get, set, construct_only, default = 1.)]
        pub(super) scale_x: Cell<f64>,
        #[property(get, set, construct_only, default = 1.)]
        pub(super) scale_y: Cell<f64>,
        #[property(set, construct_only)]
        pub(super) width: Cell<U16>,
        #[property(set, construct_only)]
        pub(super) height: Cell<U16>,
        #[property(get, set, construct_only)]
        mm_width: Cell<u32>,
        #[property(get, set, construct_only)]
        mm_height: Cell<u32>,
    }

    #[object_subclass]
    impl ObjectSubclass for Output {
        type Type = super::Output;

        const NAME: &'static str = "Output";
    }

    #[derived_properties]
    impl ObjectImpl for Output {}

    impl Output {
        fn set_mode(&self, mode: Option<&Mode>) {
            if mode == self.mode.borrow().as_ref() {
                return;
            }
            self.mode.take();
            if let Some(mode) = mode {
                if let Some(m) = self.modes.borrow().find_by_id(mode.id()) {
                    if *mode == m {
                        self.mode.set(Some(mode.clone()));
                    } else {
                        panic!("Different GObject with same Mode ID");
                    }
                } else {
                    panic!("No mode {} for output {}", mode.id(), self.id.get());
                }
            }
            self.update_dim();
        }

        fn set_rotation(&self, rotation: Rotation) {
            if self.rotation.get() != rotation {
                self.rotation.set(rotation);
                self.update_dim();
            }
        }

        pub(super) fn update_dim(&self) {
            let obj = self.obj();

            let m = self.mode.borrow();
            let Some(mode) = m.as_ref() else {
                if obj.width() != 0 {
                    self.width.take();
                    obj.notify_width();
                }
                if obj.height() != 0 {
                    self.height.take();
                    obj.notify_height();
                }
                return;
            };

            let [w, h] = match self.rotation.get() {
                Rotation::Normal | Rotation::Inverted => [mode.width(), mode.height()],
                Rotation::Left | Rotation::Right => [mode.height(), mode.width()],
            }
            .map(f64::from);
            let width = (w * self.scale_x.get()).round() as u16;
            if width != obj.width() {
                self.width.set(width.into());
                obj.notify_width();
            }
            let height = (h * self.scale_y.get()).round() as u16;
            if height != obj.height() {
                self.height.set(height.into());
                obj.notify_height();
            }
        }
    }
}

wrapper! {
    pub struct Output(ObjectSubclass<imp::Output>);
}

impl Output {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: OutputId,
        name: String,
        product_name: Option<String>,
        enabled: bool,
        primary: bool,
        pos: [i16; 2],
        mode: Option<Mode>,
        modes: Modes,
        rotation: Rotation,
        reflection: Reflection,
        scale: [f64; 2],
        dim: [u16; 2],
        mm_dim: [u32; 2],
    ) -> Output {
        Object::builder()
            .property("id", id)
            .property("name", name)
            .property("product-name", product_name)
            .property("enabled", enabled)
            .property("primary", primary)
            .property("pos-x", I16::from(pos[0]))
            .property("pos-y", I16::from(pos[1]))
            .property("modes", modes)
            .property("mode", mode)
            .property("rotation", rotation)
            .property("reflection", reflection)
            .property("scale-x", scale[0])
            .property("scale-y", scale[1])
            .property("width", U16::from(dim[0]))
            .property("height", U16::from(dim[1]))
            .property("mm-width", mm_dim[0])
            .property("mm-height", mm_dim[1])
            .build()
    }

    pub fn x(&self) -> i16 { self.pos_x().get() }

    pub fn y(&self) -> i16 { self.pos_y().get() }

    pub fn set_x(&self, x: i16) { self.set_pos_x(I16::from(x)) }

    pub fn set_y(&self, y: i16) { self.set_pos_y(I16::from(y)) }

    pub fn width(&self) -> u16 { self.imp().width.get().get() }

    pub fn height(&self) -> u16 { self.imp().height.get().get() }

    pub fn enable(&self) { self.enable_at(-1, -1); }

    pub fn enable_at(&self, x: i16, y: i16) {
        self.set_enabled(true);
        self.set_mode(Some(self.modes().first().expect("has mode")));
        self.set_x(x);
        self.set_y(y);
    }

    pub fn disable(&self) {
        self.set_enabled(false);
        self.set_primary(false);
        self.set_x(0);
        self.set_y(0);
        self.set_mode(None::<Mode>);
        self.set_rotation(Rotation::Normal);
        self.set_reflection(Reflection::Normal);
    }

    pub fn ppi(&self) -> [f64; 2] {
        if let Some(mode) = self.mode() {
            if self.mm_width() > 0 && self.mm_height() > 0 {
                return [
                    (MM_PER_INCH * f64::from(mode.width())) / f64::from(self.mm_width()),
                    (MM_PER_INCH * f64::from(mode.height())) / f64::from(self.mm_height()),
                ];
            }
        }
        PPI_DEFAULT
    }

    pub fn rect(&self) -> Rect { Rect::new(self.x(), self.y(), self.width(), self.height()) }

    pub fn randr_rotation(&self) -> RRotation {
        RRotation::from(self.rotation()) | RRotation::from(self.reflection())
    }

    pub fn set_scale(&self, scale: f64) {
        let imp = self.imp();
        if !nearly_eq(scale, imp.scale_x.get()) {
            imp.scale_x.set(scale);
            self.notify_scale_x();
        }
        if !nearly_eq(scale, imp.scale_y.get()) {
            imp.scale_y.set(scale);
            self.notify_scale_y();
        }
        imp.update_dim();
    }
}
