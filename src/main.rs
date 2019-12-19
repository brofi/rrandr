use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::process::exit;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::{env, mem, slice};

use gio::prelude::*;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Builder, Button, CellRendererText, CheckButton, ComboBox,
    ComboBoxText, RadioButton, Switch, Type, NONE_RADIO_BUTTON,
};
use x11::xlib::{
    CurrentTime, Display, False, Status, True, Window, XCloseDisplay, XDefaultScreen,
    XDisplayHeight, XDisplayHeightMM, XDisplayName, XDisplayWidth, XDisplayWidthMM, XErrorEvent,
    XGetErrorText, XGrabServer, XOpenDisplay, XRootWindow, XSetErrorHandler, XSync, XSynchronize,
    XUngrabServer,
};
use x11::xrandr::{
    Connection, RRCrtc, RRMode, RROutput, RRSetConfigFailed, RRSetConfigInvalidConfigTime,
    RRSetConfigInvalidTime, RRSetConfigSuccess, RR_Connected, RR_DoubleScan, RR_Interlace,
    RR_Rotate_0, XRRCrtcInfo, XRRGetCrtcInfo, XRRGetOutputInfo, XRRGetOutputPrimary,
    XRRGetScreenResourcesCurrent, XRRGetScreenSizeRange, XRRModeInfo, XRROutputInfo,
    XRRScreenResources, XRRSetCrtcConfig, XRRSetOutputPrimary, XRRSetScreenSize,
};
use x11::xrandr::{
    X_RRGetCrtcInfo, X_RRGetOutputInfo, X_RRGetOutputPrimary, X_RRGetScreenResourcesCurrent,
    X_RRSetCrtcConfig, X_RRSetOutputPrimary, X_RRSetScreenSize,
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

type OutputInfo = HashMap<String, Output>;

struct OutputState {
    outputs: OutputInfo,
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

    fn get_outputs_ordered_horizontal(&self) -> Vec<&Output> {
        let mut outputs: Vec<&Output> = self.outputs.values().collect();
        outputs.sort_by(|&o1, &o2| {
            if let (Ok(new_conf1), Ok(new_conf2)) =
                (o1.new_conf.try_borrow(), o2.new_conf.try_borrow())
            {
                return new_conf1.pos.0.cmp(&new_conf2.pos.0);
            }
            panic!("Failed to get outputs in horizontal order.");
        });
        outputs
    }

    fn get_enabled_outputs(&self) -> Vec<&Output> {
        self.outputs
            .values()
            .filter(|o| {
                if let Ok(new_conf) = o.new_conf.try_borrow() {
                    return new_conf.enabled;
                }
                false
            })
            .collect()
    }
}

// TODO Display Trait?
#[derive(Debug, Clone)]
struct Output {
    xid: u64,
    name: String,
    modes: HashMap<String, Vec<OutputMode>>,
    mode_pref: Option<OutputMode>,
    mm_size: (u64, u64),
    curr_conf: OutputConfig,
    new_conf: RefCell<OutputConfig>,
    crtc_xid: RefCell<Option<u64>>, // TODO move to config? is enabled even necessary?
    crtc_xids: Vec<u64>,
}

// TODO disabled outputs don't have a current mode nor a crtc_xid => fn disable
impl Output {
    fn new(xid: u64, name: String) -> Output {
        Output {
            xid,
            name,
            modes: HashMap::new(),
            mode_pref: None,
            mm_size: (0, 0),
            curr_conf: Default::default(),
            new_conf: RefCell::new(Default::default()),
            crtc_xid: RefCell::new(None),
            crtc_xids: Vec::new(),
        }
    }

    fn add_mode(&mut self, mode_name: String, mode: OutputMode) {
        self.modes.entry(mode_name).or_insert(Vec::new()).push(mode);
    }

    fn get_resolutions_sorted(&self) -> Vec<String> {
        let mut resolutions: Vec<String> = self.modes.keys().map(|k| k.to_owned()).collect();
        resolutions.sort_by(|a, b| match Self::get_resolution_params(a) {
            Some((wa, ha)) => match Self::get_resolution_params(b) {
                Some((wb, hb)) => wb.cmp(&wa).then(hb.cmp(&ha)),
                None => Ordering::Greater,
            },
            None => match Self::get_resolution_params(b) {
                Some(_) => Ordering::Less,
                None => a.cmp(&b),
            },
        });
        resolutions
    }

    fn get_resolution_params(s: &str) -> Option<(u32, u32)> {
        let split: Vec<&str> = s.splitn(2, 'x').collect::<Vec<&str>>();
        if split.len() == 2 {
            if let Ok(w) = split[0].parse::<u32>() {
                if let Ok(h) = split[1].parse::<u32>() {
                    return Some((w, h));
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Default)]
struct OutputMode {
    xid: u64,
    name: String,
    width: u32,
    height: u32,
    refresh_rate: f64,
}

#[derive(Debug, Clone, Default)]
struct OutputConfig {
    enabled: bool,
    primary: bool,
    mode: OutputMode,
    pos: (i32, i32),
}

#[repr(u32)]
enum RefreshRateColumns {
    ModeXID,
    RefreshRate,
}

fn main() {
    let outputs: OutputInfo = get_output_info();
    let key_primary = get_primary_output_name(&outputs).expect("Failed to get primary output.");
    let output_state = OutputState::new(outputs, key_primary);

    let application = Application::new(Some("com.github.brofi.rxrandr"), Default::default())
        .expect("Failed to initialize GTK application.");

    application.connect_activate(move |app| {
        build_ui(app, &output_state);
    });

    application.run(&env::args().collect::<Vec<_>>());
}

fn build_ui(application: &Application, output_state: &Rc<OutputState>) {
    let builder = Builder::new_from_string(include_str!("gui.glade"));

    let window: ApplicationWindow = get_gtk_object(&builder, "window");
    window.set_application(Some(application));

    let box_outputs: Box = get_gtk_object(&builder, "box_outputs");

    // create radio buttons with output name as label
    let mut output_radio_buttons = Vec::new();
    for o in output_state.get_outputs_ordered_horizontal() {
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

    let sw_enabled: Switch = get_gtk_object(&builder, "sw_enabled");
    sw_enabled.connect_state_set({
        let output_state = Rc::clone(output_state);
        move |sw, state| on_enabled_changed(sw, state, &output_state)
    });

    let ch_primary: CheckButton = get_gtk_object(&builder, "ch_primary");
    if let Ok(key_selected) = output_state.key_selected.try_borrow() {
        if let Some(o) = output_state.outputs.get(key_selected.as_str()) {
            ch_primary.set_active(o.curr_conf.primary);
        }
    } else {
        println!("borrow in build_ui failed.");
    }
    ch_primary.connect_toggled({
        let output_state = Rc::clone(output_state);
        move |ch| on_primary_changed(ch, &output_state)
    });

    let cb_refresh_rate: ComboBox = get_gtk_object(&builder, "cb_refresh_rate");
    let cell = CellRendererText::new();
    cb_refresh_rate.pack_start(&cell, false);
    cb_refresh_rate.add_attribute(&cell, "text", 1);
    cb_refresh_rate.set_id_column(RefreshRateColumns::ModeXID as i32);
    cb_refresh_rate.set_entry_text_column(RefreshRateColumns::RefreshRate as i32);
    cb_refresh_rate.connect_changed({
        let output_state = Rc::clone(output_state);
        move |cb| {
            on_refresh_rate_changed(cb, &output_state);
        }
    });

    let cb_resolution: ComboBoxText = get_gtk_object(&builder, "cb_resolution");
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
            let ch_primary = ch_primary.clone();
            move |rb| {
                on_output_selected(rb, &output_state, &cb_resolution, &sw_enabled, &ch_primary);
            }
        });

        let mut rb_for_selected_output = false;
        if let Some(name) = WidgetExt::get_name(rb) {
            rb_for_selected_output = name == *output_state.key_selected.borrow();
        }
        if rb_for_selected_output {
            // The first toggle button of a group is already active and a call to `set_active`
            // wouldn't call the connected listener.
            if rb.get_active() {
                rb.toggled();
            } else {
                rb.set_active(rb_for_selected_output);
            }
        }
    }

    get_gtk_object::<Button>(&builder, "btn_cancel").connect_clicked({
        let window = window.clone();
        move |_| window.destroy()
    });

    get_gtk_object::<Button>(&builder, "btn_apply").connect_clicked({
        let output_state = Rc::clone(output_state);
        move |_| apply_new_conf(&output_state)
    });

    window.show_all();
}

fn get_gtk_object<T: IsA<gtk::Object>>(builder: &Builder, name: &str) -> T {
    builder.get_object(name).expect(&format!(
        "Failed to get {} `{}`",
        std::any::type_name::<T>(),
        name
    ))
}

fn on_output_selected(
    rb: &RadioButton,
    output_state: &OutputState,
    cb_resolution: &ComboBoxText,
    sw_enabled: &Switch,
    ch_primary: &CheckButton,
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
                for m in o.get_resolutions_sorted() {
                    cb_resolution.append_text(m.as_str());
                }

                // Trying hard to let new_conf go out of scope.
                let mut active_resolution: String = String::new();
                let mut enabled = false;
                let mut primary = false;
                if let Ok(new_conf) = o.new_conf.try_borrow() {
                    active_resolution = new_conf.mode.name.to_owned();
                    enabled = new_conf.enabled;
                    primary = new_conf.primary;
                } else {
                    println!("borrow in on_output_selected failed.");
                }
                sw_enabled.set_active(enabled);
                ch_primary.set_active(primary);
                cb_resolution.set_active_id(Some(active_resolution.as_str()));
            }
        }
    }
}

