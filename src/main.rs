extern crate gio;
extern crate gtk;

use gio::prelude::*;
use gtk::prelude::*;

use std::env;
use std::ffi::CStr;
use std::ptr::null;

use gtk::{Application, ApplicationWindow};
use x11::xlib::{Display, Window, XCloseDisplay, XDefaultScreen, XOpenDisplay, XRootWindow};
use x11::xrandr::{
    Connection, RRMode, RROutput, RR_Connected, RR_DoubleScan, RR_Interlace, XRRGetOutputInfo,
    XRRGetScreenResourcesCurrent, XRRModeInfo, XRROutputInfo, XRRScreenResources,
};

fn main() {
    let application = Application::new(Some("com.github.brofi.rxrandr"), Default::default())
        .expect("Failed to initialize GTK application.");
    application.connect_activate(|app| {
        let win = ApplicationWindow::new(app);
        win.set_default_size(320, 200);
        win.set_title("RXRandR");
        win.show_all();
    });
    application.run(&env::args().collect::<Vec<_>>());

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

            if is_connected((*output_info).connection) {
                let name = CStr::from_ptr((*output_info).name).to_str().unwrap();
                println!("{}", &name);

                println!("modes:");
                for om in 0..(*output_info).nmode {
                    let mode: RRMode = *(*output_info).modes.offset(om as isize);
                    for rm in 0..(*res).nmode {
                        let mode_info: XRRModeInfo = *(*res).modes.offset(rm as isize);
                        if mode_info.id == mode {
                            let mode_name = CStr::from_ptr(mode_info.name).to_str().unwrap();
                            println!("{} ({:.2} Hz)", &mode_name, get_refresh_rate(mode_info));
                        }
                    }
                }
            }
        }

        XCloseDisplay(dpy);
    }
}

fn is_connected(connection: Connection) -> bool {
    #[allow(non_upper_case_globals)]
    return match connection as i32 {
        RR_Connected => true,
        _ => false,
    };
}

fn get_refresh_rate(mode_info: XRRModeInfo) -> f64 {
    let mut v_total = mode_info.vTotal;

    if mode_info.modeFlags & RR_DoubleScan as u64 == 1 {
        v_total *= 2;
    }
    if mode_info.modeFlags & RR_Interlace as u64 == 1 {
        v_total /= 2;
    }

    return if mode_info.hTotal > 0 && v_total > 0 {
        mode_info.dotClock as f64 / (mode_info.hTotal as f64 * v_total as f64)
    } else {
        0.0
    };
}
