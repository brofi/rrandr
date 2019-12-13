use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ptr::null;
use std::rc::Rc;
use std::{env, slice};

use gio::prelude::*;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Builder, ComboBoxText, RadioButton, Switch,
    NONE_RADIO_BUTTON,
};
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

struct OutputState {
    outputs: HashMap<String, Output>,
    key_selected: RefCell<String>,
}

impl OutputState {
    fn new(outputs: HashMap<String, Output>, selected: String) -> Rc<OutputState> {
        let selected = RefCell::new(selected);
        Rc::new(OutputState {
            outputs,
            key_selected: selected,
        })
    }
}

// TODO Display Trait?
#[derive(Debug, Clone)]
struct Output {
    xid: u64,
    name: String,
    modes: HashMap<String, Vec<f64>>,
    curr_conf: OutputConfig,
    new_conf: RefCell<OutputConfig>,
}

impl Output {
    fn add_mode(&mut self, mode_name: String, refresh_rate: f64) {
        self.modes
            .entry(mode_name)
            .or_insert(Vec::new())
            .push(refresh_rate);
    }
}

#[derive(Debug, Clone)]
struct OutputConfig {
    enabled: bool,
    resolution: String,
    refresh_rate: f64,
}

fn main() {
    let outputs = get_output_info();
    // TODO use primary
    let selected_output = String::from("DP-2");
    let output_state = OutputState::new(outputs, selected_output);

    let application = Application::new(Some("com.github.brofi.rxrandr"), Default::default())
        .expect("Failed to initialize GTK application.");

    application.connect_activate(move |app| {
        build_ui(app, &output_state);
    });

    application.run(&env::args().collect::<Vec<_>>());
}

fn build_ui(application: &Application, output_state: &Rc<OutputState>) {
    let builder = Builder::new_from_string(include_str!("gui.glade"));

    let window_name = "window";
    let window: ApplicationWindow = builder.get_object(window_name).expect(&format!(
        "Failed to get ApplicationWindow `{}`",
        window_name
    ));
    window.set_application(Some(application));

    let box_outputs_name = "box_outputs";
    let box_outputs: Box = builder
        .get_object(box_outputs_name)
        .expect(&format!("Failed to get Box `{}`", box_outputs_name));

    // create radio buttons with output name as label
    let mut output_radio_buttons = Vec::new();
    for o in output_state.outputs.values() {
        let rb = RadioButton::new_with_label(&format!("Output: {}", o.name));
        WidgetExt::set_name(&rb, o.name.as_str());
        output_radio_buttons.push(rb);
    }

    // join output radio buttons to a group and add to grid
    let mut prev_rb = NONE_RADIO_BUTTON;
    let mut it = output_radio_buttons.iter();
    while let Some(rb) = it.next() {
        rb.join_group(prev_rb);
        box_outputs.add(rb);
        prev_rb = Some(rb);
    }

    let sw_enabled_name = "sw_enabled";
    let sw_enabled: Switch = builder
        .get_object(sw_enabled_name)
        .expect(&format!("Failed to get Switch `{}`", sw_enabled_name));

    sw_enabled.connect_state_set({
        let output_state = Rc::clone(output_state);
        move |sw, state| on_enabled_changed(sw, state, &output_state)
    });

    let cb_refresh_rate_name = "cb_refresh_rate";
    let cb_refresh_rate: ComboBoxText = builder.get_object(cb_refresh_rate_name).expect(&format!(
        "Failed to get ComboBox `{}`",
        cb_refresh_rate_name
    ));

    cb_refresh_rate.connect_changed({
        let output_state = Rc::clone(output_state);
        move |cb| {
            on_refresh_rate_changed(cb, &output_state);
        }
    });

    cb_refresh_rate.set_id_column(0);

    let cb_resolution_name = "cb_resolution";
    let cb_resolution: ComboBoxText = builder
        .get_object(cb_resolution_name)
        .expect(&format!("Failed to get ComboBox `{}`", cb_resolution_name));

    cb_resolution.connect_changed({
        let output_state = Rc::clone(output_state);
        move |cb| {
            on_resolution_changed(cb, &output_state, &cb_refresh_rate);
        }
    });

    cb_resolution.set_id_column(0);

    for rb in &output_radio_buttons {
        rb.connect_toggled({
            let output_state = Rc::clone(output_state);
            let cb_resolution = cb_resolution.clone();
            let sw_enabled = sw_enabled.clone();
            move |rb| {
                on_output_selected(rb, &output_state, &cb_resolution, &sw_enabled);
            }
        });
    }

    // TODO set to primary
    if let Some(rb) = output_radio_buttons.get(0) {
        rb.set_active(true);
    }

    window.show_all();
}

fn on_output_selected(
    rb: &RadioButton,
    output_state: &OutputState,
    cb_resolution: &ComboBoxText,
    sw_enabled: &Switch,
) {
    if rb.get_active() {
        if let Some(name) = WidgetExt::get_name(rb) {
            if let Some(o) = output_state.outputs.get(name.as_str()) {
                if let Ok(mut key_selected) = output_state.key_selected.try_borrow_mut() {
                    *key_selected = name.as_str().to_string();
                    println!("Selected output key changed to: {:?}", key_selected);
                } else {
                    println!("borrow_mut in on_output_selected failed.");
                }

                cb_resolution.remove_all();
                for m in o.modes.keys() {
                    cb_resolution.append_text(m);
                }

                // Trying hard to let new_conf go out of scope.
                let mut active_resolution: String = String::new();
                let mut enabled = false;
                if let Ok(new_conf) = o.new_conf.try_borrow() {
                    active_resolution = new_conf.resolution.to_owned();
                    enabled = new_conf.enabled;
                } else {
                    println!("borrow in on_output_selected failed.");
                }
                sw_enabled.set_active(enabled);
                cb_resolution.set_active_id(Some(active_resolution.as_str()));
            }
        }
    }
}