fn on_enabled_changed(_sw: &Switch, state: bool, output_state: &OutputState) -> Inhibit {
    if let Ok(key_selected) = output_state.key_selected.try_borrow() {
        let selected_output = output_state.outputs.get(key_selected.as_str()).unwrap();
        let ordered_outputs = output_state.get_outputs_ordered_horizontal();
        if let Ok(mut new_conf) = selected_output.new_conf.try_borrow_mut() {
            // TODO refactor
            if new_conf.enabled != state {
                new_conf.enabled = state;

                if state {
                    // set mode to preferred mode
                    if new_conf.mode.xid == 0 {
                        if let Some(pref) = &selected_output.mode_pref {
                            new_conf.mode = pref.clone();
                        }
                    }

                    // update output position to match the position in ordered_outputs
                    new_conf.pos.0 = ordered_outputs
                        .iter()
                        .take_while(|o| o.name != *key_selected)
                        .fold(0, |acc, o| acc + o.new_conf.borrow().mode.width as i32);

                    // update positions for outputs right of the selected one
                    for o in ordered_outputs
                        .iter()
                        .skip_while(|o| o.name != *key_selected)
                        .skip(1)
                    // TODO map()
                    {
                        if let Ok(mut other_conf) = o.new_conf.try_borrow_mut() {
                            other_conf.pos.0 += new_conf.mode.width as i32;
                            println!("Other conf after enabled changed: {:?}", other_conf);
                        } else {
                            println!("borrow_mut for other conf in on_enabled_changed failed.")
                        }
                    }
                } else {
                    new_conf.pos = (0, 0);

                    // update positions for outputs right of the selected one
                    for o in ordered_outputs
                        .iter()
                        .skip_while(|o| o.name != *key_selected)
                        .skip(1)
                    // TODO map()
                    {
                        if let Ok(mut other_conf) = o.new_conf.try_borrow_mut() {
                            if other_conf.enabled {
                                other_conf.pos.0 -= new_conf.mode.width as i32;
                            }
                            println!(
                                "Config of {} after enabled changed: {:?}",
                                o.name, other_conf
                            );
                        } else {
                            println!("borrow_mut for other conf in on_enabled_changed failed.")
                        }
                    }

                    // TODO there should be an Option<OutputMode>
                    new_conf.mode = Default::default();
                }
            }

            println!("New conf after enabled changed: {:?}", new_conf);
        } else {
            println!("borrow_mut in on_enabled_changed failed.");
        }
    } else {
        println!("borrow in on_enabled_changed failed.");
    }
    // Let default handler run to keep state in sync with active property.
    Inhibit(false)
}

