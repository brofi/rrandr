use std::cell::{Cell, RefCell};
use std::error::Error;
use std::sync::OnceLock;

use gdk::glib::object::IsA;
use gdk::glib::subclass::object::{ObjectImpl, ObjectImplExt};
use gdk::glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
use gdk::glib::subclass::Signal;
use gdk::glib::value::ToValue;
use gdk::glib::{clone, derived_properties, object_subclass, Bytes, Properties};
use gdk::prelude::{
    ContentProviderExtManual, DragExt, ListModelExt, ListModelExtManual, ObjectExt, StaticType,
};
use gdk::subclass::prelude::DerivedObjectProperties;
use gdk::{ContentProvider, Drag, DragAction, MemoryTexture, Paintable};
use gtk::prelude::{DrawingAreaExt, DrawingAreaExtManual, WidgetExt};
use gtk::subclass::drawing_area::DrawingAreaImpl;
use gtk::subclass::widget::WidgetImpl;
use gtk::{
    glib, DragSource, DrawingArea, DropControllerMotion, EventControllerMotion, GestureClick,
};

use crate::config::Config;
use crate::data::output::Output;
use crate::data::outputs::Outputs;
use crate::draw::DrawContext;
use crate::view::PADDING;
use crate::widget::details_box::Update;

#[derive(Default, Properties)]
#[properties(wrapper_type = super::DisabledOutputArea)]
pub struct DisabledOutputArea {
    #[property(get, set = Self::set_outputs)]
    outputs: RefCell<Outputs>,
    config: Config,
    pub(super) selected_output: Cell<Option<usize>>,
    is_dragging: Cell<bool>,
}

#[object_subclass]
impl ObjectSubclass for DisabledOutputArea {
    type ParentType = DrawingArea;
    type Type = super::DisabledOutputArea;

    const NAME: &'static str = "DisabledOutputArea";
}

#[derived_properties]
impl ObjectImpl for DisabledOutputArea {
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
        obj.set_content_width(150);

        obj.set_draw_func(clone!(
            @weak self as this  => move |_d, cr, width, height| this.on_draw(cr, width, height)
        ));

        let drag_source = DragSource::builder().actions(DragAction::MOVE).build();
        drag_source.connect_prepare(clone!(
            @weak self as this => @default-panic, move |ds, x, y| this.on_drag_prepare(ds, x, y)
        ));
        drag_source.connect_drag_begin(clone!(
            @weak self as this => move |ds, d| this.on_drag_begin(ds, d)
        ));
        drag_source.connect_drag_end(clone!(
            @weak self as this => move |ds, d, del| this.on_drag_end(ds, d, del)
        ));
        obj.add_controller(drag_source);

        let gesture_click = GestureClick::new();
        gesture_click.connect_pressed(clone!(
            @weak self as this => move |gc, n_press, x, y| this.on_click(gc, n_press, x, y)
        ));
        obj.add_controller(gesture_click);

        let event_controller_motion = EventControllerMotion::new();
        event_controller_motion.connect_motion(clone!(
            @weak self as this => move |ecm, x, y| this.on_motion(ecm, x, y)
        ));
        obj.add_controller(event_controller_motion);

        let drop_controller_motion = DropControllerMotion::new();
        drop_controller_motion.connect_motion(Self::on_drop_motion);
        obj.add_controller(drop_controller_motion);
    }
}

impl WidgetImpl for DisabledOutputArea {}

impl DrawingAreaImpl for DisabledOutputArea {}

impl DisabledOutputArea {
    fn set_outputs(&self, outputs: &Outputs) {
        self.outputs.replace(outputs.clone());
        self.deselect();
        self.obj().queue_draw();
    }

    pub(super) fn select(&self, index: usize) { self.selected_output.set(Some(index)); }

    pub(super) fn deselect(&self) { self.selected_output.set(None); }

    fn on_draw(&self, cr: &cairo::Context, width: i32, height: i32) {
        let outputs = self.outputs.borrow();
        let i_select = self.selected_output.get();
        let context = DrawContext::new(cr.clone(), self.config.clone());
        let [width, height] = Self::get_output_dim(width, height, outputs.n_items() as usize);
        let mut j: usize = 0; // separate index for closing the gaps
        for o in outputs.iter::<Output>().map(Result::unwrap) {
            if i_select.is_none()
                || i_select
                    .is_some_and(|i| !self.is_dragging.get() || outputs.index(i).id() != o.id())
            {
                let [x, y] = Self::get_output_pos(j, height);
                let rect = [f64::from(x), f64::from(y), f64::from(width), f64::from(height)];
                context.draw_output(rect);
                context.draw_output_label(rect, &o.name(), o.product_name().as_deref());
                if let Some(i) = i_select {
                    if outputs.index(i).id() == o.id() {
                        context.draw_selected_output(rect);
                    }
                }
                j += 1;
            }
        }
    }

