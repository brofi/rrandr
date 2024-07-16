pub fn nearly_eq(a: f64, b: f64) -> bool { nearly_eq_rel_and_abs(a, b, 0.0, None) }

// Floating point comparison inspired by:
// https://randomascii.wordpress.com/2012/02/25/comparing-floating-point-numbers-2012-edition/
// https://peps.python.org/pep-0485/
// https://floating-point-gui.de/errors/comparison/
pub fn nearly_eq_rel_and_abs(a: f64, b: f64, abs_tol: f64, rel_tol: Option<f64>) -> bool {
    nearly_eq_rel(a, b, rel_tol) || nearly_eq_abs(a, b, abs_tol)
}

pub fn nearly_eq_abs(a: f64, b: f64, abs_tol: f64) -> bool { (a - b).abs() <= abs_tol }

pub fn nearly_eq_rel(a: f64, b: f64, rel_tol: Option<f64>) -> bool {
    let diff = (a - b).abs();
    let a = a.abs();
    let b = b.abs();
    diff <= if b > a { b } else { a } * rel_tol.unwrap_or(f64::EPSILON)
}
