use crate::{get_bounds, Output, ScreenSizeRange};
use gdk::{
    glib::{clone, Bytes, Type, Value},
    ContentProvider, Drag, DragAction, MemoryTexture, Paintable, RGBA,
};
use gtk::{
    graphene::{Point, Rect},
    prelude::*,
    Align, Button, CheckButton, DragSource, DrawingArea, DropControllerMotion, DropDown,
    DropTarget, Entry, EventControllerMotion, FlowBox, FlowBoxChild, Frame, GestureClick,
    GestureDrag, Label, Orientation, Paned, SelectionMode, StringList, Switch, Widget,
};
use pango::{Alignment, FontDescription, Weight};
use pangocairo::functions::{create_layout, show_layout};
use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    ops::{Deref, DerefMut},
    rc::Rc,
};

pub const VIEW_PADDING: i32 = 10;
const COLOR_GREEN: RGBA = RGBA::new(0.722, 0.733, 0.149, 1.);
const COLOR_FG: RGBA = RGBA::new(0.922, 0.859, 0.698, 1.);
const COLOR_BG0_H: RGBA = RGBA::new(0.114, 0.125, 0.129, 1.);
const COLOR_BG0: RGBA = RGBA::new(0.157, 0.157, 0.157, 1.);

// needed because to tansfer ownership because: function requires argument type to outlive `'static`
// https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html
// It's important to understand this means that any owned data always passes a 'static lifetime bound, but a reference to that owned data generally does not:
// https://stackoverflow.com/questions/52464653/how-to-move-data-into-multiple-rust-closures
#[derive(Debug)]
pub struct View {
    outputs: RefCell<Vec<Output>>,
    outputs_orig: RefCell<Vec<Output>>,
    size: ScreenSizeRange,

    selected_output: RefCell<Option<usize>>,
    grab_offset: RefCell<(f64, f64)>,
    scale: RefCell<f64>,
    translate: RefCell<(f64, f64)>,
    bounds: RefCell<Rect>,

    // dragged_disabled_output: RefCell<Option<usize>>,
    dragging_disabled_output: RefCell<bool>,

    skip_update_refresh_model: RefCell<bool>,
    skip_update_output: RefCell<bool>,
}

