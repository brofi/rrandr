use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::error::Error;
use std::thread::{self, JoinHandle};

use async_channel::Sender;
use gtk::prelude::ListModelExtManual;
use log::{debug, error, warn};
use x11rb::connection::{Connection as XConnection, RequestConnection};
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_info, get_output_primary, get_output_property,
    get_screen_resources_current, get_screen_size_range, query_version, set_crtc_config,
    set_output_primary, set_screen_size, Connection, ConnectionExt, Crtc as CrtcId, CrtcChange,
    GetCrtcInfoReply, GetOutputInfoReply, GetOutputPrimaryReply, GetScreenResourcesCurrentReply,
    GetScreenSizeRangeReply, Mode as ModeId, ModeInfo, Notify, NotifyData, NotifyEvent, NotifyMask,
    Output as OutputId, OutputChange, QueryVersionReply, Rotation, ScreenChangeNotifyEvent,
    ScreenSize, SetConfig,
};
use x11rb::protocol::xproto::{intern_atom, query_extension, AtomEnum, Window as WindowId};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

use super::x_error_to_string;
use crate::data::mode::Mode;
use crate::data::modes::Modes;
use crate::data::output::{Output, PPI_DEFAULT};
use crate::data::outputs::Outputs;
use crate::math::{Rect, MM_PER_INCH};

type Version = QueryVersionReply;
pub type ScreenSizeRange = GetScreenSizeRangeReply;
type ScreenResources = GetScreenResourcesCurrentReply;
pub type OutputInfo = GetOutputInfoReply;
type CrtcInfo = GetCrtcInfoReply;
type Primary = GetOutputPrimaryReply;

const MIN_VERSION: [u32; 2] = [1, 3];
const CLIENT_VERSION: [u32; 2] = [1, 5];

pub struct Snapshot {
    root: WindowId,
    screen_size: ScreenSize,
    crtcs: HashMap<CrtcId, CrtcInfo>,
}

pub struct Randr {
    conn: RustConnection,
    root: WindowId,
    screen_size: Cell<ScreenSize>,
    screen_size_range: ScreenSizeRange,
    primary: Cell<Primary>,
    crtcs: RefCell<HashMap<CrtcId, CrtcInfo>>,
    outputs: RefCell<HashMap<OutputId, OutputInfo>>,
    modes: RefCell<HashMap<ModeId, ModeInfo>>,
}

impl Default for Randr {
    fn default() -> Self { Self::new() }
}

impl Randr {
    pub fn new() -> Self {
        let (conn, screen_num) = x11rb::connect(None).expect("connection to X Server");
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let screen_size = ScreenSize {
            width: screen.width_in_pixels,
            height: screen.height_in_pixels,
            mwidth: screen.width_in_millimeters,
            mheight: screen.height_in_millimeters,
        };
        debug!(
            "Init screen size to {}x{} px, {}x{} mm",
            screen_size.width, screen_size.height, screen_size.mwidth, screen_size.mheight
        );

        let res =
            get_screen_resources_current(&conn, root).expect("cookie to request screen resources");
        let res = res.reply().expect("reply for screen resources");

        let primary = get_output_primary(&conn, root).expect("cookie to request primary output");
        let primary = primary.reply().expect("reply for primary output");

        let screen_size_range =
            get_screen_size_range(&conn, root).expect("cookie to request screen size range");
        let screen_size_range = screen_size_range.reply().expect("reply for screen size range");

        let outputs = request_outputs(&conn, &res).expect("cookies to request outputs");
        let crtcs = request_crtcs(&conn, &res).expect("cookies to request crtcs");

        let outputs: HashMap<OutputId, OutputInfo> =
            get_outputs(outputs).expect("reply for outputs");

        let crtcs: HashMap<CrtcId, CrtcInfo> = get_crtcs(crtcs).expect("reply for crtcs");

        let modes: HashMap<ModeId, ModeInfo> =
            res.modes.iter().map(|m| (m.id, *m)).collect::<HashMap<_, _>>();

        #[cfg(debug_assertions)]
        log_crtcs(&crtcs, &modes);
        #[cfg(debug_assertions)]
        log_outputs(&outputs, &modes);

        Self {
            conn,
            root,
            screen_size: Cell::new(screen_size),
            screen_size_range,
            primary: Cell::new(primary),
            crtcs: RefCell::new(crtcs),
            outputs: RefCell::new(outputs),
            modes: RefCell::new(modes),
        }
    }

