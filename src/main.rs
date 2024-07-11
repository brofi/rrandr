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
mod widget;
mod x11;

use app::Application;
use data::output::Output;
use gio::resources_register_include;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    resources_register_include!("rrandr.gresource").expect("resources registered");
    Application::run()
}

fn nearly_eq(a: f64, b: f64) -> bool { nearly_eq_rel_and_abs(a, b, 0.0, None) }

// Floating point comparison inspired by:
// https://randomascii.wordpress.com/2012/02/25/comparing-floating-point-numbers-2012-edition/
// https://peps.python.org/pep-0485/
// https://floating-point-gui.de/errors/comparison/
fn nearly_eq_rel_and_abs(a: f64, b: f64, abs_tol: f64, rel_tol: Option<f64>) -> bool {
    nearly_eq_rel(a, b, rel_tol) || nearly_eq_abs(a, b, abs_tol)
}

fn nearly_eq_abs(a: f64, b: f64, abs_tol: f64) -> bool { (a - b).abs() <= abs_tol }

fn nearly_eq_rel(a: f64, b: f64, rel_tol: Option<f64>) -> bool {
    let diff = (a - b).abs();
    let a = a.abs();
    let b = b.abs();
    diff <= if b > a { b } else { a } * rel_tol.unwrap_or(f64::EPSILON)
}