fn on_primary_changed(ch: &CheckButton, output_state: &OutputState) {
    if let Ok(key_selected) = output_state.key_selected.try_borrow() {
        for (key, o) in &output_state.outputs {
            if let Ok(mut new_conf) = o.new_conf.try_borrow_mut() {
                if key == key_selected.as_str() {
                    new_conf.primary = ch.get_active();
                } else if ch.get_active() {
                    new_conf.primary = false;
                }
            } else {
                println!("borrow_mut in on_primary_changed failed.");
            }
        }
    } else {
        println!("borrow in on_primary_changed failed.");
    }
}

fn on_resolution_changed(
    cb: &ComboBoxText,
    output_state: &OutputState,
    cb_refresh_rate: &ComboBox,
) {
    let model = gtk::ListStore::new(&[Type::String, Type::String]);
    if let Some(resolution) = cb.get_active_text() {
        if let Ok(key_selected) = output_state.key_selected.try_borrow() {
            let selected_output = output_state.outputs.get(key_selected.as_str()).unwrap();

            let mut active_rr_id = 0;
            if let Ok(mut new_conf) = selected_output.new_conf.try_borrow_mut() {
                new_conf.mode.name = resolution.to_string();
                println!("New conf after resolution changed: {:?}", new_conf);

                if let Some(modes) = selected_output.modes.get(resolution.as_str()) {
                    for m in modes {
                        if m.xid == new_conf.mode.xid
                            || active_rr_id == 0 && m.xid == selected_output.curr_conf.mode.xid
                        {
                            active_rr_id = m.xid
                        }

                        model.set(
                            &model.append(),
                            &[
                                RefreshRateColumns::ModeXID as u32,
                                RefreshRateColumns::RefreshRate as u32,
                            ],
                            &[&m.xid.to_string(), &format!("{:6.2} Hz", m.refresh_rate)],
                        );
                    }

                    if active_rr_id == 0 {
                        if let Some(first) = modes.get(0) {
                            active_rr_id = first.xid;
                        }
                    }

                    cb_refresh_rate.set_model(Some(&model));
                }
            } else {
                println!("borrow_mut in on_resolution_changed failed.");
            }
            cb_refresh_rate.set_active_id(Some(active_rr_id.to_string().as_str()));
        } else {
            println!("borrow in on_resolution_changed failed.");
        }
    } else {
        // Set empty model. Unfortunately we can't set `None` to clear the model.
        cb_refresh_rate.set_model(Some(&model));
    }
}

