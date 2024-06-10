#![allow(clippy::module_name_repetitions)]

use cairo::Context;
use gdk::prelude::GdkCairoContextExt;
use pango::ffi::PANGO_SCALE;
use pango::{Alignment, FontDescription, Layout, Weight};
use pangocairo::functions::{create_layout, show_layout};

use crate::config::Config;
use crate::math::Rect;
use crate::view::PADDING;
use crate::POPUP_WINDOW_PAD;

pub const SCREEN_LINE_WIDTH: f64 = 2.;
const SELECTION_LINE_WIDTH: f64 = 4.;

pub struct DrawContext {
    cairo: Context,
    config: Config,
}

impl DrawContext {
    pub fn new(cairo: Context, config: Config) -> Self { DrawContext { cairo, config } }

    pub fn draw_screen(&self, rect: [f64; 4]) {
        self.cairo.rectangle(
            rect[0] - SCREEN_LINE_WIDTH / 2.,
            rect[1] - SCREEN_LINE_WIDTH / 2.,
            rect[2] + SCREEN_LINE_WIDTH,
            rect[3] + SCREEN_LINE_WIDTH,
        );
        self.cairo.set_source_color(&self.config.colors.bounds.clone().into());
        self.cairo.set_line_width(SCREEN_LINE_WIDTH);
        self.cairo.set_dash(&[4.], 1.);
        self.cairo.stroke().unwrap();
    }

    pub fn draw_output(&self, rect: [f64; 4]) {
        self.cairo.rectangle(rect[0], rect[1], rect[2], rect[3]);
        self.cairo.set_source_color(&self.config.colors.output.to_rgba(0.75));
        self.cairo.fill().unwrap();
    }

    pub fn draw_selected_output(&self, rect: [f64; 4]) {
        self.cairo.rectangle(
            rect[0] + SELECTION_LINE_WIDTH / 2.,
            rect[1] + SELECTION_LINE_WIDTH / 2.,
            rect[2] - SELECTION_LINE_WIDTH,
            rect[3] - SELECTION_LINE_WIDTH,
        );
        self.cairo.set_source_color(&self.config.colors.selection.clone().into());
        self.cairo.set_line_width(SELECTION_LINE_WIDTH);
        self.cairo.set_dash(&[1., 0.], 0.);
        self.cairo.stroke().unwrap();
    }

    pub fn draw_output_label(&self, rect: [f64; 4], name: &str, product_name: Option<&str>) {
        self.cairo.save().unwrap();
        let mut desc = FontDescription::new();
        desc.set_family("monospace");
        // desc.set_size(12);
        desc.set_weight(Weight::Bold);

        let layout = create_layout(&self.cairo);
        layout.set_font_description(Some(&desc));
        layout.set_alignment(Alignment::Center);
        layout.set_text(product_name.unwrap_or(name));

        let ps = layout.pixel_size();
        if f64::from(ps.0) <= rect[2] - f64::from(PADDING) * 2.
            && f64::from(ps.1) <= rect[3] - f64::from(PADDING) * 2.
        {
            self.cairo.set_source_color(&self.config.colors.text.clone().into());
            self.cairo.move_to(rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.);
            self.cairo.rel_move_to(f64::from(-ps.0) / 2., f64::from(-ps.1) / 2.);
            show_layout(&self.cairo, &layout);
        }
        self.cairo.restore().unwrap();
    }

    pub fn draw_popup(
        &self,
        rect: &Rect,
        desc: &mut FontDescription,
        text: &str,
    ) -> Result<(), cairo::Error> {
        self.cairo.set_source_color(&self.config.colors.output.to_rgba(0.75));
        self.cairo.rectangle(0., 0., f64::from(rect.width()), f64::from(rect.height()));
        self.cairo.fill()?;

        self.cairo.set_source_color(&self.config.colors.text.clone().into());
        let layout =
            self.pango_layout_popup(rect.width(), rect.height(), POPUP_WINDOW_PAD, desc, text);
        let (w, h) = layout.pixel_size();
        self.cairo.move_to(
            f64::from(i32::from(rect.width()) - w) / 2.,
            f64::from(i32::from(rect.height()) - h) / 2.,
        );
        show_layout(&self.cairo, &layout);
        Ok(())
    }

    fn pango_layout_popup(
        &self,
        width: u16,
        height: u16,
        pad: f64,
        desc: &mut FontDescription,
        text: &str,
    ) -> Layout {
        let layout = create_layout(&self.cairo);
        layout.set_alignment(Alignment::Center);
        layout.set_text(text);
        let pscale = f64::from(PANGO_SCALE);
        let size = f64::from(width.max(height)) * pscale;
        desc.set_absolute_size(size);
        layout.set_font_description(Some(desc));
        let (w, h) = layout.pixel_size();
        let scale = ((f64::from(width) - 2. * pad) / f64::from(w))
            .min((f64::from(height) - 2. * pad) / f64::from(h));
        let size = (size * scale / pscale).floor() * pscale;
        desc.set_absolute_size(size);
        layout.set_font_description(Some(desc));
        layout
    }
}
