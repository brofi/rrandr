use std::ffi::CStr;
use std::ptr::null;
use std::{env, slice};

use gio::prelude::*;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, ComboBoxText, Grid, PositionType, RadioButton, ToggleButton,
    NONE_RADIO_BUTTON,
};
use std::collections::HashMap;
use x11::xlib::{Display, Window, XCloseDisplay, XDefaultScreen, XOpenDisplay, XRootWindow};
use x11::xrandr::{
    Connection, RRCrtc, RRMode, RROutput, RR_Connected, RR_DoubleScan, RR_Interlace,
    XRRGetOutputInfo, XRRGetScreenResourcesCurrent, XRRModeInfo, XRROutputInfo, XRRScreenResources,
};

// TODO Display Trait?
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
        win.set_border_width(20);

        let grid = Grid::new();

        let output_info: HashMap<u64, OutputInfo> = get_output_info();
        for oi in output_info.values() {
            println!("{:?}", oi);
        }

        // create radio buttons with output name as label
        let mut output_radio_buttons = Vec::new();
        for o in output_info.values() {
            output_radio_buttons.push(RadioButton::new_with_label(o.name.as_str()));
        }

        // join output radio buttons to a group and add to grid
        let mut prev_rb = NONE_RADIO_BUTTON;
        let mut it = output_radio_buttons.iter();
        while let Some(rb) = it.next() {
            rb.join_group(prev_rb);
            rb.connect_toggled(|b| {
                if b.get_active() {
                    if let Some(label) = b.get_label() {
                        println!("RadioButton {} was turned on.", label);
                    }
                }
            });
            grid.attach_next_to(rb, prev_rb, PositionType::Right, 1, 1);
            prev_rb = Some(rb);
        }

        // TODO set to primary
        if let Some(rb) = output_radio_buttons.get(1) {
            rb.set_active(true);
        }

        let tb_enable = ToggleButton::new_with_label("Enable");
        grid.attach(&tb_enable, 0, 1, 1, 1);

        let cb_refresh_rate = ComboBoxText::new();
        let cb_resolution = ComboBoxText::new();

        cb_resolution.connect_changed(|cb| {
            if let Some(resolution) = cb.get_active_text() {
                println!("resolution {} selected.", resolution);
                // TODO dependent on radio button choice
                // TODO cb may outlive cb_refresh_rate
//                if let Some(first) = output_info.values().next() {
//                    if let Some(o) = output_info.get(&first.xid) {
//                        if let Some(rrs) = o.modes.get(resolution.as_str()) {
//                            cb_refresh_rate.remove_all();
//                            for r in rrs {
//                                cb_refresh_rate.append_text(format!("{:2}", r).as_str());
//                            }
//                        }
//                    }
//                }
            }
        });

        // TODO dependent on radio button choice
        // TODO rethink the HashMap
        if let Some(first) = output_info.values().next() {
            if let Some(o) = output_info.get(&first.xid) {
                for m in o.modes.keys() {
                    cb_resolution.append_text(m);
                }
            }
        }
        grid.attach_next_to(&cb_resolution, Some(&tb_enable), PositionType::Right, 1, 1);
        grid.attach_next_to(
            &cb_refresh_rate,
            Some(&cb_resolution),
            PositionType::Right,
            1,
            1,
        );

        win.add(&grid);
        win.show_all();
    });
    application.run(&env::args().collect::<Vec<_>>());
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