fn on_refresh_rate_changed(cb: &ComboBox, output_state: &OutputState) {
    if let Some(active_id) = cb.get_active_id() {
        if let Ok(key_selected) = output_state.key_selected.try_borrow() {
            let selected_output = output_state.outputs.get(key_selected.as_str()).unwrap();
            let ordered_outputs = output_state.get_outputs_ordered_horizontal();
            if let Ok(mut new_conf) = selected_output.new_conf.try_borrow_mut() {
                if let Some(modes) = selected_output.modes.get(new_conf.mode.name.as_str()) {
                    let refresh_rate_id = active_id.parse().unwrap();
                    for m in modes {
                        if m.xid == refresh_rate_id {
                            new_conf.mode.xid = m.xid;
                            new_conf.mode.refresh_rate = m.refresh_rate;

                            // TODO move positioning somewhere else
                            if m.width != new_conf.mode.width {
                                for o in ordered_outputs
                                    .iter()
                                    .skip_while(|o| o.name != *key_selected)
                                    .skip(1)
                                // TODO map()
                                {
                                    if let Ok(mut other_conf) = o.new_conf.try_borrow_mut() {
                                        other_conf.pos.0 -=
                                            new_conf.mode.width as i32 - m.width as i32;
                                    } else {
                                        println!("borrow_mut for other conf in on_refresh_rate_changed failed.")
                                    }
                                }
                            }

                            new_conf.mode.width = m.width;
                            new_conf.mode.height = m.height;
                            println!("New conf after refresh rate changed: {:?}", new_conf);
                            break;
                        }
                    }
                }
            } else {
                println!("borrow_mut in on_refresh_rate_changed failed.");
            }
        } else {
            println!("borrow in on_refresh_rate_changed failed.");
        }
    }
}

