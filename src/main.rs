mod view;

use core::fmt;
use gtk::glib::ExitCode;
use gtk::graphene::Rect;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::collections::HashMap;
use std::error::Error;
use x11rb::protocol::xproto::{intern_atom, AtomEnum, Screen};
use x11rb::x11_utils::X11Error;

use view::View;
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_primary, get_output_property, get_screen_size_range, set_crtc_config,
    set_output_primary, set_screen_size, Connection, Crtc as CrtcId, GetCrtcInfoReply,
    GetScreenResourcesCurrentReply, GetScreenSizeRangeReply, Mode as ModeId, ModeFlag, ModeInfo,
    Output as OutputId, Rotation, ScreenSize, SetConfig,
};

use x11rb::CURRENT_TIME;
use x11rb::{
    connection::Connection as XConnection,
    protocol::randr::{get_output_info, get_screen_resources_current, GetOutputInfoReply},
    rust_connection::RustConnection,
};

type ScreenSizeRange = GetScreenSizeRangeReply;
type OutputInfo = GetOutputInfoReply;
type CrtcInfo = GetCrtcInfoReply;
type Resolution = [u16; 2];

const APP_ID: &str = "com.github.brofi.rrandr";
const RESOLUTION_JOIN_CHAR: char = 'x';
const MM_PER_INCH: f32 = 25.4;
const PPI_DEFAULT: u8 = 96;

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
    fn ppi(&self) -> f32 {
        if let Some(mode) = &self.mode {
            if self.dim[1] > 0 {
                return (MM_PER_INCH * mode.height as f32) / self.dim[1] as f32;
            }
        }
        PPI_DEFAULT as f32
    }

    fn left(&self) -> i32 {
        self.pos.unwrap().0.into()
    }

    fn right(&self) -> i32 {
        self.left() + self.mode.as_ref().unwrap().width as i32
    }

    fn top(&self) -> i32 {
        self.pos.unwrap().1.into()
    }

    fn bottom(&self) -> i32 {
        self.top() + self.mode.as_ref().unwrap().height as i32
    }

    // fn is_above(&self, other: &Output) -> bool {
    //     self.bottom() <= other.top()
    // }

    // fn is_below(&self, other: &Output) -> bool {
    //     self.top() >= other.bottom()
    // }

    // fn is_left_of(&self, other: &Output) -> bool {
    //     self.right() <= other.left()
    // }

    // fn is_right_of(&self, other: &Output) -> bool {
    //     self.left() >= other.right()
    // }

    fn get_resolutions_dropdown(&self) -> Vec<String> {
        self.get_resolutions()
            .iter()
            .map(Self::resolution_str)
            .collect::<Vec<String>>()
    }

    fn get_current_resolution_dropdown_index(&self) -> Option<usize> {
        if let Some(mode) = &self.mode {
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
        self.modes
            .iter()
            .position(|m| m.width == res[0] && m.height == res[1])
            .unwrap()
    }

    fn refresh_rate_dropdown_mode_index(&self, resolution_index: usize, index: usize) -> usize {
        let res = self.get_resolutions()[resolution_index];
        let refresh = self.get_refresh_rates(resolution_index)[index];
        self.modes
            .iter()
            .position(|m| m.width == res[0] && m.height == res[1] && m.refresh == refresh)
            .unwrap()
    }

    fn get_current_refresh_rate_dropdown_index(&self, resolution_index: usize) -> Option<usize> {
        if let Some(mode) = &self.mode {
            return self
                .get_refresh_rates(resolution_index)
                .iter()
                .position(|&refresh| refresh == mode.refresh)?
                .into();
        }
        None
    }

    fn get_refresh_rates_dropdown(&self, resolution_index: usize) -> Vec<String> {
        self.get_refresh_rates(resolution_index)
            .iter()
            .map(Self::refresh_str)
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

    fn resolution_str(res: &Resolution) -> String {
        format!("{}{RESOLUTION_JOIN_CHAR}{}", res[0], res[1])
    }

    fn refresh_str(refresh: &f64) -> String {
        format!("{:6.2} Hz", refresh)
    }

    fn rect(&self) -> Rect {
        if self.pos.is_none() || self.mode.is_none() {
            return Rect::zero();
        }
        Rect::new(
            self.pos.unwrap().0 as f32,
            self.pos.unwrap().1 as f32,
            self.mode.as_ref().unwrap().width as f32,
            self.mode.as_ref().unwrap().height as f32,
        )
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

fn get_bounds(outputs: &Vec<Output>) -> Rect {
    let enabled_outputs = outputs.iter().filter(|&o| o.enabled).collect::<Vec<_>>();
    if enabled_outputs.is_empty() {
        return Rect::zero();
    }
    enabled_outputs
        .iter()
        .fold(enabled_outputs[0].rect(), |acc, &o| acc.union(&o.rect()))
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
    let screen_size_range: ScreenSizeRange = screen_size_range
        .reply()
        .expect("reply for screen size range");

    let rr_outputs = request_outputs(&conn, &res).expect("cookies to request outputs");
    let rr_crtcs = request_crtcs(&conn, &res).expect("cookies to request crtcs");

    let rr_outputs: HashMap<OutputId, OutputInfo> =
        get_outputs(rr_outputs).expect("reply for outputs");
    let rr_crtcs: HashMap<CrtcId, CrtcInfo> = get_crtcs(rr_crtcs).expect("reply for crtcs");
    let rr_modes: HashMap<ModeId, ModeInfo> =
        res.modes.into_iter().map(|m| return (m.id, m)).collect();

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
            modes: get_modes_for_output(&output_info, &rr_modes),
            dim: [output_info.mm_width, output_info.mm_height],
        });
    }
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        build_ui(app, outputs.clone(), screen_size_range, move |outputs| {
            on_apply_clicked(&screen_size_range, &outputs)
        })
    });
    app.run()
}

