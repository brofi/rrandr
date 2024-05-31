#![warn(clippy::pedantic)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
// #![warn(clippy::restriction)]

mod cairo_surface;
mod math;
mod view;

use core::fmt;
use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};

use cairo::ffi::cairo_device_finish;
use cairo::XCBDrawable;
use cairo_surface::XCBSurfaceS;
use gtk::glib::ExitCode;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use math::Rect;
use pango::{Alignment, FontDescription, Weight};
use pangocairo::functions::{create_layout, show_layout};
use view::View;
use x11rb::connection::{Connection as XConnection, RequestConnection};
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_info, get_output_primary, get_output_property,
    get_screen_resources_current, get_screen_size_range, set_crtc_config, set_output_primary,
    set_screen_size, Connection, Crtc as CrtcId, GetCrtcInfoReply, GetOutputInfoReply,
    GetScreenResourcesCurrentReply, GetScreenSizeRangeReply, Mode as ModeId, ModeFlag, ModeInfo,
    Output as OutputId, Rotation, ScreenSize, SetConfig,
};
use x11rb::protocol::xproto::{
    intern_atom, AtomEnum, ConnectionExt, CreateWindowAux, EventMask, Screen, Visualtype, Window,
    WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::x11_utils::{Serialize, X11Error};
use x11rb::xcb_ffi::XCBConnection;
use x11rb::CURRENT_TIME;

use crate::view::{COLOR_BG0, COLOR_FG};

type ScreenSizeRange = GetScreenSizeRangeReply;
type ScreenResources = GetScreenResourcesCurrentReply;
type OutputInfo = GetOutputInfoReply;
type CrtcInfo = GetCrtcInfoReply;
type Resolution = [u16; 2];

const APP_ID: &str = "com.github.brofi.rrandr";
const RESOLUTION_JOIN_CHAR: char = 'x';
const MM_PER_INCH: f32 = 25.4;
const PPI_DEFAULT: u8 = 96;

const POPUP_WINDOW_PAD: f64 = 20.;
const POPUP_OUTPUT_RATIO: f64 = 1. / 8.;
const POPUP_SHOW_SECS: f32 = 2.5;

#[derive(Clone, Debug)]
pub struct Output {
    id: OutputId,
    name: String,
    product_name: Option<String>,
    enabled: bool,
    primary: bool,
    pos: Option<(i16, i16)>,
    mode: Option<Mode>,
    modes: Vec<Mode>,
    dim: [u32; 2],
}

impl Output {
    fn ppi(&self) -> f64 {
        if let Some(mode) = self.mode.as_ref() {
            if self.dim[1] > 0 {
                return (f64::from(MM_PER_INCH) * f64::from(mode.height)) / f64::from(self.dim[1]);
            }
        }
        f64::from(PPI_DEFAULT)
    }

    fn get_resolutions_dropdown(&self) -> Vec<String> {
        self.get_resolutions().iter().map(|&r| Self::resolution_str(r)).collect::<Vec<String>>()
    }

    fn get_current_resolution_dropdown_index(&self) -> Option<usize> {
        if let Some(mode) = self.mode.as_ref() {
            return self
                .get_resolutions()
                .iter()
                .position(|res: &Resolution| res[0] == mode.width && res[1] == mode.height)?
                .into();
        }
        None
    }

    fn resolution_dropdown_mode_index(&self, index: usize) -> usize {
        let res = self.get_resolutions()[index];
        self.modes.iter().position(|m| m.width == res[0] && m.height == res[1]).unwrap()
    }

    fn refresh_rate_dropdown_mode_index(&self, resolution_index: usize, index: usize) -> usize {
        let res = self.get_resolutions()[resolution_index];
        let refresh = self.get_refresh_rates(resolution_index)[index];
        self.modes
            .iter()
            .position(|m| m.width == res[0] && m.height == res[1] && nearly_eq(m.refresh, refresh))
            .unwrap()
    }

    fn get_current_refresh_rate_dropdown_index(&self, resolution_index: usize) -> Option<usize> {
        if let Some(mode) = self.mode.as_ref() {
            return self
                .get_refresh_rates(resolution_index)
                .iter()
                .position(|&refresh| nearly_eq(refresh, mode.refresh))?
                .into();
        }
        None
    }

    fn get_refresh_rates_dropdown(&self, resolution_index: usize) -> Vec<String> {
        self.get_refresh_rates(resolution_index)
            .iter()
            .map(|&r| Self::refresh_str(r))
            .collect::<Vec<String>>()
    }

    fn get_resolutions(&self) -> Vec<Resolution> {
        let mut dd_list = Vec::new();
        for mode in &self.modes {
            let r = [mode.width, mode.height];
            if !dd_list.contains(&r) {
                dd_list.push(r);
            }
        }
        dd_list
    }

    fn get_refresh_rates(&self, resolution_index: usize) -> Vec<f64> {
        let res = self.get_resolutions()[resolution_index];
        self.modes
            .iter()
            .filter(|m| m.width == res[0] && m.height == res[1])
            .map(|m| m.refresh)
            .collect::<Vec<f64>>()
    }

    fn resolution_str(res: Resolution) -> String {
        format!("{}{RESOLUTION_JOIN_CHAR}{}", res[0], res[1])
    }

    fn refresh_str(refresh: f64) -> String { format!("{refresh:6.2} Hz") }

    fn rect(&self) -> Rect {
        if let (Some((x, y)), Some(mode)) = (self.pos, self.mode.as_ref()) {
            return Rect::new(x, y, mode.width, mode.height);
        };
        Rect::default()
    }
}

#[derive(Clone, Debug)]
struct Mode {
    id: ModeId,
    width: u16,
    height: u16,
    refresh: f64,
}

impl From<ModeInfo> for Mode {
    fn from(mode_info: ModeInfo) -> Self {
        Self {
            id: mode_info.id,
            width: mode_info.width,
            height: mode_info.height,
            refresh: get_refresh_rate(&mode_info),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}_{:.2}", self.width, self.height, self.refresh)
    }
}

fn get_bounds(outputs: &[Output]) -> Rect {
    Rect::bounds(outputs.iter().filter(|&o| o.enabled).map(Output::rect).collect::<Vec<_>>())
}

fn main() -> ExitCode {
    let (conn, screen_num) = x11rb::connect(None).expect("connection to X Server");
    let screen = &conn.setup().roots[screen_num];
    let res = get_screen_resources_current(&conn, screen.root)
        .expect("cookie to request screen resources");
    let res = res.reply().expect("reply for screen resources");
    let primary = get_output_primary(&conn, screen.root).expect("cookie to request primary output");
    let primary = primary.reply().expect("reply for primary output");
    let screen_size_range =
        get_screen_size_range(&conn, screen.root).expect("cookie to request screen size range");
    let screen_size_range: ScreenSizeRange =
        screen_size_range.reply().expect("reply for screen size range");

    let rr_outputs = request_outputs(&conn, &res).expect("cookies to request outputs");
    let rr_crtcs = request_crtcs(&conn, &res).expect("cookies to request crtcs");

    let rr_outputs: HashMap<OutputId, OutputInfo> =
        get_outputs(rr_outputs).expect("reply for outputs");
    let rr_crtcs: HashMap<CrtcId, CrtcInfo> = get_crtcs(rr_crtcs).expect("reply for crtcs");
    let rr_modes: HashMap<ModeId, ModeInfo> = res.modes.into_iter().map(|m| (m.id, m)).collect();

    if cfg!(debug_assertions) {
        print_crtcs(&rr_crtcs, &rr_modes);
        print_outputs(&rr_outputs, &rr_modes);
    }

    let mut outputs = Vec::new();
    for (id, output_info) in &rr_outputs {
        if output_info.connection != Connection::CONNECTED {
            continue;
        }
        let enabled = output_info.crtc > 0;
        let mut mode = None;
        let mut pos = None;
        if enabled {
            let crtc_info = &rr_crtcs[&output_info.crtc];
            mode = Some(Mode::from(rr_modes[&crtc_info.mode]));
            pos = Some((crtc_info.x, crtc_info.y));
        }
        outputs.push(Output {
            id: *id,
            name: String::from_utf8_lossy(&output_info.name).into_owned(),
            product_name: get_monitor_name(&conn, *id),
            enabled,
            primary: *id == primary.output,
            pos,
            mode,
            modes: get_modes_for_output(output_info, &rr_modes),
            dim: [output_info.mm_width, output_info.mm_height],
        });
    }
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        build_ui(
            app,
            outputs.clone(),
            screen_size_range,
            move |outputs| on_apply_clicked(&screen_size_range, &outputs),
            || {
                if let Err(e) = on_identify_clicked() {
                    println!("Failed to identify outputs: {e:?}");
                }
            },
        );
    });
    app.run()
}

