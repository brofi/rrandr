use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::num::IntErrorKind;
use std::rc::Rc;

use gdk::glib::{clone, Bytes, Type, Value};
use gdk::{ContentProvider, Drag, DragAction, MemoryTexture, Paintable};
use gtk::prelude::*;
use gtk::{
    Align, Button, DragSource, DrawingArea, DropControllerMotion, DropTarget,
    EventControllerMotion, FlowBox, FlowBoxChild, Frame, GestureClick, GestureDrag, InputPurpose,
    Label, Orientation, Paned, SelectionMode, StringList, Widget,
};

use crate::draw::{
    draw_output, draw_output_label, draw_screen, draw_selected_output, SCREEN_LINE_WIDTH,
};
use crate::math::{Point, Rect};
use crate::widget::{CheckButton, DropDown, Entry, Switch};
use crate::{get_bounds, Output, ScreenSizeRange};

type OutputUpdatedCallback = dyn Fn(&Output, &Update);

pub const PADDING: u16 = 10;

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

// needed because to tansfer ownership because: function requires argument type
// to outlive `'static` https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html
// It's important to understand this means that any owned data always passes a
// 'static lifetime bound, but a reference to that owned data generally does
// not: https://stackoverflow.com/questions/52464653/how-to-move-data-into-multiple-rust-closures
#[derive(Clone)]
pub struct View {
    outputs: Rc<RefCell<Vec<Output>>>,
    outputs_orig: Rc<RefCell<Vec<Output>>>,
    size: ScreenSizeRange,

    selected_output: Rc<RefCell<Option<usize>>>,
    grab_offset: Rc<RefCell<(f64, f64)>>,
    scale: Rc<RefCell<f64>>,
    translate: Rc<RefCell<[i16; 2]>>,
    bounds: Rc<RefCell<Rect>>,

    dragging_disabled_output: Rc<RefCell<bool>>,

    enabled_area: DrawingArea,
    disabled_area: DrawingArea,
    details: DetailsView,
}

impl View {
    pub fn create(
        outputs: Vec<Output>,
        size: ScreenSizeRange,
        apply_callback: impl Fn(Vec<Output>) -> bool + 'static,
        identify_callback: impl Fn(&Button) + 'static,
    ) -> impl IsA<Widget> {
        let outputs_orig = outputs.clone();
        let mut this = Self {
            outputs: Rc::new(RefCell::new(outputs)),
            outputs_orig: Rc::new(RefCell::new(outputs_orig)),
            size,

            selected_output: Rc::new(RefCell::new(None)),
            grab_offset: Rc::new(RefCell::new((0.0, 0.0))),
            scale: Rc::new(RefCell::new(1.0)),
            translate: Rc::new(RefCell::new([0, 0])),
            bounds: Rc::new(RefCell::new(Rect::default())),

            dragging_disabled_output: Rc::new(RefCell::new(false)),

            enabled_area: DrawingArea::new(),
            disabled_area: DrawingArea::new(),
            details: DetailsView::new(size),
        };

        let root = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_start(i32::from(PADDING))
            .margin_end(i32::from(PADDING))
            .margin_top(i32::from(PADDING))
            .margin_bottom(i32::from(PADDING))
            .spacing(i32::from(PADDING))
            .build();

        let frame_enabled = Frame::builder().label("Layout").child(&this.enabled_area).build();
        let frame_disabled = Frame::builder()
            .label("Disabled")
            .child(&this.disabled_area)
            .width_request(150)
            .build();

        let paned = Paned::builder()
            .start_child(&frame_enabled)
            .end_child(&frame_disabled)
            .resize_start_child(true)
            .resize_end_child(false)
            .vexpand(true)
            .build();
        root.append(&paned);

        let box_bottom = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(i32::from(PADDING))
            .build();

        box_bottom.append(&this.details.root);

        let box_controls = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(i32::from(PADDING))
            .halign(Align::End)
            .valign(Align::End)
            .build();
        let btn_id = Button::builder()
            .label("_Identify")
            .use_underline(true)
            .tooltip_text("Identify outputs")
            .build();
        btn_id.connect_clicked(move |btn| identify_callback(btn));
        box_controls.append(&btn_id);
        let box_apply_reset = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["linked"])
            .build();
        let btn_apply = Button::builder()
            .label("_Apply")
            .use_underline(true)
            .tooltip_text("Apply changes")
            .build();
        btn_apply.connect_clicked(clone!(
            @strong this => move |_btn| {
                if apply_callback(this.outputs.borrow().clone()) {
                    this.outputs_orig.borrow_mut().clone_from(&this.outputs.borrow());
                } else {
                    this.outputs.borrow_mut().clone_from(&this.outputs_orig.borrow());
                    Self::resize(
                        this.enabled_area.width(),
                        this.enabled_area.height(),
                        this.size,
                        &mut this.scale.borrow_mut(),
                        &mut this.translate.borrow_mut(),
                        &mut this.bounds.borrow_mut(),
                        &mut this.outputs.borrow_mut(),
                    );
                    this.enabled_area.queue_draw();
                    this.disabled_area.queue_draw();
                }
        }));
        box_apply_reset.append(&btn_apply);
        let btn_reset = Button::builder()
            .label("_Reset")
            .use_underline(true)
            .tooltip_text("Reset changes")
            .build();
        btn_reset.connect_clicked(clone!(
            @strong this => move |_btn| this.on_reset_clicked()
        ));
        box_apply_reset.append(&btn_reset);

        box_controls.append(&box_apply_reset);
        box_bottom.append(&box_controls);
        root.append(&box_bottom);

