#![allow(clippy::module_name_repetitions)]

use cairo::Context;
use config::Config;
use gdk::prelude::GdkCairoContextExt;
use pango::ffi::PANGO_SCALE;
use pango::{Alignment, FontDescription, Layout, Weight};
use pangocairo::functions::{create_layout, show_layout};

use crate::math::Rect;
use crate::window::PADDING;

pub const SCREEN_LINE_WIDTH: f64 = 2.;
const SELECTION_LINE_WIDTH: f64 = 4.;

pub struct DrawContext {
    cairo: Context,
    config: Config,
}

impl DrawContext {
    pub fn new(cairo: &Context, config: &Config) -> Self {
        DrawContext { cairo: cairo.clone(), config: config.clone() }
    }

    pub fn draw_screen(&self, rect: [f64; 4]) {
        self.cairo.rectangle(
            rect[0] - SCREEN_LINE_WIDTH / 2.,
            rect[1] - SCREEN_LINE_WIDTH / 2.,
            rect[2] + SCREEN_LINE_WIDTH,
            rect[3] + SCREEN_LINE_WIDTH,
        );
        self.cairo.set_source_color(&self.config.bounds_color().into());
        self.cairo.set_line_width(SCREEN_LINE_WIDTH);
        self.cairo.set_dash(&[4.], 1.);
        self.cairo.stroke().unwrap();
    }

    pub fn draw_output(&self, rect: [f64; 4]) {
        self.cairo.rectangle(rect[0], rect[1], rect[2], rect[3]);
        self.cairo.set_source_color(&self.config.output_color().to_rgba(0.75));
        self.cairo.fill().unwrap();
    }

    pub fn draw_selected_output(&self, rect: [f64; 4]) {
        self.cairo.rectangle(
            rect[0] + SELECTION_LINE_WIDTH / 2.,
            rect[1] + SELECTION_LINE_WIDTH / 2.,
            rect[2] - SELECTION_LINE_WIDTH,
            rect[3] - SELECTION_LINE_WIDTH,
        );
        self.cairo.set_source_color(&self.config.selection_color().into());
        self.cairo.set_line_width(SELECTION_LINE_WIDTH);
        self.cairo.set_dash(&[1., 0.], 0.);
        self.cairo.stroke().unwrap();
    }

    pub fn draw_output_label(&self, rect: [f64; 4], name: &str, product_name: Option<&str>) {
        self.cairo.save().unwrap();

        let layout = create_layout(&self.cairo);
        layout.set_alignment(Alignment::Center);
        layout.set_text(product_name.unwrap_or(name));

        let mut desc = FontDescription::new();
        desc.set_family(&self.config.font.family);
        desc.set_weight(Weight::Bold);
        desc.set_size(i32::from(self.config.font.size) * PANGO_SCALE);

        layout.set_font_description(Some(&desc));

        let ps = layout.pixel_size();
        if f64::from(ps.0) <= rect[2] - f64::from(PADDING) * 2.
            && f64::from(ps.1) <= rect[3] - f64::from(PADDING) * 2.
        {
            self.cairo.set_source_color(&self.config.text_color().into());
            self.cairo.move_to(rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.);
            self.cairo.rel_move_to(f64::from(-ps.0) / 2., f64::from(-ps.1) / 2.);
            show_layout(&self.cairo, &layout);
        }
        self.cairo.restore().unwrap();
    }

    pub fn draw_popup(&self, rect: &Rect, pad: f64, text: &str) -> Result<(), cairo::Error> {
        self.cairo.set_source_color(&self.config.output_color().to_rgba(0.75));
        self.cairo.rectangle(0., 0., f64::from(rect.width()), f64::from(rect.height()));
        self.cairo.fill()?;

        self.cairo.set_source_color(&self.config.text_color().into());
        let layout = self.pango_layout_popup(rect.width(), rect.height(), pad, text);
        let (w, h) = layout.pixel_size();
        self.cairo.move_to(
            f64::from(i32::from(rect.width()) - w) / 2.,
            f64::from(i32::from(rect.height()) - h) / 2.,
        );
        show_layout(&self.cairo, &layout);
        Ok(())
    }

    fn pango_layout_popup(&self, width: u16, height: u16, pad: f64, text: &str) -> Layout {
        let pscale = f64::from(PANGO_SCALE);
        let height = (f64::from(height) - (2. * pad)).round().max(0.);
        let width = (f64::from(width) - (2. * pad)).round().max(0.);
        let layout = create_layout(&self.cairo);
        layout.set_text(text);

        let mut desc = FontDescription::new();
        desc.set_family(&self.config.popup.font.family);
        desc.set_weight(Weight::Bold);

        if self.config.popup.font.size.is_some_and(|size| {
            desc.set_size(i32::from(size) * PANGO_SCALE);
            layout.set_font_description(Some(&desc));
            let (w, h) = layout.pixel_size();
            f64::from(w) < width && f64::from(h) < height
        }) {
        } else {
            // Set absolute pixel size to height
            let size = height * pscale;
            desc.set_absolute_size(size);
            layout.set_font_description(Some(&desc));

            // Get the actual pixel height reported by pango and scale it so it fits the
            // desired height
            let size = size * (height / f64::from(layout.pixel_size().1));
            desc.set_absolute_size(size);
            layout.set_font_description(Some(&desc));

            // Get the actual pixel width of the new size and scale it down so it fits the
            // width
            let w = layout.pixel_size().0;
            if f64::from(w) > width {
                let size = size * (width / f64::from(w));
                desc.set_absolute_size(size);
                layout.set_font_description(Some(&desc));
            }
        }

        layout
    }
}
