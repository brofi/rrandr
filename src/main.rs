extern crate gio;
extern crate gtk;

use std::ffi::CStr;
use std::ptr::null;
use std::{env, slice};

use gio::prelude::*;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, ComboBoxText, Grid, PositionType, ToggleButton,
    NONE_TOGGLE_BUTTON,
};
use std::collections::HashMap;
use x11::xlib::{Display, Window, XCloseDisplay, XDefaultScreen, XOpenDisplay, XRootWindow};
use x11::xrandr::{
    Connection, RRCrtc, RRMode, RROutput, RR_Connected, RR_DoubleScan, RR_Interlace,
    XRRGetOutputInfo, XRRGetScreenResourcesCurrent, XRRModeInfo, XRROutputInfo,
    XRRScreenResources,
};

#[derive(Debug)]
struct OutputInfo {
    xid: u64,
    name: String,
    enabled: bool,
    modes: HashMap<String, Vec<f64>>,
}

fn main() {
    let application = Application::new(Some("com.github.brofi.rxrandr"), Default::default())
        .expect("Failed to initialize GTK application.");
    application.connect_activate(|app| {
        let win = ApplicationWindow::new(app);
        win.set_title("RXRandR");
        win.set_default_size(320, 200);

        let grid = Grid::new();

        let tb_enable = ToggleButton::new_with_label("Enable");
        grid.attach_next_to(&tb_enable, NONE_TOGGLE_BUTTON, PositionType::Left, 20, 5);

        let cb_resolution = ComboBoxText::new();
        cb_resolution.append_text("2560x1440");
        cb_resolution.append_text("1920x1080");
        grid.attach_next_to(&cb_resolution, Some(&tb_enable), PositionType::Right, 20, 5);

        let cb_refresh_rate = ComboBoxText::new();
        cb_refresh_rate.append_text("144 Hz");
        cb_refresh_rate.append_text("60 Hz");
        grid.attach_next_to(
            &cb_refresh_rate,
            Some(&cb_resolution),
            PositionType::Right,
            20,
            5,
        );

        win.add(&grid);
        win.show_all();
    });
    application.run(&env::args().collect::<Vec<_>>());

    let all_output_info: HashMap<u64, OutputInfo> = get_output_info();
    for oi in all_output_info.values() {
        println!("{:?}", oi);
    }
}

fn get_output_info() -> HashMap<u64, OutputInfo> {
    //    let crtcs: Vec<RRCrtc> = get_crtcs_as_vec(&mut *res);
    //    for c in crtcs {
    //        print!("crtc: {}, outputs: ", c);
    //        let crtc_info: *mut XRRCrtcInfo = XRRGetCrtcInfo(dpy, res, c);
    //        let crtc_outputs: *mut u64 = (*crtc_info).outputs;
    //        for o in 0..(*crtc_info).noutput {
    //            let output = *crtc_outputs.offset(o as isize);
    //            print!("{}", output);
    //        }
    //        println!();
    //    }

    let mut output_info: HashMap<u64, OutputInfo> = HashMap::new();

    unsafe {
        let dpy: *mut Display = XOpenDisplay(null());

        if dpy.is_null() {
            panic!("Failed to open display.");
        }

        let res: *mut XRRScreenResources = get_screen_res(&mut *dpy);
        let outputs: Vec<RROutput> = get_outputs_as_vec(&mut *res);
        for o in outputs {
            let x_output_info: *mut XRROutputInfo = XRRGetOutputInfo(dpy, res, o);

            if !is_connected((*x_output_info).connection) {
                continue;
            }

            let name: String = CStr::from_ptr((*x_output_info).name)
                .to_str()
                .unwrap()
                .to_owned();
            let enabled: bool = is_output_enabled(&mut *res, (*x_output_info).crtc);
            let mut modes: HashMap<String, Vec<f64>> = HashMap::new();
            let mode_info: Vec<XRRModeInfo> =
                get_mode_info_for_output_as_vec(&mut *res, &mut *x_output_info);
            for mode_i in mode_info {
                let mode_name = CStr::from_ptr(mode_i.name).to_str().unwrap().to_owned();
                modes
                    .entry(mode_name)
                    .or_insert(Vec::new())
                    .push(get_refresh_rate(mode_i));
            }
            output_info.insert(
                o,
                OutputInfo {
                    xid: o,
                    name,
                    enabled,
                    modes,
                },
            );
        }

        XCloseDisplay(dpy);
    }

    output_info
}

