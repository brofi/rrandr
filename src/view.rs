use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gdk::glib::{clone, Propagation, Type, Value};
use gdk::{DragAction, Key, ModifierType, Texture};
use gtk::prelude::*;
use gtk::{
    AboutDialog, Align, Button, DrawingArea, DropTarget, EventControllerKey, EventControllerMotion,
    FlowBox, GestureDrag, License, Orientation, Paned, SelectionMode, Separator,
};
use x11rb::protocol::randr::Output as OutputId;

use crate::config::Config;
use crate::data::outputs::Outputs;
use crate::draw::{DrawContext, SCREEN_LINE_WIDTH};
use crate::math::{Point, Rect};
use crate::widget::details_box::{DetailsBox, Update};
use crate::widget::disabled_output_area::DisabledOutputArea;
use crate::{Output, ScreenSizeRange};

type ApplyCallback = dyn Fn(Vec<Output>);

pub const PADDING: u16 = 12;
pub const SPACING: u16 = 6;

#[derive(Clone, Copy)]
pub enum Axis {
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
    pub root: gtk::Box,
    config: Config,
    size: ScreenSizeRange,
    outputs: Rc<RefCell<Vec<Output>>>,
    outputs_orig: Rc<RefCell<Vec<Output>>>,
    selected_output: Rc<RefCell<Option<usize>>>,
    grab_offset: Rc<Cell<[f64; 2]>>,
    scale: Rc<Cell<f64>>,
    translate: Rc<Cell<[i16; 2]>>,
    bounds: Rc<RefCell<Rect>>,
    paned: Paned,
    last_handle_pos: Rc<Cell<i32>>,
    drawing_area: DrawingArea,
    details: DetailsBox,
    disabled: DisabledOutputArea,
    apply_callback: Rc<RefCell<Option<Rc<ApplyCallback>>>>,
}