fn build_ui(
    app: &Application,
    outputs: Vec<Output>,
    size: ScreenSizeRange,
    on_apply: impl Fn(Vec<Output>) -> bool + 'static,
    on_identify: impl Fn() + 'static,
) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(800)
        .default_height(600)
        .title("RRandR")
        .build();

    let view = View::create(outputs, size, on_apply, on_identify);
    window.set_child(Some(&view));
    window.present();
}

fn create_popup_window(
    conn: &impl XConnection,
    screen_num: usize,
    rect: &Rect,
) -> Result<Window, ReplyOrIdError> {
    let screen = &conn.setup().roots[screen_num];
    let wid = conn.generate_id()?;
    let waux = CreateWindowAux::new()
        .event_mask(EventMask::BUTTON_PRESS)
        .background_pixel(x11rb::NONE)
        .override_redirect(1);
    conn.create_window(
        screen.root_depth,
        wid,
        screen.root,
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &waux,
    )?;
    conn.map_window(wid)?;
    Ok(wid)
}

fn create_popup_surface(
    conn: &XCBConnection,
    screen_num: usize,
    wid: Window,
    width: i32,
    height: i32,
) -> Result<XCBSurfaceS, cairo::Error> {
    let cairo_conn =
        unsafe { cairo::XCBConnection::from_raw_none(conn.get_raw_xcb_connection() as _) };
    let cairo_visual = unsafe {
        cairo::XCBVisualType::from_raw_none(
            get_root_visual_type(&conn, screen_num).unwrap().serialize().as_mut_ptr() as _,
        )
    };
    XCBSurfaceS::create(&cairo_conn, &XCBDrawable(wid), &cairo_visual, width, height)
}

