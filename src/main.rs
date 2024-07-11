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
use data::output::Output;
use gio::resources_register_include;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    resources_register_include!("rrandr.gresource").expect("resources registered");
    Application::run()
}