    pub fn handle_event(&self, event: &Event) {
        match *event {
            Event::RandrScreenChangeNotify(e) => self.handle_screen_change(&e),
            Event::RandrNotify(NotifyEvent { sub_code, u: data, .. }) => match sub_code {
                Notify::CRTC_CHANGE => self.handle_crtc_change(&data),
                Notify::OUTPUT_CHANGE => self.handle_output_change(&data),
                _ => (),
            },
            _ => (),
        }
    }

    fn handle_screen_change(&self, event: &ScreenChangeNotifyEvent) {
        let ScreenChangeNotifyEvent {
            root,
            request_window: window,
            width,
            height,
            mwidth,
            mheight,
            ..
        } = *event;

        debug!("ScreenChangeNotifyEvent: {width}x{height}");

        if root != window || root != self.root {
            warn!("Unknown window");
            return;
        }

        self.screen_size.set(ScreenSize { width, height, mwidth, mheight });
    }

    fn handle_crtc_change(&self, data: &NotifyData) {
        let CrtcChange {
            timestamp, window, crtc, mode, rotation: rot, x, y, width, height, ..
        } = data.as_cc();

        debug!("CrtcChangeNotify for CRTC: {crtc}");

        if self.root != window {
            warn!("Unknown window");
            return;
        }

        debug!("Mode: {mode}");
        debug!("Rotation: {rot:#?}");
        debug!("Position: ({x},{y})");
        debug!("Dimension: {width}x{height}");

        let mut crtcs = self.crtcs.borrow_mut();
        let Some(crtc) = crtcs.get_mut(&crtc) else {
            debug!("New CRTC found: {crtc}");
            let crtc_info = self
                .conn
                .randr_get_crtc_info(crtc, timestamp)
                .expect("should send crtc request")
                .reply()
                .expect("should get crtc reply");
            crtcs.insert(crtc, crtc_info);
            return;
        };

        crtc.mode = mode;
        crtc.rotation = rot;
        crtc.x = x;
        crtc.y = y;
        crtc.width = width;
        crtc.height = height;
    }

    fn handle_output_change(&self, data: &NotifyData) {
        let OutputChange {
            timestamp,
            window,
            output,
            crtc,
            mode,
            connection: conn,
            subpixel_order: subp,
            ..
        } = data.as_oc();

        debug!("OutputChangeNotify for output: {output}");

        if self.root != window {
            warn!("Unknown window");
            return;
        }

        debug!("CRTC: {crtc}");
        debug!("Mode: {mode}");
        debug!("Connection: {conn:#?}");
        debug!("Subpixel order: {subp:#?}");

        let mut outputs = self.outputs.borrow_mut();
        let Some(output_info) = outputs.get_mut(&output) else {
            warn!("Output: {output} not found");
            return;
        };

        // Update CRTC association
        if crtc != output_info.crtc {
            // Remove output from old CRTC
            if output_info.crtc > 0 {
                if let Some(crtc_info) = self.crtcs.borrow_mut().get_mut(&output_info.crtc) {
                    crtc_info.outputs.retain(|o| *o != output);
                }
            }
            // Add output to new CRTC
            if crtc > 0 {
                if let Some(crtc_info) = self.crtcs.borrow_mut().get_mut(&crtc) {
                    if !crtc_info.outputs.contains(&output) {
                        crtc_info.outputs.push(output);
                    }
                }
            }
        }

        // Update modes (there can be new and/or deleted modes)
        let modes = get_screen_resources_current(&self.conn, self.root)
                .expect("should send screen resources request")
                .reply()
                .expect("should get screen resources reply")
                .modes;
        *self.modes.borrow_mut() = modes.iter().map(|m| (m.id, *m)).collect::<HashMap<_, _>>();

        // Update full output info (since output modes can change)
        *output_info = get_output_info(&self.conn, output, timestamp).unwrap().reply().unwrap();

        // Update primary output
        self.primary.set(
            get_output_primary(&self.conn, self.root)
                .expect("should send primary request")
                .reply()
                .expect("should get primary reply"),
        );
    }

    pub fn screen_size_range(&self) -> ScreenSizeRange { self.screen_size_range }