fn build_ui(
    app: &Application,
    outputs: Vec<Output>,
    size: ScreenSizeRange,
    on_apply: impl Fn(Vec<Output>) -> bool + 'static,
) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(800)
        .default_height(600)
        .title("RRandR")
        .build();

    let view = View::new(outputs, size, on_apply);
    window.set_child(Some(&view));
    window.present();
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
                && (crtc.x as i32 + crtc.width as i32 > screen_size.width as i32
                    || crtc.y as i32 + crtc.height as i32 > screen_size.height as i32))
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
        let crtc_info = &rr_crtcs[&crtc_id];
        if crtc_id == 0 || (crtc_info.outputs.len() > 1 && crtc_info.outputs[0] != output.id) {
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
    let primary_id = primary.and_then(|p| Some(p.id)).unwrap_or_default();
    if handle_no_reply_error(
        set_output_primary(&conn, screen.root, primary_id),
        "set primary output",
    ) {
        return false;
    }
    true
}

fn get_screen_size(
    screen_size_range: &ScreenSizeRange,
    outputs: &Vec<Output>,
    primary: Option<&Output>,
) -> ScreenSize {
    let bounds = get_bounds(&outputs);
    let width = screen_size_range
        .min_width
        .max(screen_size_range.max_width.min(bounds.width() as u16));
    let height = screen_size_range
        .min_height
        .max(screen_size_range.max_width.min(bounds.height() as u16));

    let ppi = primary
        .and_then(|p| Some(p.ppi()))
        .unwrap_or(PPI_DEFAULT as f32);

    ScreenSize {
        width,
        height,
        mwidth: ((MM_PER_INCH * width as f32) / ppi) as u16,
        mheight: ((MM_PER_INCH * height as f32) / ppi) as u16,
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
    let Some(mode) = &output.mode else {
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
    Ok(set_crtc_config(
        conn,
        crtc,
        CURRENT_TIME,
        CURRENT_TIME,
        0,
        0,
        0,
        Rotation::ROTATE0,
        &[],
    )?
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
    println!(
        "Failed to get empty CRTC for output {}",
        String::from_utf8_lossy(&output_info.name)
    );
    None
}

fn handle_reply_error(result: Result<SetConfig, ReplyError>, msg: &str) -> bool {
    let mut error = true;
    match result {
        Ok(SetConfig::SUCCESS) => error = false,
        Ok(SetConfig::FAILED) => println!("Failed to {}.", msg),
        Ok(status) => println!("Failed to {}. Cause: {:#?}", msg, status),
        Err(ReplyError::X11Error(e)) => println!("{}", x_error_to_string(e)),
        Err(e) => println!("Failed to {}. Cause: {:?}", msg, e),
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
            Ok(_) => error = false,
            Err(ReplyError::X11Error(e)) => println!("{}", x_error_to_string(e)),
            Err(e) => println!("Failed to {}. Cause: {}", msg, e),
        },
        Err(e) => println!("Failed to request {}. Cause: {}", msg, e),
    }
    error
}

fn x_error_to_string(e: X11Error) -> String {
    format!(
        "X11 {:?} error for value {}{}.",
        e.error_kind,
        e.bad_value,
        e.request_name
            .and_then(|s| Some(" in request ".to_string() + s))
            .unwrap_or_default()
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
fn request_outputs<'a>(
    conn: &'a RustConnection,
    res: &GetScreenResourcesCurrentReply,
) -> Result<HashMap<OutputId, Cookie<'a, RustConnection, OutputInfo>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for output in &res.outputs {
        cookies.insert(*output, get_output_info(conn, *output, res.timestamp)?);
    }
    Ok(cookies)
}

fn request_crtcs<'a>(
    conn: &'a RustConnection,
    res: &GetScreenResourcesCurrentReply,
) -> Result<HashMap<CrtcId, Cookie<'a, RustConnection, CrtcInfo>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for crtc in &res.crtcs {
        cookies.insert(*crtc, get_crtc_info(conn, *crtc, res.timestamp)?);
    }
    Ok(cookies)
}