fn get_root_visual_type(conn: &impl XConnection, screen_num: usize) -> Option<Visualtype> {
    let screen = &conn.setup().roots[screen_num];
    for depth in &screen.allowed_depths {
        for visual in &depth.visuals {
            if visual.visual_id == screen.root_visual {
                return Some(*visual);
            }
        }
    }
    None
}

fn create_popup_windows(
    conn: &XCBConnection,
    screen_num: usize,
) -> Result<HashMap<Window, XCBSurfaceS>, Box<dyn Error>> {
    let mut windows = HashMap::new();
    let screen = &conn.setup().roots[screen_num];
    let res = get_screen_resources_current(&conn, screen.root)?.reply()?;
    let rr_modes: HashMap<ModeId, &ModeInfo> = res.modes.iter().map(|m| (m.id, m)).collect();
    let mut desc = FontDescription::new();
    desc.set_family("monospace");
    // desc.set_size(14);
    desc.set_weight(Weight::Bold);
    for output in &res.outputs {
        let output_info = get_output_info(conn, *output, res.timestamp)?.reply()?;
        if output_info.crtc > 0 {
            let crtc_info = get_crtc_info(conn, output_info.crtc, res.timestamp)?.reply()?;
            let mode = rr_modes[&crtc_info.mode];
            let width = (f64::from(mode.width) * POPUP_OUTPUT_RATIO).round() as u16;
            let height = (f64::from(mode.height) * POPUP_OUTPUT_RATIO).round() as u16;
            let x = (f64::from(crtc_info.x) + POPUP_WINDOW_PAD).round() as i16;
            let y = (f64::from(crtc_info.y) - POPUP_WINDOW_PAD + f64::from(mode.height)
                - f64::from(height))
            .round() as i16;
            let rect = Rect::new(x, y, width, height);
            let wid = create_popup_window(&conn, screen_num, &rect)?;
            let surface =
                create_popup_surface(conn, screen_num, wid, i32::from(width), i32::from(height))?;
            let cr = cairo::Context::new(&surface)?;
            draw_popup(&cr, &rect, &desc, &String::from_utf8_lossy(&output_info.name).to_string())?;
            surface.flush();
            windows.insert(wid, surface);
        }
    }
    Ok(windows)
}