impl View {
    pub fn new(
        config: Config,
        size: ScreenSizeRange,
        outputs: Vec<Output>,
        identify_callback: impl Fn(&Button) + 'static,
    ) -> Self {
        let root = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_start(PADDING.into())
            .margin_end(PADDING.into())
            .margin_top(PADDING.into())
            .margin_bottom(PADDING.into())
            .spacing(PADDING.into())
            .build();

        let enabled_outputs =
            outputs.iter().filter(|&n| n.enabled()).map(Output::clone).collect::<Vec<_>>();
        let disabled_outputs =
            outputs.iter().filter(|&n| !n.enabled()).map(Output::clone).collect::<Vec<_>>();

        let drawing_area = DrawingArea::builder().focusable(true).build();
        let disabled = DisabledOutputArea::new(&disabled_outputs);

        let paned = Paned::builder()
            .start_child(&drawing_area)
            .end_child(&disabled)
            .resize_start_child(true)
            .resize_end_child(false)
            .vexpand(true)
            .build();
        root.append(&paned);

        let this = Self {
            root,
            config,
            size,
            outputs: Rc::new(RefCell::new(enabled_outputs)),
            outputs_orig: Rc::new(RefCell::new(outputs)),
            selected_output: Rc::new(RefCell::new(None)),
            grab_offset: Rc::new(Cell::new([0.0, 0.0])),
            scale: Rc::new(Cell::new(1.0)),
            translate: Rc::new(Cell::new([0, 0])),
            bounds: Rc::new(RefCell::new(Rect::default())),
            paned,
            last_handle_pos: Rc::new(Cell::new(0)),
            drawing_area,
            details: DetailsBox::new(size.max_width, size.max_height),
            disabled,
            apply_callback: Rc::new(RefCell::new(None)),
        };

        let event_controller_key = EventControllerKey::new();
        event_controller_key.connect_key_pressed(clone!(
                @strong this => move |eck, keyval, keycode, state| this.on_key_pressed(eck, keyval, keycode, state)
            ));
        this.paned.add_controller(event_controller_key);

        let box_bottom = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(SPACING.into())
            .build();

        box_bottom.append(&this.details);

        // Use FlowBox like a horizontal Box to get the same style (especially padding)
        // on its children as the details view children.
        let box_controls = FlowBox::builder()
            .orientation(Orientation::Horizontal)
            .selection_mode(SelectionMode::None)
            .min_children_per_line(3)
            .max_children_per_line(3)
            .valign(Align::End)
            .build();
        let btn_about = Button::builder()
            .margin_end(SPACING.into())
            .label("_About")
            .use_underline(true)
            .tooltip_text("About")
            .build();
        box_controls.append(&btn_about);
        btn_about.connect_clicked(move |_btn| {
            let about = AboutDialog::builder()
                .program_name(env!("CARGO_PKG_NAME"))
                .version(env!("CARGO_PKG_VERSION"))
                .comments(env!("CARGO_PKG_DESCRIPTION"))
                .website_label("Repository")
                .website(env!("CARGO_PKG_REPOSITORY"))
                .copyright(env!("RRANDR_COPYRIGHT_NOTICE"))
                .license_type(License::Gpl30)
                .authors(env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>())
                .build();
            if let Ok(logo) = Texture::from_filename("res/logo.svg") {
                about.set_logo(Some(&logo));
            }
            about.show();
        });
        let btn_id = Button::builder()
            .margin_end(SPACING.into())
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
                if let Some(callback) = this.apply_callback.borrow().as_ref() {
                    callback(this.get_outputs());
                }
        }));
        box_apply_reset.append(&btn_apply);
        let btn_reset = Button::builder()
            .label("_Reset")
            .use_underline(true)
            .tooltip_text("Reset changes")
            .build();
        btn_reset.connect_clicked(clone!(@strong this => move |_btn| this.reset()));
        box_apply_reset.append(&btn_reset);
        box_controls.append(&box_apply_reset);

        // Remove focusable from automatically added FlowBoxChild
        let mut ctrl_child = box_controls.first_child();
        while let Some(c) = ctrl_child {
            c.set_focusable(false);
            ctrl_child = c.next_sibling();
        }

        box_bottom.append(&box_controls);
        this.root.append(&Separator::new(Orientation::Horizontal));
        this.root.append(&box_bottom);

        this.drawing_area.set_draw_func(clone!(
            @strong this => move |_d, cr, width, height| this.on_draw(cr, width, height)
        ));
        this.drawing_area.connect_resize(clone!(
            @strong this => move |_d, width, height| this.resize(width, height)
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
        this.drawing_area.add_controller(gesture_drag);

        let event_controller_motion = EventControllerMotion::new();
        event_controller_motion.connect_motion(clone!(
            @strong this => move |ecm, x, y| this.on_motion(ecm, x, y)
        ));
        event_controller_motion.connect_enter(Self::on_enter);
        event_controller_motion.connect_leave(Self::on_leave);
        this.drawing_area.add_controller(event_controller_motion);

        let drop_target = DropTarget::new(Type::U32, DragAction::MOVE);
        drop_target.connect_drop(clone!(
            @strong this => move |dt, v, x, y| this.on_drop(dt, v, x, y)
        ));
        drop_target.connect_motion(Self::on_drop_motion);
        this.drawing_area.add_controller(drop_target);

        this.details.connect_output_changed(clone!(
            @strong this => move |_details, output, update| this.update(output, update)
        ));

        this.disabled.connect_output_selected(clone!(
            @strong this => move |_, output| this.on_disabled_selected(output)
        ));

        this.disabled.connect_output_deselected(clone!(
            @strong this => move |_| this.on_disabled_deselected()
        ));

        this
    }

    fn get_outputs(&self) -> Vec<Output> {
        let mut outputs = self.outputs.borrow_mut().to_vec();
        outputs.extend(self.disabled.outputs().to_vec());
        outputs
    }

    fn update(&self, output: &Output, update: Update) {
        match update {
            Update::Enabled => _ = self.enable_output(output.id()),
            Update::Disabled => _ = self.disable_output(output.id()),
            _ => {
                for o in self.outputs.borrow_mut().iter_mut() {
                    if o == output {
                        *o = output.clone();
                    } else if output.primary() {
                        o.set_primary(false);
                    }
                }
            }
        }
        self.update_view(update);
    }

    fn update_view(&self, update: Update) {
        // Mind the gap
        match update {
            Update::Enabled | Update::Disabled | Update::Resolution => {
                Self::mind_the_gap_and_overlap(&mut self.outputs.borrow_mut());
            }
            _ => (),
        }

        // Resize
        match update {
            Update::Enabled
            | Update::Disabled
            | Update::Resolution
            | Update::Position
            | Update::Reset => self.resize(self.drawing_area.width(), self.drawing_area.height()),
            _ => (),
        }

        // Redraw
        match update {
            Update::Refresh => (),
            _ => self.drawing_area.queue_draw(),
        }

        // Update disabled view
        match update {
            Update::Enabled | Update::Disabled | Update::Reset => self.disabled.update_view(),
            _ => (),
        }

        // Update details view
        match update {
            Update::Enabled | Update::Resolution => self.update_details(),
            _ => (),
        }
    }

    fn update_details(&self) {
        let outputs = self.outputs.borrow();
        self.details.update(self.selected_output.borrow().and_then(|i| Some(&outputs[i])));
    }

    fn enable_output(&self, output_id: OutputId) -> Output {
        let mut outputs = self.outputs.borrow_mut();
        let enabled_output = self.disabled.remove_output(output_id);
        enabled_output.enable();
        outputs.push(enabled_output.clone());
        self.select(outputs.len() - 1);
        enabled_output
    }

    fn disable_output(&self, output_id: OutputId) -> Output {
        let mut outputs = self.outputs.borrow_mut();
        let index = outputs
            .iter()
            .position(|o| output_id == o.id())
            .unwrap_or_else(|| panic!("enabled outputs contains output {output_id}"));
        let disabled_output = outputs.remove(index);
        disabled_output.disable();
        self.disabled.add_output(&disabled_output);
        self.deselect();
        disabled_output
    }

    fn select(&self, index: usize) {
        self.drawing_area.grab_focus();
        *self.selected_output.borrow_mut() = Some(index);
    }

    fn deselect(&self) { *self.selected_output.borrow_mut() = None; }

    #[allow(clippy::cast_possible_truncation)]
    fn resize(&self, width: i32, height: i32) {
        let outputs = self.outputs.borrow();
        let mut bounds = self.bounds.borrow_mut();
        // Translate to x = y = 0
        *bounds = Self::get_bounds(&outputs);
        for output in outputs.iter() {
            if let Some(mode) = output.mode() {
                let max_x = i16::try_from(self.size.max_width.saturating_sub(mode.width() as u16))
                    .unwrap_or(i16::MAX);
                let max_y =
                    i16::try_from(self.size.max_height.saturating_sub(mode.height() as u16))
                        .unwrap_or(i16::MAX);
                output.set_pos_x(
                    output.pos_x().saturating_sub(i32::from(bounds.x())).min(i32::from(max_x)),
                );
                output.set_pos_y(
                    output.pos_y().saturating_sub(i32::from(bounds.y())).min(i32::from(max_y)),
                );
            }
        }
        *bounds = Self::get_bounds(&outputs);
        self.scale.set(
            ((f64::from(width) - (f64::from(PADDING) + SCREEN_LINE_WIDTH) * 2.)
                / f64::from(bounds.width()))
            .min(
                (f64::from(height) - (f64::from(PADDING) + SCREEN_LINE_WIDTH) * 2.)
                    / f64::from(bounds.height()),
            ),
        );
        let dxy = i16::try_from(PADDING).unwrap() + SCREEN_LINE_WIDTH.round() as i16;
        self.translate.set([dxy, dxy]);
    }

    fn get_bounds(outputs: &[Output]) -> Rect {
        Rect::bounds(outputs.iter().map(Output::rect).collect::<Vec<_>>())
    }

    fn on_draw(&self, cr: &cairo::Context, _w: i32, _h: i32) {
        let bounds = self.bounds.borrow();
        let scale = self.scale.get();
        let translate = self.translate.get();
        let context = DrawContext::new(cr.clone(), self.config.clone());

        let screen_rect = bounds.transform(scale, translate);
        context.draw_screen(screen_rect);

        for (i, o) in self.outputs.borrow().iter().enumerate() {
            let output_rect = o.rect().transform(scale, translate);
            context.draw_output(output_rect);
            if let Some(j) = *self.selected_output.borrow() {
                if i == j {
                    context.draw_selected_output(output_rect);
                }
            }
            let mut name = o.name();
            let mut product_name = o.product_name();
            if o.primary() {
                name = format!("[{name}]");
                product_name = product_name.map(|s| format!("[{s}]"));
            }
            context.draw_output_label(output_rect, &name, product_name.as_deref());
        }
    }

    fn on_drag_begin(&self, _g: &GestureDrag, start_x: f64, start_y: f64) {
        if let Some(i) = self.get_output_index_at(start_x, start_y) {
            let scale = self.scale.get();
            let [dx, dy] = self.translate.get().map(f64::from);
            let mut outputs = self.outputs.borrow_mut();

            // Grab offset to output origin in global coordinates
            self.grab_offset.set([
                f64::from(outputs[i].pos_x()) - (start_x - dx) / scale,
                f64::from(outputs[i].pos_y()) - (start_y - dy) / scale,
            ]);

            // Push output to back, so it gets drawn last
            let output = outputs.remove(i);
            outputs.push(output);
            self.select(outputs.len() - 1);
            self.disabled.deselect();

            // Update cursor
            self.drawing_area.set_cursor_from_name(Some("grabbing"));
        } else {
            self.deselect();
            self.disabled.deselect();
        }
        self.drawing_area.queue_draw();
        self.disabled.update_view();
        self.update_details();
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_drag_update(&self, g: &GestureDrag, offset_x: f64, offset_y: f64) {
        if let Some(i) = *self.selected_output.borrow() {
            let outputs = self.outputs.borrow_mut();
            let output = &outputs[i];
            let scale = self.scale.get();
            let [dx, dy] = self.translate.get().map(f64::from);
            let [grab_dx, grab_dy] = self.grab_offset.get();

            let mut min_side = f64::MAX;
            for output in outputs.iter() {
                let mode = output.mode().expect("dragged output has mode");
                min_side = min_side.min(f64::from(mode.height()));
                min_side = min_side.min(f64::from(mode.width()));
            }
            // Snap to all snap values should be possible on all scaled sizes.
            // Give some leeway so it doesn't have to be pixel perfect.
            let snap_strength = (min_side / 4.) - (min_side / 12.);

            // Calculate snap
            let snap = Self::calculate_snap(&outputs, i);

            // Calculate new position
            let start = g.start_point().unwrap();
            let mut new_x = (((start.0 + offset_x - dx) / scale) + grab_dx).round() as i16;
            let mut new_y = (((start.1 + offset_y - dy) / scale) + grab_dy).round() as i16;

            // Apply snap
            if snap.x == 0 {
                if f64::from((new_x - output.pos_x() as i16).abs()) < snap_strength {
                    new_x = output.pos_x() as i16;
                }
            } else if f64::from(snap.x.abs()) < snap_strength {
                new_x = (output.pos_x() as i16).saturating_add(i16::try_from(snap.x).unwrap());
            }
            if snap.y == 0 {
                if f64::from((new_y - output.pos_y() as i16).abs()) < snap_strength {
                    new_y = output.pos_y() as i16;
                }
            } else if f64::from(snap.y.abs()) < snap_strength {
                new_y = (output.pos_y() as i16).saturating_add(i16::try_from(snap.y).unwrap());
            }

            // Update new position
            if new_x != output.pos_x() as i16 || new_y != output.pos_y() as i16 {
                outputs[i].set_pos_x(new_x as i32);
                outputs[i].set_pos_y(new_y as i32);
            }
        }
        self.update_view(Update::Position);
        self.update_details();
    }

    fn calculate_snap(outputs: &[Output], output_index: usize) -> Point {
        let output_r = &outputs[output_index].rect();
        let output_center = output_r.center();
        let mut dist = Point::max();
        let mut snap = Point::default();
        for (j, other) in outputs.iter().enumerate() {
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
        let bounds = Self::get_bounds(outputs);
        let bc = bounds.center();

        for output in outputs.iter() {
            let r = output.rect();
            let c = r.center();
            let mut e = [0., 0.];
            if bc != c {
                let d = [f64::from(bc.x - c.x), f64::from(bc.y - c.y)];
                let d_len = (d[0].powi(2) + d[1].powi(2)).sqrt();
                e = [d[0] / d_len, d[1] / d_len];
            }
            data.insert(output.id(), (r, e));
        }

        let step = 50.;
        let mut moved = Vec::new();
        let mut max_loops = (f64::from(bounds.width().max(bounds.height())) / step) as u16;
        loop {
            for i in 0..outputs.len() {
                // Current position
                let mut r = data[&outputs[i].id()].0.clone();
                // Unit direction
                let e = data[&outputs[i].id()].1;
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
                    if other.id() == outputs[i].id() {
                        continue;
                    }
                    if let Some(intersect) = r.intersect(&data[&other.id()].0) {
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
                let old_r = &data[&outputs[i].id()].0;
                if r.x() != old_r.x() || r.y() != old_r.y() {
                    moved.push(true);
                }
                data.insert(outputs[i].id(), (r, e));
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
            output.set_pos_x(data[&output.id()].0.x() as i32);
            output.set_pos_y(data[&output.id()].0.y() as i32);
        }
    }

    fn on_drag_end(&self, g: &GestureDrag, offset_x: f64, offset_y: f64) {
        self.grab_offset.set([0., 0.]);
        // Update cursor
        if let Some((x, y)) = g.start_point() {
            match self.get_output_index_at(x + offset_x, y + offset_y) {
                Some(_) => self.drawing_area.set_cursor_from_name(Some("pointer")),
                None => self.drawing_area.set_cursor_from_name(Some("default")),
            }
        }
    }

    fn on_motion(&self, _ecm: &EventControllerMotion, x: f64, y: f64) {
        // TODO if not is_dragging instead
        let [grab_dx, grab_dy] = self.grab_offset.get();
        if grab_dx == 0. || grab_dy == 0. {
            // Update cursor
            match self.get_output_index_at(x, y) {
                Some(_) => self.drawing_area.set_cursor_from_name(Some("pointer")),
                None => self.drawing_area.set_cursor_from_name(Some("default")),
            }
        }
    }

    fn on_enter(_ecm: &EventControllerMotion, _x: f64, _y: f64) {}

    fn on_leave(_ecm: &EventControllerMotion) {}

    #[allow(clippy::cast_possible_truncation)]
    fn on_drop(&self, _dt: &DropTarget, v: &Value, x: f64, y: f64) -> bool {
        let Ok(id) = v.get::<u32>() else {
            return false;
        };

        let output = self.disabled.remove_output(id);
        let scale = self.scale.get();
        let [dx, dy] = self.translate.get().map(f64::from);
        output.enable_at(
            ((x - dx).max(0.) / scale).round() as i16,
            ((y - dy).max(0.) / scale).round() as i16,
        );
        self.outputs.borrow_mut().push(output);
        self.select(self.outputs.borrow_mut().len() - 1);
        self.update_view(Update::Enabled);

        true
    }

    fn on_drop_motion(_dt: &DropTarget, _x: f64, _y: f64) -> DragAction { DragAction::MOVE }

    fn on_key_pressed(
        &self,
        _eck: &EventControllerKey,
        keyval: Key,
        _keycode: u32,
        _state: ModifierType,
    ) -> Propagation {
        match keyval {
            Key::Delete => {
                if let Some(output_id) = self.get_selected_output() {
                    self.details.update(Some(&self.disable_output(output_id)));
                    self.update_view(Update::Disabled);
                    return Propagation::Stop;
                }
                if let Some(output_id) = self.disabled.selected_output() {
                    self.details.update(Some(&self.enable_output(output_id)));
                    self.update_view(Update::Enabled);
                    return Propagation::Stop;
                }
            }
            Key::F9 => {
                if let Some(handle) =
                    self.paned.start_child().and_then(|start| start.next_sibling())
                {
                    let cur_pos = self.paned.position();
                    let max_pos = self.paned.width() - handle.width();
                    self.paned.set_position(if cur_pos == max_pos {
                        self.last_handle_pos.get()
                    } else {
                        self.last_handle_pos.set(cur_pos);
                        max_pos
                    });
                }
            }
            _ => (),
        };
        Propagation::Proceed
    }

    fn get_selected_output(&self) -> Option<OutputId> {
        if let Some(i) = *self.selected_output.borrow() {
            return Some(self.outputs.borrow()[i].id());
        }
        None
    }

    fn get_output_index_at(&self, x: f64, y: f64) -> Option<usize> {
        let scale = self.scale.get();
        let [dx, dy] = self.translate.get();

        for (i, output) in self.outputs.borrow().iter().enumerate() {
            let mut scaled_rect = output.rect();
            scaled_rect.scale(scale);
            scaled_rect.translate(dx, dy);
            if scaled_rect.contains(x, y) {
                return Some(i);
            }
        }
        None
    }

    fn on_disabled_selected(&self, output: &Output) {
        self.deselect();
        self.drawing_area.queue_draw();
        self.details.update(Some(output));
    }

    fn on_disabled_deselected(&self) {
        self.deselect();
        self.drawing_area.queue_draw();
        self.details.update(None);
    }

    pub fn apply(&self) { *self.outputs_orig.borrow_mut() = self.get_outputs() }

    pub fn reset(&self) {
        let enabled_outputs = self
            .outputs_orig
            .borrow()
            .iter()
            .filter(|&n| n.enabled())
            .map(Output::clone)
            .collect::<Vec<_>>();
        let disabled_outputs = self
            .outputs_orig
            .borrow()
            .iter()
            .filter(|&n| !n.enabled())
            .map(Output::clone)
            .collect::<Vec<_>>();
        *self.outputs.borrow_mut() = enabled_outputs;
        self.disabled.set_outputs(Outputs::new(&disabled_outputs));

        self.deselect();
        self.disabled.deselect();
        self.update_view(Update::Reset);
        self.details.update(None);
    }

    pub fn set_apply_callback(&self, callback: impl Fn(Vec<Output>) + 'static) {
        *self.apply_callback.borrow_mut() = Some(Rc::new(callback));
    }
}