fn get_outputs(
    cookies: HashMap<OutputId, Cookie<RustConnection, OutputInfo>>,
) -> Result<HashMap<OutputId, OutputInfo>, ReplyError> {
    let mut outputs = HashMap::new();
    for (output, c) in cookies {
        outputs.insert(output, c.reply()?);
    }
    Ok(outputs)
}

fn get_crtcs(
    cookies: HashMap<CrtcId, Cookie<RustConnection, CrtcInfo>>,
) -> Result<HashMap<CrtcId, CrtcInfo>, ReplyError> {
    let mut crtcs = HashMap::new();
    for (crtc, c) in cookies {
        crtcs.insert(crtc, c.reply()?);
    }
    Ok(crtcs)
}

fn get_modes_for_output(output_info: &OutputInfo, modes: &HashMap<ModeId, ModeInfo>) -> Vec<Mode> {
    output_info
        .modes
        .iter()
        .map(|mode_id| Mode::from(modes[mode_id]))
        .collect::<Vec<Mode>>()
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
        mode_info.dot_clock as f64 / (mode_info.htotal as f64 * vtotal as f64)
    } else {
        0.0
    }
}

fn get_edid(conn: &RustConnection, output: OutputId) -> Result<Vec<u8>, Box<dyn Error>> {
    let name = "EDID";
    let property = intern_atom(conn, true, name.as_bytes())?.reply()?.atom;
    if property == AtomEnum::NONE.into() {
        return Err(format!("No property named: {}", name).into());
    }
    Ok(get_output_property(
        conn,
        output,
        property,
        AtomEnum::INTEGER,
        0,
        256,
        false,
        false,
    )?
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
                            String::from_utf8_lossy(&edid[(i + 5)..(i + 18)])
                                .trim_end()
                                .to_string(),
                        );
                    }
                    i += 18;
                }
            }
        }
    }
    None
}

#[cfg(debug_assertions)]
fn print_crtcs(rr_crtcs: &HashMap<CrtcId, CrtcInfo>, rr_modes: &HashMap<ModeId, ModeInfo>) {
    for (crtc_id, crtc) in rr_crtcs {
        println!("{:-^40}", format!(" CRTC {crtc_id} "));
        println!("XID:      {}", crtc_id);
        println!("Pos:      +{}+{}", crtc.x, crtc.y);
        println!("Res:      {}x{}", crtc.width, crtc.height);
        if crtc.mode > 0 {
            println!(
                "Mode:     {}: {}",
                crtc.mode,
                Mode::from(rr_modes[&crtc.mode])
            );
        }
        println!("Outputs:  {:?}", crtc.outputs);
        println!("Rot:      {:#?}", crtc.rotation);
        println!("Possible: {:?}", crtc.possible);
        println!();
    }
}

#[cfg(debug_assertions)]
fn print_outputs(rr_outputs: &HashMap<OutputId, OutputInfo>, rr_modes: &HashMap<ModeId, ModeInfo>) {
    for (output_id, output) in rr_outputs {
        if output.connection == Connection::CONNECTED {
            println!("{:-^40}", format!(" Output {output_id} "));
            println!("XID:   {}", output_id);
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