fn apply_new_conf(output_state: &OutputState) {
    let dpy = get_display();
    let screen = get_screen(dpy);
    let root: Window = get_window(dpy, screen);
    let res = get_resources(dpy, root);

    #[allow(unused_assignments)]
    let mut previous_handler = None;
    unsafe {
        previous_handler = XSetErrorHandler(Some(x_error_handler));
        XGrabServer(dpy);
        XSynchronize(dpy, True);
    }

    let screen_size = get_screen_size(dpy, screen, root, &output_state);

    for o in output_state.outputs.values() {
        if let (Ok(mut new_conf), Ok(mut crtc_xid)) =
            (o.new_conf.try_borrow_mut(), o.crtc_xid.try_borrow_mut())
        {
            if let Some(crtc) = *crtc_xid {
                if !new_conf.enabled {
                    // Disable outputs that were enabled before
                    let s: Status = disable_crtc(dpy, res, crtc);
                    print_config_status(o.name.clone(), s);
                    // TODO see above: disable
                    new_conf.enabled = false;
                    new_conf.pos = (0, 0);
                    new_conf.mode = Default::default();
                    *crtc_xid = None;
                } else {
                    // Disable outputs that are still enabled but don't fit the new screen size.
                    if o.curr_conf.pos.0 as u32 + o.curr_conf.mode.width > screen_size.0 as u32
                        || o.curr_conf.pos.1 as u32 + o.curr_conf.mode.height > screen_size.1 as u32
                    {
                        let s: Status = disable_crtc(dpy, res, crtc);
                        print_config_status(o.name.clone(), s);
                        *crtc_xid = None;
                    }
                }
            }
        } else {
            println!("borrow in apply_new_conf failed.");
        }
    }

    set_screen_size(dpy, screen, root, screen_size);

    for o in output_state.outputs.values() {
        if let (Ok(new_conf), Ok(mut crtc_xid)) =
            (o.new_conf.try_borrow(), o.crtc_xid.try_borrow_mut())
        {
            if new_conf.enabled {
                if let Some(crtc) = *crtc_xid {
                    let s: Status = modify_crtc(dpy, res, crtc, &new_conf.clone());
                    print_config_status(o.name.clone(), s);
                } else {
                    if let Some(crtc) = get_empty_crtc(dpy, res, o) {
                        // TODO check if this works
                        *crtc_xid = Some(crtc);
                        let s: Status = enable_crtc(dpy, res, crtc, o, &new_conf.clone());
                        print_config_status(o.name.clone(), s);
                    } else {
                        println!("Failed to find available CRTC.");
                    }
                }
                set_panning();
            }
        } else {
            println!("borrow in apply_new_conf failed.")
        }
    }

    set_primary_output(dpy, root, &output_state);

    unsafe {
        XUngrabServer(dpy);
        XSynchronize(dpy, False);
        XSync(dpy, False);
        XSetErrorHandler(mem::transmute(previous_handler));
    }

    close_display(dpy);

    // TODO if successful: curr_conf should be new_conf
    // TODO make sure state is correct so we don't have to restart the application
    // TODO revert if not successful
}

#[allow(non_upper_case_globals)]
fn print_config_status(output_name: String, status: Status) {
    println!(
        "Applying new configuration for output {} {}",
        output_name,
        match status {
            RRSetConfigSuccess => "successful.",
            RRSetConfigFailed => "failed.",
            RRSetConfigInvalidTime => "failed: invalid time.",
            RRSetConfigInvalidConfigTime => "failed: invalid config time.",
            _ => "failed: unknown.",
        }
    );
}

fn set_screen_size(
    dpy: *mut Display,
    screen: i32,
    window: Window,
    screen_size: (i32, i32, i32, i32),
) {
    let width = screen_size.0;
    let height = screen_size.1;
    let mm_width = screen_size.2;
    let mm_height = screen_size.3;

    unsafe {
        if width == XDisplayWidth(dpy, screen)
            && height == XDisplayHeight(dpy, screen)
            && mm_width == XDisplayWidthMM(dpy, screen)
            && mm_height == XDisplayHeightMM(dpy, screen)
        {
            return;
        }

        println!(
            "Setting screen size to (width,height,mmWidth,mmHeight) = ({},{},{},{})",
            width, height, mm_width, mm_height
        );
        XRRSetScreenSize(dpy, window, width, height, mm_width, mm_height);
    }
}

