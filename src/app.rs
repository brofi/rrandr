use gio::{ActionGroup, ActionMap};
use glib::{wrapper, ExitCode, Object};
use gtk::prelude::ApplicationExtManual;
use gtk::{gio, glib};

pub const APP_ID: &str = "com.github.brofi.rrandr";
pub const APP_NAME: &str = "rrandr";

mod imp {

    use std::rc::Rc;

    use glib::subclass::object::ObjectImpl;
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use glib::{clone, object_subclass};
    use gtk::glib;
    use gtk::prelude::{GtkApplicationExt, GtkWindowExt};
    use gtk::subclass::application::GtkApplicationImpl;
    use gtk::subclass::prelude::{ApplicationImpl, ApplicationImplExt};
    use log::error;

    use crate::config::Config;
    use crate::window::Window;
    use crate::x11::popup::show_popup_windows;

    #[derive(Default)]
    pub struct Application;

    #[object_subclass]
    impl ObjectSubclass for Application {
        type ParentType = gtk::Application;
        type Type = super::Application;

        const NAME: &'static str = "Application";
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();
            let obj = self.obj();
            obj.set_accels_for_action("win.apply", &["<Ctrl>S"]);
            obj.set_accels_for_action("win.reset", &["<Ctrl>BackSpace"]);
            obj.set_accels_for_action("win.redraw", &["<Ctrl>R"]);
            obj.set_accels_for_action("window.close", &["<Ctrl>Q", "<Ctrl>W"]);
        }

        fn activate(&self) {
            let cfg = Rc::new(Config::new());
            let window = Window::new(&*self.obj());
            window.set_config(&cfg);
            window.connect_identify(clone!(
                #[strong]
                cfg,
                move |_, btn| {
                    if let Err(e) = show_popup_windows(&cfg, btn) {
                        error!("Failed to identify outputs: {e:?}");
                    };
                }
            ));
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
