use gio::{ActionGroup, ActionMap};
use glib::{wrapper, ExitCode, Object};
use gtk::prelude::ApplicationExtManual;
use gtk::{gio, glib};

const APP_ID: &str = "com.github.brofi.rrandr";

mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::{clone, object_subclass};
    use gtk::glib;
    use gtk::prelude::{GtkApplicationExt, GtkWindowExt};
    use gtk::subclass::application::GtkApplicationImpl;
    use gtk::subclass::prelude::ApplicationImpl;
    use log::error;

    use crate::window::{Action, Window};
    use crate::x11::popup::show_popup_windows;
    use crate::x11::randr::Randr;

    #[derive(Default)]
    pub struct Application {
        randr: Rc<RefCell<Randr>>,
    }

    #[object_subclass]
    impl ObjectSubclass for Application {
        type ParentType = gtk::Application;
        type Type = super::Application;

        const NAME: &'static str = "Application";
    }

    impl ObjectImpl for Application {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_accels_for_action("window.close", &["<Ctrl>Q", "<Ctrl>W"]);
        }
    }

    impl ApplicationImpl for Application {
        fn activate(&self) {
            let window = Window::new(&*self.obj());
            window.set_screen_max_size(
                self.randr.borrow().screen_size_range().max_width,
                self.randr.borrow().screen_size_range().max_height,
            );
            window.set_outputs(&self.randr.borrow().output_model());
            window.connect_apply(clone!(
                @weak self as app => @default-panic, move |window, _, outputs| {
                    app.randr.replace(Randr::new());
                    let randr = app.randr.borrow();
                    let success = randr.apply(outputs);
                    if !success {
                        randr.revert();
                        window.set_outputs(&randr.output_model());
                    }
                    success
                }
            ));
            window.connect_confirm_action(clone!(
                @weak self as app => move |window, action| {
                    match action {
                        Action::Keep => _ = app.randr.replace(Randr::new()),
                        Action::Revert => {
                            let randr = app.randr.borrow();
                            randr.revert();
                            window.set_outputs(&randr.output_model());
                        },
                    }
                }
            ));
            window.connect_reset(clone!(
                @weak self as app => move |window, _| {
                    let randr = app.randr.borrow();
                    window.set_outputs(&randr.output_model());
                }
            ));
            window.connect_identify(|_, btn| {
                if let Err(e) = show_popup_windows(btn) {
                    error!("Failed to identify outputs: {e:?}");
                }
            });
            window.present();
        }
    }

    impl GtkApplicationImpl for Application {}
}

wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application,
        @implements ActionMap, ActionGroup;
}

impl Application {
    pub fn run() -> ExitCode { Application::default().run() }
}

impl Default for Application {
    fn default() -> Self { Object::builder().property("application-id", APP_ID).build() }
}