fn get_screen_size(
    dpy: *mut Display,
    screen: i32,
    window: Window,
    output_state: &OutputState,
) -> (i32, i32, i32, i32) {
    let mut width: i32 = 0;
    let mut height: i32 = 0;

    // TODO merge
    // Calculate width and height from modes
    for o in output_state.outputs.values() {
        if let Ok(new_conf) = o.new_conf.try_borrow() {
            width += new_conf.mode.width as i32;
            if new_conf.mode.height as i32 > height {
                height = new_conf.mode.height as i32;
            }
        } else {
            println!("borrow in get_screen_size failed.");
        }
    }

    // TODO merge
    // Check if outputs fit the calculated size
    for o in output_state.outputs.values() {
        let new_conf = o
            .new_conf
            .try_borrow()
            .expect("Failed to obtain new configuration.");

        if new_conf.pos.0 as u32 + new_conf.mode.width > width as u32
            || new_conf.pos.1 as u32 + new_conf.mode.height > height as u32
        {
            panic!(
                "Output {} at position ({}, {}) with mode size ({}, {}) exceeds calculated screen boundaries ({}, {})",
                o.name, new_conf.pos.0, new_conf.pos.1, new_conf.mode.width, new_conf.mode.height, width, height);
        }
    }

    // Check if width and height are within range
    let bounds = get_screen_size_range(dpy, window);
    if width < bounds.0 || height < bounds.1 {
        panic!(
            "Screen size must be bigger than ({},{})",
            bounds.0, bounds.1
        );
    }
    if width > bounds.2 || height > bounds.3 {
        panic!(
            "Screen size must be smaller than ({},{})",
            bounds.2, bounds.3
        );
    }

    let mut dpi = 0.0;
    // TODO const
    let mm_per_inch = 25.4;

    let mut mm_width = 0;
    let mut mm_height = 0;

    // TODO maybe only if explicitly specified by the user
    let enabled_outputs = output_state.get_enabled_outputs();
    if enabled_outputs.len() == 1 {
        let &single_output = enabled_outputs.get(0).unwrap();
        if let Ok(new_conf) = single_output.new_conf.try_borrow() {
            if width as u32 == new_conf.mode.width && height as u32 == new_conf.mode.height {
                mm_width = single_output.mm_size.0 as i32;
                mm_height = single_output.mm_size.1 as i32;
                println!(
                    "Using mm_size {:?} of output {}",
                    single_output.mm_size, single_output.name
                );
            } else {
                dpi = (mm_per_inch * new_conf.mode.height as f64) / single_output.mm_size.1 as f64;
            }
        }
    }

    // TODO check
    unsafe {
        if mm_width == 0 || mm_height == 0 {
            if width != XDisplayWidth(dpy, screen)
                || height != XDisplayHeight(dpy, screen)
                || dpi != 0.0
            {
                if dpi <= 0.0 {
                    dpi = (mm_per_inch * XDisplayHeight(dpy, screen) as f64)
                        / XDisplayHeightMM(dpy, screen) as f64;
                }
                mm_width = ((mm_per_inch * width as f64) / dpi) as i32;
                mm_height = ((mm_per_inch * height as f64) / dpi) as i32;
                println!(
                    "Calculated mm_size ({}, {}) with display dpi = {}",
                    mm_width, mm_height, dpi
                );
            } else {
                mm_width = XDisplayHeightMM(dpy, screen);
                mm_height = XDisplayHeightMM(dpy, screen);
                println!("Using display mm_size ({}, {})", mm_width, mm_height);
            }
        }
    }

    (width, height, mm_width, mm_height)
}

fn get_screen_size_range(dpy: *mut Display, window: Window) -> (i32, i32, i32, i32) {
    let mut min_width = 0;
    let mut min_height = 0;
    let mut max_width = 0;
    let mut max_height = 0;
    unsafe {
        XRRGetScreenSizeRange(
            dpy,
            window,
            &mut min_width,
            &mut min_height,
            &mut max_width,
            &mut max_height,
        );
    }
    (min_width, min_height, max_width, max_height)
}

// TODO explicitly disable panning?
fn set_panning() {}

fn set_primary_output(dpy: *mut Display, window: Window, output_state: &OutputState) {
    unsafe {
        if let Some(primary) = get_primary_output_xid(&output_state.outputs) {
            XRRSetOutputPrimary(dpy, window, primary);
        } else {
            XRRSetOutputPrimary(dpy, window, 0);
        }
    }
}