fn on_identify_clicked() -> Result<(), Box<dyn Error>> {
    let (conn, screen_num) = XCBConnection::connect(None)?;
    let popups = create_popup_windows(&conn, screen_num)?;
    conn.flush()?;

    thread::spawn(move || -> Result<(), ReplyError> {
        let now = Instant::now();
        let secs = Duration::from_secs_f32(POPUP_SHOW_SECS);
        let mut result = Ok(());
        while now.elapsed() < secs {
            match conn.poll_for_event()? {
                Some(Event::ButtonPress(e)) => {
                    if e.detail == 1 {
                        break;
                    }
                }
                Some(Event::Error(e)) => {
                    println!("{}", x_error_to_string(&e));
                    result = Err(e.into());
                    break;
                }
                _ => (),
            }
        }
        for surface in popups.values() {
            unsafe { cairo_device_finish(surface.device().unwrap().to_raw_none()) };
            surface.finish();
        }
        result
    });
    Ok(())
}

fn draw_popup(
    cr: &cairo::Context,
    rect: &Rect,
    desc: &FontDescription,
    text: &str,
) -> Result<(), cairo::Error> {
    let w = f64::from(rect.width());
    let h = f64::from(rect.height());

    cr.set_source_rgba(
        f64::from(COLOR_FG.red()),
        f64::from(COLOR_FG.green()),
        f64::from(COLOR_FG.blue()),
        0.75,
    );
    cr.rectangle(0., 0., w, h);
    cr.fill()?;

    cr.set_source_color(&COLOR_BG0);
    let layout = create_layout(&cr);
    layout.set_font_description(Some(&desc));
    layout.set_alignment(Alignment::Center);
    layout.set_text(text);
    let ps = layout.pixel_size();
    cr.move_to((w - f64::from(ps.0)) / 2., (h - f64::from(ps.1)) / 2.);
    show_layout(&cr, &layout);
    Ok(())
}

fn on_apply_clicked(screen_size_range: &ScreenSizeRange, outputs: &Vec<Output>) -> bool {
    let (conn, screen_num) = x11rb::connect(None).expect("connection to X Server");
    let screen = &conn.setup().roots[screen_num];
    let res = get_screen_resources_current(&conn, screen.root)
        .expect("cookie to request screen resources");
    let res = res.reply().expect("reply for screen resources");
    let rr_outputs = request_outputs(&conn, &res).expect("cookies to request outputs");
    let rr_crtcs = request_crtcs(&conn, &res).expect("cookies to request crtcs");
    let rr_outputs: HashMap<OutputId, OutputInfo> =
        get_outputs(rr_outputs).expect("reply for outputs");
    let rr_crtcs: HashMap<CrtcId, CrtcInfo> = get_crtcs(rr_crtcs).expect("reply for crtcs");

    let primary = outputs.iter().find(|&o| o.primary);
    let screen_size = get_screen_size(screen_size_range, outputs, primary);
    let screen_size_changed = screen.width_in_pixels != screen_size.width
        || screen.height_in_pixels != screen_size.height;

    // Disable outputs
    for output in outputs {
        let crtc_id = rr_outputs[&output.id].crtc;
        if crtc_id == 0 {
            // Output already disabled
            continue;
        }
        let crtc = &rr_crtcs[&crtc_id];
        if !output.enabled
            || (screen_size_changed
                && (i32::from(crtc.x) + i32::from(crtc.width) > i32::from(screen_size.width)
                    || i32::from(crtc.y) + i32::from(crtc.height) > i32::from(screen_size.height)))
        {
            // Disable outputs that are still enabled but shouldn't be and outputs
            // that stay enabled but currently don't fit the new screen size.
            // The latter needs to be done to avoid an invalid intermediate
            // configuration when actually setting the new screen size.
            if handle_reply_error(disable_crtc(&conn, crtc_id), "disable CRTC") {
                revert(&conn, screen, &rr_crtcs);
                return false;
            }
        }
    }

    if screen_size_changed {
        println!(
            "Setting screen size to {}x{} px, {}x{} mm",
            screen_size.width, screen_size.height, screen_size.mwidth, screen_size.mheight
        );

        if handle_no_reply_error(
            set_screen_size(
                &conn,
                screen.root,
                screen_size.width,
                screen_size.height,
                screen_size.mwidth.into(),
                screen_size.mheight.into(),
            ),
            "set screen size",
        ) {
            revert(&conn, screen, &rr_crtcs);
            return false;
        }
    }

    // Update outputs
    for output in outputs {
        if !output.enabled {
            continue;
        }
        let output_info = &rr_outputs[&output.id];
        let mut crtc_id = output_info.crtc;
        if crtc_id == 0
            || rr_crtcs
                .get(&crtc_id)
                .is_some_and(|ci: &CrtcInfo| ci.outputs.len() > 1 && ci.outputs[0] != output.id)
        {
            // If this output was disabled before get it a new empty CRTC.
            // If this output is enabled, shares a CRTC with other outputs and
            // its not the first one listed, move it to a new empty CRTC.
            if let Some(empty_id) = get_valid_empty_crtc(&rr_crtcs, output.id, output_info) {
                crtc_id = empty_id;
            } else {
                revert(&conn, screen, &rr_crtcs);
                return false;
            }
        }
        if handle_reply_error(update_crtc(&conn, crtc_id, output), "update CRTC") {
            revert(&conn, screen, &rr_crtcs);
            return false;
        }
    }

    // Set primary output
    let primary_id = primary.map(|p| p.id).unwrap_or_default();
    if handle_no_reply_error(
        set_output_primary(&conn, screen.root, primary_id),
        "set primary output",
    ) {
        return false;
    }
    true
}

