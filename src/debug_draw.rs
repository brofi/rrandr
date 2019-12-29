use cairo::Context;
use gdk::RGBA;
use glib::signal::Inhibit;
use gtk::{DrawingArea, WidgetExt};

use crate::g_math::{OverlapDebugInfo, Rect};

#[cfg(debug_assertions)]
pub struct RGBABuilder {
    color: RGBA,
}

#[cfg(debug_assertions)]
impl RGBABuilder {
    pub fn new() -> Self {
        Self {
            color: RGBA {
                red: 0f64,
                green: 0f64,
                blue: 0f64,
                alpha: 1f64,
            },
        }
    }

    pub fn new_from(rgba: RGBA) -> Self {
        Self { color: rgba }
    }

    pub fn red(mut self, red: f64) -> Self {
        self.color.red = red;
        self
    }

    pub fn green(mut self, green: f64) -> Self {
        self.color.green = green;
        self
    }

    pub fn blue(mut self, blue: f64) -> Self {
        self.color.blue = blue;
        self
    }

    pub fn alpha(mut self, alpha: f64) -> Self {
        self.color.alpha = alpha;
        self
    }

    pub fn build(&self) -> RGBA {
        self.color
    }
}

#[cfg(debug_assertions)]
pub fn set_color(cr: &Context, rgba: &RGBA) {
    cr.set_source_rgba(rgba.red, rgba.green, rgba.blue, rgba.alpha)
}

#[cfg(debug_assertions)]
pub fn add_rect(cr: &Context, rect: &Rect) {
    cr.rectangle(
        rect.x as f64,
        rect.y as f64,
        rect.width as f64,
        rect.height as f64,
    );
}

pub fn on_debug_draw(
    drawing_area: &DrawingArea,
    cr: &Context,
    debug_info: &OverlapDebugInfo,
) -> Inhibit {
    cr.save();
    let alloc = drawing_area.get_allocation();
    let size = (alloc.width as f64, alloc.height as f64);

    cr.set_source_rgba(0., 0., 0., 1.);
    cr.rectangle(0., 0., size.0, size.1);
    cr.fill();

    let bounds = debug_info
        .step_nodes
        .borrow()
        .iter()
        .fold(Rect::default(), |acc, n| acc.union(&n.rect));

    cr.save();

    if bounds.width > 0 && bounds.height > 0 {
        let sx = (size.0 - 100.) / bounds.width as f64;
        let sy = (size.1 - 100.) / bounds.height as f64;
        cr.translate(50. - (bounds.x as f64 * sx), 50. - (bounds.y as f64 * sy));
        cr.scale(sx, sy);
    }

    let nodes = debug_info.step_nodes.borrow();
    for (i, n) in nodes.iter().skip(1).enumerate() {
        // stroke final rect in green
        if i == nodes.len() - 2 {
            cr.set_line_width(2.);
            cr.set_dash(&[], 0.);
            set_color(cr, &RGBABuilder::new().blue(1.).build());
            add_rect(cr, &n.rect);
            cr.stroke();
        } else {
            // stroke dashed steps
            cr.set_line_width(1.);
            cr.set_dash(&[20., 10.], 0.);
            set_color(cr, &RGBABuilder::new_from(RGBA::white()).alpha(0.5).build());
            add_rect(cr, &n.rect);
            cr.stroke();
        }
    }
    // stroke initial rect last so we can see it
    if let Some(first) = nodes.get(0) {
        cr.set_dash(&[], 0.);
        cr.set_line_width(2.);
        set_color(cr, &RGBABuilder::new().green(1.).build());
        add_rect(cr, &first.rect);
        cr.stroke();
    }

    // stroke spiral
    if let Some(first) = nodes.get(0) {
        cr.set_dash(&[], 0.);
        cr.set_line_width(3.);
        set_color(cr, &RGBABuilder::new().red(1.).build());
        cr.move_to(first.rect.x as f64, first.rect.y as f64);
        for n in nodes.iter() {
            cr.line_to(n.rect.x as f64, n.rect.y as f64);
        }
        cr.stroke();
    }

    cr.restore();
    cr.restore();

    Inhibit(false)
}