fn get_primary_output_xid(output_info: &OutputInfo) -> Option<u64> {
    if let Some(primary) = get_primary_output(output_info) {
        return Some(primary.xid);
    }
    None
}

fn get_primary_output_name(output_info: &OutputInfo) -> Option<String> {
    if let Some(primary) = get_primary_output(output_info) {
        return Some(primary.name);
    }
    None
}

fn get_primary_output(output_info: &OutputInfo) -> Option<Output> {
    for o in output_info.values() {
        if o.new_conf.borrow().primary {
            return Some(o.clone());
        }
    }
    None
}

fn modify_crtc(
    dpy: *mut Display,
    resources: *mut XRRScreenResources,
    crtc: u64,
    config: &OutputConfig,
) -> Status {
    unsafe {
        let crtc_info = XRRGetCrtcInfo(dpy, resources, crtc);
        XRRSetCrtcConfig(
            dpy,
            resources,
            crtc,
            CurrentTime,
            config.pos.0,
            config.pos.1,
            config.mode.xid,
            (*crtc_info).rotation,
            (*crtc_info).outputs,
            (*crtc_info).noutput,
        )
    }
}

fn enable_crtc(
    dpy: *mut Display,
    resources: *mut XRRScreenResources,
    crtc: u64,
    output: &Output,
    config: &OutputConfig,
) -> Status {
    unsafe {
        // TODO does x need ownership?
        // https://stackoverflow.com/questions/39224904/how-to-expose-a-rust-vect-to-ffi
        let mut outputs: Vec<RROutput> = Vec::new();
        outputs.push(output.xid);
        outputs.shrink_to_fit();
        assert_eq!(outputs.len(), outputs.capacity());
        let outputs_ptr = outputs.as_mut_ptr();
        let outputs_len = outputs.len() as i32;
        mem::forget(outputs);

        println!("Enabling CRTC {}", crtc);

        XRRSetCrtcConfig(
            dpy,
            resources,
            crtc,
            CurrentTime,
            config.pos.0,
            config.pos.1,
            config.mode.xid,
            RR_Rotate_0 as u16,
            outputs_ptr,
            outputs_len,
        )
    }
}

fn disable_crtc(dpy: *mut Display, resources: *mut XRRScreenResources, crtc: u64) -> Status {
    println!("Disabling CRTC {}", crtc);
    unsafe {
        XRRSetCrtcConfig(
            dpy,
            resources,
            crtc,
            CurrentTime,
            0,
            0,
            0,
            RR_Rotate_0 as u16,
            null_mut(),
            0,
        )
    }
}

