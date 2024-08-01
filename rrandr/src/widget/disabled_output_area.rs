use config::Config;
use glib::subclass::types::ObjectSubclassIsExt;
use glib::{closure_local, wrapper, Object};
use gtk::prelude::{ObjectExt, WidgetExt};
use gtk::{glib, Accessible, Buildable, ConstraintTarget, DrawingArea, Widget};

use super::details_box::Update;
use crate::data::output::Output;
use crate::data::outputs::Outputs;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::error::Error;
    use std::sync::OnceLock;

    use config::Config;
    use gdk::{ContentProvider, Drag, DragAction, MemoryFormat, MemoryTexture, Paintable};
    use glib::object::IsA;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::subclass::Signal;
    use glib::value::ToValue;
    use glib::{clone, derived_properties, object_subclass, Bytes, Properties};
    use gtk::prelude::{
        ContentProviderExtManual, DragExt, DrawingAreaExt, DrawingAreaExtManual, ListModelExt,
        ListModelExtManual, ObjectExt, StaticType, WidgetExt,
    };
    use gtk::subclass::drawing_area::DrawingAreaImpl;
    use gtk::subclass::prelude::DerivedObjectProperties;
    use gtk::subclass::widget::WidgetImpl;
    use gtk::{
        glib, DragSource, DrawingArea, DropControllerMotion, EventControllerMotion, GestureClick,
    };
    use log::error;

    use crate::data::output::Output;
    use crate::data::outputs::Outputs;
    use crate::draw::DrawContext;
    use crate::widget::details_box::Update;
    use crate::window::PADDING;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::DisabledOutputArea)]
    pub struct DisabledOutputArea {
        pub(super) config: RefCell<Config>,
        #[property(get, set = Self::set_outputs)]
        outputs: RefCell<Outputs>,
        pub(super) selected_output: RefCell<Option<Output>>,
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
                #[weak(rename_to = this)]
                self,
                move |_d, cr, width, height| this.on_draw(cr, width, height)
            ));

            let drag_source = DragSource::builder().actions(DragAction::MOVE).build();
            drag_source.connect_prepare(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |ds, x, y| this.on_drag_prepare(ds, x, y)
            ));
            drag_source.connect_drag_begin(clone!(
                #[weak(rename_to = this)]
                self,
                move |ds, d| this.on_drag_begin(ds, d)
            ));
            drag_source.connect_drag_end(clone!(
                #[weak(rename_to = this)]
                self,
                move |ds, d, del| this.on_drag_end(ds, d, del)
            ));
            obj.add_controller(drag_source);

            let gesture_click = GestureClick::new();
            gesture_click.connect_pressed(clone!(
                #[weak(rename_to = this)]
                self,
                move |gc, n_press, x, y| this.on_click(gc, n_press, x, y)
            ));
            obj.add_controller(gesture_click);

            let event_controller_motion = EventControllerMotion::new();
            event_controller_motion.connect_motion(clone!(
                #[weak(rename_to = this)]
                self,
                move |ecm, x, y| this.on_motion(ecm, x, y)
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
            let selected = self.selected_output.take();
            self.selected_output.replace(selected.and_then(|s| outputs.find_by_id(s.id())));
            self.obj().queue_draw();
        }

        pub(super) fn select(&self, output: &Output) {
            self.selected_output.replace(Some(output.clone()));
        }

        pub(super) fn deselect(&self) { self.selected_output.replace(None); }

        fn on_draw(&self, cr: &cairo::Context, width: i32, height: i32) {
            let outputs = self.outputs.borrow();
            let selected = self.selected_output.borrow();
            let context = DrawContext::new(cr, &self.config.borrow());
            let [width, height] = Self::get_output_dim(width, height, outputs.n_items() as usize);
            let mut j: usize = 0; // separate index for closing the gaps
            for o in outputs.iter::<Output>().map(Result::unwrap) {
                if selected.is_none()
                    || selected
                        .as_ref()
                        .is_some_and(|s| !self.is_dragging.get() || s.id() != o.id())
                {
                    let [x, y] = Self::get_output_pos(j, height);
                    let rect = cairo::Rectangle::new(
                        f64::from(x),
                        f64::from(y),
                        f64::from(width),
                        f64::from(height),
                    );
                    context.draw_output(&rect);
                    context.draw_output_label(&rect, &o.name(), o.product_name().as_deref());
                    if let Some(s) = selected.as_ref() {
                        if s.id() == o.id() {
                            context.draw_selected_output(&rect);
                        }
                    }
                    j += 1;
                }
            }
        }

        fn get_output_pos(index: usize, output_height: u16) -> [i16; 2] {
            let index = u32::try_from(index).expect("less disabled outputs");
            let x = i16::try_from(PADDING).unwrap_or(i16::MAX);
            let y =
                i16::try_from((index + 1) * u32::from(PADDING) + index * u32::from(output_height))
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
            if let Some(o) = self.get_output_at(x, y, self.obj().width(), self.obj().height()) {
                self.select(&o);
                self.obj().grab_focus();
                self.obj().emit_by_name::<()>("output-selected", &[&o]);
            } else {
                self.deselect();
                self.obj().emit_by_name::<()>("output-deselected", &[]);
            }
            self.obj().queue_draw();
        }

        fn on_motion(&self, _ecm: &EventControllerMotion, x: f64, y: f64) {
            match self.get_output_at(x, y, self.obj().width(), self.obj().height()) {
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
            if let Some(o) = self.get_output_at(x, y, width, height) {
                let [width, height] =
                    Self::get_output_dim(width, height, outputs.n_items() as usize);
                match self.create_drag_icon(width, height, &o.name(), o.product_name().as_deref()) {
                    Ok(icon) => {
                        let [_, oy] =
                            Self::get_output_pos(outputs.position(&o).unwrap() as usize, height);
                        ds.set_icon(Some(&icon), x as i32, (y - f64::from(oy)) as i32);
                    }
                    Err(e) => error!("Failed to create drag icon: {e}"),
                }
                return Some(ContentProvider::for_value(&o.to_value()));
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
            self.obj().queue_draw();
            self.is_dragging.set(false);
        }

        fn create_drag_icon(
            &self,
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
            {
                // Use separate scope for cairo and draw context since exclusive
                // access is needed to get data from surface
                let cr = cairo::Context::new(&surface)?;
                let rect = cairo::Rectangle::new(0., 0., f64::from(width), f64::from(height));
                let context = DrawContext::new(&cr, &self.config.borrow());
                context.draw_output(&rect);
                context.draw_output_label(&rect, name, product_name);
            }
            surface.flush();
            let stride = surface.stride().try_into()?;
            Ok(MemoryTexture::new(
                i32::from(width),
                i32::from(height),
                MemoryFormat::B8g8r8a8Premultiplied,
                &Bytes::from_owned(surface.take_data()?),
                stride,
            ))
        }

        fn get_output_at(&self, x: f64, y: f64, width: i32, height: i32) -> Option<Output> {
            let outputs = self.outputs.borrow();
            let [width, height] = Self::get_output_dim(width, height, outputs.n_items() as usize);
            for (i, o) in outputs.iter::<Output>().map(Result::unwrap).enumerate() {
                let [ox, oy] = Self::get_output_pos(i, height);
                if x >= f64::from(ox)
                    && x <= f64::from(i32::from(ox) + i32::from(width))
                    && y >= f64::from(oy)
                    && y <= f64::from(i32::from(oy) + i32::from(height))
                {
                    return Some(o);
                }
            }
            None
        }
    }
}

wrapper! {
    pub struct DisabledOutputArea(ObjectSubclass<imp::DisabledOutputArea>)
        @extends DrawingArea, Widget,
        @implements Accessible, Buildable, ConstraintTarget;
}

impl DisabledOutputArea {
    pub fn new(outputs: &Outputs) -> Self { Object::builder().property("outputs", outputs).build() }

    pub fn set_config(&self, cfg: &Config) { self.imp().config.replace(cfg.clone()); }

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
            Update::Enabled => {
                self.imp().deselect();
                self.outputs().remove(output.id());
            }
            Update::Disabled => {
                self.outputs().append(output);
                self.imp().select(output);
            }
            _ => (),
        }
        // Redraw
        match update {
            Update::Enabled | Update::Disabled => self.queue_draw(),
            _ => (),
        }
    }

    pub fn selected_output(&self) -> Option<Output> { self.imp().selected_output.borrow().clone() }

    pub fn select(&self, output: &Output) { self.imp().select(output); }

    pub fn deselect(&self) { self.imp().deselect(); }
}