#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
fn get_screen_size(
    screen_size_range: &ScreenSizeRange,
    outputs: &[Output],
    primary: Option<&Output>,
) -> ScreenSize {
    let bounds = get_bounds(outputs);
    let width = screen_size_range.min_width.max(screen_size_range.max_width.min(bounds.width()));
    let height = screen_size_range.min_height.max(screen_size_range.max_width.min(bounds.height()));

    let ppi = primary.map_or(f64::from(PPI_DEFAULT), Output::ppi);

    ScreenSize {
        width,
        height,
        mwidth: ((f64::from(MM_PER_INCH) * f64::from(width)) / ppi) as u16,
        mheight: ((f64::from(MM_PER_INCH) * f64::from(height)) / ppi) as u16,
    }
}

fn update_crtc(
    conn: &RustConnection,
    crtc: CrtcId,
    output: &Output,
) -> Result<SetConfig, ReplyError> {
    let Some(pos) = output.pos else {
        println!("Output {} is missing a position.", output.name);
        return Ok(SetConfig::FAILED);
    };
    let Some(mode) = output.mode.as_ref() else {
        println!("Output {} is missing a mode.", output.name);
        return Ok(SetConfig::FAILED);
    };
    println!(
        "Trying to set output {} to CTRC {} at position +{}+{} with mode {}",
        output.name, crtc, pos.0, pos.1, mode
    );
    Ok(set_crtc_config(
        conn,
        crtc,
        CURRENT_TIME,
        CURRENT_TIME,
        pos.0,
        pos.1,
        mode.id,
        Rotation::ROTATE0,
        &[output.id],
    )?
    .reply()?
    .status)
}

fn disable_crtc(conn: &RustConnection, crtc: CrtcId) -> Result<SetConfig, ReplyError> {
    Ok(set_crtc_config(conn, crtc, CURRENT_TIME, CURRENT_TIME, 0, 0, 0, Rotation::ROTATE0, &[])?
        .reply()?
        .status)
}

fn get_valid_empty_crtc(
    rr_crtcs: &HashMap<CrtcId, CrtcInfo>,
    output_id: OutputId,
    output_info: &OutputInfo,
) -> Option<CrtcId> {
    for (crtc_id, crtc) in rr_crtcs {
        if crtc.outputs.is_empty()
            && output_info.crtcs.contains(crtc_id)
            && crtc.possible.contains(&output_id)
        {
            return Some(*crtc_id);
        }
    }
    println!("Failed to get empty CRTC for output {}", String::from_utf8_lossy(&output_info.name));
    None
}

