use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gdk::glib::{self, clone, Propagation};
use gdk::{Key, ModifierType, Texture};
use gtk::prelude::*;
use gtk::{
    AboutDialog, Align, Button, EventControllerKey, FlowBox, License, Orientation, Paned,
    SelectionMode, Separator,
};

use crate::data::outputs::Outputs;
use crate::widget::details_box::{DetailsBox, Update};
use crate::widget::disabled_output_area::DisabledOutputArea;
use crate::widget::output_area::OutputArea;
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
    outputs_orig: Rc<RefCell<Vec<Output>>>,
    paned: Paned,
    last_handle_pos: Rc<Cell<i32>>,
    enabled_area: OutputArea,
    details: DetailsBox,
    disabled_area: DisabledOutputArea,
    apply_callback: Rc<RefCell<Option<Rc<ApplyCallback>>>>,
}

impl View {
    pub fn new(
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
            outputs.iter().filter(|&n| n.enabled()).map(Output::new_from).collect::<Vec<_>>();
        let disabled_outputs =
            outputs.iter().filter(|&n| !n.enabled()).map(Output::new_from).collect::<Vec<_>>();

        let enabled_area = OutputArea::new(&enabled_outputs, size.max_width, size.max_height);
        let disabled_area = DisabledOutputArea::new(&disabled_outputs);

        let paned = Paned::builder()
            .start_child(&enabled_area)
            .end_child(&disabled_area)
            .resize_start_child(true)
            .resize_end_child(false)
            .vexpand(true)
            .build();
        root.append(&paned);

        let this = Self {
            root,
            outputs_orig: Rc::new(RefCell::new(outputs)),
            paned,
            last_handle_pos: Rc::new(Cell::new(0)),
            enabled_area,
            details: DetailsBox::new(size.max_width, size.max_height),
            disabled_area,
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

        this.details.connect_output_changed(clone!(
            @weak this.enabled_area as enabled, @weak this.disabled_area as disabled => move |_, output, update| {
                enabled.update(output, update);
                disabled.update(output, update);
            }
        ));

        this.enabled_area.connect_output_selected(clone!(
            @strong this => move |_, output| this.on_enabled_selected(output)
        ));
        this.enabled_area.connect_output_deselected(clone!(
            @strong this => move |_| this.on_enabled_deselected()
        ));

        this.disabled_area.connect_output_selected(clone!(
            @strong this => move |_, output| this.on_disabled_selected(output)
        ));
        this.disabled_area.connect_output_deselected(clone!(
            @strong this => move |_| this.on_disabled_deselected()
        ));

        this
    }

    fn get_outputs(&self) -> Vec<Output> {
        let mut outputs = self.enabled_area.outputs().to_vec();
        outputs.extend(self.disabled_area.outputs().to_vec());
        outputs
    }

    fn on_key_pressed(
        &self,
        _eck: &EventControllerKey,
        keyval: Key,
        _keycode: u32,
        _state: ModifierType,
    ) -> Propagation {
        match keyval {
            Key::Delete => {
                if let Some(output) = self.enabled_area.selected_output() {
                    output.disable();
                    self.enabled_area.update(&output, Update::Disabled);
                    self.disabled_area.update(&output, Update::Disabled);
                    return Propagation::Stop;
                }
                if let Some(output) = self.disabled_area.selected_output() {
                    output.enable();
                    self.enabled_area.update(&output, Update::Enabled);
                    self.disabled_area.update(&output, Update::Enabled);
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

    fn on_enabled_selected(&self, output: &Output) {
        self.disabled_area.deselect();
        self.disabled_area.queue_draw();
        self.details.set_output(Some(output));
    }

    fn on_enabled_deselected(&self) {
        self.disabled_area.deselect();
        self.disabled_area.queue_draw();
        self.details.set_output(None::<Output>);
    }

    fn on_disabled_selected(&self, output: &Output) {
        self.enabled_area.deselect();
        self.enabled_area.queue_draw();
        self.details.set_output(Some(output));
    }

    fn on_disabled_deselected(&self) {
        self.enabled_area.deselect();
        self.enabled_area.queue_draw();
        self.details.set_output(None::<Output>);
    }

    pub fn apply(&self) {
        *self.outputs_orig.borrow_mut() = self.get_outputs().iter().map(Output::new_from).collect()
    }

    pub fn reset(&self) {
        let enabled_outputs = self
            .outputs_orig
            .borrow()
            .iter()
            .filter(|&n| n.enabled())
            .map(Output::new_from)
            .collect();
        let disabled_outputs = self
            .outputs_orig
            .borrow()
            .iter()
            .filter(|&n| !n.enabled())
            .map(Output::new_from)
            .collect();
        self.enabled_area.set_outputs(Outputs::new(&enabled_outputs));
        self.disabled_area.set_outputs(Outputs::new(&disabled_outputs));
        self.details.set_output(None::<Output>);
    }

    pub fn set_apply_callback(&self, callback: impl Fn(Vec<Output>) + 'static) {
        *self.apply_callback.borrow_mut() = Some(Rc::new(callback));
    }
}