fn on_enabled_changed(_sw: &Switch, state: bool, output_state: &OutputState) -> Inhibit {
    if let Ok(key_selected) = output_state.key_selected.try_borrow() {
        let selected_output = output_state.outputs.get(key_selected.as_str()).unwrap();
        if let Ok(mut new_conf) = selected_output.new_conf.try_borrow_mut() {
            new_conf.enabled = state;
        } else {
            println!("borrow_mut in on_enabled_changed failed.");
        }
    } else {
        println!("borrow in on_enabled_changed failed.");
    }
    // Let default handler run to keep state in sync with active property.
    Inhibit(false)
}

fn on_resolution_changed(
    cb: &ComboBoxText,
    output_state: &OutputState,
    cb_refresh_rate: &ComboBoxText,
) {
    if let Some(resolution) = cb.get_active_text() {
        if let Ok(key_selected) = output_state.key_selected.try_borrow() {
            let selected_output = output_state.outputs.get(key_selected.as_str()).unwrap();

            let mut active_refresh_rate = 0.0;
            if let Ok(mut new_conf) = selected_output.new_conf.try_borrow_mut() {
                new_conf.resolution = resolution.to_string();
                println!("Selected output with new resolution: {:?}", selected_output);

                if let Some(rrs) = selected_output.modes.get(resolution.as_str()) {
                    cb_refresh_rate.remove_all();
                    for r in rrs {
                        cb_refresh_rate.append_text(format!("{:2}", r).as_str());
                    }
                    active_refresh_rate = new_conf.refresh_rate.to_owned();
                }
            } else {
                println!("borrow_mut in on_resolution_changed failed.");
            }
            cb_refresh_rate.set_active_id(Some(active_refresh_rate.to_string().as_str()));
        } else {
            println!("borrow in on_resolution_changed failed.");
        }
    } else {
        cb_refresh_rate.remove_all();
    }
}

fn on_refresh_rate_changed(cb: &ComboBoxText, output_state: &OutputState) {
    if let Some(refresh_rate) = cb.get_active_text() {
        if let Ok(key_selected) = output_state.key_selected.try_borrow() {
            let selected_output = output_state.outputs.get(key_selected.as_str()).unwrap();
            if let Ok(mut new_conf) = selected_output.new_conf.try_borrow_mut() {
                new_conf.refresh_rate = refresh_rate.parse().unwrap();
            } else {
                println!("borrow_mut in on_refresh_rate_changed failed.");
            }
            println!(
                "Selected output with new refresh rate: {:?}",
                selected_output
            );
        } else {
            println!("borrow in on_refresh_rate_changed failed.");
        }
    }
}

fn get_output_info() -> HashMap<String, Output> {
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

    let mut output_info: HashMap<String, Output> = HashMap::new();

    unsafe {
        let dpy: *mut Display = XOpenDisplay(null());

        if dpy.is_null() {
            panic!("Failed to open display.");
        }

        let res: *mut XRRScreenResources = get_screen_res(&mut *dpy);
        let outputs: Vec<RROutput> = get_as_vec((*res).outputs, (*res).noutput);
        for o in outputs {
            let x_output_info: *mut XRROutputInfo = XRRGetOutputInfo(dpy, res, o);

            if !is_connected((*x_output_info).connection) {
                continue;
            }

            let name: &str = CStr::from_ptr((*x_output_info).name).to_str().unwrap();
            let enabled: bool = is_output_enabled(&mut *res, (*x_output_info).crtc);
            let modes: HashMap<String, Vec<f64>> = HashMap::new();
            let mode_info: Vec<XRRModeInfo> =
                get_mode_info_for_output(&mut *res, &mut *x_output_info);

            // TODO
            let curr_res = "";
            let curr_refresh_rate = 0.0;

            let curr_conf = OutputConfig {
                enabled,
                resolution: curr_res.into(),
                refresh_rate: curr_refresh_rate,
            };
            let new_conf = RefCell::new(curr_conf.clone());
            let mut output = {
                let name = name.to_owned();
                Output {
                    xid: o,
                    name,
                    modes,
                    curr_conf,
                    new_conf,
                }
            };
            for mode_i in mode_info {
                let mode_name = CStr::from_ptr(mode_i.name).to_str().unwrap().to_owned();
                output.add_mode(mode_name, get_refresh_rate(mode_i));
            }
            output_info.insert(name.to_owned(), output);
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

fn get_as_vec<T: Clone>(array: *const T, len: i32) -> Vec<T> {
    assert!(!array.is_null());
    assert!(len >= 0);
    unsafe { slice::from_raw_parts(array, len as usize) }.to_vec()
}

fn get_mode_info_for_output(
    res: &mut XRRScreenResources,
    output_info: &mut XRROutputInfo,
) -> Vec<XRRModeInfo> {
    let mut mode_info_for_output: Vec<XRRModeInfo> = Vec::new();
    let mode_ids_for_output: Vec<RRMode> = get_as_vec(output_info.modes, output_info.nmode);
    let mode_info: Vec<XRRModeInfo> = get_as_vec(res.modes, res.nmode);
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