fn get_empty_crtc(dpy: *mut Display, res: *mut XRRScreenResources, output: &Output) -> Option<u64> {
    for crtc in &output.crtc_xids {
        println!("Looking up CRTC {} for Output {}", crtc, output.xid);
        unsafe {
            let crtc_info = XRRGetCrtcInfo(dpy, res, *crtc);
            if (*crtc_info).noutput == 0 {
                println!("Returning CRTC {}", *crtc);
                return Some(*crtc);
            } else {
                println!(
                    "CRTC {} has {} outputs. First: {}",
                    crtc,
                    (*crtc_info).noutput,
                    *(*crtc_info).outputs.offset(0)
                );
            }
        }
    }
    None
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

    let dpy = get_display();
    let root: Window = get_window(dpy, get_screen(dpy));

    unsafe {
        let res = get_resources(dpy, root);
        let primary: RROutput = XRRGetOutputPrimary(dpy, root);
        let outputs: Vec<RROutput> = get_as_vec((*res).outputs, (*res).noutput);
        for o in outputs {
            let x_output_info: *mut XRROutputInfo = XRRGetOutputInfo(dpy, res, o);

            if !is_connected((*x_output_info).connection) {
                continue;
            }

            let name: String = from_x_string((*x_output_info).name);
            let mut output = Output::new(o, name.clone());
            output.mm_size = ((*x_output_info).mm_width, (*x_output_info).mm_height);

            let mut maybe_crtc_info: Option<*mut XRRCrtcInfo> = None;
            let enabled = is_output_enabled(&mut *res, (*x_output_info).crtc);
            output.curr_conf.enabled = enabled;
            output.curr_conf.primary = o == primary;
            // TODO should be if (*x_output_info).crtc != 0 (None)
            if enabled {
                // Otherwise we pass an invalid Crtc to XRRGetCrtcInfo.
                let crtc_info = XRRGetCrtcInfo(dpy, res, (*x_output_info).crtc);
                maybe_crtc_info = Some(crtc_info);
                output.crtc_xid = RefCell::new(Some((*x_output_info).crtc));
                output.curr_conf.pos = ((*crtc_info).x, (*crtc_info).y);
            }
            output.crtc_xids = get_as_vec((*x_output_info).crtcs, (*x_output_info).ncrtc);
            let mode_info: Vec<XRRModeInfo> =
                get_mode_info_for_output(&mut *res, &mut *x_output_info);

            for (i, mode_i) in mode_info.iter().enumerate() {
                let mut mode: OutputMode = Default::default();
                mode.xid = mode_i.id;
                mode.name = from_x_string(mode_i.name);
                mode.refresh_rate = get_refresh_rate(mode_i);
                mode.width = mode_i.width;
                mode.height = mode_i.height;

                output.add_mode(mode.name.clone(), mode.clone());

                // Get current mode.
                if let Some(crtc_info) = maybe_crtc_info {
                    if mode_i.id == (*crtc_info).mode {
                        output.curr_conf.mode = mode.clone();
                    }
                }

                // Get preferred mode.
                if i < (*x_output_info).npreferred as usize && output.mode_pref.is_none() {
                    output.mode_pref = Some(mode.clone());
                }

                output.new_conf = RefCell::new(output.curr_conf.clone());
            }
            output_info.insert(name, output);
        }

        close_display(dpy);
    }

    output_info
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

fn get_refresh_rate(mode_info: &XRRModeInfo) -> f64 {
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

fn get_display() -> *mut Display {
    unsafe {
        let dpy_name: *const c_char = null();
        let dpy: *mut Display = XOpenDisplay(dpy_name);
        if dpy.is_null() {
            panic!(
                "Failed to open display {}",
                from_x_string(XDisplayName(dpy_name))
            );
        }
        dpy
    }
}

fn get_screen(dpy: *mut Display) -> i32 {
    unsafe { XDefaultScreen(dpy) }
}

fn get_window(dpy: *mut Display, screen: i32) -> Window {
    unsafe { XRootWindow(dpy, screen) }
}

fn get_resources(dpy: *mut Display, window: Window) -> *mut XRRScreenResources {
    unsafe { XRRGetScreenResourcesCurrent(dpy, window) }
}

fn close_display(dpy: *mut Display) -> i32 {
    unsafe { XCloseDisplay(dpy) }
}

fn from_x_string(ptr: *const c_char) -> String {
    assert!(!ptr.is_null());
    unsafe { String::from_utf8_lossy(CStr::from_ptr(ptr).to_bytes()).into_owned() }
}

unsafe extern "C" fn x_error_handler(dpy: *mut Display, event: *mut XErrorEvent) -> i32 {
    print_x_error(dpy, "X Error", event);
    exit(1);
}

#[allow(non_upper_case_globals)]
unsafe fn print_x_error(dpy: *mut Display, prefix: &str, event: *mut XErrorEvent) {
    let mut error_text = [0i8; 2048];
    XGetErrorText(
        dpy,
        (*event).error_code as i32,
        error_text.as_mut_ptr(),
        error_text.len() as i32,
    );
    print!("{}: {}", prefix, from_x_string(error_text.as_ptr()));

    // TODO the 140 is dynamically assigned, so it is different on each system.
    // see https://www.x.org/wiki/Development/Documentation/Protocol/OpCodes/
    if (*event).request_code as i32 == 140 {
        println!(
            " in {}.",
            match (*event).minor_code as i32 {
                X_RRGetCrtcInfo => "RRGetCrtcInfo",
                X_RRGetOutputInfo => "RRGetOutputInfo",
                X_RRGetOutputPrimary => "RRGetOutputPrimary",
                X_RRGetScreenResourcesCurrent => "RRGetScreenResourcesCurrent",
                X_RRSetCrtcConfig => "RRSetCrtcConfig",
                X_RRSetOutputPrimary => "RRSetOutputPrimary",
                X_RRSetScreenSize => "RRSetScreenSize",
                _ => "unknown minor opcode",
            }
        );
    } else {
        println!();
    }
}