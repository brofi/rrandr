use glib::{wrapper, Object};
use gtk::glib;
use gtk::prelude::{CastNone, ListModelExt};
use x11rb::protocol::randr::Output as OutputId;

use super::mode::Mode;
use crate::data::modes::Modes;
use crate::math::{Rect, MM_PER_INCH};
use crate::utils::nearly_eq;

type Resolution = [u16; 2];

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
        #[property(get, set)]
        id: Cell<OutputId>,
        #[property(get, set)]
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
        #[property(get, set, nullable)]
        mode: RefCell<Option<Mode>>,
        #[property(get, set)]
        modes: RefCell<Modes>,
        #[property(get, set)]
        width: Cell<u32>,
        #[property(get, set)]
        height: Cell<u32>,
    }

    #[object_subclass]
    impl ObjectSubclass for Output {
        type Type = super::Output;

        const NAME: &'static str = "Output";
    }

    #[derived_properties]
    impl ObjectImpl for Output {}
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
            .property("mode", mode)
            .property("modes", modes)
            .property("width", width)
            .property("height", height)
            .build()
    }

    pub fn new_from(o: &Output) -> Output {
        Self::new(
            o.id(),
            o.name(),
            o.product_name(),
            o.enabled(),
            o.primary(),
            o.pos_x() as i16,
            o.pos_y() as i16,
            o.mode().map(|mode| Mode::new_from(&mode)),
            o.modes(),
            o.width(),
            o.height(),
        )
    }

    pub fn modes_vec(&self) -> Vec<Mode> {
        let mut modes = Vec::new();
        for i in 0..self.modes().n_items() {
            modes.push(self.modes().item(i).and_downcast::<Mode>().unwrap())
        }
        modes
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
        let mode: Option<Mode> = None;
        self.set_mode(mode);
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

    pub fn get_resolutions_dropdown(&self) -> Vec<String> {
        let resolutions = self.get_resolutions();
        let format_width =
            resolutions.iter().map(|r| r[1].to_string().len()).max().unwrap_or_default();
        resolutions.iter().map(|&r| Self::resolution_str(r, format_width)).collect::<Vec<String>>()
    }

    pub fn get_current_resolution_dropdown_index(&self) -> Option<usize> {
        if let Some(mode) = self.mode() {
            return self
                .get_resolutions()
                .iter()
                .position(|res: &Resolution| {
                    u32::from(res[0]) == mode.width() && u32::from(res[1]) == mode.height()
                })?
                .into();
        }
        None
    }

    pub fn resolution_dropdown_mode_index(&self, index: usize) -> usize {
        let res = self.get_resolutions()[index];
        self.modes_vec()
            .iter()
            .position(|m| m.width() == u32::from(res[0]) && m.height() == u32::from(res[1]))
            .unwrap()
    }

    pub fn refresh_rate_dropdown_mode_index(&self, resolution_index: usize, index: usize) -> usize {
        let res = self.get_resolutions()[resolution_index];
        let refresh = self.get_refresh_rates(resolution_index)[index];
        self.modes_vec()
            .iter()
            .position(|m| {
                m.width() == u32::from(res[0])
                    && m.height() == u32::from(res[1])
                    && nearly_eq(m.refresh(), refresh)
            })
            .unwrap()
    }

    pub fn get_current_refresh_rate_dropdown_index(
        &self,
        resolution_index: usize,
    ) -> Option<usize> {
        if let Some(mode) = self.mode() {
            return self
                .get_refresh_rates(resolution_index)
                .iter()
                .position(|&refresh| nearly_eq(refresh, mode.refresh()))?
                .into();
        }
        None
    }

    pub fn get_refresh_rates_dropdown(&self, resolution_index: usize) -> Vec<String> {
        self.get_refresh_rates(resolution_index)
            .iter()
            .map(|&r| Self::refresh_str(r))
            .collect::<Vec<String>>()
    }

    fn get_resolutions(&self) -> Vec<Resolution> {
        let mut dd_list = Vec::new();
        for mode in self.modes_vec() {
            let r = [mode.width() as u16, mode.height() as u16];
            if !dd_list.contains(&r) {
                dd_list.push(r);
            }
        }
        dd_list
    }

    fn get_refresh_rates(&self, resolution_index: usize) -> Vec<f64> {
        let res = self.get_resolutions()[resolution_index];
        self.modes_vec()
            .iter()
            .filter(|m| m.width() == u32::from(res[0]) && m.height() == u32::from(res[1]))
            .map(|m| m.refresh())
            .collect::<Vec<f64>>()
    }

    fn resolution_str(res: Resolution, format_width: usize) -> String {
        let [w, h] = res;
        format!("{w} x {h:<format_width$}")
    }

    fn refresh_str(refresh: f64) -> String { format!("{refresh:.2} Hz") }

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