        this.enabled_area.set_draw_func(clone!(
            @strong this => move |_d, cr, w, h| this.on_draw(cr, w, h)
        ));
        this.disabled_area.set_draw_func(clone!(
            @strong this => move |_d, cr, w, h| this.on_draw_disabled(cr, w, h)
        ));
        this.enabled_area.connect_resize(clone!(
            @strong this => move |_d, w, h| {
                Self::resize(
                    w,
                    h,
                    this.size,
                    &mut this.scale.borrow_mut(),
                    &mut this.translate.borrow_mut(),
                    &mut this.bounds.borrow_mut(),
                    &mut this.outputs.borrow_mut(),
            );}
        ));

        let gesture_drag = GestureDrag::new();
        gesture_drag.connect_drag_begin(clone!(
            @strong this => move |g, start_x, start_y| this.on_drag_begin(g, start_x, start_y)
        ));
        gesture_drag.connect_drag_update(clone!(
            @strong this => move |g, offset_x, offset_y| this.on_drag_update(g, offset_x, offset_y)
        ));
        gesture_drag.connect_drag_end(clone!(
            @strong this => move |g, offset_x, offset_y| this.on_drag_end(g, offset_x, offset_y)
        ));
        this.enabled_area.add_controller(gesture_drag);

        let event_controller_motion = EventControllerMotion::new();
        event_controller_motion.connect_motion(clone!(
            @strong this => move |ecm, x, y| this.on_motion(ecm, x, y)
        ));
        event_controller_motion.connect_enter(Self::on_enter);
        event_controller_motion.connect_leave(Self::on_leave);
        this.enabled_area.add_controller(event_controller_motion);

        let drag_source = DragSource::builder().actions(DragAction::MOVE).build();
        drag_source.connect_prepare(clone!(
            @strong this => move |ds, x, y| this.on_dragdrop_prepare(ds, x, y)
        ));
        drag_source.connect_drag_begin(clone!(
            @strong this => move |ds, d| this.on_dragdrop_begin(ds, d)
        ));
        drag_source.connect_drag_end(clone!(
            @strong this => move |ds, d, del| this.on_dragdrop_end(ds, d, del)
        ));
        this.disabled_area.add_controller(drag_source);

        let gesture_click = GestureClick::new();
        gesture_click.connect_pressed(clone!(
            @strong this => move |gc, n_press, x, y| this.on_disabled_click(gc, n_press, x, y)
        ));
        this.disabled_area.add_controller(gesture_click);

        let event_controller_motion = EventControllerMotion::new();
        event_controller_motion.connect_motion(clone!(
            @strong this => move |ecm, x, y| this.on_disabled_motion(ecm, x, y)
        ));
        this.disabled_area.add_controller(event_controller_motion);

        let drop_controller_motion = DropControllerMotion::new();
        drop_controller_motion.connect_motion(Self::on_disabled_dragdrop_motion);
        this.disabled_area.add_controller(drop_controller_motion);

        let drop_target = DropTarget::new(Type::U64, DragAction::MOVE);
        drop_target.connect_drop(clone!(
            @strong this => move |dt, v, x, y| this.on_dragdrop_drop(dt, v, x, y)
        ));
        drop_target.connect_motion(Self::on_dragdrop_motion);
        this.enabled_area.add_controller(drop_target);

        this.details.add_output_updated_callback(clone!(
            @strong this => move |output, update| this.update(output, update)
        ));

