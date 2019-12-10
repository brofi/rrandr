use std::ffi::CStr;
use std::ptr::null;
use std::{env, slice};

use gio::prelude::*;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Builder, ComboBoxText, RadioButton, NONE_RADIO_BUTTON,
};
use std::collections::HashMap;
use x11::xlib::{Display, Window, XCloseDisplay, XDefaultScreen, XOpenDisplay, XRootWindow};
use x11::xrandr::{
    Connection, RRCrtc, RRMode, RROutput, RR_Connected, RR_DoubleScan, RR_Interlace,
    XRRGetOutputInfo, XRRGetScreenResourcesCurrent, XRRModeInfo, XRROutputInfo, XRRScreenResources,
};

// TODO consider using
#[allow(unused_macros)]
macro_rules! enclose {
    ( ($( $x:ident ),*) $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}

// TODO Display Trait?
#[derive(Debug, Clone)]
struct OutputInfo {
    xid: u64,
    name: String,
    enabled: bool,
    modes: HashMap<String, Vec<f64>>,
}

fn main() {
    let output_info: HashMap<u64, OutputInfo> = get_output_info();
    for oi in output_info.values() {
        println!("{:?}", oi);
    }

    let application = Application::new(Some("com.github.brofi.rxrandr"), Default::default())
        .expect("Failed to initialize GTK application.");

    application.connect_activate(move |app| {
        let output_info = output_info.clone();
        build_ui(app, output_info);
    });

    application.run(&env::args().collect::<Vec<_>>());
}

fn build_ui(application: &Application, output_info: HashMap<u64, OutputInfo>) {
    let builder = Builder::new_from_string(include_str!("gui.glade"));

    let window_name = "window";
    let window: ApplicationWindow = builder.get_object(window_name).expect(&format!(
        "Failed to get ApplicationWindow '{}'",
        window_name
    ));
    window.set_application(Some(application));

    let box_outputs_name = "box_outputs";
    let box_outputs: Box = builder
        .get_object(box_outputs_name)
        .expect(&format!("Failed to get Box '{}'", box_outputs_name));

    // create radio buttons with output name as label
    let mut output_radio_buttons = Vec::new();
    for o in output_info.values() {
        let rb = RadioButton::new_with_label(&format!("Output: {}", o.name));
        WidgetExt::set_name(&rb, o.name.as_str());
        output_radio_buttons.push(rb);
    }

    // join output radio buttons to a group and add to grid
    let mut prev_rb = NONE_RADIO_BUTTON;
    let mut it = output_radio_buttons.iter();
    while let Some(rb) = it.next() {
        rb.join_group(prev_rb);
        rb.connect_toggled(|b| {
            if b.get_active() {
                if let Some(name) = WidgetExt::get_name(b) {
                    println!("RadioButton {} was turned on.", name);
                }
            }
        });
        box_outputs.add(rb);
        prev_rb = Some(rb);
    }

    // TODO set to primary
    if let Some(rb) = output_radio_buttons.get(0) {
        rb.set_active(true);
    }

    let cb_refresh_rate_name = "cb_refresh_rate";
    let cb_refresh_rate: ComboBoxText = builder.get_object(cb_refresh_rate_name).expect(&format!(
        "Failed to get ComboBox '{}'",
        cb_refresh_rate_name
    ));

    let cb_resolution_name = "cb_resolution";
    let cb_resolution: ComboBoxText = builder
        .get_object(cb_resolution_name)
        .expect(&format!("Failed to get ComboBox '{}'", cb_resolution_name));

    // TODO connect with selected output
    let output_info_first_c = match output_info.values().next() {
        Some(o) => (*o).clone(),
        None => panic!("no first"),
    };
    cb_resolution.connect_changed({
        let cb_refresh_rate = cb_refresh_rate.clone();
        move |cb| {
            if let Some(resolution) = cb.get_active_text() {
                println!("resolution {} selected.", resolution);
                // TODO dependent on radio button choice
                if let Some(rrs) = output_info_first_c.modes.get(resolution.as_str()) {
                    cb_refresh_rate.remove_all();
                    for r in rrs {
                        cb_refresh_rate.append_text(format!("{:2}", r).as_str());
                    }
                    // TODO use current if current res
                    cb_refresh_rate.set_active(Some(0));
                }
            }
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

    // TODO set current resolution
    cb_resolution.set_id_column(0);
    cb_resolution.set_active_id(Some("1024x768"));

    window.show_all();
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
