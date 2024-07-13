use glib::{wrapper, Object};
use gtk::glib;
use gtk::prelude::{CastNone, ListModelExt};
use x11rb::protocol::randr::Output as OutputId;

use crate::data::mode::Mode;
use crate::data::modes::Modes;
use crate::math::{Rect, MM_PER_INCH};

pub const PPI_DEFAULT: u8 = 96;

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::ObjectSubclass;
    use glib::{derived_properties, object_subclass, Properties};
    use gtk::glib;
    use gtk::prelude::ObjectExt;
    use gtk::subclass::prelude::DerivedObjectProperties;
    use x11rb::protocol::randr::Output as OutputId;

    use crate::data::mode::Mode;
    use crate::data::modes::Modes;

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
        #[property(get, set, maximum = i16::MAX.into())]
        pos_y: Cell<i32>,
        #[property(get, set, maximum = i16::MAX.into())]
        pos_x: Cell<i32>,
        #[property(get, set, construct_only)]
        modes: RefCell<Modes>,
        #[property(get, set = Self::set_mode, nullable)]
        mode: RefCell<Option<Mode>>,
        #[property(get, set, construct_only)]
        width: Cell<u32>,
        #[property(get, set, construct_only)]
        height: Cell<u32>,
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
        }
    }
}

wrapper! {
    pub struct Output(ObjectSubclass<imp::Output>);
}

impl Output {
    pub fn new(
        id: OutputId,
        name: String,
        product_name: Option<String>,
        enabled: bool,
        primary: bool,
        pos_x: i16,
        pos_y: i16,
        mode: Option<Mode>,
        modes: Modes,
        width: u32,
        height: u32,
    ) -> Output {
        Object::builder()
            .property("id", id)
            .property("name", name)
            .property("product-name", product_name)
            .property("enabled", enabled)
            .property("primary", primary)
            .property("pos-x", i32::from(pos_x))
            .property("pos-y", i32::from(pos_y))
            .property("modes", modes)
            .property("mode", mode)
            .property("width", width)
            .property("height", height)
            .build()
    }

    pub fn enable(&self) { self.enable_at(-1, -1); }

    pub fn enable_at(&self, x: i16, y: i16) {
        self.set_enabled(true);
        self.set_mode(Some(self.modes().item(0).and_downcast::<Mode>().expect("has mode")));
        self.set_pos_x(i32::from(x));
        self.set_pos_y(i32::from(y));
    }

    pub fn disable(&self) {
        self.set_enabled(false);
        self.set_primary(false);
        self.set_pos_x(0);
        self.set_pos_y(0);
        self.set_mode(None::<Mode>);
    }

    pub fn ppi(&self) -> f64 {
        if let Some(mode) = self.mode() {
            if self.pos_y() > 0 {
                return (f64::from(MM_PER_INCH) * f64::from(mode.height()))
                    / f64::from(self.pos_y());
            }
        }
        f64::from(PPI_DEFAULT)
    }

    pub fn rect(&self) -> Rect {
        if let Some(mode) = self.mode() {
            return Rect::new(
                self.pos_x() as i16,
                self.pos_y() as i16,
                mode.width() as u16,
                mode.height() as u16,
            );
        };
        Rect::default()
    }
}