    pub fn output_model(&self) -> Outputs {
        let outputs = Outputs::default();
        for (id, output_info) in self.outputs.borrow().iter() {
            if output_info.connection != Connection::CONNECTED {
                continue;
            }

            let enabled = output_info.crtc > 0;
            let modes: Modes = Modes::new();
            let mut mode = None;
            for mode_id in &output_info.modes {
                modes.append(&Mode::from(self.modes.borrow()[mode_id]));
            }
            let mut pos = [0, 0];
            if enabled {
                let crtc_info = &self.crtcs.borrow()[&output_info.crtc];
                mode = modes.find_by_id(crtc_info.mode);
                pos = [crtc_info.x, crtc_info.y];
            }
            outputs.append(&Output::new(
                *id,
                String::from_utf8_lossy(&output_info.name).into_owned(),
                self.get_monitor_name(*id),
                enabled,
                *id == self.primary.get().output,
                pos[0],
                pos[1],
                mode,
                modes,
                output_info.mm_width,
                output_info.mm_height,
            ));
        }
        outputs
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            root: self.root,
            screen_size: self.screen_size.get(),
            crtcs: self.crtcs.borrow().clone(),
        }
    }

    fn get_edid(&self, output: OutputId) -> Result<Vec<u8>, Box<dyn Error>> {
        let name = "EDID";
        let property = intern_atom(&self.conn, true, name.as_bytes())?.reply()?.atom;
        if property == AtomEnum::NONE.into() {
            return Err(format!("No property named: {name}").into());
        }
        Ok(get_output_property(
            &self.conn,
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

    fn get_monitor_name(&self, output: OutputId) -> Option<String> {
        if let Ok(edid) = self.get_edid(output) {
            if edid.len() >= 128 {
                let version = edid[0x12];
                let revision = edid[0x13];
                if version == 1 && (revision == 3 || revision == 4) {
                    let mut i = 0x48;
                    while i <= 0x6C {
                        // This 18 byte descriptor is a used as a display descriptor with a tag 0xFC
                        // (display product name).
                        if edid[i..(i + 3)] == [0, 0, 0] && edid[i + 3] == 0xFC && edid[i + 4] == 0
                        {
                            return Some(
                                String::from_utf8_lossy(&edid[(i + 5)..(i + 18)])
                                    .trim_end()
                                    .to_owned(),
                            );
                        }
                        i += 18;
                    }
                }
            }
        }
        None
    }

    pub fn apply(&self, outputs: &Outputs) -> bool {
        debug!("Applying changes");
        let primary = outputs.iter::<Output>().map(Result::unwrap).find(Output::primary);
        let screen_size = self.get_screen_size(outputs, primary.as_ref());

        let screen_size_px_changed = self.screen_size.get().width != screen_size.width
            || self.screen_size.get().height != screen_size.height;
        let screen_size_mm_changed = self.screen_size.get().mwidth != screen_size.mwidth
            || self.screen_size.get().mheight != screen_size.mheight;

        // Disable outputs
        for output in outputs.iter::<Output>().map(Result::unwrap) {
            let crtc_id = self.outputs.borrow()[&output.id()].crtc;
            if crtc_id == 0 {
                // Output already disabled
                continue;
            }
            let crtc = &self.crtcs.borrow()[&crtc_id];
            if !output.enabled()
                || (screen_size_px_changed
                    && (i32::from(crtc.x) + i32::from(crtc.width) > i32::from(screen_size.width)
                        || i32::from(crtc.y) + i32::from(crtc.height)
                            > i32::from(screen_size.height)))
            {
                // Disable outputs that are still enabled but shouldn't be and outputs that stay
                // enabled but currently don't fit the new screen size. The latter needs to be
                // done to avoid an invalid intermediate configuration when actually setting the
                // new screen size.
                if handle_reply_error(self.disable_crtc(crtc_id), "disable CRTC") {
                    return false;
                }
            }
        }

        if screen_size_px_changed || screen_size_mm_changed {
            debug!(
                "Setting screen size to {}x{} px, {}x{} mm",
                screen_size.width, screen_size.height, screen_size.mwidth, screen_size.mheight
            );

            if handle_no_reply_error(
                set_screen_size(
                    &self.conn,
                    self.root,
                    screen_size.width,
                    screen_size.height,
                    screen_size.mwidth.into(),
                    screen_size.mheight.into(),
                ),
                "set screen size",
            ) {
                return false;
            }
        }

        // Update outputs
        for output in outputs.iter::<Output>().map(Result::unwrap) {
            if !output.enabled() {
                continue;
            }
            let output_info = &self.outputs.borrow()[&output.id()];
            let mut crtc_id = output_info.crtc;
            if crtc_id == 0
                || self.crtcs.borrow().get(&crtc_id).is_some_and(|ci: &CrtcInfo| {
                    ci.outputs.len() > 1 && ci.outputs[0] != output.id()
                })
            {
                // If this output was disabled before get it a new empty CRTC. If this output is
                // enabled, shares a CRTC with other outputs and it's not the first one listed,
                // move it to a new empty CRTC.
                if let Some(empty_id) = self.get_valid_empty_crtc(output.id()) {
                    crtc_id = empty_id;
                } else {
                    return false;
                }
            }
            if handle_reply_error(self.update_crtc(crtc_id, &output), "update CRTC") {
                return false;
            }
        }

        // Set primary output
        let primary_id = primary.map(|p| p.id()).unwrap_or_default();
        if handle_no_reply_error(
            set_output_primary(&self.conn, self.root, primary_id),
            "set primary output",
        ) {
            return false;
        }
        true
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    fn get_screen_size(&self, outputs: &Outputs, primary: Option<&Output>) -> ScreenSize {
        let bounds = Rect::bounds(
            outputs
                .iter::<Output>()
                .map(Result::unwrap)
                .filter(Output::enabled)
                .map(|o| o.rect())
                .collect(),
        );
        let width = self
            .screen_size_range
            .min_width
            .max(self.screen_size_range.max_width.min(bounds.width()));
        let height = self
            .screen_size_range
            .min_height
            .max(self.screen_size_range.max_height.min(bounds.height()));

        let ppi = primary.map_or(f64::from(PPI_DEFAULT), Output::ppi);
        debug!("Using PPI {ppi:.2}");

        ScreenSize {
            width,
            height,
            mwidth: ((f64::from(MM_PER_INCH) * f64::from(width)) / ppi) as u16,
            mheight: ((f64::from(MM_PER_INCH) * f64::from(height)) / ppi) as u16,
        }
    }

    fn disable_crtc(&self, crtc: CrtcId) -> Result<SetConfig, ReplyError> {
        Ok(set_crtc_config(
            &self.conn,
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

    fn get_valid_empty_crtc(&self, output_id: OutputId) -> Option<CrtcId> {
        let output_info = &self.outputs.borrow()[&output_id];
        for (crtc_id, crtc) in self.crtcs.borrow().iter() {
            if crtc.outputs.is_empty()
                && output_info.crtcs.contains(crtc_id)
                && crtc.possible.contains(&output_id)
            {
                return Some(*crtc_id);
            }
        }
        error!(
            "Failed to get empty CRTC for output {}",
            String::from_utf8_lossy(&output_info.name)
        );
        None
    }

    fn update_crtc(&self, crtc: CrtcId, output: &Output) -> Result<SetConfig, ReplyError> {
        let Some(mode) = output.mode() else {
            error!("Output {} is missing a mode.", output.name());
            return Ok(SetConfig::FAILED);
        };
        debug!(
            "Trying to set output {} to CTRC {} at position +{}+{} with mode {}",
            output.name(),
            crtc,
            output.x(),
            output.y(),
            mode
        );
        Ok(set_crtc_config(
            &self.conn,
            crtc,
            CURRENT_TIME,
            CURRENT_TIME,
            output.x(),
            output.y(),
            mode.id(),
            Rotation::ROTATE0,
            &[output.id()],
        )?
        .reply()?
        .status)
    }

    pub fn revert(&self, snapshot: Snapshot) {
        debug!("Reverting changes");
        for crtc_id in self.crtcs.borrow().keys() {
            self.disable_crtc(*crtc_id).expect("disable CRTC");
        }
        debug!(
            "Reverting screen size to {}x{} px, {}x{} mm",
            snapshot.screen_size.width,
            snapshot.screen_size.height,
            snapshot.screen_size.mwidth,
            snapshot.screen_size.mheight
        );
        set_screen_size(
            &self.conn,
            snapshot.root,
            snapshot.screen_size.width,
            snapshot.screen_size.height,
            snapshot.screen_size.mwidth.into(),
            snapshot.screen_size.mheight.into(),
        )
        .expect("revert screen size request")
        .check()
        .expect("revert screen size");
        for (crtc_id, crtc_info) in snapshot.crtcs {
            set_crtc_config(
                &self.conn,
                crtc_id,
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
}

pub fn check() -> Result<(), Box<dyn Error>> {
    if let Ok((conn, _)) = x11rb::connect(None) {
        let extension = query_extension(&conn, "RANDR".as_bytes())?.reply()?;
        if extension.present {
            let Version { major_version: major, minor_version: minor, .. } =
                query_version(&conn, CLIENT_VERSION[0], CLIENT_VERSION[1])?.reply()?;
            if major < MIN_VERSION[0] || (major == MIN_VERSION[0] && minor < MIN_VERSION[1]) {
                return Err(format!("RandR version {major}.{minor} not supported").into());
            }
        } else {
            return Err("RandR extension not found".into());
        }
    } else {
        return Err("Failed to connect to X Server".into());
    }
    Ok(())
}

pub fn run_event_loop(sender: Sender<Event>) -> Result<JoinHandle<()>, Box<dyn Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;

    conn.randr_select_input(
        root,
        NotifyMask::SCREEN_CHANGE
            | NotifyMask::RESOURCE_CHANGE
            | NotifyMask::CRTC_CHANGE
            | NotifyMask::OUTPUT_CHANGE,
    )?
    .check()?;

    let handle = thread::spawn(move || {
        while let Ok(event) = conn.wait_for_event() {
            sender.send_blocking(event).expect("channel should be open");
        }
    });

    Ok(handle)
}

// TODO checkout GetXIDListRequest
fn request_outputs<'a, Conn: RequestConnection>(
    conn: &'a Conn,
    res: &ScreenResources,
) -> Result<HashMap<OutputId, Cookie<'a, Conn, OutputInfo>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for output in &res.outputs {
        cookies.insert(*output, get_output_info(conn, *output, res.config_timestamp)?);
    }
    Ok(cookies)
}

fn request_crtcs<'a, Conn: RequestConnection>(
    conn: &'a Conn,
    res: &ScreenResources,
) -> Result<HashMap<CrtcId, Cookie<'a, Conn, CrtcInfo>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for crtc in &res.crtcs {
        cookies.insert(*crtc, get_crtc_info(conn, *crtc, res.config_timestamp)?);
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

#[cfg(debug_assertions)]
#[allow(clippy::use_debug)]
fn log_crtcs(crtcs: &HashMap<CrtcId, CrtcInfo>, modes: &HashMap<ModeId, ModeInfo>) {
    for (crtc_id, crtc) in crtcs {
        debug!("{:-^40}", format!(" CRTC {crtc_id} "));
        debug!("XID:      {crtc_id}");
        debug!("Pos:      +{}+{}", crtc.x, crtc.y);
        debug!("Res:      {}x{}", crtc.width, crtc.height);
        if crtc.mode > 0 {
            debug!("Mode:     {}: {}", crtc.mode, Mode::from(modes[&crtc.mode]));
        }
        debug!("Outputs:  {:?}", crtc.outputs);
        debug!("Rot:      {:#?}", crtc.rotation);
        debug!("Possible: {:?}", crtc.possible);
    }
}

#[cfg(debug_assertions)]
#[allow(clippy::use_debug)]
fn log_outputs(outputs: &HashMap<OutputId, OutputInfo>, modes: &HashMap<ModeId, ModeInfo>) {
    for (output_id, output) in outputs {
        if output.connection == Connection::CONNECTED {
            debug!("{:-^40}", format!(" Output {} ", output_id));
            debug!("XID:   {output_id}");
            debug!("Name:  {}", String::from_utf8_lossy(&output.name));
            debug!("CRTC:  {}", output.crtc);
            debug!("CRTCs: {:?}", output.crtcs);
            debug!("Dim:   {}x{} mm", output.mm_width, output.mm_height);
            debug!("Preferred modes:");
            for mode_id in &output.modes[0..output.num_preferred.into()] {
                debug!("    {}: {}", mode_id, Mode::from(modes[mode_id]));
            }
            debug!("Modes:");
            for mode_id in &output.modes {
                debug!("    {}: {}", mode_id, Mode::from(modes[mode_id]));
            }
            debug!("Clones: {:?}", output.clones);
        }
    }
}

fn handle_reply_error(result: Result<SetConfig, ReplyError>, msg: &str) -> bool {
    let mut error = true;
    match result {
        Ok(SetConfig::SUCCESS) => error = false,
        Ok(SetConfig::FAILED) => error!("Failed to {msg}."),
        Ok(status) => error!("Failed to {msg}. Cause: {status:#?}"),
        Err(ReplyError::X11Error(e)) => error!("{}", x_error_to_string(&e)),
        Err(e) => error!("Failed to {msg}. Cause: {e:?}"),
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
            Err(ReplyError::X11Error(e)) => error!("{}", x_error_to_string(&e)),
            Err(e) => error!("Failed to {msg}. Cause: {e}"),
        },
        Err(e) => error!("Failed to request {msg}. Cause: {e}"),
    }
    error
}
