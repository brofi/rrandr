use std::ffi::CStr;
use std::ptr::null;

use x11::xlib::{Display, Window, XCloseDisplay, XDefaultScreen, XOpenDisplay, XRootWindow};
use x11::xrandr::{RR_Connected, RROutput, XRRGetOutputInfo, XRRGetScreenResourcesCurrent, XRROutputInfo, XRRScreenResources};

fn main() {
    print_connected_outputs();
}

fn print_connected_outputs() {
    unsafe {
        let dpy: *mut Display = XOpenDisplay(null());

        if dpy.is_null() {
            panic!("Failed to open display.");
        }

        let screen = XDefaultScreen(dpy);
        let root: Window = XRootWindow(dpy, screen);
        let res: *mut XRRScreenResources = XRRGetScreenResourcesCurrent(dpy, root);

        for o in 0..(*res).noutput {
            let output: RROutput = *(*res).outputs.offset(o as isize);
            let output_info: *mut XRROutputInfo = XRRGetOutputInfo(dpy, res, output);
            let name = CStr::from_ptr((*output_info).name).to_str().unwrap();

            #[allow(non_upper_case_globals)]
                match (*output_info).connection as i32 {
                RR_Connected => println!("{}", &name),
                _ => ()
            }
        }

        XCloseDisplay(dpy);
    }
}