fn get_screen_res(dpy: &mut Display) -> *mut XRRScreenResources {
    unsafe {
        let screen = XDefaultScreen(dpy);
        let root: Window = XRootWindow(dpy, screen);
        let res: *mut XRRScreenResources = XRRGetScreenResourcesCurrent(dpy, root);
        assert!(!res.is_null());
        res
    }
}

#[allow(dead_code)]
fn get_crtcs_as_vec(res: &mut XRRScreenResources) -> Vec<RRCrtc> {
    let crtcs: *mut RRCrtc = res.crtcs;
    let len = res.ncrtc;
    assert!(!crtcs.is_null());
    assert!(len >= 0);
    unsafe { slice::from_raw_parts(crtcs, len as usize) }.to_vec()
}

fn get_outputs_as_vec(res: &mut XRRScreenResources) -> Vec<RROutput> {
    let outputs: *mut RROutput = res.outputs;
    let len = res.noutput;
    assert!(!outputs.is_null());
    assert!(len >= 0);
    unsafe { slice::from_raw_parts(outputs, len as usize) }.to_vec()
}

fn get_mode_info_as_vec(res: &mut XRRScreenResources) -> Vec<XRRModeInfo> {
    let mode_info: *mut XRRModeInfo = res.modes;
    let len = res.nmode;
    assert!(!mode_info.is_null());
    assert!(len >= 0);
    unsafe { slice::from_raw_parts(mode_info, len as usize) }.to_vec()
}

fn get_modes_as_vec(output_info: &mut XRROutputInfo) -> Vec<RRMode> {
    let modes: *mut RRMode = output_info.modes;
    let len = output_info.nmode;
    assert!(!modes.is_null());
    assert!(len >= 0);
    unsafe { slice::from_raw_parts(modes, len as usize) }.to_vec()
}

fn get_mode_info_for_output_as_vec(
    res: &mut XRRScreenResources,
    output_info: &mut XRROutputInfo,
) -> Vec<XRRModeInfo> {
    let mut mode_info_for_output: Vec<XRRModeInfo> = Vec::new();
    let mode_ids_for_output: Vec<RRMode> = get_modes_as_vec(output_info);
    let mode_info: Vec<XRRModeInfo> = get_mode_info_as_vec(res);
    for mode_id in mode_ids_for_output {
        for mode_i in &mode_info {
            if mode_id == (*mode_i).id {
                mode_info_for_output.push(*mode_i);
            }
        }
    }
    mode_info_for_output
}

fn is_connected(connection: Connection) -> bool {
    #[allow(non_upper_case_globals)]
    match connection as i32 {
        RR_Connected => true,
        _ => false,
    }
}

fn is_output_enabled(res: &XRRScreenResources, output_crtc: RRCrtc) -> bool {
    let crtcs = res.crtcs;
    for i in 0..res.ncrtc {
        let c = unsafe { *crtcs.offset(i as isize) };
        if c == output_crtc {
            return true;
        }
    }
    false
}

fn get_refresh_rate(mode_info: XRRModeInfo) -> f64 {
    let mut v_total = mode_info.vTotal;

    if mode_info.modeFlags & RR_DoubleScan as u64 == 1 {
        v_total *= 2;
    }
    if mode_info.modeFlags & RR_Interlace as u64 == 1 {
        v_total /= 2;
    }

    if mode_info.hTotal > 0 && v_total > 0 {
        mode_info.dotClock as f64 / (mode_info.hTotal as f64 * v_total as f64)
    } else {
        0.0
    }
}
