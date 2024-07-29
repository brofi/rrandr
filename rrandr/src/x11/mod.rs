pub mod popup;
pub mod randr;

fn x_error_to_string(e: &x11rb::x11_utils::X11Error) -> String {
    format!(
        "X11 {:?} error for value {}{}.",
        e.error_kind,
        e.bad_value,
        e.request_name.map(|s| " in request ".to_owned() + s).unwrap_or_default()
    )
}