    fn get_output_pos(index: usize, output_height: u16) -> [i16; 2] {
        let index = u32::try_from(index).expect("less disabled outputs");
        let x = i16::try_from(PADDING).unwrap_or(i16::MAX);
        let y = i16::try_from((index + 1) * u32::from(PADDING) + index * u32::from(output_height))
            .unwrap_or(i16::MAX);
        [x, y]
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    fn get_output_dim(w: i32, h: i32, n_disabled: usize) -> [u16; 2] {
        if n_disabled == 0 {
            return [0, 0];
        }
        let w = u32::try_from(w).expect("drawing area width is positive");
        let h = u32::try_from(h).expect("drawing area height is positive");
        let n_disabled = u32::try_from(n_disabled).expect("less disabled outputs");
        let max_width = (w.saturating_sub(2 * u32::from(PADDING))) as u16;
        let max_height =
            ((h.saturating_sub((n_disabled + 1) * u32::from(PADDING))) / n_disabled) as u16;
        let width = max_width.min((f64::from(max_height) * 16. / 9.).round() as u16);
        let height = max_height.min((f64::from(max_width) * 9. / 16.).round() as u16);
        [width, height]
    }

    fn on_click(&self, _gc: &GestureClick, _n_press: i32, x: f64, y: f64) {
        if let Some(i) = self.get_output_index_at(x, y, self.obj().width(), self.obj().height()) {
            self.select(i);
            self.obj().grab_focus();
            self.obj().emit_by_name::<()>("output-selected", &[&self.outputs.borrow().index(i)]);
        } else {
            self.deselect();
            self.obj().emit_by_name::<()>("output-deselected", &[]);
        }
        self.obj().queue_draw();
    }

    fn on_motion(&self, _ecm: &EventControllerMotion, x: f64, y: f64) {
        match self.get_output_index_at(x, y, self.obj().width(), self.obj().height()) {
            Some(_) => self.obj().set_cursor_from_name(Some("pointer")),
            None => self.obj().set_cursor_from_name(Some("default")),
        }
    }

    fn on_drop_motion(_dcm: &DropControllerMotion, _x: f64, _y: f64) {}

    #[allow(clippy::cast_possible_truncation)]
    fn on_drag_prepare(&self, ds: &DragSource, x: f64, y: f64) -> Option<ContentProvider> {
        let outputs = self.outputs.borrow();
        let width = self.obj().width();
        let height = self.obj().height();
        if let Some(i) = self.get_output_index_at(x, y, width, height) {
            let [width, height] = Self::get_output_dim(width, height, outputs.n_items() as usize);
            if let Ok(icon) = Self::create_drag_icon(
                &self.config,
                width,
                height,
                &outputs.index(i).name(),
                outputs.index(i).product_name().as_deref(),
            ) {
                let [_, oy] = Self::get_output_pos(i, height);
                ds.set_icon(Some(&icon), x as i32, (y - f64::from(oy)) as i32);
            }
            return Some(ContentProvider::for_value(&outputs.index(i).to_value()));
        }
        None
    }

    fn on_drag_begin(&self, _ds: &DragSource, _d: &Drag) {
        self.obj().queue_draw();
        self.is_dragging.set(true);
    }

    fn on_drag_end(&self, _ds: &DragSource, d: &Drag, del: bool) {
        if del {
            if let Ok(value) = d.content().value(Output::static_type()) {
                if let Ok(output) = value.get::<Output>() {
                    self.obj().update(&output, Update::Enabled);
                }
            }
        }
        self.is_dragging.set(false);
    }

    fn create_drag_icon(
        config: &Config,
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
        let context = DrawContext::new(cr, config.clone());
        context.draw_output(rect);
        context.draw_output_label(rect, name, product_name);
        drop(context);
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

    fn get_output_index_at(&self, x: f64, y: f64, width: i32, height: i32) -> Option<usize> {
        let outputs = self.outputs.borrow();
        let [width, height] = Self::get_output_dim(width, height, outputs.n_items() as usize);
        for (i, _) in outputs.iter::<Output>().map(Result::unwrap).enumerate() {
            let [ox, oy] = Self::get_output_pos(i, height);
            if x >= f64::from(ox)
                && x <= f64::from(i32::from(ox) + i32::from(width))
                && y >= f64::from(oy)
                && y <= f64::from(i32::from(oy) + i32::from(height))
            {
                return Some(i);
            }
        }
        None
    }
}