        root
    }

    fn update(&self, output: &Output, update: &Update) {
        for o in self.outputs.borrow_mut().iter_mut() {
            if o.id == output.id {
                *o = output.clone();
            } else if output.primary {
                o.primary = false;
            }
        }

        let pos_changed = match update {
            Update::Enabled | Update::Resolution => {
                Self::mind_the_gap_and_overlap(&mut self.outputs.borrow_mut());
                true
            }
            _ => false,
        };

        match update {
            Update::Enabled | Update::Resolution | Update::Position => Self::resize(
                self.enabled_area.width(),
                self.enabled_area.height(),
                self.size,
                &mut self.scale.borrow_mut(),
                &mut self.translate.borrow_mut(),
                &mut self.bounds.borrow_mut(),
                &mut self.outputs.borrow_mut(),
            ),
            _ => (),
        }

        match update {
            Update::Enabled => {
                self.enabled_area.queue_draw();
                self.disabled_area.queue_draw();
            }
            Update::Refresh => (),
            _ => self.enabled_area.queue_draw(),
        }

        if pos_changed {
            let outputs = self.outputs.borrow();
            self.details.update(self.selected_output.borrow().and_then(|i| Some(&outputs[i])));
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn resize(
        w: i32,
        h: i32,
        size: ScreenSizeRange,
        scale: &mut f64,
        translate: &mut [i16; 2],
        bounds: &mut Rect,
        outputs: &mut [Output],
    ) {
        // Translate to x = y = 0
        *bounds = get_bounds(outputs);
        for output in outputs.iter_mut() {
            if let (Some(pos), Some(mode)) = (output.pos.as_mut(), output.mode.as_ref()) {
                let max_x =
                    i16::try_from(size.max_width.saturating_sub(mode.width)).unwrap_or(i16::MAX);
                let max_y =
                    i16::try_from(size.max_height.saturating_sub(mode.height)).unwrap_or(i16::MAX);
                pos.0 = pos.0.saturating_sub(bounds.x()).min(max_x);
                pos.1 = pos.1.saturating_sub(bounds.y()).min(max_y);
            }
        }
        *bounds = get_bounds(outputs);
        *scale = ((f64::from(w) - (f64::from(PADDING) + SCREEN_LINE_WIDTH) * 2.)
            / f64::from(bounds.width()))
        .min(
            (f64::from(h) - (f64::from(PADDING) + SCREEN_LINE_WIDTH) * 2.)
                / f64::from(bounds.height()),
        );
        let dxy = i16::try_from(PADDING).unwrap() + SCREEN_LINE_WIDTH.round() as i16;
        *translate = [dxy, dxy];
    }

    fn on_draw(&self, cr: &cairo::Context, _w: i32, _h: i32) {
        let bounds = self.bounds.borrow();
        let scale = self.scale.borrow();
        let translate = self.translate.borrow();

        let screen_rect = bounds.transform(*scale, *translate);
        draw_screen(cr, screen_rect);

        for (i, o) in self.outputs.borrow().iter().enumerate() {
            if !o.enabled {
                continue;
            }
            let output_rect = o.rect().transform(*scale, *translate);
            draw_output(cr, output_rect);
            if let Some(j) = *self.selected_output.borrow() {
                if i == j {
                    draw_selected_output(cr, output_rect);
                }
            }
            let mut name = o.name.clone();
            let mut product_name = o.product_name.clone();
            if o.primary {
                name = format!("[{name}]");
                product_name = product_name.map(|s| format!("[{s}]"));
            }
            draw_output_label(cr, output_rect, &name, product_name.as_deref());
        }
    }

    fn on_draw_disabled(&self, cr: &cairo::Context, w: i32, h: i32) {
        let outputs = self.outputs.borrow();
        let disabled_outputs = Self::get_disabled_outputs(&outputs);
        let i_select = self.selected_output.borrow();
        let is_dragging = *self.dragging_disabled_output.borrow();
        let dim = Self::get_disabled_output_dim(w, h, disabled_outputs.len());
        let mut j: usize = 0; // seperate index for closing the gaps
        for &o in &disabled_outputs {
            if i_select.is_none() || i_select.is_some_and(|i| !is_dragging || outputs[i].id != o.id)
            {
                let pos = Self::get_disabled_output_pos(j, dim[1]);
                let rect =
                    [f64::from(pos[0]), f64::from(pos[1]), f64::from(dim[0]), f64::from(dim[1])];
                draw_output(cr, rect);
                draw_output_label(cr, rect, &o.name, o.product_name.as_deref());
                if let Some(i) = *i_select {
                    if outputs[i].id == o.id {
                        draw_selected_output(cr, rect);
                    }
                }
                j += 1;
            }
        }
    }

    fn get_disabled_outputs(outputs: &[Output]) -> Vec<&Output> {
        outputs.iter().filter(|&n| !n.enabled).collect::<Vec<_>>()
    }

    fn get_disabled_output_pos(index: usize, output_height: u16) -> [i16; 2] {
        let index = u32::try_from(index).expect("less disabled outputs");
        let x = i16::try_from(PADDING).unwrap_or(i16::MAX);
        let y = i16::try_from((index + 1) * u32::from(PADDING) + index * u32::from(output_height))
            .unwrap_or(i16::MAX);
        [x, y]
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    fn get_disabled_output_dim(w: i32, h: i32, n_disabled: usize) -> [u16; 2] {
        if n_disabled == 0 {
            return [0, 0];
        }
        let w = u32::try_from(w).expect("disabled area width is positive");
        let h = u32::try_from(h).expect("disabled area height is positive");
        let n_disabled = u32::try_from(n_disabled).expect("less disabled outputs");
        let max_width = (w.saturating_sub(2 * u32::from(PADDING))) as u16;
        let max_height =
            (h.saturating_sub((n_disabled + 1) * u32::from(PADDING) / n_disabled)) as u16;
        let width = max_width.min((f64::from(max_height) * 16. / 9.).round() as u16);
        let height = max_height.min((f64::from(max_width) * 9. / 16.).round() as u16);
        [width, height]
    }

    fn on_drag_begin(&self, g: &GestureDrag, start_x: f64, start_y: f64) {
        let drawing_area = g.widget().downcast::<DrawingArea>().unwrap();
        if let Some(i) = self.get_output_index_at(start_x, start_y) {
            let scale = self.scale.borrow();
            let translate = self.translate.borrow();
            let mut outputs = self.outputs.borrow_mut();

            // Grab offset to output origin in global coordinates
            let pos = outputs[i].pos.expect("dragged output has position");
            *self.grab_offset.borrow_mut() = (
                f64::from(pos.0) - (start_x - f64::from(translate[0])) / *scale,
                f64::from(pos.1) - (start_y - f64::from(translate[1])) / *scale,
            );

            // Push output to back, so it gets drawn last
            let output = outputs.remove(i);
            outputs.push(output);
            *self.selected_output.borrow_mut() = Some(outputs.len() - 1);

            // Update cursor
            drawing_area.set_cursor_from_name(Some("grabbing"));
        } else {
            *self.selected_output.borrow_mut() = None;
        }
        drawing_area.queue_draw();
        self.disabled_area.queue_draw();
        // Do details UI updates out of scope because their triggered callbacks need to
        // borrow
        let outputs = self.outputs.borrow();
        self.details.update(self.selected_output.borrow().and_then(|i| Some(&outputs[i])));
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_drag_update(&self, g: &GestureDrag, offset_x: f64, offset_y: f64) {
        if let Some(i) = *self.selected_output.borrow() {
            let mut outputs = self.outputs.borrow_mut();
            let output = &outputs[i];
            let mut scale = self.scale.borrow_mut();
            let mut translate = self.translate.borrow_mut();
            let grab_offset = self.grab_offset.borrow();

            let mut min_side = f64::MAX;
            for output in outputs.iter().filter(|n| n.enabled) {
                let mode = output.mode.as_ref().expect("dragged output has mode");
                min_side = min_side.min(f64::from(mode.height));
                min_side = min_side.min(f64::from(mode.width));
            }
            // Snap to all snap values should be possible on all scaled sizes.
            // Give some leeway so it doesn't have to be pixel perfect.
            let snap_strength = (min_side / 4.) - (min_side / 12.);

            // Calculate snap
            let snap = Self::calculate_snap(&outputs, i);

            // Calculate new position
            let start = g.start_point().unwrap();
            let mut new_x = (((start.0 + offset_x - f64::from(translate[0])) / *scale)
                + grab_offset.0)
                .round() as i16;
            let mut new_y = (((start.1 + offset_y - f64::from(translate[1])) / *scale)
                + grab_offset.1)
                .round() as i16;

            // Apply snap
            let pos = output.pos.expect("dragged output has position");
            if snap.x == 0 {
                if f64::from((new_x - pos.0).abs()) < snap_strength {
                    new_x = pos.0;
                }
            } else if f64::from(snap.x.abs()) < snap_strength {
                new_x = pos.0.saturating_add(i16::try_from(snap.x).unwrap());
            }
            if snap.y == 0 {
                if f64::from((new_y - pos.1).abs()) < snap_strength {
                    new_y = pos.1;
                }
            } else if f64::from(snap.y.abs()) < snap_strength {
                new_y = pos.1.saturating_add(i16::try_from(snap.y).unwrap());
            }

            // Update new position
            if new_x != pos.0 || new_y != pos.1 {
                outputs[i].pos = Some((new_x, new_y));
                let drawing_area = g.widget().downcast::<DrawingArea>().unwrap();
                Self::resize(
                    drawing_area.width(),
                    drawing_area.height(),
                    self.size,
                    &mut scale,
                    &mut translate,
                    &mut self.bounds.borrow_mut(),
                    &mut outputs,
                );
                self.details.update(Some(&outputs[i]));
                drawing_area.queue_draw();
            }
        }
    }

    fn calculate_snap(outputs: &[Output], output_index: usize) -> Point {
        let output_r = &outputs[output_index].rect();
        let output_center = output_r.center();
        let mut dist = Point::max();
        let mut snap = Point::default();
        for (j, other) in outputs.iter().enumerate() {
            if !other.enabled {
                continue;
            }
            if output_index != j {
                let other_r = other.rect();
                let other_center = other_r.center();

                // Horizontal snap
                for dist_h in [
                    other_r.left() - output_r.left(),
                    other_r.right() - output_r.left(),
                    other_r.left() - output_r.right(),
                    other_r.right() - output_r.right(),
                    other_center.x - output_center.x,
                ] {
                    if dist_h.abs() < dist.x {
                        dist.x = dist_h.abs();
                        snap.x = dist_h;
                    }
                }

                // Vertical snap
                for dist_v in [
                    other_r.top() - output_r.top(),
                    other_r.bottom() - output_r.top(),
                    other_r.top() - output_r.bottom(),
                    other_r.bottom() - output_r.bottom(),
                    other_center.y - output_center.y,
                ] {
                    if dist_v.abs() < dist.y {
                        dist.y = dist_v.abs();
                        snap.y = dist_v;
                    }
                }
            }
        }
        snap
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    fn mind_the_gap_and_overlap(outputs: &mut Vec<Output>) {
        let mut data = HashMap::new();
        let bounds = get_bounds(outputs);
        let bc = bounds.center();

        for output in outputs.iter() {
            if !output.enabled {
                continue;
            }
            let r = output.rect();
            let c = r.center();
            let mut e = [0., 0.];
            if bc != c {
                let d = [f64::from(bc.x - c.x), f64::from(bc.y - c.y)];
                let d_len = (d[0].powi(2) + d[1].powi(2)).sqrt();
                e = [d[0] / d_len, d[1] / d_len];
            }
            data.insert(output.id, (r, e));
        }

        let step = 50.;
        let mut moved = Vec::new();
        let mut max_loops = (f64::from(bounds.width().max(bounds.height())) / step) as u16;
        loop {
            for i in 0..outputs.len() {
                if !outputs[i].enabled {
                    continue;
                }
                // Current position
                let mut r = data[&outputs[i].id].0.clone();
                // Unit direction
                let e = data[&outputs[i].id].1;
                // Signs
                let sx = e[0].signum();
                let sy = e[1].signum();
                // Next move
                let dx = step * e[0];
                let dy = step * e[1];

                // let mut dx = step * e[0];
                // let mut dy = step * e[1];

                // let cdx = bc.x() - r.x();
                // dx = if dx >= 0. {
                //     dx.min(cdx as f64)
                // } else {
                //     dx.max(cdx as f64)
                // };

                // let cdy = bc.y() - r.y();
                // dy = if dy >= 0. {
                //     dy.min(cdy as f64)
                // } else {
                //     dy.max(cdy as f64)
                // };

                // Move
                r.translate(dx.round() as i16, dy.round() as i16);

                // Check if move has caused an overlap with other rects
                for other in outputs.iter() {
                    if other.id == outputs[i].id || !other.enabled {
                        continue;
                    }
                    if let Some(intersect) = r.intersect(&data[&other.id].0) {
                        let mut dx = -sx * f64::from(intersect.width());
                        let mut dy = -sy * f64::from(intersect.height());

                        if e[1].abs() > 0. {
                            // Calculate the x where a line in the flipped direction towards center
                            // intersects the bottom of the intersection rectangle.
                            let ix = sx * e[0] * f64::from(intersect.height()) / (sy * e[1]);
                            // If the intersection rectangle bottom is intersected
                            if ix.abs() <= f64::from(intersect.width()) {
                                dx = -sx * ix;
                            }
                        }
                        if e[0].abs() > 0. {
                            // Calculate the y where a line in the flipped direction towards center
                            // intersects the right of the intersection rectangle.
                            let iy = sy * e[1] * f64::from(intersect.width()) / (sx * e[0]);
                            // If the intersection rectangle right side is intersected
                            if iy.abs() <= f64::from(intersect.height()) {
                                dy = -sy * iy;
                            }
                        }
                        // Move back to remove the overlap
                        r.translate(dx.round() as i16, dy.round() as i16);
                    }
                }
                // Check if rect has moved
                let old_r = &data[&outputs[i].id].0;
                if r.x() != old_r.x() || r.y() != old_r.y() {
                    moved.push(true);
                }
                data.insert(outputs[i].id, (r, e));
            }
            if !moved.iter().any(|&b| b) {
                // No more moves to make
                break;
            }
            moved.clear();
            max_loops -= 1;
            if max_loops == 0 {
                // Max iterations exceeded
                break;
            }
        }
        for output in outputs {
            if !output.enabled {
                continue;
            }
            let pos = output.pos.as_mut().unwrap();
            pos.0 = data[&output.id].0.x();
            pos.1 = data[&output.id].0.y();
        }
    }

    fn on_disabled_click(&self, gc: &GestureClick, _n_press: i32, x: f64, y: f64) {
        let disabled_area = gc.widget().downcast::<DrawingArea>().unwrap();
        *self.selected_output.borrow_mut() =
            self.get_disabled_output_index_at(x, y, disabled_area.width(), disabled_area.height());
        self.enabled_area.queue_draw();
        disabled_area.queue_draw();
        let outputs = self.outputs.borrow();
        self.details.update(self.selected_output.borrow().and_then(|i| Some(&outputs[i])));
    }

    fn on_drag_end(&self, g: &GestureDrag, offset_x: f64, offset_y: f64) {
        *self.grab_offset.borrow_mut() = (0., 0.);
        // Update cursor
        let start = g.start_point().unwrap(); // TODO failed again, how?
        let drawing_area = g.widget().downcast::<DrawingArea>().unwrap();
        match self.get_output_index_at(start.0 + offset_x, start.1 + offset_y) {
            Some(_) => drawing_area.set_cursor_from_name(Some("pointer")),
            None => drawing_area.set_cursor_from_name(Some("default")),
        }
    }

    fn on_motion(&self, ecm: &EventControllerMotion, x: f64, y: f64) {
        // TODO if not is_dragging instead
        if self.grab_offset.borrow().0 == 0. || self.grab_offset.borrow().1 == 0. {
            // Update cursor
            let drawing_area = ecm.widget().downcast::<DrawingArea>().unwrap();
            match self.get_output_index_at(x, y) {
                Some(_) => drawing_area.set_cursor_from_name(Some("pointer")),
                None => drawing_area.set_cursor_from_name(Some("default")),
            }
        }
    }

    fn on_disabled_motion(&self, ecm: &EventControllerMotion, x: f64, y: f64) {
        let disabled_area = ecm.widget().downcast::<DrawingArea>().unwrap();
        match self.get_disabled_output_index_at(x, y, disabled_area.width(), disabled_area.height())
        {
            Some(_) => disabled_area.set_cursor_from_name(Some("pointer")),
            None => disabled_area.set_cursor_from_name(Some("default")),
        }
    }

    fn on_disabled_dragdrop_motion(_dcm: &DropControllerMotion, _x: f64, _y: f64) {}

    fn on_enter(_ecm: &EventControllerMotion, _x: f64, _y: f64) {}

    fn on_leave(_ecm: &EventControllerMotion) {}

    #[allow(clippy::cast_possible_truncation)]
    fn on_dragdrop_prepare(&self, ds: &DragSource, x: f64, y: f64) -> Option<ContentProvider> {
        let outputs = self.outputs.borrow();
        let disabled_outputs = Self::get_disabled_outputs(&outputs);
        let disabled_area = ds.widget().downcast::<DrawingArea>().unwrap();
        let width = disabled_area.width();
        let height = disabled_area.height();
        if let Some(i) = self.get_disabled_output_index_at(x, y, width, height) {
            let dim = Self::get_disabled_output_dim(width, height, disabled_outputs.len());
            if let Ok(icon) = Self::create_drag_icon(
                dim[0],
                dim[1],
                &outputs[i].name,
                outputs[i].product_name.as_deref(),
            ) {
                let j = outputs
                    .iter()
                    .filter(|&o| !o.enabled)
                    .position(|o| o.id == outputs[i].id)
                    .unwrap();
                let pos = Self::get_disabled_output_pos(j, dim[1]);
                ds.set_icon(Some(&icon), x as i32, (y - f64::from(pos[1])) as i32);
            }
            return Some(ContentProvider::for_value(&Value::from(u64::try_from(i).ok()?)));
        }
        None
    }

    fn on_dragdrop_begin(&self, ds: &DragSource, _d: &Drag) {
        let disabled_area = ds.widget().downcast::<DrawingArea>().unwrap();
        disabled_area.queue_draw();
        *self.dragging_disabled_output.borrow_mut() = true;
    }

    fn on_dragdrop_end(&self, ds: &DragSource, _d: &Drag, _del: bool) {
        let disabled_area = ds.widget().downcast::<DrawingArea>().unwrap();
        disabled_area.queue_draw();
        *self.dragging_disabled_output.borrow_mut() = false;
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_dragdrop_drop(&self, dt: &DropTarget, v: &Value, x: f64, y: f64) -> bool {
        let Ok(i) = v.get::<u64>() else {
            return false;
        };
        let Ok(i) = usize::try_from(i) else {
            return false;
        };

        let drawing_area = dt.widget().downcast::<DrawingArea>().unwrap();
        {
            let mut outputs = self.outputs.borrow_mut();
            if i >= outputs.len() || outputs[i].enabled {
                return false;
            }
            // Insert output
            let mut scale = self.scale.borrow_mut();
            let mut translate = self.translate.borrow_mut();
            outputs[i].enabled = true;
            outputs[i].mode = Some(outputs[i].modes[0].clone());
            outputs[i].pos = Some((
                ((x - f64::from(translate[0])).max(0.) / *scale).round() as i16,
                ((y - f64::from(translate[1])).max(0.) / *scale).round() as i16,
            ));

            Self::mind_the_gap_and_overlap(&mut outputs);
            Self::resize(
                drawing_area.width(),
                drawing_area.height(),
                self.size,
                &mut scale,
                &mut translate,
                &mut self.bounds.borrow_mut(),
                &mut outputs,
            );
        }
        // Enable selection
        *self.selected_output.borrow_mut() = Some(i);
        self.details.update(Some(&self.outputs.borrow()[i]));
        // Update drawing areas
        self.disabled_area.queue_draw();
        drawing_area.queue_draw();
        true
    }

    fn on_dragdrop_motion(_dt: &DropTarget, _x: f64, _y: f64) -> DragAction { DragAction::MOVE }

    fn create_drag_icon(
        width: u16,
        height: u16,
        name: &str,
        product_name: Option<&str>,
    ) -> Result<impl IsA<Paintable>, Box<dyn Error>> {
        let surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            i32::from(width),
            i32::from(height),
        )?;
        let cr = cairo::Context::new(&surface)?;
        let rect = [0., 0., f64::from(width), f64::from(height)];
        draw_output(&cr, rect);
        draw_output_label(&cr, rect, name, product_name);
        cr.fill()?;
        drop(cr);
        surface.flush();
        let stride = surface.stride().try_into()?;
        Ok(MemoryTexture::new(
            i32::from(width),
            i32::from(height),
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &Bytes::from_owned(surface.take_data()?),
            stride,
        ))
    }

    fn on_reset_clicked(&self) {
        self.outputs.borrow_mut().clone_from(&self.outputs_orig.borrow());
        // Disable selection
        *self.selected_output.borrow_mut() = None;
        self.details.update(None);
        // Update drawing areas
        Self::resize(
            self.enabled_area.width(),
            self.enabled_area.height(),
            self.size,
            &mut self.scale.borrow_mut(),
            &mut self.translate.borrow_mut(),
            &mut self.bounds.borrow_mut(),
            &mut self.outputs.borrow_mut(),
        );
        self.enabled_area.queue_draw();
        self.disabled_area.queue_draw();
    }

    fn get_output_index_at(&self, x: f64, y: f64) -> Option<usize> {
        let scale = self.scale.borrow();
        let translate = self.translate.borrow();

        for (i, output) in self.outputs.borrow().iter().enumerate() {
            if output.enabled {
                let mut scaled_rect = output.rect();
                scaled_rect.scale(*scale);
                scaled_rect.translate(translate[0], translate[1]);
                if scaled_rect.contains(x, y) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn get_disabled_output_index_at(
        &self,
        x: f64,
        y: f64,
        width: i32,
        height: i32,
    ) -> Option<usize> {
        let outputs = self.outputs.borrow();
        let disabled_outputs = Self::get_disabled_outputs(&outputs);
        let dim = Self::get_disabled_output_dim(width, height, disabled_outputs.len());
        for (i, &disabled_output) in disabled_outputs.iter().enumerate() {
            let pos = Self::get_disabled_output_pos(i, dim[1]);
            if x >= f64::from(pos[0])
                && x <= f64::from(i32::from(pos[0]) + i32::from(dim[0]))
                && y >= f64::from(pos[1])
                && y <= f64::from(i32::from(pos[1]) + i32::from(dim[1]))
            {
                for (j, output) in outputs.iter().enumerate() {
                    if output.id == disabled_output.id {
                        return Some(j);
                    }
                }
            }
        }
        None
    }
}

enum Update {
    Enabled,
    Resolution,
    Refresh,
    Position,
    Primary,
}

#[derive(Clone)]
struct DetailsView {
    output: Rc<RefCell<Option<Output>>>,
    output_updated_callbacks: Rc<RefCell<Vec<Rc<OutputUpdatedCallback>>>>,
    size: ScreenSizeRange,
    root: FlowBox,
    sw_enabled: Switch,
    dd_resolution: DropDown,
    dd_refresh: DropDown,
    en_position_x: Entry,
    en_position_y: Entry,
    cb_primary: CheckButton,
}

impl DetailsView {
    fn new(size: ScreenSizeRange) -> Self {
        let root = FlowBox::builder()
            .row_spacing(u32::from(PADDING))
            .column_spacing(u32::from(PADDING))
            .orientation(Orientation::Horizontal)
            .selection_mode(SelectionMode::None)
            .halign(Align::Start)
            .min_children_per_line(2)
            .hexpand(true)
            .build();

        let sw_enabled = gtk::Switch::builder().tooltip_text("Enable/Disable").build();
        let sw_enabled = Switch::new(sw_enabled);
        root.append(&Self::create_detail_child("Enabled", &sw_enabled.widget));

        let box_mode = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["linked"])
            .build();
        let dd_resolution = gtk::DropDown::builder().tooltip_text("Resolution").build();
        let dd_resolution = DropDown::new(dd_resolution);
        let dd_refresh = gtk::DropDown::builder().tooltip_text("Refresh rate").build();
        let dd_refresh = DropDown::new(dd_refresh);
        box_mode.append(&dd_resolution.widget);
        box_mode.append(&dd_refresh.widget);
        root.append(&Self::create_detail_child("Mode", &box_mode));

        let box_pos = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["linked"])
            .build();
        let en_position_x = gtk::Entry::builder()
            .input_purpose(InputPurpose::Digits)
            .text("0")
            .placeholder_text("x")
            .tooltip_text("Horizontal position")
            .max_length(6)
            .width_chars(5)
            .max_width_chars(5)
            .build();
        EntryExt::set_alignment(&en_position_x, 1.);
        let en_position_x = Entry::new(en_position_x);

        let en_position_y = gtk::Entry::builder()
            .input_purpose(InputPurpose::Digits)
            .text("0")
            .placeholder_text("y")
            .tooltip_text("Vertical position")
            .max_length(6)
            .width_chars(5)
            .max_width_chars(5)
            .build();
        EntryExt::set_alignment(&en_position_y, 1.);
        let en_position_y = Entry::new(en_position_y);

        box_pos.append(&en_position_x.widget);
        box_pos.append(&en_position_y.widget);
        root.append(&Self::create_detail_child("Position", &box_pos));

        let cb_primary = gtk::CheckButton::builder().tooltip_text("Set as primary").build();
        let cb_primary = CheckButton::new(cb_primary);
        root.append(&Self::create_detail_child("Primary", &cb_primary.widget));

        let mut this = Self {
            output: Rc::new(RefCell::new(None)),
            output_updated_callbacks: Rc::new(RefCell::new(Vec::new())),
            size,
            root,
            sw_enabled,
            dd_resolution,
            dd_refresh,
            en_position_x,
            en_position_y,
            cb_primary,
        };

        this.sw_enabled.connect_active_notify(clone!(
            @strong this => move |sw| this.on_enabled_switched(sw)
        ));
        this.dd_resolution.connect_selected_item_notify(clone!(
            @strong this => move |dd| this.on_resolution_selected(dd)
        ));
        this.dd_refresh.connect_selected_item_notify(clone!(
            @strong this => move |dd| this.on_refresh_rate_selected(dd)
        ));
        this.en_position_x.connect_insert_text(clone!(
            @strong this => move |entry, text, position| this.on_position_insert(entry, text, position, Axis::X)
        ));
        this.en_position_x.connect_delete_text(clone!(
            @strong this => move |entry, start, end| this.on_position_delete(entry, start, end, Axis::X)
        ));
        this.en_position_y.connect_insert_text(clone!(
            @strong this => move |entry, text, position| this.on_position_insert(entry, text, position, Axis::Y)
        ));
        this.en_position_y.connect_delete_text(clone!(
            @strong this => move |entry, start, end| this.on_position_delete(entry, start, end, Axis::Y)
        ));
        this.cb_primary.connect_active_notify(clone!(
            @strong this => move |cb| this.on_primary_checked(cb)
        ));

        this
    }

    fn create_detail_child<W: IsA<Widget>>(label: &str, ctrl: &W) -> impl IsA<Widget> {
        let fbc = FlowBoxChild::builder()
            .name(&("fbc_".to_owned() + &label.to_owned().replace('_', "").to_lowercase()))
            .halign(Align::Start)
            .valign(Align::Center)
            .hexpand(false)
            .vexpand(false)
            .focusable(false)
            .visible(false)
            .build();
        let hbox = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .valign(Align::Center)
            .spacing(i32::from(PADDING))
            .build();
        let mut child: Widget = (*ctrl).clone().into();
        if ctrl.is::<gtk::Box>() {
            child = ctrl.first_child().expect("Box has a child");
        }
        let label = if label.contains('_') { label.to_owned() } else { format!("_{label}") };
        let label = Label::with_mnemonic(&label);
        label.set_mnemonic_widget(Some(&child));
        let gesture_click = GestureClick::new();
        gesture_click
            .connect_released(clone!(@strong child => move |_, _, _, _| _ = child.activate()));
        label.add_controller(gesture_click);
        hbox.append(&label);
        hbox.append(ctrl);
        fbc.set_child(Some(&hbox));
        fbc
    }

    fn update(&self, output: Option<&Output>) {
        if let Some(output) = output {
            self.sw_enabled.set_active(output.enabled);
            self.cb_primary.set_active(output.primary);
            if let Some(pos) = output.pos {
                self.en_position_x.set_text(&pos.0.to_string());
                self.en_position_y.set_text(&pos.1.to_string());
            }
            let resolutions = output.get_resolutions_dropdown();
            self.dd_resolution.set_model(Some(&into_string_list(&resolutions)));
            if let Some(res_idx) = output.get_current_resolution_dropdown_index() {
                self.dd_resolution.set_selected(u32::try_from(res_idx).expect("less resolutions"));
                let refresh_rates = output.get_refresh_rates_dropdown(res_idx);
                self.dd_refresh.set_model(Some(&into_string_list(&refresh_rates)));
                if let Some(ref_idx) = output.get_current_refresh_rate_dropdown_index(res_idx) {
                    self.dd_refresh
                        .set_selected(u32::try_from(ref_idx).expect("less refresh rates"));
                }
            }
        }
        *self.output.borrow_mut() = output.cloned();
        self.update_visibility();
    }

    fn update_visibility(&self) {
        let mut child = self.root.first_child();
        while let Some(c) = child {
            let visible = self
                .output
                .borrow()
                .as_ref()
                .is_some_and(|o| o.enabled || c.widget_name() == "fbc_enabled");
            c.set_visible(visible);
            child = c.next_sibling();
        }
    }

    fn on_enabled_switched(&self, sw: &Switch) {
        let mut updated = None;
        if let Some(output) = self.output.borrow_mut().as_mut() {
            let active = sw.widget.is_active();
            // Update output
            output.enabled = active;
            if active {
                // Insert output
                output.enabled = true;
                output.mode = Some(output.modes[0].clone());
                output.pos = Some((0, 0));
            } else {
                // Remove output
                output.primary = false;
                output.pos = None;
                output.mode = None;
            }
            updated = Some(output.clone());
        }
        if let Some(updated) = updated {
            self.notify_updated(&updated, &Update::Enabled);
        }
        self.update_visibility();
    }

    fn on_resolution_selected(&self, dd: &DropDown) {
        let mut updated = None;
        if let Some(output) = self.output.borrow_mut().as_mut() {
            if !output.enabled {
                return;
            }

            let dd_selected = dd.widget.selected() as usize;

            // Update current mode
            let mode = &output.modes[output.resolution_dropdown_mode_index(dd_selected)];
            if output.mode.as_ref().is_some_and(|m| m.id != mode.id) || output.mode.is_none() {
                output.mode = Some(mode.clone());
                updated = Some(output.clone());
            }

            // Update refresh rate dropdown
            self.dd_refresh.set_model(Some(&into_string_list(
                &output.get_refresh_rates_dropdown(dd_selected),
            )));
            if let Some(idx) = output.get_current_refresh_rate_dropdown_index(dd_selected) {
                self.dd_refresh.set_selected(u32::try_from(idx).expect("less refresh rates"));
            }
        }
        if let Some(updated) = updated {
            self.notify_updated(&updated, &Update::Resolution);
        }
    }

    fn on_refresh_rate_selected(&self, dd: &DropDown) {
        if let Some(output) = self.output.borrow_mut().as_mut() {
            if !output.enabled {
                return;
            }

            // Update current mode
            let mode = &output.modes[output.refresh_rate_dropdown_mode_index(
                self.dd_resolution.widget.selected() as usize,
                dd.widget.selected() as usize,
            )];
            if output.mode.as_ref().is_some_and(|m| m.id != mode.id) || output.mode.is_none() {
                output.mode = Some(mode.clone());
                self.notify_updated(output, &Update::Refresh);
            }
        }
    }

    fn on_position_insert(&self, entry: &Entry, text: &str, position: &mut i32, axis: Axis) {
        let idx = usize::try_from(*position).expect("smaller position");
        let mut new_text = entry.widget.text().to_string();
        new_text.insert_str(idx, text);
        if let Some(coord) = self.parse_coord(&new_text, axis) {
            if coord.to_string() == new_text {
                entry.insert_text(text, position);
            } else if coord.to_string() != entry.widget.text() {
                entry.set_text(&coord.to_string());
            }
            self.update_position(axis, coord);
        } else if entry.widget.text().is_empty() {
            entry.insert_text("0", &mut 0);
        }
    }

    fn on_position_delete(&self, entry: &Entry, start_pos: i32, end_pos: i32, axis: Axis) {
        let start_idx = usize::try_from(start_pos).expect("smaller start position");
        let end_idx = usize::try_from(end_pos).expect("smaller end position");
        let mut new_text = entry.widget.text().to_string();
        new_text.replace_range(start_idx..end_idx, "");
        if let Some(coord) = self.parse_coord(&new_text, axis) {
            if coord.to_string() == new_text {
                entry.delete_text(start_pos, end_pos);
            } else {
                entry.set_text(&coord.to_string());
            }
            self.update_position(axis, coord);
        } else {
            entry.delete_text(start_pos, end_pos);
            self.update_position(axis, 0);
        }
    }

    fn parse_coord(&self, text: &str, axis: Axis) -> Option<i16> {
        if let Some(output) = self.output.borrow().as_ref() {
            if let Some(mode) = output.mode.as_ref() {
                let max = match axis {
                    Axis::X => i16::try_from(self.size.max_width.saturating_sub(mode.width))
                        .unwrap_or(i16::MAX),
                    Axis::Y => i16::try_from(self.size.max_height.saturating_sub(mode.height))
                        .unwrap_or(i16::MAX),
                };
                return match text
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<i16>()
                {
                    Ok(c) => Some(c.min(max)),
                    Err(e) => match e.kind() {
                        IntErrorKind::PosOverflow => Some(max),
                        _ => None,
                    },
                };
            }
        }
        None
    }

    fn update_position(&self, axis: Axis, coord: i16) {
        if let Some(output) = self.output.borrow_mut().as_mut() {
            if let Some(pos) = output.pos {
                let new_pos = match axis {
                    Axis::X => (coord, pos.1),
                    Axis::Y => (pos.0, coord),
                };
                if new_pos != pos {
                    output.pos = Some(new_pos);
                    self.notify_updated(output, &Update::Position);
                }
            }
        }
    }

    fn on_primary_checked(&self, cb: &CheckButton) {
        if let Some(output) = self.output.borrow_mut().as_mut() {
            output.primary = output.enabled && cb.widget.is_active();
            self.notify_updated(output, &Update::Primary);
        }
    }

    fn notify_updated(&self, output: &Output, update: &Update) {
        for callback in self.output_updated_callbacks.borrow().iter() {
            callback(output, update);
        }
    }

    fn add_output_updated_callback(&mut self, callback: impl Fn(&Output, &Update) + 'static) {
        self.output_updated_callbacks.borrow_mut().push(Rc::new(callback));
    }
}

fn into_string_list(list: &[String]) -> StringList {
    let list = list.iter().map(String::as_str).collect::<Vec<&str>>();
    StringList::new(list.as_slice())
}
