mod view;

use gtk::glib::ExitCode;
use gtk::graphene::Rect;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::collections::HashMap;
use std::error::Error;

use std::rc::Rc;
use view::View;
use x11rb::cookie::Cookie;
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_primary, get_screen_size_range, set_crtc_config, set_output_primary,
    Connection, Crtc as CrtcId, GetCrtcInfoReply, GetScreenResourcesCurrentReply,
    GetScreenSizeRangeReply, Mode as ModeId, ModeFlag, ModeInfo, Output as OutputId, Rotation,
    SetConfig,
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

#[derive(Clone, Debug)]
pub struct Output {
    id: OutputId,
    name: String,
    enabled: bool,
    primary: bool,
    pos: Option<(i16, i16)>,
    mode: Option<Mode>,
    modes: Vec<Mode>,
}

impl Output {
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
            enabled,
            primary: *id == primary.output,
            pos,
            mode,
            modes: get_modes_for_output(&output_info, &rr_modes),
        });
    }
    let conn = Rc::new(conn);
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        build_ui(app, outputs.clone(), screen_size_range, {
            let conn = Rc::clone(&conn);
            move |outputs| on_apply_clicked(&conn, screen_num, &outputs)
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

fn on_apply_clicked(conn: &RustConnection, screen_num: usize, outputs: &Vec<Output>) -> bool {
    let screen = &conn.setup().roots[screen_num];
    let res = get_screen_resources_current(&conn, screen.root)
        .expect("cookie to request screen resources");
    let res = res.reply().expect("reply for screen resources");
    let rr_outputs = request_outputs(&conn, &res).expect("cookies to request outputs");
    let rr_crtcs = request_crtcs(&conn, &res).expect("cookies to request crtcs");
    let rr_outputs: HashMap<OutputId, OutputInfo> =
        get_outputs(rr_outputs).expect("reply for outputs");
    let rr_crtcs: HashMap<CrtcId, CrtcInfo> = get_crtcs(rr_crtcs).expect("reply for crtcs");

    for output in outputs {
        match apply_output_config(conn, output, &rr_crtcs, &rr_outputs) {
            Ok(SetConfig::SUCCESS) => println!("Great success"),
            Ok(status) => {
                println!("Failed to set config: {:#?}", status);
                return false;
            }
            Err(_) => {
                println!("General fail");
                return false;
            }
        }
        if output.primary {
            if set_output_primary(conn, screen.root, output.id).is_err() {
                println!("Failed to set primary output");
                return false;
            }
        }
    }
    true
}

fn apply_output_config(
    conn: &RustConnection,
    output: &Output,
    rr_crtcs: &HashMap<CrtcId, CrtcInfo>,
    rr_outputs: &HashMap<OutputId, OutputInfo>,
) -> Result<SetConfig, Box<dyn Error>> {
    let Some(crtc_id) = get_crtc_id(output, rr_crtcs, rr_outputs) else {
        return Ok(SetConfig::FAILED);
    };

    if !output.enabled {
        if crtc_id > 0 {
            return disable_crtc(conn, crtc_id);
        }
        return Ok(SetConfig::SUCCESS);
    }

    update_crtc(conn, crtc_id, output)
}

fn update_crtc(
    conn: &RustConnection,
    crtc: CrtcId,
    output: &Output,
) -> Result<SetConfig, Box<dyn Error>> {
    let (Some(pos), Some(mode)) = (output.pos, &output.mode) else {
        return Ok(SetConfig::FAILED);
    };

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

fn disable_crtc(conn: &RustConnection, crtc: CrtcId) -> Result<SetConfig, Box<dyn Error>> {
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

fn get_crtc_id(
    output: &Output,
    rr_crtcs: &HashMap<CrtcId, CrtcInfo>,
    rr_outputs: &HashMap<OutputId, OutputInfo>,
) -> Option<CrtcId> {
    let mut crtc_id = rr_outputs[&output.id].crtc;
    if crtc_id == 0 {
        crtc_id = get_empty_crtc(rr_crtcs)?;
    } else {
        let crtc_info = &rr_crtcs[&crtc_id];
        if crtc_info.outputs.len() > 1 && crtc_info.outputs[0] != output.id {
            crtc_id = get_empty_crtc(rr_crtcs)?;
        }
    }
    Some(crtc_id)
}

fn get_empty_crtc(rr_crtcs: &HashMap<CrtcId, CrtcInfo>) -> Option<CrtcId> {
    for (xid, crtc) in rr_crtcs {
        if crtc.outputs.is_empty() {
            return Some(*xid);
        }
    }
    None
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