impl View {
    pub fn create(
        outputs: Vec<Output>,
        size: ScreenSizeRange,
        apply_callback: impl Fn(Vec<Output>) -> bool + 'static,
    ) -> impl IsA<Widget> {
        let outputs_orig = outputs.clone();
        let shared = Rc::new(Self {
            outputs: RefCell::new(outputs),
            outputs_orig: RefCell::new(outputs_orig),
            size,

            selected_output: RefCell::new(None),
            grab_offset: RefCell::new((0.0, 0.0)),
            scale: RefCell::new(1.0),
            translate: RefCell::new((0., 0.)),
            bounds: RefCell::new(Rect::zero()),

            // dragged_disabled_output: RefCell::new(None),
            dragging_disabled_output: RefCell::new(false),

            skip_update_refresh_model: RefCell::new(false),
            skip_update_output: RefCell::new(false),
        });

        let root = gtk::Box::builder()
            .orientation(Orientation::Vertical)
            .margin_start(VIEW_PADDING)
            .margin_end(VIEW_PADDING)
            .margin_top(VIEW_PADDING)
            .margin_bottom(VIEW_PADDING)
            .spacing(VIEW_PADDING)
            .build();

        let enabled_area = DrawingArea::new();
        let frame_enabled = Frame::builder()
            .label("Layout")
            .child(&enabled_area)
            .build();

        let disabled_area = DrawingArea::new();
        let frame_disabled = Frame::builder()
            .label("Disabled")
            .child(&disabled_area)
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
            .spacing(VIEW_PADDING)
            .build();
        let flow_box_details = FlowBox::builder()
            .row_spacing(VIEW_PADDING as u32)
            .column_spacing(VIEW_PADDING as u32)
            .orientation(Orientation::Horizontal)
            .selection_mode(SelectionMode::None)
            .halign(Align::Start)
            .min_children_per_line(2)
            .hexpand(true)
            .build();

        let sw_enabled = Switch::new();
        flow_box_details.append(&Self::create_detail_child("Enabled", &sw_enabled));
        let dd_resolution = DropDown::builder().build();
        flow_box_details.append(&Self::create_detail_child("Resolution", &dd_resolution));
        let dd_refresh = DropDown::builder().build();
        flow_box_details.append(&Self::create_detail_child("Re_fresh Rate", &dd_refresh));
        let en_position = Entry::builder()
            .text("+0+0")
            .placeholder_text("+x+y")
            .editable(false)
            .width_chars(13)
            .max_width_chars(12)
            .build();
        EntryExt::set_alignment(&en_position, 1.);
        flow_box_details.append(&Self::create_detail_child("Position", &en_position));
        let cb_primary = CheckButton::builder().build();
        flow_box_details.append(&Self::create_detail_child("Pr_imary", &cb_primary));

        box_bottom.append(&flow_box_details);

        let box_controls = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(VIEW_PADDING)
            .halign(Align::End)
            .valign(Align::End)
            .build();
        let btn_apply = Button::with_mnemonic("Apply");
        btn_apply.connect_clicked(clone!(
            @strong shared,
            @strong enabled_area,
            @strong disabled_area
            => move |_btn| {
                if apply_callback(shared.outputs.borrow().clone()) {
                    shared.outputs_orig.borrow_mut().clone_from(&shared.outputs.borrow());
                } else {
                    shared.outputs.borrow_mut().clone_from(&shared.outputs_orig.borrow());
                    Self::resize(
                        enabled_area.width(),
                        enabled_area.height(),
                        shared.size,
                        shared.scale.borrow_mut().deref_mut(),
                        shared.translate.borrow_mut().deref_mut(),
                        shared.bounds.borrow_mut().deref_mut(),
                        shared.outputs.borrow_mut().deref_mut(),
                    );
                    enabled_area.queue_draw();
                    disabled_area.queue_draw();
                }
        }));
        box_controls.append(&btn_apply);
        let btn_reset = Button::with_mnemonic("Reset");
        btn_reset.connect_clicked(clone!(
            @strong shared,
            @strong enabled_area,
            @strong disabled_area,
            @strong flow_box_details as details_ui
            => move |_btn| shared.on_reset_clicked(&enabled_area, &disabled_area, &details_ui)));
        box_controls.append(&btn_reset);

        box_bottom.append(&box_controls);
        root.append(&box_bottom);

        enabled_area.set_draw_func({
            let shared = Rc::clone(&shared);
            move |_d, cr, w, h| shared.on_draw(cr, w, h)
        });
        disabled_area.set_draw_func({
            let shared = Rc::clone(&shared);
            move |_d, cr, w, h| shared.on_draw_disabled(cr, w, h)
        });
        enabled_area.connect_resize({
            let shared = Rc::clone(&shared);
            move |_d, w, h| {
                Self::resize(
                    w,
                    h,
                    shared.size,
                    shared.scale.borrow_mut().deref_mut(),
                    shared.translate.borrow_mut().deref_mut(),
                    shared.bounds.borrow_mut().deref_mut(),
                    shared.outputs.borrow_mut().deref_mut(),
                )
            }
        });

        let gesture_drag = GestureDrag::new();
        gesture_drag.connect_drag_begin({
            let shared = Rc::clone(&shared);
            let sw_enabled = sw_enabled.clone();
            let dd_resolution = dd_resolution.clone();
            let cb_primary = cb_primary.clone();
            let en_position = en_position.clone();
            let details_ui = flow_box_details.clone();
            let disabled_area = disabled_area.clone();
            move |g, start_x, start_y| {
                shared.on_drag_begin(
                    g,
                    start_x,
                    start_y,
                    &sw_enabled,
                    &dd_resolution,
                    &cb_primary,
                    &en_position,
                    &details_ui,
                    &disabled_area,
                )
            }
        });
        gesture_drag.connect_drag_update({
            let shared = Rc::clone(&shared);
            let en_position = en_position.clone();
            move |g, offset_x, offset_y| shared.on_drag_update(g, offset_x, offset_y, &en_position)
        });
        gesture_drag.connect_drag_end({
            let shared = Rc::clone(&shared);
            move |_g, _offset_x, _offset_y| shared.on_drag_end(_g, _offset_x, _offset_y)
        });
        enabled_area.add_controller(gesture_drag);

        let event_controller_motion = EventControllerMotion::new();
        event_controller_motion.connect_motion({
            let shared = Rc::clone(&shared);
            move |ecm, x, y| shared.on_motion(ecm, x, y)
        });
        event_controller_motion.connect_enter({
            let shared = Rc::clone(&shared);
            move |ecm, x, y| shared.on_enter(ecm, x, y)
        });
        event_controller_motion.connect_leave({
            let shared = Rc::clone(&shared);
            move |ecm| shared.on_leave(ecm)
        });
        enabled_area.add_controller(event_controller_motion);

        let drag_source = DragSource::builder().actions(DragAction::MOVE).build();
        drag_source.connect_prepare(
            clone!(@strong shared => move |ds, x, y| shared.on_dragdrop_prepare(ds, x, y)),
        );
        drag_source.connect_drag_begin(
            clone!(@strong shared => move |ds, d| shared.on_dragdrop_begin(ds, d)),
        );
        drag_source.connect_drag_end(
            clone!(@strong shared => move |ds, d, del| shared.on_dragdrop_end(ds, d, del)),
        );
        disabled_area.add_controller(drag_source);

        let gesture_click = GestureClick::new();
        gesture_click.connect_pressed(clone!(
            @strong shared,
            @strong sw_enabled,
            @strong enabled_area,
            @strong flow_box_details as details_ui
            => move |gc, n_press, x, y| shared.on_disabled_click(gc, n_press, x, y, &sw_enabled, &enabled_area, &details_ui)));
        disabled_area.add_controller(gesture_click);

        let event_controller_motion = EventControllerMotion::new();
        event_controller_motion.connect_motion(
            clone!(@strong shared => move |ecm, x, y| shared.on_disabled_motion(ecm, x, y)),
        );
        disabled_area.add_controller(event_controller_motion);

        let drop_controller_motion = DropControllerMotion::new();
        drop_controller_motion.connect_motion(
            clone!(@strong shared => move |dcm, x, y| shared.on_disabled_dragdrop_motion(dcm, x, y)),
        );
        disabled_area.add_controller(drop_controller_motion);

        let drop_target = DropTarget::new(Type::U64, DragAction::MOVE);
        drop_target.connect_drop(
            clone!(
                @strong shared,
                @strong disabled_area,
                @strong sw_enabled,
                @strong dd_resolution,
                @strong cb_primary,
                @strong en_position,
                @strong flow_box_details as details_ui
                => move |dt, v, x, y| shared.on_dragdrop_drop(dt, v, x, y, &disabled_area, &sw_enabled, &dd_resolution, &cb_primary, &en_position, &details_ui))
        );
        drop_target.connect_motion(
            clone!(@strong shared => move |dt, x, y| shared.on_dragdrop_motion(dt, x, y)),
        );
        enabled_area.add_controller(drop_target);

        dd_resolution.connect_selected_item_notify(
            clone!(@strong shared, @strong dd_refresh, @strong enabled_area => move |dd_resolution| shared.on_resolution_selected(dd_resolution, &dd_refresh, &en_position, &enabled_area)),
        );
        dd_refresh.connect_selected_item_notify(
            clone!(@strong shared => move |dd_refresh| shared.on_refresh_rate_selected(dd_refresh, &dd_resolution)),
        );
        cb_primary.connect_active_notify(
            clone!(@strong shared, @strong enabled_area => move |cb_primary| shared.on_primary_checked(cb_primary, &enabled_area)),
        );
        sw_enabled.connect_active_notify(
            clone!(@strong shared => move |sw_enabled| shared.on_enabled_switched(sw_enabled, &enabled_area, &disabled_area, &flow_box_details)),
        );

        root
    }

