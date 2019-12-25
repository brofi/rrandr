use std::cell::RefCell;
use std::rc::Rc;

use gdk::{DragAction, DragContext, ModifierType, TARGET_STRING};
use gtk::prelude::*;
use gtk::{
    Align, Button, Container, CssProvider, DestDefaults, Grid, PositionType, SelectionData,
    StateFlags, StyleContext, TargetEntry, TargetFlags, Widget, NONE_BUTTON,
    STYLE_PROVIDER_PRIORITY_APPLICATION,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DragPosition {
    Swap,
    Top,
    Bottom,
    Left,
    Right,
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

    pub fn add_output(&self, name: &str, width: i32, height: i32, enabled: bool) {
        let btn = Self::create_output_button(name, width, height);

        if let Some(callback) = &self.output_selected_callback {
            let callback = Rc::clone(callback);
            btn.connect_clicked(move |b| {
                if let Some(name) = b.get_widget_name() {
                    callback(name.as_str());
                }
            });
        }

        Self::connect_drag_source(&btn);
        if enabled {
            self.connect_drop_target(&btn);
            self.grid_outputs
                .attach_next_to(&btn, NONE_BUTTON, PositionType::Right, 1, 1);
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
        let gtk_pos_type = get_gtk_pos_type(position);
        if self
            .grid_outputs
            .get_child_at(target_pos.0, target_pos.1)
            .is_some()
        {
            self.grid_outputs.insert_next_to(sibling, gtk_pos_type);
        }
        self.grid_outputs
            .attach_next_to(widget, Some(sibling), gtk_pos_type, 1, 1);
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

    fn create_output_button(name: &str, width: i32, height: i32) -> Button {
        let btn: Button = Button::new_with_label(&format!("Output: {}", name));
        btn.set_widget_name(name);
        Self::add_drag_drop_style(&btn);
        if width > 0 && height > 0 {
            btn.set_size_request(width, height);
        }
        btn.set_valign(Align::Center);
        btn.set_halign(Align::Center);
        btn
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

fn get_gtk_pos_type(position: DragPosition) -> PositionType {
    match position {
        DragPosition::Left => PositionType::Left,
        DragPosition::Right => PositionType::Right,
        DragPosition::Top => PositionType::Top,
        DragPosition::Bottom => PositionType::Bottom,
        DragPosition::Swap => panic!("Cannot translate {:?} to a GTK PositionType", position),
    }
}