fn handle_reply_error(result: Result<SetConfig, ReplyError>, msg: &str) -> bool {
    let mut error = true;
    match result {
        Ok(SetConfig::SUCCESS) => error = false,
        Ok(SetConfig::FAILED) => println!("Failed to {msg}."),
        Ok(status) => println!("Failed to {msg}. Cause: {status:#?}"),
        Err(ReplyError::X11Error(e)) => println!("{}", x_error_to_string(&e)),
        Err(e) => println!("Failed to {msg}. Cause: {e:?}"),
    }
    error
}

fn handle_no_reply_error(
    result: Result<VoidCookie<RustConnection>, ConnectionError>,
    msg: &str,
) -> bool {
    let mut error = true;
    match result {
        Ok(cookie) => match cookie.check() {
            Ok(()) => error = false,
            Err(ReplyError::X11Error(e)) => println!("{}", x_error_to_string(&e)),
            Err(e) => println!("Failed to {msg}. Cause: {e}"),
        },
        Err(e) => println!("Failed to request {msg}. Cause: {e}"),
    }
    error
}

fn x_error_to_string(e: &X11Error) -> String {
    format!(
        "X11 {:?} error for value {}{}.",
        e.error_kind,
        e.bad_value,
        e.request_name.map(|s| " in request ".to_owned() + s).unwrap_or_default()
    )
}

fn revert(conn: &RustConnection, screen: &Screen, rr_crtcs: &HashMap<CrtcId, CrtcInfo>) {
    println!("Reverting changes");
    for crtc_id in rr_crtcs.keys() {
        disable_crtc(conn, *crtc_id).expect("disable CRTC");
    }
    set_screen_size(
        conn,
        screen.root,
        screen.width_in_pixels,
        screen.height_in_pixels,
        screen.width_in_millimeters.into(),
        screen.height_in_millimeters.into(),
    )
    .expect("revert screen size request")
    .check()
    .expect("revert screen size");
    for (crtc_id, crtc_info) in rr_crtcs {
        set_crtc_config(
            conn,
            *crtc_id,
            CURRENT_TIME,
            CURRENT_TIME,
            crtc_info.x,
            crtc_info.y,
            crtc_info.mode,
            crtc_info.rotation,
            &crtc_info.outputs,
        )
        .expect("revert CRTC request")
        .reply()
        .expect("revert CRTC");
    }
}

// TODO checkout GetXIDListRequest
fn request_outputs<'a, Conn: RequestConnection>(
    conn: &'a Conn,
    res: &ScreenResources,
) -> Result<HashMap<OutputId, Cookie<'a, Conn, OutputInfo>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for output in &res.outputs {
        cookies.insert(*output, get_output_info(conn, *output, res.timestamp)?);
    }
    Ok(cookies)
}

fn request_crtcs<'a, Conn: RequestConnection>(
    conn: &'a Conn,
    res: &ScreenResources,
) -> Result<HashMap<CrtcId, Cookie<'a, Conn, CrtcInfo>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for crtc in &res.crtcs {
        cookies.insert(*crtc, get_crtc_info(conn, *crtc, res.timestamp)?);
    }
    Ok(cookies)
}

fn get_outputs(
    cookies: HashMap<OutputId, Cookie<impl RequestConnection, OutputInfo>>,
) -> Result<HashMap<OutputId, OutputInfo>, ReplyError> {
    let mut outputs = HashMap::new();
    for (output, c) in cookies {
        outputs.insert(output, c.reply()?);
    }
    Ok(outputs)
}

fn get_crtcs(
    cookies: HashMap<CrtcId, Cookie<impl RequestConnection, CrtcInfo>>,
) -> Result<HashMap<CrtcId, CrtcInfo>, ReplyError> {
    let mut crtcs = HashMap::new();
    for (crtc, c) in cookies {
        crtcs.insert(crtc, c.reply()?);
    }
    Ok(crtcs)
}