    fn create_detail_child<W: IsA<Widget>>(label: &str, ctrl: &W) -> impl IsA<Widget> {
        let child = FlowBoxChild::builder()
            .halign(Align::Start)
            .valign(Align::Center)
            .hexpand(false)
            .vexpand(false)
            .focusable(false)
            .build();
        child.set_visible(false);
        child.set_widget_name(
            &("fbc_".to_string() + &label.to_string().replace('_', "").to_lowercase()),
        );
        let hbox = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .valign(Align::Center)
            .spacing(VIEW_PADDING)
            .build();
        let label = if label.contains('_') {
            label.to_string()
        } else {
            format!("_{label}")
        };
        let label = Label::with_mnemonic(&label);
        label.set_mnemonic_widget(Some(ctrl));
        let gesture_click = GestureClick::new();
        gesture_click
            .connect_released(clone!(@strong ctrl => move |_, _, _, _| _ = ctrl.activate()));
        label.add_controller(gesture_click);
        hbox.append(&label);
        hbox.append(ctrl);
        child.set_child(Some(&hbox));
        child
    }

    fn resize(
        w: i32,
        h: i32,
        size: ScreenSizeRange,
        scale: &mut f64,
        translate: &mut (f64, f64),
        bounds: &mut Rect,
        outputs: &mut [Output],
    ) {
        // Translate to x = y = 0
        *bounds = get_bounds(outputs);
        for output in outputs.iter_mut() {
            if let (Some(pos), Some(mode)) = (output.pos.as_mut(), &output.mode) {
                let max_x = (size.max_width - mode.width).max(0) as i16;
                let max_y = (size.max_height - mode.height).max(0) as i16;
                pos.0 = pos.0.saturating_sub(bounds.x() as i16).min(max_x);
                pos.1 = pos.1.saturating_sub(bounds.y() as i16).min(max_y);
            }
        }
        *bounds = get_bounds(outputs);
        *scale = ((w - VIEW_PADDING * 2) as f32 / bounds.width())
            .min((h - VIEW_PADDING * 2) as f32 / bounds.height()) as f64;
        *translate = (VIEW_PADDING as f64 / *scale, VIEW_PADDING as f64 / *scale);
    }

    fn on_draw(&self, cr: &cairo::Context, w: i32, h: i32) {
        let bounds = self.bounds.borrow();
        let scale = self.scale.borrow();
        let translate = self.translate.borrow();

        Self::draw_area_background(cr, w, h);

        Self::draw_screen(
            cr,
            (bounds.x() as f64 + translate.0) * *scale,
            (bounds.y() as f64 + translate.1) * *scale,
            bounds.width() as f64 * *scale,
            bounds.height() as f64 * *scale,
        );

        for (i, o) in self.outputs.borrow().iter().enumerate() {
            if !o.enabled {
                continue;
            }
            let pos = o.pos.expect("enabled output has position");
            let mode = o.mode.as_ref().expect("enabled output has mode");
            let x = (pos.0 as f64 + translate.0) * *scale;
            let y = (pos.1 as f64 + translate.1) * *scale;
            let w = mode.width as f64 * *scale;
            let h = mode.height as f64 * *scale;

            Self::draw_output(cr, x, y, w, h);
            if let Some(j) = *self.selected_output.borrow() {
                if i == j {
                    Self::draw_selected_output(cr, x, y, w, h);
                }
            }
            let mut name = o.name.to_owned();
            let mut product_name = o.product_name.to_owned();
            if o.primary {
                name = format!("[{name}]");
                product_name = product_name.map(|s| format!("[{s}]"));
            }
            Self::draw_output_label(cr, x, y, w, h, &name, product_name.as_deref());
        }
    }

