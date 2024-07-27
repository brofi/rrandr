use glib::object::ObjectExt;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{closure_local, wrapper, Object};
use gtk::prelude::{ListModelExtManual, WidgetExt};
use gtk::subclass::drawing_area::DrawingAreaImpl;
use gtk::{glib, Accessible, Buildable, ConstraintTarget, DrawingArea, Widget};

use super::details_box::Update;
use crate::config::Config;
use crate::data::output::Output;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use gdk::{DragAction, Key, ModifierType};
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::Signal;
    use glib::{clone, derived_properties, object_subclass, Propagation, Properties, Value};
    use gtk::prelude::{
        DrawingAreaExtManual, GestureDragExt, ListModelExt, ListModelExtManual, ObjectExt,
        StaticType, WidgetExt,
    };
    use gtk::subclass::drawing_area::{DrawingAreaImpl, DrawingAreaImplExt};
    use gtk::subclass::prelude::{DerivedObjectProperties, ObjectSubclass, ObjectSubclassExt};
    use gtk::subclass::widget::WidgetImpl;
    use gtk::{
        glib, DrawingArea, DropTarget, EventControllerKey, EventControllerMotion,
        EventControllerScroll, EventControllerScrollFlags, GestureClick, GestureDrag,
    };

    use crate::config::Config;
    use crate::data::output::Output;
    use crate::data::outputs::Outputs;
    use crate::draw::{DrawContext, SCREEN_LINE_WIDTH};
    use crate::math::{Point, Rect};
    use crate::widget::details_box::Update;
    use crate::window::PADDING;

    const MOVE_DISTANCE: i16 = 10;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::OutputArea)]
    pub struct OutputArea {
        pub(super) config: RefCell<Config>,
        #[property(get, set = Self::set_outputs)]
        outputs: RefCell<Outputs>,
        pub(super) screen_max_width: Cell<u16>,
        pub(super) screen_max_height: Cell<u16>,
        pub(super) selected_output: RefCell<Option<Output>>,
        grab_offset: Cell<[f64; 2]>,
        scale: Cell<f64>,
        translate: Cell<[i16; 2]>,
        bounds: RefCell<Rect>,
    }

    #[object_subclass]
    impl ObjectSubclass for OutputArea {
        type ParentType = DrawingArea;
        type Type = super::OutputArea;

        const NAME: &'static str = "OutputArea";
    }

    #[derived_properties]
    impl ObjectImpl for OutputArea {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("output-selected").param_types([Output::static_type()]).build(),
                    Signal::builder("output-deselected").build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_focusable(true);

            obj.set_draw_func(clone!(
                #[weak(rename_to = this)]
                self,
                move |_d, cr, width, height| this.on_draw(cr, width, height)
            ));

            let gesture_drag = GestureDrag::new();
            gesture_drag.connect_drag_begin(clone!(
                #[weak(rename_to = this)]
                self,
                move |g, start_x, start_y| this.on_drag_begin(g, start_x, start_y)
            ));
            gesture_drag.connect_drag_update(clone!(
                #[weak(rename_to = this)]
                self,
                move |g, offset_x, offset_y| this.on_drag_update(g, offset_x, offset_y)
            ));
            gesture_drag.connect_drag_end(clone!(
                #[weak(rename_to = this)]
                self,
                move |g, offset_x, offset_y| this.on_drag_end(g, offset_x, offset_y)
            ));
            obj.add_controller(gesture_drag);

            let event_controller_motion = EventControllerMotion::new();
            event_controller_motion.connect_motion(clone!(
                #[weak(rename_to = this)]
                self,
                move |ecm, x, y| this.on_motion(ecm, x, y)
            ));
            event_controller_motion.connect_enter(Self::on_enter);
            event_controller_motion.connect_leave(Self::on_leave);
            obj.add_controller(event_controller_motion);

            let drop_target = DropTarget::new(Output::static_type(), DragAction::MOVE);
            drop_target.connect_drop(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |dt, v, x, y| this.on_drop(dt, v, x, y)
            ));
            drop_target.connect_motion(Self::on_drop_motion);
            obj.add_controller(drop_target);

            let gesture_click = GestureClick::new();
            gesture_click.connect_pressed(clone!(
                #[weak(rename_to = this)]
                self,
                move |gc, n_press, x, y| this.on_click(gc, n_press, x, y)
            ));
            obj.add_controller(gesture_click);

            let event_controller_scroll = EventControllerScroll::builder()
                .flags(EventControllerScrollFlags::DISCRETE | EventControllerScrollFlags::VERTICAL)
                .build();
            event_controller_scroll.connect_scroll(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |ecs, x, y| this.on_discrete_vertical_scroll(ecs, x, y)
            ));
            obj.add_controller(event_controller_scroll);

            let event_controller_key = EventControllerKey::new();
            event_controller_key.connect_key_pressed(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |eck, keyval, keycode, state| this.on_key_pressed(eck, keyval, keycode, state)
            ));
            self.obj().add_controller(event_controller_key);
        }
    }

    impl WidgetImpl for OutputArea {}

    impl DrawingAreaImpl for OutputArea {
        #[allow(clippy::cast_possible_truncation)]
        fn resize(&self, width: i32, height: i32) {
            self.parent_resize(width, height);

            let outputs = self.outputs.borrow();
            let mut bounds = self.bounds.borrow_mut();
            // Translate to x = y = 0
            *bounds = Self::get_bounds(&outputs);
            for output in outputs.iter::<Output>().map(Result::unwrap) {
                if let Some(mode) = output.mode() {
                    let max_x =
                        i16::try_from(self.screen_max_width.get().saturating_sub(mode.width()))
                            .unwrap_or(i16::MAX);
                    let x = output.x().saturating_sub(bounds.x()).min(max_x);
                    if x != output.x() {
                        output.set_x(x);
                    }
                    let max_y =
                        i16::try_from(self.screen_max_height.get().saturating_sub(mode.height()))
                            .unwrap_or(i16::MAX);
                    let y = output.y().saturating_sub(bounds.y()).min(max_y);
                    if y != output.y() {
                        output.set_y(y);
                    }
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
    }

    impl OutputArea {
        fn set_outputs(&self, outputs: &Outputs) {
            self.outputs.replace(outputs.clone());
            let selected = self.selected_output.take();
            self.selected_output.replace(selected.and_then(|s| outputs.find_by_id(s.id())));
            self.resize(self.obj().width(), self.obj().height());
            self.obj().queue_draw();
        }

        pub(super) fn add_output(&self, output: &Output) {
            let outputs = self.outputs.borrow();
            outputs.append(output);
            self.select(output);
        }

        pub(super) fn remove_output(&self, output: &Output) {
            self.deselect();
            self.outputs.borrow().remove(output.id());
        }

        pub(super) fn select(&self, output: &Output) {
            self.obj().grab_focus();
            self.selected_output.replace(Some(output.clone()));
        }

        pub(super) fn deselect(&self) { self.selected_output.replace(None); }

        fn get_bounds(outputs: &Outputs) -> Rect {
            Rect::bounds(outputs.iter::<Output>().map(Result::unwrap).map(|o| o.rect()).collect())
        }

        fn on_draw(&self, cr: &cairo::Context, _w: i32, _h: i32) {
            let bounds = self.bounds.borrow();
            let scale = self.scale.get();
            let translate = self.translate.get();
            let context = DrawContext::new(cr, &self.config.borrow());

            let screen_rect = bounds.transform(scale, translate);
            context.draw_screen(screen_rect);

            for o in self.outputs.borrow().iter::<Output>().map(Result::unwrap) {
                let output_rect = o.rect().transform(scale, translate);
                context.draw_output(output_rect);
                if let Some(selected) = self.selected_output.borrow().as_ref() {
                    if o == *selected {
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
            if let Some(output) = self.get_output_at(start_x, start_y) {
                let scale = self.scale.get();
                let [dx, dy] = self.translate.get().map(f64::from);
                let outputs = self.outputs.borrow();

                // Grab offset to output origin in global coordinates
                self.grab_offset.set([
                    f64::from(output.x()) - (start_x - dx) / scale,
                    f64::from(output.y()) - (start_y - dy) / scale,
                ]);

                self.obj().emit_by_name::<()>("output-selected", &[&output]);
                // Push output to back, so it gets drawn last
                outputs.push_back(&output);
                self.select(&output);

                // Update cursor
                self.obj().set_cursor_from_name(Some("grabbing"));
            } else {
                self.deselect();
                self.obj().emit_by_name::<()>("output-deselected", &[]);
            }
            self.obj().queue_draw();
        }

        #[allow(clippy::cast_possible_truncation)]
        fn on_drag_update(&self, g: &GestureDrag, offset_x: f64, offset_y: f64) {
            if let Some(output) = self.selected_output.borrow().as_ref() {
                let outputs = self.outputs.borrow();

                let mut min_side = f64::MAX;
                for output in outputs.iter::<Output>().map(Result::unwrap) {
                    let mode = output.mode().expect("dragged output has mode");
                    min_side = min_side.min(f64::from(mode.height()));
                    min_side = min_side.min(f64::from(mode.width()));
                }
                // Snap to all snap values should be possible on all scaled sizes.
                // Give some leeway so it doesn't have to be pixel perfect.
                let snap_strength = (min_side / 4.) - (min_side / 12.);

                // Calculate snap
                let snap = Self::calculate_snap(&outputs, output);

                // Calculate new position
                let scale = self.scale.get();
                let start = g.start_point().unwrap();
                let grab = self.grab_offset.get();
                let [dx, dy] = self.translate.get().map(f64::from);
                let mut new_x = (((start.0 + offset_x - dx) / scale) + grab[0]).round() as i16;
                let mut new_y = (((start.1 + offset_y - dy) / scale) + grab[1]).round() as i16;

                // Apply snap
                if snap.x == 0 {
                    if f64::from((new_x - output.x()).abs()) < snap_strength {
                        new_x = output.x();
                    }
                } else if f64::from(snap.x.abs()) < snap_strength {
                    new_x = (output.x()).saturating_add(i16::try_from(snap.x).unwrap());
                }
                if snap.y == 0 {
                    if f64::from((new_y - output.y()).abs()) < snap_strength {
                        new_y = output.y();
                    }
                } else if f64::from(snap.y.abs()) < snap_strength {
                    new_y = (output.y()).saturating_add(i16::try_from(snap.y).unwrap());
                }

                // Update new position
                if new_x != output.x() || new_y != output.y() {
                    output.set_x(new_x);
                    output.set_y(new_y);
                    self.resize(self.obj().width(), self.obj().height());
                    self.obj().queue_draw();
                }
            }
        }

        fn calculate_snap(outputs: &Outputs, selected_output: &Output) -> Point {
            let output_r = &selected_output.rect();
            let output_center = output_r.center();
            let mut dist = Point::max();
            let mut snap = Point::default();
            for other in outputs.iter::<Output>().map(Result::unwrap) {
                if *selected_output != other {
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
        pub(super) fn mind_the_gap_and_overlap(outputs: &Outputs) {
            let mut data = HashMap::new();
            let bounds = Self::get_bounds(outputs);
            let bc = bounds.center();

            for output in outputs.iter::<Output>().map(Result::unwrap) {
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
                for i in 0..(outputs.n_items() as usize) {
                    // Current position
                    let mut r = data[&outputs.index(i).id()].0.clone();
                    // Unit direction
                    let e = data[&outputs.index(i).id()].1;
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
                    for other in outputs.iter::<Output>().map(Result::unwrap) {
                        if other.id() == outputs.index(i).id() {
                            continue;
                        }
                        if let Some(intersect) = r.intersect(&data[&other.id()].0) {
                            let mut dx = -sx * f64::from(intersect.width());
                            let mut dy = -sy * f64::from(intersect.height());

                            if e[1].abs() > 0. {
                                // Calculate the x where a line in the flipped direction towards
                                // center intersects the bottom of
                                // the intersection rectangle.
                                let ix = sx * e[0] * f64::from(intersect.height()) / (sy * e[1]);
                                // If the intersection rectangle bottom is intersected
                                if ix.abs() <= f64::from(intersect.width()) {
                                    dx = -sx * ix;
                                }
                            }
                            if e[0].abs() > 0. {
                                // Calculate the y where a line in the flipped direction towards
                                // center intersects the right of
                                // the intersection rectangle.
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
                    let old_r = &data[&outputs.index(i).id()].0;
                    if r.x() != old_r.x() || r.y() != old_r.y() {
                        moved.push(true);
                    }
                    data.insert(outputs.index(i).id(), (r, e));
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
            for output in outputs.iter::<Output>().map(Result::unwrap) {
                let x = data[&output.id()].0.x();
                if x != output.x() {
                    output.set_x(x);
                }
                let y = data[&output.id()].0.y();
                if y != output.y() {
                    output.set_y(y);
                }
            }
        }

        fn on_drag_end(&self, g: &GestureDrag, offset_x: f64, offset_y: f64) {
            self.grab_offset.set([0., 0.]);
            // Update cursor
            if let Some((x, y)) = g.start_point() {
                match self.get_output_at(x + offset_x, y + offset_y) {
                    Some(_) => self.obj().set_cursor_from_name(Some("pointer")),
                    None => self.obj().set_cursor_from_name(Some("default")),
                }
            }
        }

        fn on_motion(&self, _ecm: &EventControllerMotion, x: f64, y: f64) {
            // TODO if not is_dragging instead
            let [dx, dy] = self.grab_offset.get();
            if dx == 0. || dy == 0. {
                // Update cursor
                match self.get_output_at(x, y) {
                    Some(_) => self.obj().set_cursor_from_name(Some("pointer")),
                    None => self.obj().set_cursor_from_name(Some("default")),
                }
            }
        }

        fn on_enter(_ecm: &EventControllerMotion, _x: f64, _y: f64) {}

        fn on_leave(_ecm: &EventControllerMotion) {}

        #[allow(clippy::cast_possible_truncation)]
        fn on_drop(&self, _dt: &DropTarget, v: &Value, x: f64, y: f64) -> bool {
            let Ok(output) = v.get::<Output>() else {
                return false;
            };

            let scale = self.scale.get();
            let [dx, dy] = self.translate.get().map(f64::from);
            output.enable_at(
                ((x - dx).max(0.) / scale).round() as i16,
                ((y - dy).max(0.) / scale).round() as i16,
            );
            self.obj().update(&output, Update::Enabled);

            true
        }

        fn on_drop_motion(_dt: &DropTarget, _x: f64, _y: f64) -> DragAction { DragAction::MOVE }

        fn on_click(&self, _gc: &GestureClick, n_press: i32, x: f64, y: f64) {
            if n_press == 2 {
                if let Some(output) = self.get_output_at(x, y) {
                    output.set_primary(!output.primary());
                    self.obj().update(&output, Update::Primary);
                }
            }
        }

        fn on_discrete_vertical_scroll(
            &self,
            _ecs: &EventControllerScroll,
            _x: f64,
            y: f64,
        ) -> Propagation {
            if let Some(selected) = self.selected_output.borrow().as_ref() {
                let mode = selected.mode().expect("output should have a mode");
                let next = if y > 0. {
                    selected.modes().next_scroll_mode(&mode)
                } else {
                    selected.modes().prev_scroll_mode(&mode)
                };
                if let Some(next) = next {
                    let update = if mode.width() != next.width() || mode.height() != next.height() {
                        Update::Resolution
                    } else {
                        Update::Refresh
                    };
                    selected.set_mode(Some(&next));
                    self.obj().update(selected, update);
                    return Propagation::Stop;
                }
            }
            Propagation::Proceed
        }

        fn on_key_pressed(
            &self,
            _eck: &EventControllerKey,
            keyval: Key,
            _keycode: u32,
            _state: ModifierType,
        ) -> Propagation {
            if let Some(selected) = self.selected_output.borrow().as_ref() {
                let [x, y] = [selected.x(), selected.y()];
                let update_pos = match keyval {
                    Key::Up | Key::k => {
                        selected.set_y(y - MOVE_DISTANCE);
                        true
                    }
                    Key::Down | Key::j => {
                        selected.set_y(y + MOVE_DISTANCE);
                        true
                    }
                    Key::Left | Key::h => {
                        selected.set_x(x - MOVE_DISTANCE);
                        true
                    }
                    Key::Right | Key::l => {
                        selected.set_x(x + MOVE_DISTANCE);
                        true
                    }
                    _ => false,
                };
                if update_pos {
                    self.resize(self.obj().width(), self.obj().height());
                    self.obj().queue_draw();
                    return Propagation::Stop;
                }
            }
            Propagation::Proceed
        }

        fn get_output_at(&self, x: f64, y: f64) -> Option<Output> {
            let scale = self.scale.get();
            let [dx, dy] = self.translate.get();

            for output in self.outputs.borrow().iter::<Output>().map(Result::unwrap) {
                let mut scaled_rect = output.rect();
                scaled_rect.scale(scale);
                scaled_rect.translate(dx, dy);
                if scaled_rect.contains(x, y) {
                    return Some(output);
                }
            }
            None
        }
    }
}

wrapper! {
    pub struct OutputArea(ObjectSubclass<imp::OutputArea>)
        @extends DrawingArea, Widget,
        @implements Accessible, Buildable, ConstraintTarget;
}

impl OutputArea {
    pub fn new() -> Self { Object::new() }

    pub fn set_config(&self, cfg: &Config) {
        self.imp().config.replace(cfg.clone());
    }

    pub fn set_screen_max_width(&self, screen_max_width: u16) {
        let imp = self.imp();
        imp.screen_max_width.set(screen_max_width);
        imp.resize(self.width(), self.height());
        self.queue_draw();
    }

    pub fn set_screen_max_height(&self, screen_max_height: u16) {
        let imp = self.imp();
        imp.screen_max_height.set(screen_max_height);
        imp.resize(self.width(), self.height());
        self.queue_draw();
    }

    pub fn connect_output_selected(&self, callback: impl Fn(&Self, &Output) + 'static) {
        self.connect_closure(
            "output-selected",
            false,
            closure_local!(|details, output| callback(details, output)),
        );
    }

    pub fn connect_output_deselected(&self, callback: impl Fn(&Self) + 'static) {
        self.connect_closure(
            "output-deselected",
            false,
            closure_local!(|details| callback(details)),
        );
    }

    pub fn update(&self, output: &Output, update: Update) {
        // Add/Remove
        match update {
            Update::Enabled => self.imp().add_output(output),
            Update::Disabled => self.imp().remove_output(output),
            _ => (),
        }
        // Set/unset primary
        if let Update::Primary = update {
            for o in self.outputs().iter::<Output>().map(Result::unwrap) {
                o.set_primary(o == *output && output.primary());
            }
        }
        // Mind the gap
        match update {
            Update::Enabled | Update::Disabled | Update::Resolution => {
                imp::OutputArea::mind_the_gap_and_overlap(&self.outputs());
            }
            _ => (),
        }
        // Resize
        match update {
            Update::Enabled | Update::Disabled | Update::Resolution | Update::Position => {
                self.imp().resize(self.width(), self.height());
            }
            _ => (),
        }
        // Redraw
        match update {
            Update::Refresh => (),
            _ => self.queue_draw(),
        }
    }

    pub fn selected_output(&self) -> Option<Output> { self.imp().selected_output.borrow().clone() }

    pub fn select(&self, output: &Output) { self.imp().select(output); }

    pub fn deselect(&self) { self.imp().deselect(); }
}

impl Default for OutputArea {
    fn default() -> Self { Self::new() }
}