fn get_modes_for_output(output_info: &OutputInfo, modes: &HashMap<ModeId, ModeInfo>) -> Vec<Mode> {
    output_info.modes.iter().map(|mode_id| Mode::from(modes[mode_id])).collect::<Vec<Mode>>()
}

fn get_refresh_rate(mode_info: &ModeInfo) -> f64 {
    let mut vtotal = mode_info.vtotal;

    if mode_info.mode_flags.contains(ModeFlag::DOUBLE_SCAN) {
        vtotal *= 2;
    }
    if mode_info.mode_flags.contains(ModeFlag::INTERLACE) {
        vtotal /= 2;
    }

    if mode_info.htotal > 0 && vtotal > 0 {
        f64::from(mode_info.dot_clock) / (f64::from(mode_info.htotal) * f64::from(vtotal))
    } else {
        0.0
    }
}

fn get_edid(conn: &RustConnection, output: OutputId) -> Result<Vec<u8>, Box<dyn Error>> {
    let name = "EDID";
    let property = intern_atom(conn, true, name.as_bytes())?.reply()?.atom;
    if property == AtomEnum::NONE.into() {
        return Err(format!("No property named: {name}").into());
    }
    Ok(get_output_property(conn, output, property, AtomEnum::INTEGER, 0, 256, false, false)?
        .reply()?
        .data)
}

fn get_monitor_name(conn: &RustConnection, output: OutputId) -> Option<String> {
    if let Ok(edid) = get_edid(conn, output) {
        if edid.len() >= 128 {
            let version = edid[0x12];
            let revision = edid[0x13];
            if version == 1 && (revision == 3 || revision == 4) {
                let mut i = 0x48;
                while i <= 0x6C {
                    // This 18 byte descriptor is a used as a display descriptor
                    // with a tag 0xFC (display product name).
                    if edid[i..(i + 3)] == [0, 0, 0] && edid[i + 3] == 0xFC && edid[i + 4] == 0 {
                        return Some(
                            String::from_utf8_lossy(&edid[(i + 5)..(i + 18)]).trim_end().to_owned(),
                        );
                    }
                    i += 18;
                }
            }
        }
    }
    None
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

#[cfg(debug_assertions)]
#[allow(clippy::use_debug)]
fn print_crtcs(rr_crtcs: &HashMap<CrtcId, CrtcInfo>, rr_modes: &HashMap<ModeId, ModeInfo>) {
    for (crtc_id, crtc) in rr_crtcs {
        println!("{:-^40}", format!(" CRTC {crtc_id} "));
        println!("XID:      {crtc_id}");
        println!("Pos:      +{}+{}", crtc.x, crtc.y);
        println!("Res:      {}x{}", crtc.width, crtc.height);
        if crtc.mode > 0 {
            println!("Mode:     {}: {}", crtc.mode, Mode::from(rr_modes[&crtc.mode]));
        }
        println!("Outputs:  {:?}", crtc.outputs);
        println!("Rot:      {:#?}", crtc.rotation);
        println!("Possible: {:?}", crtc.possible);
        println!();
    }
}

#[cfg(debug_assertions)]
#[allow(clippy::use_debug)]
fn print_outputs(rr_outputs: &HashMap<OutputId, OutputInfo>, rr_modes: &HashMap<ModeId, ModeInfo>) {
    for (output_id, output) in rr_outputs {
        if output.connection == Connection::CONNECTED {
            println!("{:-^40}", format!(" Output {} ", output_id));
            println!("XID:   {output_id}");
            println!("Name:  {}", String::from_utf8_lossy(&output.name));
            println!("CRTC:  {}", output.crtc);
            println!("CRTCs: {:?}", output.crtcs);
            println!("Dim:   {}x{} mm", output.mm_width, output.mm_height);
            println!("Preferred modes:");
            for mode_id in &output.modes[0..output.num_preferred.into()] {
                println!("    {}: {}", mode_id, Mode::from(rr_modes[mode_id]));
            }
            println!("Modes:");
            for mode_id in &output.modes {
                println!("    {}: {}", mode_id, Mode::from(rr_modes[mode_id]));
            }
            println!("Clones: {:?}", output.clones);
            println!();
        }
    }
}