    fn on_draw_disabled(&self, cr: &cairo::Context, w: i32, h: i32) {
        let outputs = self.outputs.borrow();

        Self::draw_area_background(cr, w, h);

        let disabled_outputs = Self::get_disabled_outputs(&outputs);
        let i_select = self.selected_output.borrow();
        let is_dragging = *self.dragging_disabled_output.borrow();
        let dim = Self::get_disabled_output_dim(w, h, disabled_outputs.len());
        let mut j: usize = 0; // seperate index for closing the gaps
        for &o in &disabled_outputs {
            if i_select.is_none() || i_select.is_some_and(|i| !is_dragging || outputs[i].id != o.id)
            {
                let pos = Self::get_disabled_output_pos(j, dim[1]);
                Self::draw_output(cr, pos[0], pos[1], dim[0], dim[1]);
                Self::draw_output_label(
                    cr,
                    pos[0],
                    pos[1],
                    dim[0],
                    dim[1],
                    &o.name,
                    o.product_name.as_deref(),
                );
                if let Some(i) = *i_select {
                    if outputs[i].id == o.id {
                        Self::draw_selected_output(cr, pos[0], pos[1], dim[0], dim[1]);
                    }
                }
                j += 1;
            }
        }
    }

    fn get_disabled_outputs(outputs: &[Output]) -> Vec<&Output> {
        outputs.iter().filter(|&n| !n.enabled).collect::<Vec<_>>()
    }

    fn get_disabled_output_pos(index: usize, output_height: f64) -> [f64; 2] {
        let x = VIEW_PADDING as f64;
        let y = (index + 1) as f64 * VIEW_PADDING as f64 + index as f64 * output_height;
        [x, y]
    }

    fn get_disabled_output_dim(w: i32, h: i32, n_disabled: usize) -> [f64; 2] {
        let max_width = (w - 2 * VIEW_PADDING) as f64;
        let max_height = (h - (n_disabled + 1) as i32 * VIEW_PADDING) as f64 / n_disabled as f64;
        let width = max_width.min(max_height * 16. / 9.);
        let height = max_height.min(max_width * 9. / 16.);
        [width, height]
    }

    fn draw_area_background(cr: &cairo::Context, w: i32, h: i32) {
        cr.rectangle(0.0, 0.0, w as f64, h as f64);
        cr.set_source_color(&COLOR_BG0_H);
        cr.fill().unwrap();
    }

    fn draw_screen(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
        cr.rectangle(x, y, w, h);
        cr.set_source_color(&COLOR_FG);
        cr.set_line_width(2.);
        cr.set_dash(&[4.], 1.);
        cr.stroke().unwrap();
    }

    fn draw_output(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
        cr.rectangle(x, y, w, h);
        cr.set_source_rgba(
            COLOR_FG.red() as f64,
            COLOR_FG.green() as f64,
            COLOR_FG.blue() as f64,
            0.75,
        );
        cr.fill().unwrap();
    }

    fn draw_selected_output(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
        cr.rectangle(x, y, w, h);
        cr.set_source_color(&COLOR_GREEN);
        cr.set_line_width(4.);
        cr.set_dash(&[1., 0.], 0.);
        cr.stroke().unwrap();
    }

    fn draw_output_label(
        cr: &cairo::Context,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
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

        cr.set_source_color(&COLOR_BG0);
        cr.move_to(x + w / 2., y + h / 2.);

        layout.set_text(product_name.unwrap_or(name));
        let ps = layout.pixel_size();
        cr.rel_move_to(-ps.0 as f64 / 2., -ps.1 as f64 / 2.);
        show_layout(cr, &layout);

        cr.restore().unwrap();
    }

