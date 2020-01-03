use std::cell::RefCell;
use std::rc::Rc;

use gdk::{DragAction, DragContext, EventButton, EventType, ModifierType, TARGET_STRING};
use gtk::prelude::*;
use gtk::{
    Align, Button, ButtonBuilder, Container, CssProvider, DestDefaults, Grid, PositionType,
    SelectionData, StateFlags, StyleContext, TargetEntry, TargetFlags, Widget, NONE_BUTTON,
    STYLE_PROVIDER_PRIORITY_APPLICATION,
};

trait ToGtk {
    fn to_gtk(&self) -> PositionType;
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DragPosition {
    Swap,
    Top,
    Bottom,
    Left,
    Right,
}

impl ToGtk for DragPosition {
    fn to_gtk(&self) -> PositionType {
        match *self {
            DragPosition::Left => PositionType::Left,
            DragPosition::Right => PositionType::Right,
            DragPosition::Top => PositionType::Top,
            DragPosition::Bottom => PositionType::Bottom,
            _ => PositionType::__Unknown(std::i32::MAX),
        }
    }
}

#[derive(Clone)]
pub struct OutputView {
    grid_outputs: Grid,
    grid_outputs_disabled: Grid,
    output_selected_callback: Option<Rc<dyn Fn(&str)>>,
    output_moved_callback: Option<Rc<dyn Fn(&str, DragPosition, &str)>>,
}

impl OutputView {
    pub fn new(grid_outputs: Grid, grid_outputs_disabled: Grid) -> OutputView {
        OutputView {
            grid_outputs,
            grid_outputs_disabled,
            output_selected_callback: None,
            output_moved_callback: None,
        }
    }

    pub fn add_output(
        &self,
        name: &str,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
        enabled: bool,
    ) {
        let btn = Self::create_output_button(name);

        if let Some(callback) = &self.output_selected_callback {
            let callback = Rc::clone(callback);
            btn.connect_clicked(move |b| {
                if let Some(name) = b.get_widget_name() {
                    callback(name.as_str());
                }
            });
        }

        btn.connect_button_press_event({
            let output_view = self.clone();
            move |widget, event| Self::on_button_pressed(&output_view, widget, event)
        });

        if width > 0 && height > 0 {
            btn.set_size_request(width, height);
        }

        Self::add_drag_drop_style(&btn);
        Self::connect_drag_source(&btn);
        if enabled {
            self.connect_drop_target(&btn);
            self.grid_outputs.attach(&btn, left, top, 1, 1);
        } else {
            self.grid_outputs_disabled.attach_next_to(
                &btn,
                NONE_BUTTON,
                PositionType::Bottom,
                1,
                1,
            );
        }
    }

    pub fn resize_output(&self, name: &str, width: i32, height: i32) {
        if let Some(output_widget) = self.find_enabled_output_widget(name) {
            output_widget.set_size_request(width, height);
        }
    }

    pub fn set_output_selected_callback<F: 'static + Fn(&str)>(&mut self, f: F) {
        self.output_selected_callback = Some(Rc::new(f));
    }

