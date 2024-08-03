#![allow(clippy::module_name_repetitions)]

use cairo::{Context, LineCap, LineJoin, Rectangle};
use config::data::enums::BorderStyle;
use config::Config;
use gtk::prelude::GdkCairoContextExt;
use pango::ffi::PANGO_SCALE;
use pango::{Alignment, FontDescription, Layout};
use pangocairo::functions::{create_layout, show_layout};

use crate::math::Rect;
use crate::window::PADDING;

pub struct DrawContext {
    cairo: Context,
    config: Config,
}

impl DrawContext {
    pub fn new(cairo: &Context, config: &Config) -> Self {
        DrawContext { cairo: cairo.clone(), config: config.clone() }
    }

    pub fn draw_screen(&self, rect: &Rectangle, line_width: f64) {
        self.cairo.rectangle(
            rect.x() - line_width / 2.,
            rect.y() - line_width / 2.,
            rect.width() + line_width,
            rect.height() + line_width,
        );
        self.cairo.set_source_color(&self.config.display_screen_color().into());
        self.set_stroke_style(self.config.display.screen_line_style, line_width);
        self.cairo.stroke().unwrap();
    }

    pub fn draw_output(&self, rect: &Rectangle) {
        self.cairo.rectangle(rect.x(), rect.y(), rect.width(), rect.height());
        self.cairo.set_source_color(&self.config.display_output_color().to_rgba(0.75));
        self.cairo.fill().unwrap();
    }

    pub fn draw_selected_output(&self, rect: &Rectangle) {
        let line_width = self
            .config
            .display
            .selection_line_width
            .clamp(0., rect.width().min(rect.height()) / 2.);
        self.cairo.rectangle(
            rect.x() + line_width / 2.,
            rect.y() + line_width / 2.,
            rect.width() - line_width,
            rect.height() - line_width,
        );
        self.cairo.set_source_color(&self.config.display_selection_color().into());
        self.set_stroke_style(self.config.display.selection_line_style, line_width);
        self.cairo.stroke().unwrap();
    }

    pub fn draw_output_label(&self, rect: &Rectangle, name: &str, product_name: Option<&str>) {
        self.cairo.save().unwrap();

        let layout = create_layout(&self.cairo);
        layout.set_alignment(Alignment::Center);
        layout.set_text(product_name.unwrap_or(name));

        let mut desc = FontDescription::new();
        desc.set_family(&self.config.display.font.family);
        desc.set_weight(self.config.display.font.weight.into());
        desc.set_size(i32::from(self.config.display.font.size) * PANGO_SCALE);

        layout.set_font_description(Some(&desc));

        let (w, h) = layout.pixel_size();
        if f64::from(w) <= rect.width() - f64::from(PADDING) * 2.
            && f64::from(h) <= rect.height() - f64::from(PADDING) * 2.
        {
            self.cairo.set_source_color(&self.config.display_text_color().into());
            self.cairo.move_to(rect.x() + rect.width() / 2., rect.y() + rect.height() / 2.);
            self.cairo.rel_move_to(f64::from(-w) / 2., f64::from(-h) / 2.);
            show_layout(&self.cairo, &layout);
        }
        self.cairo.restore().unwrap();
    }

    pub fn draw_popup(
        &self,
        rect: &Rect,
        border: f64,
        pad: f64,
        text: &str,
    ) -> Result<(), cairo::Error> {
        self.cairo.set_source_color(&self.config.popup_background_color().to_rgba(0.75));
        self.cairo.rectangle(0., 0., f64::from(rect.width()), f64::from(rect.height()));
        self.cairo.fill()?;

        self.cairo.set_source_color(&self.config.popup_border_color().into());
        self.cairo.rectangle(
            border / 2.,
            border / 2.,
            f64::from(rect.width()) - border,
            f64::from(rect.height()) - border,
        );
        self.cairo.set_line_width(border);
        self.cairo.set_dash(&[1., 0.], 0.);
        self.cairo.stroke()?;

        self.cairo.set_source_color(&self.config.popup_text_color().into());
        let layout = self.pango_layout_popup(rect.width(), rect.height(), border, pad, text);
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
        border: f64,
        pad: f64,
        text: &str,
    ) -> Layout {
        let pscale = f64::from(PANGO_SCALE);
        let height = (f64::from(height) - (2. * pad) - (2. * border)).round().max(1.);
        let width = (f64::from(width) - (2. * pad) - (2. * border)).round().max(1.);
        let layout = create_layout(&self.cairo);
        layout.set_text(text);

        let mut desc = FontDescription::new();
        desc.set_family(&self.config.popup.font.family);
        desc.set_weight(self.config.popup.font.weight.into());

        if self.config.popup.font.size.is_value_and(|size| {
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

    fn set_stroke_style(&self, style: BorderStyle, line_width: f64) {
        let line_width = line_width.max(0.);
        self.cairo.set_line_width(line_width);
        self.cairo.set_dash(&[], 0.);
        self.cairo.set_line_cap(LineCap::Butt);
        self.cairo.set_line_join(LineJoin::Miter);

        match style {
            BorderStyle::Solid => {
                self.cairo.set_dash(&[1., 0.], 0.);
            }
            BorderStyle::Dashed => {
                let dashes = if line_width > 0. { [line_width, 2. * line_width] } else { [1., 2.] };
                self.cairo.set_dash(&dashes, 0.);
                self.cairo.set_line_cap(LineCap::Square);
            }
            BorderStyle::Dotted => {
                let dashes = if line_width > 0. { [0., 2. * line_width] } else { [0., 2.] };
                self.cairo.set_dash(&dashes, 0.);
                self.cairo.set_line_cap(LineCap::Round);
                self.cairo.set_line_join(LineJoin::Round);
            }
            BorderStyle::None => {
                self.cairo.set_line_width(0.);
            }
        }
    }
}