    fn on_drag_begin(
        &self,
        g: &GestureDrag,
        start_x: f64,
        start_y: f64,
        sw_enabled: &Switch,
        dd_resolution: &DropDown,
        cb_primary: &CheckButton,
        en_position: &Entry,
        details_ui: &impl IsA<Widget>,
        disabled_area: &DrawingArea,
    ) {
        let drawing_area = g.widget().downcast::<DrawingArea>().unwrap();
        if let Some(i) = self.get_output_index_at(start_x, start_y) {
            let scale = self.scale.borrow();
            let translate = self.translate.borrow();
            let mut outputs = self.outputs.borrow_mut();

            // Grab offset to output origin in global coordinates
            let pos = outputs[i].pos.expect("dragged output has position");
            *self.grab_offset.borrow_mut() = (
                pos.0 as f64 - (start_x / *scale - translate.0),
                pos.1 as f64 - (start_y / *scale - translate.1),
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
        disabled_area.queue_draw();
        // Do details UI updates out of scope because their triggered callbacks need to borrow
        self.update_details_ui(sw_enabled, dd_resolution, cb_primary, en_position);
        self.update_details_visibility(details_ui);
    }

    fn update_details_visibility(&self, details_ui: &impl IsA<Widget>) {
        let outputs = self.outputs.borrow();
        let selected_output = self.selected_output.borrow();
        let mut child = details_ui.first_child();
        while let Some(c) = child {
            let visible = selected_output
                .is_some_and(|i| outputs[i].enabled || c.widget_name() == "fbc_enabled");
            c.set_visible(visible);
            child = c.next_sibling();
        }
    }

    fn update_details_ui(
        &self,
        sw_enabled: &Switch,
        dd_resolution: &DropDown,
        cb_primary: &CheckButton,
        en_position: &Entry,
    ) {
        if let Some(i) = *self.selected_output.borrow() {
            *self.skip_update_output.borrow_mut() = true;

            // Update Actionables
            let enabled = self.outputs.borrow()[i].enabled;
            sw_enabled.set_active(enabled);
            let primary = self.outputs.borrow()[i].primary;
            cb_primary.set_active(primary);
            if let Some(pos) = self.outputs.borrow()[i].pos {
                en_position.set_text(&format!("+{}+{}", pos.0, pos.1));
            }

            // Update resolution drop down.
            // Note: `DropDown::set_model` and `DropDown::set_selected` trigger
            // `on_refresh_rate_selected`.
            // When changing the dropdown model the triggered call should not
            // change the output data. Otherwise switching through outputs
            // would mean resetting them back to default values.

            // When the index of the resolution dropdown should be 0 the refresh
            // rate dropdown has to be updated after a model change because there
            // won't be another triggered call. That is because setting the
            // selection to 0 after a model change doesn't have an effect (because
            // 0 is the default).
            let dd_res_index = self.outputs.borrow()[i].get_current_resolution_dropdown_index();
            *self.skip_update_refresh_model.borrow_mut() =
                dd_res_index.is_none() || dd_res_index.is_some_and(|i| i > 0);

            // Change the dropdown model
            let resolutions = self.outputs.borrow()[i].get_resolutions_dropdown();
            dd_resolution.set_model(Some(&Self::into_string_list(resolutions)));

            if dd_res_index.is_some() {
                // Always update the refresh rate dropdown
                *self.skip_update_refresh_model.borrow_mut() = false;
                dd_resolution.set_selected(dd_res_index.unwrap() as u32);
            }
            *self.skip_update_refresh_model.borrow_mut() = false;
            *self.skip_update_output.borrow_mut() = false;
        }
    }

    fn on_drag_update(&self, g: &GestureDrag, offset_x: f64, offset_y: f64, en_position: &Entry) {
        if let Some(i) = *self.selected_output.borrow() {
            let mut outputs = self.outputs.borrow_mut();
            let output = &outputs[i];
            let mut scale = self.scale.borrow_mut();
            let mut translate = self.translate.borrow_mut();
            let grab_offset = self.grab_offset.borrow();

            let mut min_side = f64::MAX;
            for output in outputs.iter().filter(|n| n.enabled) {
                let mode = output.mode.as_ref().expect("dragged output has mode");
                min_side = min_side.min(mode.height as f64);
                min_side = min_side.min(mode.width as f64);
            }
            // Snap to all snap values should be possible on all scaled sizes.
            // Give some leeway so it doesn't have to be pixel perfect.
            let snap_strength = (min_side / 4.) - (min_side / 12.);

            // Calculate snap
            let snap = Self::calculate_snap(&outputs, i);

            // Calculate new position
            let start = g.start_point().unwrap();
            // let mut new_x = ((start.0 + offset_x + *scale * grab_offset.0) / *scale - translate.0) as i16;
            // let mut new_y = ((start.1 + offset_y + *scale * grab_offset.1) / *scale - translate.1) as i16;
            let mut new_x = (((start.0 + offset_x) / *scale) - translate.0 + grab_offset.0) as i16;
            let mut new_y = (((start.1 + offset_y) / *scale) - translate.1 + grab_offset.1) as i16;

            // Apply snap
            let pos = output.pos.expect("dragged output has position");
            if snap[0] == 0 {
                if ((new_x - pos.0).abs() as f64) < snap_strength {
                    new_x = pos.0;
                }
            } else if (snap[0].abs() as f64) < snap_strength {
                new_x = (pos.0 as i32 + snap[0]) as i16;
            }
            if snap[1] == 0 {
                if ((new_y - pos.1).abs() as f64) < snap_strength {
                    new_y = pos.1;
                }
            } else if (snap[1].abs() as f64) < snap_strength {
                new_y = (pos.1 as i32 + snap[1]) as i16;
            }

            // Update new position
            if new_x != pos.0 || new_y != pos.1 {
                outputs[i].pos = Some((new_x, new_y));
                let drawing_area = g.widget().downcast::<DrawingArea>().unwrap();
                Self::resize(
                    drawing_area.width(),
                    drawing_area.height(),
                    self.size,
                    scale.deref_mut(),
                    translate.deref_mut(),
                    self.bounds.borrow_mut().deref_mut(),
                    outputs.deref_mut(),
                );
                let resized_pos = outputs[i].pos.unwrap();
                en_position.set_text(&format!("+{}+{}", resized_pos.0, resized_pos.1));
                drawing_area.queue_draw();
            }
        }
    }

    fn calculate_snap(outputs: &Vec<Output>, output_index: usize) -> [i32; 2] {
        let output = &outputs[output_index];

        let mut dist_v = i32::MAX;
        let mut snap_v = 0;
        let mut dist_h = i32::MAX;
        let mut snap_h = 0;

        let center = outputs[output_index].rect().center();
        for (j, other) in outputs.deref().iter().enumerate() {
            if !other.enabled {
                continue;
            }
            if output_index != j {
                let other_center = other.rect().center();

                // left to other left
                let dist = other.left() - output.left();
                if dist.abs() < dist_h {
                    dist_h = dist.abs();
                    snap_h = dist;
                }
                // left to other right
                let dist = other.right() - output.left();
                if dist.abs() < dist_h {
                    dist_h = dist.abs();
                    snap_h = dist;
                }
                // right to other left
                let dist = other.left() - output.right();
                if dist.abs() < dist_h {
                    dist_h = dist.abs();
                    snap_h = dist;
                }
                // right to other right
                let dist = other.right() - output.right();
                if dist.abs() < dist_h {
                    dist_h = dist.abs();
                    snap_h = dist;
                }
                // center to other center horizontal
                let dist = (other_center.x() - center.x()) as i32;
                if dist.abs() < dist_h {
                    dist_h = dist.abs();
                    snap_h = dist;
                }

                // top to other top
                let dist = other.top() - output.top();
                if dist.abs() < dist_v {
                    dist_v = dist.abs();
                    snap_v = dist;
                }
                // top to other bottom
                let dist = other.bottom() - output.top();
                if dist.abs() < dist_v {
                    dist_v = dist.abs();
                    snap_v = dist;
                }
                // bottom to other top
                let dist = other.top() - output.bottom();
                if dist.abs() < dist_v {
                    dist_v = dist.abs();
                    snap_v = dist;
                }
                // bottom to other bottom
                let dist = other.bottom() - output.bottom();
                if dist.abs() < dist_v {
                    dist_v = dist.abs();
                    snap_v = dist;
                }
                // center to other center vertical
                let dist = (other_center.y() - center.y()) as i32;
                if dist.abs() < dist_v {
                    dist_v = dist.abs();
                    snap_v = dist;
                }
            }
        }
        [snap_h, snap_v]
    }

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
            if !bc.eq(&c) {
                let d = [(bc.x() - c.x()) as f64, (bc.y() - c.y()) as f64];
                let d_len = (d[0].powi(2) + d[1].powi(2)).sqrt();
                e = [d[0] / d_len, d[1] / d_len];
            }
            data.insert(output.id, (r, e));
        }

        let step = 50.;
        let mut moved = Vec::new();
        let mut max_loops = (bounds.width().max(bounds.height()) as f64 / step) as u16;
        loop {
            for i in 0..outputs.len() {
                if !outputs[i].enabled {
                    continue;
                }
                // Current position
                let mut r = data[&outputs[i].id].0;
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
                r.offset(dx as f32, dy as f32);

                // Check if move has caused an overlap with other rects
                for other in outputs.iter() {
                    if other.id == outputs[i].id || !other.enabled {
                        continue;
                    }
                    if let Some(intersect) = r.intersection(&data[&other.id].0) {
                        let mut dx = -sx * intersect.width() as f64;
                        let mut dy = -sy * intersect.height() as f64;

                        if e[1].abs() > 0. {
                            // Calculate the x where a line in the flipped direction towards center
                            // intersects the bottom of the intersection rectangle.
                            let ix = sx * e[0] * intersect.height() as f64 / (sy * e[1]);
                            // If the intersection rectangle bottom is intersected
                            if ix.abs() <= intersect.width() as f64 {
                                dx = -sx * ix;
                            }
                        }
                        if e[0].abs() > 0. {
                            // Calculate the y where a line in the flipped direction towards center
                            // intersects the right of the intersection rectangle.
                            let iy = sy * e[1] * intersect.width() as f64 / (sx * e[0]);
                            // If the intersection rectangle right side is intersected
                            if iy.abs() <= intersect.height() as f64 {
                                dy = -sy * iy;
                            }
                        }
                        // Move back to remove the overlap
                        r.offset(dx as f32, dy as f32);
                    }
                }
                // Check if rect has moved
                let old_r = &data[&outputs[i].id].0;
                if r.x().round() != old_r.x().round() || r.y().round() != old_r.y().round() {
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
            pos.0 = data[&output.id].0.x().round() as i16;
            pos.1 = data[&output.id].0.y().round() as i16;
        }
    }

    fn on_disabled_click(
        &self,
        gc: &GestureClick,
        _n_press: i32,
        x: f64,
        y: f64,
        sw_enabled: &Switch,
        enabled_area: &DrawingArea,
        details_ui: &impl IsA<Widget>,
    ) {
        let disabled_area = gc.widget().downcast::<DrawingArea>().unwrap();
        if let Some(i) =
            self.get_disabled_output_index_at(x, y, disabled_area.width(), disabled_area.height())
        {
            *self.selected_output.borrow_mut() = Some(i);
            *self.skip_update_output.borrow_mut() = true;
            let enabled = self.outputs.borrow()[i].enabled;
            sw_enabled.set_active(enabled);
            *self.skip_update_output.borrow_mut() = false;
        } else {
            *self.selected_output.borrow_mut() = None;
        }
        enabled_area.queue_draw();
        disabled_area.queue_draw();
        self.update_details_visibility(details_ui);
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

    fn on_disabled_dragdrop_motion(&self, _dcm: &DropControllerMotion, _x: f64, _y: f64) {}

    fn on_enter(&self, _ecm: &EventControllerMotion, _x: f64, _y: f64) {}

    fn on_leave(&self, _ecm: &EventControllerMotion) {}

    fn on_dragdrop_prepare(&self, ds: &DragSource, x: f64, y: f64) -> Option<ContentProvider> {
        let outputs = self.outputs.borrow();
        let disabled_outputs = Self::get_disabled_outputs(&outputs);
        let disabled_area = ds.widget().downcast::<DrawingArea>().unwrap();
        if let Some(i) =
            self.get_disabled_output_index_at(x, y, disabled_area.width(), disabled_area.height())
        {
            let dim = Self::get_disabled_output_dim(
                disabled_area.width(),
                disabled_area.height(),
                disabled_outputs.len(),
            );
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
                ds.set_icon(Some(&icon), x as i32, (y - pos[1]) as i32);
            }
            return Some(ContentProvider::for_value(&Value::from(
                u64::try_from(i).ok()?,
            )));
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

    fn on_dragdrop_drop(
        &self,
        dt: &DropTarget,
        v: &Value,
        x: f64,
        y: f64,
        disabled_area: &DrawingArea,
        sw_enabled: &Switch,
        dd_resolution: &DropDown,
        cb_primary: &CheckButton,
        en_position: &Entry,
        details_ui: &impl IsA<Widget>,
    ) -> bool {
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
                (x / *scale - translate.0) as i16,
                (y / *scale - translate.1) as i16,
            ));

            Self::mind_the_gap_and_overlap(&mut outputs);
            Self::resize(
                drawing_area.width(),
                drawing_area.height(),
                self.size,
                scale.deref_mut(),
                translate.deref_mut(),
                self.bounds.borrow_mut().deref_mut(),
                outputs.deref_mut(),
            );
        }
        // Enable selection
        *self.selected_output.borrow_mut() = Some(i);
        self.update_details_ui(sw_enabled, dd_resolution, cb_primary, en_position);
        self.update_details_visibility(details_ui);
        // Update drawing areas
        disabled_area.queue_draw();
        drawing_area.queue_draw();
        true
    }

    fn on_dragdrop_motion(&self, _dt: &DropTarget, _x: f64, _y: f64) -> DragAction {
        DragAction::MOVE
    }

    fn create_drag_icon(
        w: f64,
        h: f64,
        name: &str,
        product_name: Option<&str>,
    ) -> Result<impl IsA<Paintable>, Box<dyn Error>> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32)?;
        let cr = cairo::Context::new(&surface)?;
        Self::draw_output(&cr, 0., 0., w, h);
        Self::draw_output_label(&cr, 0., 0., w, h, name, product_name);
        cr.fill()?;
        drop(cr);
        surface.flush();
        let stride = surface.stride().try_into()?;
        Ok(MemoryTexture::new(
            w as i32,
            h as i32,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &Bytes::from_owned(surface.take_data()?),
            stride,
        ))
    }

    fn on_resolution_selected(
        &self,
        dd_resolution: &DropDown,
        dd_refresh: &DropDown,
        en_position: &Entry,
        drawing_area: &DrawingArea,
    ) {
        let mut dd_refresh_model = None;
        let mut dd_refresh_index = None;

        if let Some(i) = *self.selected_output.borrow() {
            let mut outputs = self.outputs.borrow_mut();
            if !outputs[i].enabled {
                return;
            }

            let dd_selected = dd_resolution.selected() as usize;

            // Update current mode
            if !*self.skip_update_output.borrow() {
                let mode =
                    &outputs[i].modes[outputs[i].resolution_dropdown_mode_index(dd_selected)];
                if outputs[i].mode.as_ref().is_some_and(|m| m.id != mode.id)
                    || outputs[i].mode.is_none()
                {
                    outputs[i].mode = Some(mode.clone());
                    Self::mind_the_gap_and_overlap(outputs.deref_mut());
                    Self::resize(
                        drawing_area.width(),
                        drawing_area.height(),
                        self.size,
                        self.scale.borrow_mut().deref_mut(),
                        self.translate.borrow_mut().deref_mut(),
                        self.bounds.borrow_mut().deref_mut(),
                        outputs.deref_mut(),
                    );
                    let new_pos = outputs[i].pos.unwrap();
                    en_position.set_text(&format!("+{}+{}", new_pos.0, new_pos.1));
                    drawing_area.queue_draw();
                }
            }

            // Update refresh rate dropdown
            if !*self.skip_update_refresh_model.borrow() {
                dd_refresh_model = Some(Self::into_string_list(
                    outputs[i].get_refresh_rates_dropdown(dd_selected),
                ));
                dd_refresh_index = outputs[i].get_current_refresh_rate_dropdown_index(dd_selected);
            }
        }
        // Do outside scope so borrowing doesn't fail in on_refresh_rate_selected
        if dd_refresh_model.is_some() {
            *self.skip_update_output.borrow_mut() = true;
            dd_refresh.set_model(dd_refresh_model.as_ref());
            if let Some(idx) = dd_refresh_index {
                dd_refresh.set_selected(idx as u32);
            }
            *self.skip_update_output.borrow_mut() = false;
        }
    }

    fn on_refresh_rate_selected(&self, dd_refresh: &DropDown, dd_resolution: &DropDown) {
        if *self.skip_update_output.borrow() {
            return;
        }
        if let Some(i) = *self.selected_output.borrow() {
            let mut outputs = self.outputs.borrow_mut();
            if !outputs[i].enabled {
                return;
            }

            // Update current mode
            let mode = &outputs[i].modes[outputs[i].refresh_rate_dropdown_mode_index(
                dd_resolution.selected() as usize,
                dd_refresh.selected() as usize,
            )];
            if outputs[i].mode.as_ref().is_some_and(|m| m.id != mode.id)
                || outputs[i].mode.is_none()
            {
                outputs[i].mode = Some(mode.clone());
            }
        }
    }

    fn on_primary_checked(&self, cb_primary: &CheckButton, drawing_area: &DrawingArea) {
        if *self.skip_update_output.borrow() {
            return;
        }
        if let Some(i) = *self.selected_output.borrow() {
            let mut outputs = self.outputs.borrow_mut();
            if !outputs[i].enabled {
                return;
            }
            let active = cb_primary.is_active();
            if active != outputs[i].primary {
                outputs[i].primary = active;
                if active {
                    for (j, output) in outputs.iter_mut().enumerate() {
                        if i != j {
                            output.primary = false;
                        }
                    }
                }
                drawing_area.queue_draw();
            }
        }
    }

    fn on_enabled_switched(
        &self,
        sw_enabled: &Switch,
        enabled_area: &DrawingArea,
        disabled_area: &DrawingArea,
        details_ui: &impl IsA<Widget>,
    ) {
        if *self.skip_update_output.borrow() {
            return;
        }
        let Some(i) = *self.selected_output.borrow() else {
            return;
        };

        {
            let mut outputs = self.outputs.borrow_mut();
            let active = sw_enabled.is_active();
            if outputs[i].enabled != active {
                // Update output
                outputs[i].enabled = active;
                if !active {
                    // Remove output
                    outputs[i].primary = false;
                    outputs[i].pos = None;
                    outputs[i].mode = None;
                    // Disable selection
                    *self.selected_output.borrow_mut() = None;
                } else {
                    // Insert output
                    outputs[i].enabled = true;
                    outputs[i].mode = Some(outputs[i].modes[0].clone());
                    outputs[i].pos = Some((0, 0));
                    // Enable selection
                    *self.selected_output.borrow_mut() = Some(i);
                }
            }
        }

        Self::mind_the_gap_and_overlap(&mut self.outputs.borrow_mut());
        self.update_details_visibility(details_ui);

        // Update drawing areas
        Self::resize(
            enabled_area.width(),
            enabled_area.height(),
            self.size,
            &mut self.scale.borrow_mut(),
            &mut self.translate.borrow_mut(),
            &mut self.bounds.borrow_mut(),
            &mut self.outputs.borrow_mut(),
        );
        enabled_area.queue_draw();
        disabled_area.queue_draw();
    }

    fn on_reset_clicked(
        &self,
        enabled_area: &DrawingArea,
        disabled_area: &DrawingArea,
        details_ui: &impl IsA<Widget>,
    ) {
        self.outputs
            .borrow_mut()
            .clone_from(&self.outputs_orig.borrow());
        // Disable selection
        *self.selected_output.borrow_mut() = None;
        self.update_details_visibility(details_ui);
        // Update drawing areas
        Self::resize(
            enabled_area.width(),
            enabled_area.height(),
            self.size,
            &mut self.scale.borrow_mut(),
            &mut self.translate.borrow_mut(),
            &mut self.bounds.borrow_mut(),
            &mut self.outputs.borrow_mut(),
        );
        enabled_area.queue_draw();
        disabled_area.queue_draw();
    }

    fn get_output_index_at(&self, x: f64, y: f64) -> Option<usize> {
        let scale = self.scale.borrow();
        let translate = self.translate.borrow();

        let p = Point::new(x as f32, y as f32);
        for (i, output) in self.outputs.borrow().iter().enumerate() {
            if output.enabled {
                let pos = output.pos.expect("enabled output has position");
                let mode = output.mode.as_ref().expect("enabled output has mode");
                let scaled_rect = Rect::new(
                    ((pos.0 as f64 + translate.0) * *scale) as f32,
                    ((pos.1 as f64 + translate.1) * *scale) as f32,
                    mode.width as f32 * *scale as f32,
                    mode.height as f32 * *scale as f32,
                );
                if scaled_rect.contains_point(&p) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn get_disabled_output_index_at(&self, x: f64, y: f64, w: i32, h: i32) -> Option<usize> {
        let outputs = self.outputs.borrow();
        let disabled_outputs = Self::get_disabled_outputs(&outputs);
        let dim = Self::get_disabled_output_dim(w, h, disabled_outputs.len());
        let p = Point::new(x as f32, y as f32);
        for (i, &disabled_output) in disabled_outputs.iter().enumerate() {
            let pos = Self::get_disabled_output_pos(i, dim[1]);
            let rect = Rect::new(pos[0] as f32, pos[1] as f32, dim[0] as f32, dim[1] as f32);
            if rect.contains_point(&p) {
                for (j, output) in outputs.iter().enumerate() {
                    if output.id == disabled_output.id {
                        return Some(j);
                    }
                }
            }
        }
        None
    }

    fn into_string_list(list: Vec<String>) -> StringList {
        let list = list.iter().map(String::as_str).collect::<Vec<&str>>();
        StringList::new(list.as_slice())
    }

    // fn to_local(&self, value: (f64, f64)) -> (f64, f64) {
    //     let translate = *self.translate.borrow();
    //     let scale = *self.scale.borrow();
    //     (
    //         (value.0 + translate.0) * scale,
    //         (value.1 + translate.1) * scale,
    //     )
    // }

    // fn to_global(&self, value: (f64, f64)) -> (f64, f64) {
    //     let translate = *self.translate.borrow();
    //     let scale = *self.scale.borrow();
    //     (value.0 / scale - translate.0, value.1 / scale - translate.1)
    // }
}