    pub fn set_output_moved_callback<F: 'static + Fn(&str, DragPosition, &str)>(&mut self, f: F) {
        self.output_moved_callback = Some(Rc::new(f));
    }

    fn find_enabled_output_widget(&self, output_name: &str) -> Option<Widget> {
        find_widget_by_name(&self.grid_outputs, output_name)
    }

    fn find_disabled_output_widget(&self, output_name: &str) -> Option<Widget> {
        find_widget_by_name(&self.grid_outputs_disabled, output_name)
    }

    fn update_layout_from_drop_params<W: IsA<Widget>>(
        &self,
        src_name: &str,
        target: &W,
        position: DragPosition,
    ) {
        if let Some(c) = self.find_enabled_output_widget(src_name) {
            if DragPosition::Swap == position {
                self.swap_output(&c, target);
            } else {
                if self.is_next_to(&c, target, position) {
                    return;
                }
                self.grid_outputs.remove(&c);
                self.insert_output_next_to(&c, target, position);
            }
        } else if let Some(c) = self.find_disabled_output_widget(src_name) {
            if DragPosition::Swap != position {
                self.grid_outputs_disabled.remove(&c);
                self.insert_output_next_to(&c, target, position);
                self.connect_drop_target(&c);
            }
        }
    }

    fn is_next_to<W: IsA<Widget>, S: IsA<Widget>>(
        &self,
        widget: &W,
        sibling: &S,
        position: DragPosition,
    ) -> bool {
        let target_pos = self.get_pos_next_to(sibling, position);
        target_pos.0 == self.grid_outputs.get_cell_left_attach(widget)
            && target_pos.1 == self.grid_outputs.get_cell_top_attach(widget)
    }

    fn insert_output_next_to<W: IsA<Widget>, S: IsA<Widget>>(
        &self,
        widget: &W,
        sibling: &S,
        position: DragPosition,
    ) {
        let target_pos = self.get_pos_next_to(sibling, position);
        if self
            .grid_outputs
            .get_child_at(target_pos.0, target_pos.1)
            .is_some()
        {
            self.grid_outputs.insert_next_to(sibling, position.to_gtk());
        }
        self.grid_outputs
            .attach_next_to(widget, Some(sibling), position.to_gtk(), 1, 1);

        match position {
            DragPosition::Left => {
                sibling.set_halign(Align::Start);
                widget.set_halign(Align::End);
                widget.set_valign(sibling.get_valign());
            }
            DragPosition::Right => {
                sibling.set_halign(Align::End);
                widget.set_halign(Align::Start);
                widget.set_valign(sibling.get_valign());
            }
            DragPosition::Top => {
                sibling.set_valign(Align::Start);
                widget.set_valign(Align::End);
                widget.set_halign(sibling.get_halign());
            }
            DragPosition::Bottom => {
                sibling.set_valign(Align::End);
                widget.set_valign(Align::Start);
                widget.set_halign(sibling.get_halign());
            }
            _ => (),
        }
    }

    fn swap_output<S: IsA<Widget>, T: IsA<Widget>>(&self, w1: &S, w2: &T) {
        let src_left = self.grid_outputs.get_cell_left_attach(w1);
        let src_top = self.grid_outputs.get_cell_top_attach(w1);
        let target_left = self.grid_outputs.get_cell_left_attach(w2);
        let target_top = self.grid_outputs.get_cell_top_attach(w2);
        self.grid_outputs.remove(w1);
        self.grid_outputs.remove(w2);
        self.grid_outputs.attach(w1, target_left, target_top, 1, 1);
        self.grid_outputs.attach(w2, src_left, src_top, 1, 1);
        let h_align = w1.get_halign();
        let v_align = w1.get_valign();
        w1.set_halign(w2.get_halign());
        w1.set_valign(w2.get_valign());
        w2.set_halign(h_align);
        w2.set_valign(v_align);
    }

    // Assuming row or column spans for every child in given grid are always 1
    fn get_pos_next_to<S: IsA<Widget>>(&self, sibling: &S, position: DragPosition) -> (i32, i32) {
        let left_attach = self.grid_outputs.get_cell_left_attach(sibling);
        let top_attach = self.grid_outputs.get_cell_top_attach(sibling);

        match position {
            DragPosition::Left => (left_attach - 1, top_attach),
            DragPosition::Right => (left_attach + 1, top_attach),
            DragPosition::Top => (left_attach, top_attach - 1),
            DragPosition::Bottom => (left_attach, top_attach + 1),
            DragPosition::Swap => (left_attach, top_attach),
        }
    }

    fn get_row_height(&self, row: i32) -> i32 {
        let mut row_height = 0;
        for w in self.grid_outputs.get_children() {
            let top = self.grid_outputs.get_cell_top_attach(&w);
            if top == row {
                let height = w.get_allocation().height;
                if height > row_height {
                    row_height = height;
                }
            }
        }
        row_height
    }

    fn get_column_width(&self, column: i32) -> i32 {
        let mut column_width = 0;
        for w in self.grid_outputs.get_children() {
            let left = self.grid_outputs.get_cell_left_attach(&w);
            if left == column {
                let width = w.get_allocation().width;
                if width > column_width {
                    column_width = width;
                }
            }
        }
        column_width
    }

    fn on_button_pressed(&self, btn: &Button, event: &EventButton) -> Inhibit {
        if event.get_event_type() == EventType::ButtonPress && event.get_button() == 3 {
            let alloc = btn.get_allocation();

            let next_alignment = |align: Align| -> Align {
                let alignments = [Align::Start, Align::Center, Align::End];
                for (i, a) in alignments.iter().enumerate() {
                    if *a == align {
                        return alignments[(i + 1) % alignments.len()];
                    }
                }
                Align::Center
            };

            let left = self.grid_outputs.get_cell_left_attach(btn);
            if alloc.width < self.get_column_width(left) {
                btn.set_halign(next_alignment(btn.get_halign()));
            } else {
                for c in self.grid_outputs.get_children() {
                    if get_widget_name(&c) != get_widget_name(btn)
                        && self.grid_outputs.get_cell_left_attach(&c) == left
                    {
                        c.set_halign(next_alignment(c.get_halign()));
                    }
                }
            }

            let top = self.grid_outputs.get_cell_top_attach(btn);
            if alloc.height < self.get_row_height(top) {
                btn.set_valign(next_alignment(btn.get_valign()));
            } else {
                for c in self.grid_outputs.get_children() {
                    if get_widget_name(&c) != get_widget_name(btn)
                        && self.grid_outputs.get_cell_top_attach(&c) == top
                    {
                        c.set_valign(next_alignment(c.get_valign()));
                    }
                }
            }
        }

        Inhibit(false)
    }

    fn create_output_button(name: &str) -> Button {
        ButtonBuilder::new()
            .name(name)
            .label(&format!("{}", name))
            .valign(Align::Center)
            .halign(Align::Center)
            .build()
    }

    fn add_drag_drop_style(btn: &Button) -> StyleContext {
        let style_context = btn.get_style_context();
        let color = style_context.get_border_color(StateFlags::DROP_ACTIVE);
        let provider = CssProvider::new();

        let css: String = format!("
                .text-button.drop-left:drop(active) {{ border-left: 5px solid {0}; border-right-width: 0px; border-top-width: 0px; border-bottom-width: 0px; }}
                .text-button.drop-right:drop(active) {{ border-right: 5px solid {0}; border-left-width: 0px; border-top-width: 0px; border-bottom-width: 0px; }}
                .text-button.drop-top:drop(active) {{ border-top: 5px solid {0}; border-left-width: 0px; border-right-width: 0px; border-bottom-width: 0px; }}
                .text-button.drop-bottom:drop(active) {{ border-bottom: 5px solid {0}; border-left-width: 0px; border-right-width: 0px; border-top-width: 0px; }}
                .text-button.drop-swap:drop(active) {{ border: 5px solid {0}; }}", color);

        match provider.load_from_data(css.as_str().as_ref()) {
            Ok(_) => style_context.add_provider(&provider, STYLE_PROVIDER_PRIORITY_APPLICATION),
            Err(e) => println!("Error while loading CSS data: {}", e),
        }

        style_context
    }

    fn connect_drag_source(btn: &Button) {
        let targets = &[TargetEntry::new(
            TARGET_STRING.name().as_str(),
            TargetFlags::OTHER_WIDGET,
            0,
        )];

        btn.drag_source_set(ModifierType::BUTTON1_MASK, targets, DragAction::MOVE);
        btn.connect_drag_data_get(|widget, _context, data, _info, _time| {
            if let Some(name) = widget.get_widget_name() {
                data.set_text(name.as_str());
            }
        });

        btn.connect_drag_failed(|widget, _context, _result| -> Inhibit {
            let mut widget_name = String::from("unknown");
            if let Some(name) = widget.get_widget_name() {
                widget_name = name.to_string();
            }
            println!("Drag failed for widget `{}`", widget_name);
            Inhibit(true)
        });

        btn.connect_drag_begin(|widget, _context| {
            let mut widget_name = String::from("unknown");
            if let Some(name) = widget.get_widget_name() {
                widget_name = name.to_string();
            }
            println!("Drag began for widget `{}`", widget_name);
        });
    }

    fn connect_drop_target<W: IsA<Widget>>(&self, widget: &W) {
        let curr_drag_pos = Rc::new(RefCell::new(DragPosition::Swap));
        let targets = &[TargetEntry::new(
            TARGET_STRING.name().as_str(),
            TargetFlags::OTHER_WIDGET,
            0,
        )];

        widget.drag_dest_set(DestDefaults::all(), targets, DragAction::MOVE);
        widget.connect_drag_data_received({
            let curr_drag_pos = Rc::clone(&curr_drag_pos);
            let output_view = self.clone();
            move |widget, context, _x, _y, data, _info, time| {
                Self::on_drag_data_received(
                    &output_view,
                    widget,
                    context,
                    data,
                    time,
                    &curr_drag_pos,
                )
            }
        });

        widget.connect_drag_motion({
            let curr_drag_pos = Rc::clone(&curr_drag_pos);
            move |widget, _context, x, y, _time| -> Inhibit {
                Self::on_drag_motion(widget, x, y, &curr_drag_pos)
            }
        });
    }

    fn on_drag_data_received<W: IsA<Widget>>(
        &self,
        widget: &W,
        context: &DragContext,
        data: &SelectionData,
        time: u32,
        curr_drag_pos: &Rc<RefCell<DragPosition>>,
    ) {
        let target_name = widget
            .get_widget_name()
            .expect("Failed to get target widget name");
        let src_name = data.get_text().expect("Failed to get source widget name");
        let target_pos_type = curr_drag_pos
            .try_borrow()
            .expect("Failed to get drag position");

        println!(
            "Widget {} received position `{:?}` from {}",
            target_name, target_pos_type, src_name
        );

        self.update_layout_from_drop_params(src_name.as_str(), widget, *target_pos_type);
        if let Some(callback) = &self.output_moved_callback {
            callback(src_name.as_str(), *target_pos_type, target_name.as_str());
        }
        context.drag_finish(true, true, time);
    }

    fn on_drag_motion<W: IsA<Widget>>(
        widget: &W,
        x: i32,
        y: i32,
        curr_drag_pos: &Rc<RefCell<DragPosition>>,
    ) -> Inhibit {
        let w = widget.get_allocation().width;
        let h = widget.get_allocation().height;
        let t_x = w as f64 * 0.25;
        let t_y = h as f64 * 0.25;

        let mut position = DragPosition::Swap;

        let xf = x as f64;
        let yf = y as f64;

        let t_diag = |x: f64| (t_y / t_x) * x;
        let t_top_left = t_diag(xf);
        let t_bottom_left = -t_diag(xf) + h as f64;
        let t_top_right = -t_diag(xf - w as f64);
        let t_bottom_right = t_diag(xf - w as f64) + h as f64;

        if xf <= t_x && x >= 0 && yf >= t_top_left && yf <= t_bottom_left {
            position = DragPosition::Left;
        } else if x <= w && xf >= w as f64 - t_x && yf >= t_top_right && yf <= t_bottom_right {
            position = DragPosition::Right;
        } else if yf <= t_y && y >= 0 {
            position = DragPosition::Top;
        } else if y <= h && yf >= h as f64 - t_y {
            position = DragPosition::Bottom;
        }

        let style_context = widget.get_style_context();
        style_context.remove_class("drop-left");
        style_context.remove_class("drop-right");
        style_context.remove_class("drop-top");
        style_context.remove_class("drop-bottom");
        style_context.remove_class("drop-swap");

        let css_class = match position {
            DragPosition::Left => "drop-left",
            DragPosition::Right => "drop-right",
            DragPosition::Top => "drop-top",
            DragPosition::Bottom => "drop-bottom",
            DragPosition::Swap => "drop-swap",
        };

        style_context.add_class(css_class);

        if let Ok(mut curr_drag_pos) = curr_drag_pos.try_borrow_mut() {
            *curr_drag_pos = position;
        }

        Inhibit(true)
    }
}

fn find_widget_by_name<C: IsA<Container>>(container: &C, widget_name: &str) -> Option<Widget> {
    let mut widget = None;
    for c in container.get_children() {
        if let Some(name) = c.get_widget_name() {
            if name.as_str() == widget_name {
                widget = Some(c);
                break;
            }
        }
    }
    widget
}

fn get_widget_name<W: IsA<Widget>>(w: &W) -> String {
    String::from(w.get_widget_name().expect("Widget doesn't have a name"))
}
