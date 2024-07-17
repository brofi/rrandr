#![warn(clippy::pedantic)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
// #![warn(clippy::restriction)]

mod app;
mod color;
mod config;
mod data;
mod draw;
mod math;
mod utils;
mod widget;
mod window;
mod x11;

use app::Application;
use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};
use gio::resources_register_include;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    env_logger::init();
    setlocale(LocaleCategory::LcAll, "");
    let domainname = env!("CARGO_PKG_NAME");
    bindtextdomain(domainname, env!("RRANDR_LOCALE_DIR")).expect("bind text domain");
    bind_textdomain_codeset(domainname, "UTF-8").expect("bind text domain encoding");
    textdomain(domainname).expect("text domain");
    resources_register_include!("rrandr.gresource").expect("resources registered");
    Application::run()
}
