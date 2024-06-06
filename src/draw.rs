#![allow(clippy::module_name_repetitions)]

use gdk::prelude::GdkCairoContextExt;
use gdk::RGBA;
use pango::{Alignment, FontDescription, Weight};
use pangocairo::functions::{create_layout, show_layout};

use crate::view::PADDING;

const COLOR_GREEN: RGBA = RGBA::new(0.722, 0.733, 0.149, 1.);
pub const COLOR_FG: RGBA = RGBA::new(0.922, 0.859, 0.698, 1.);
// const COLOR_BG0_H: RGBA = RGBA::new(0.114, 0.125, 0.129, 1.);
pub const COLOR_BG0: RGBA = RGBA::new(0.157, 0.157, 0.157, 1.);

pub const SCREEN_LINE_WIDTH: f64 = 2.;
const SELECTION_LINE_WIDTH: f64 = 4.;

pub fn draw_screen(cr: &cairo::Context, rect: [f64; 4]) {
    cr.rectangle(
        rect[0] - SCREEN_LINE_WIDTH / 2.,
        rect[1] - SCREEN_LINE_WIDTH / 2.,
        rect[2] + SCREEN_LINE_WIDTH,
        rect[3] + SCREEN_LINE_WIDTH,
    );
    cr.set_source_color(&COLOR_FG);
    cr.set_line_width(SCREEN_LINE_WIDTH);
    cr.set_dash(&[4.], 1.);
    cr.stroke().unwrap();
}

pub fn draw_output(cr: &cairo::Context, rect: [f64; 4]) {
    cr.rectangle(rect[0], rect[1], rect[2], rect[3]);
    cr.set_source_rgba(
        f64::from(COLOR_FG.red()),
        f64::from(COLOR_FG.green()),
        f64::from(COLOR_FG.blue()),
        0.75,
    );
    cr.fill().unwrap();
}

pub fn draw_selected_output(cr: &cairo::Context, rect: [f64; 4]) {
    cr.rectangle(
        rect[0] + SELECTION_LINE_WIDTH / 2.,
        rect[1] + SELECTION_LINE_WIDTH / 2.,
        rect[2] - SELECTION_LINE_WIDTH,
        rect[3] - SELECTION_LINE_WIDTH,
    );
    cr.set_source_color(&COLOR_GREEN);
    cr.set_line_width(SELECTION_LINE_WIDTH);
    cr.set_dash(&[1., 0.], 0.);
    cr.stroke().unwrap();
}

pub fn draw_output_label(
    cr: &cairo::Context,
    rect: [f64; 4],
    name: &str,
    product_name: Option<&str>,
) {
    cr.save().unwrap();
    let mut desc = FontDescription::new();
    desc.set_family("monospace");
    // desc.set_size(12);
    desc.set_weight(Weight::Bold);

    let layout = create_layout(cr);
    layout.set_font_description(Some(&desc));
    layout.set_alignment(Alignment::Center);
    layout.set_text(product_name.unwrap_or(name));

    let ps = layout.pixel_size();
    if f64::from(ps.0) <= rect[2] - f64::from(PADDING) * 2.
        && f64::from(ps.1) <= rect[3] - f64::from(PADDING) * 2.
    {
        cr.set_source_color(&COLOR_BG0);
        cr.move_to(rect[0] + rect[2] / 2., rect[1] + rect[3] / 2.);
        cr.rel_move_to(f64::from(-ps.0) / 2., f64::from(-ps.1) / 2.);
        show_layout(cr, &layout);
    }
    cr.restore().unwrap();
}
