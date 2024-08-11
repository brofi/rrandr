use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::error::Error;
use std::thread::{self, JoinHandle};

use async_channel::Sender;
use gtk::prelude::{ListModelExt, ListModelExtManual};
use log::{debug, error, warn};
use x11rb::connection::{Connection as XConnection, RequestConnection};
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::randr::{
    self, get_crtc_info, get_crtc_transform, get_output_info, get_output_primary,
    get_output_property, get_screen_resources_current, get_screen_size_range, query_version,
    set_crtc_config, set_crtc_transform, set_output_primary, set_screen_size, Connection,
    ConnectionExt, Crtc as CrtcId, CrtcChange, GetCrtcInfoReply, GetCrtcTransformReply,
    GetOutputInfoReply, GetOutputPrimaryReply, GetScreenResourcesCurrentReply,
    GetScreenSizeRangeReply, Mode as ModeId, ModeInfo, Notify, NotifyData, NotifyEvent, NotifyMask,
    Output as OutputId, OutputChange, QueryVersionReply, Rotation as RRotation,
    ScreenChangeNotifyEvent, ScreenSize, SetConfig,
};
use x11rb::protocol::render::Transform;
use x11rb::protocol::xproto::{intern_atom, query_extension, AtomEnum, Window as WindowId};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

use super::x_error_to_string;
use crate::data::enums::Rotation;
use crate::data::mode::Mode;
use crate::data::modes::Modes;
use crate::data::output::{Output, PPI_DEFAULT};
use crate::data::outputs::Outputs;
use crate::math::{Rect, MM_PER_INCH};
use crate::utils::nearly_eq;

type Version = QueryVersionReply;
pub type ScreenSizeRange = GetScreenSizeRangeReply;
type ScreenResources = GetScreenResourcesCurrentReply;
pub type OutputInfo = GetOutputInfoReply;
type CrtcInfo = GetCrtcInfoReply;
type Primary = GetOutputPrimaryReply;
type Edid = Vec<u8>;

pub const DISPLAY: Option<&str> = None;
// pub const DISPLAY: Option<&str> = Some(":1");

const MIN_VERSION: [u32; 2] = [1, 3];
const CLIENT_VERSION: [u32; 2] = [1, 5];

pub struct Snapshot {
    root: WindowId,
    screen_size: ScreenSize,
    crtcs: HashMap<CrtcId, CrtcInfo>,
    transforms: HashMap<CrtcId, Transform>,
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
    transforms: RefCell<HashMap<CrtcId, Transform>>,
    edids: HashMap<OutputId, Option<Edid>>,
}

impl Default for Randr {
    fn default() -> Self { Self::new() }
}

impl Randr {
    pub fn new() -> Self {
        let (conn, screen_num) = x11rb::connect(DISPLAY).expect("connection to X Server");
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
        let transforms = request_transforms(&conn, &res).expect("cookies to request transforms");

        let outputs: HashMap<OutputId, OutputInfo> =
            get_outputs(outputs).expect("reply for outputs");
        let crtcs: HashMap<CrtcId, CrtcInfo> = get_crtcs(crtcs).expect("reply for crtcs");
        let modes: HashMap<ModeId, ModeInfo> = res.modes.iter().map(|m| (m.id, *m)).collect();
        let transforms = get_transforms(transforms).expect("reply for transforms");

        let edids: HashMap<OutputId, Option<Edid>> =
            res.outputs.iter().map(|&o| (o, get_edid(&conn, o).ok())).collect();

        #[cfg(debug_assertions)]
        log_crtcs(&crtcs, &modes, &transforms);
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
            transforms: RefCell::new(transforms),
            edids,
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
            rotation: rot,
            root,
            request_window: window,
            width,
            height,
            mwidth,
            mheight,
            ..
        } = *event;

        debug!("ScreenChangeNotifyEvent: {width}x{height} px, {mwidth}x{mheight} mm, {rot:#?}");

        if root != window || root != self.root {
            warn!("Unknown window");
            return;
        }

        self.screen_size.set(if rot.intersects(RRotation::ROTATE90 | RRotation::ROTATE270) {
            ScreenSize { height, width, mheight, mwidth }
        } else {
            ScreenSize { width, height, mwidth, mheight }
        });
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
        let Some(crtc_info) = crtcs.get_mut(&crtc) else {
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

        crtc_info.mode = mode;
        crtc_info.rotation = rot;
        crtc_info.x = x;
        crtc_info.y = y;

        let transform = self
            .conn
            .randr_get_crtc_transform(crtc)
            .expect("should send transform request")
            .reply()
            .expect("should get transform reply")
            .current_transform;
        self.transforms.borrow_mut().insert(crtc, transform);

        let sx = f64::from(Fixed(transform.matrix11));
        let sy = f64::from(Fixed(transform.matrix22));
        let [w, h] = if rot.intersects(RRotation::ROTATE90 | RRotation::ROTATE270) {
            [height, width]
        } else {
            [width, height]
        }
        .map(f64::from);

        crtc_info.width = (w * sx).round() as u16;
        crtc_info.height = (h * sy).round() as u16;
    }

    fn handle_output_change(&self, data: &NotifyData) {
        let OutputChange {
            window, output, crtc, mode, connection: conn, subpixel_order: subp, ..
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
            output_info.crtc = crtc;
        }

        output_info.connection = conn;
        output_info.subpixel_order = subp;

        // Update modes (there can be new and/or deleted modes)
        let res = get_screen_resources_current(&self.conn, self.root)
            .expect("should send screen resources request")
            .reply()
            .expect("should get screen resources reply");
        *self.modes.borrow_mut() = res.modes.iter().map(|m| (m.id, *m)).collect::<HashMap<_, _>>();

        // Update output modes
        if mode > 0 {
            output_info.modes = get_output_info(&self.conn, output, res.config_timestamp)
                .expect("should request output info")
                .reply()
                .expect("should get output info reply")
                .modes;
        }

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
            let mut rotation = RRotation::ROTATE0;
            let mut pos = [0, 0];
            let mut dim = [0, 0];
            let mut scale = [1., 1.];
            if enabled {
                let crtc_info = &self.crtcs.borrow()[&output_info.crtc];
                mode = modes.find_by_id(crtc_info.mode);
                rotation = crtc_info.rotation;
                pos = [crtc_info.x, crtc_info.y];
                dim = [crtc_info.width, crtc_info.height];
                let transform = self.transforms.borrow()[&output_info.crtc];
                scale =
                    [f64::from(Fixed(transform.matrix11)), f64::from(Fixed(transform.matrix22))];
            }
            let product_name = self
                .edids
                .get(id)
                .and_then(|e| e.as_ref().map(|edid| get_monitor_name(edid)))
                .unwrap_or_default();
            outputs.append(&Output::new(
                *id,
                String::from_utf8_lossy(&output_info.name).into_owned(),
                product_name,
                enabled,
                *id == self.primary.get().output,
                pos,
                mode,
                modes,
                rotation.into(),
                rotation.into(),
                scale,
                dim,
                [output_info.mm_width, output_info.mm_height],
            ));
        }
        outputs
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            root: self.root,
            screen_size: self.screen_size.get(),
            crtcs: self.crtcs.borrow().clone(),
            transforms: self.transforms.borrow().clone(),
        }
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
            // Even though the crtc dimension contains the transformation, a match error
            // occurs when setting the screen size, so disable scaled CRTCs.
            let transform = self.transforms.borrow()[&crtc_id];
            let has_scale = transform.matrix11 / 65536 != 1 || transform.matrix22 / 65536 != 1;
            if !output.enabled()
                || has_scale
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
        for output in outputs.iter::<Output>().map(Result::unwrap).filter(Output::enabled) {
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
                if let Some(empty_id) = self.get_valid_empty_crtc(&output) {
                    crtc_id = empty_id;
                } else {
                    return false;
                }
            }

            let mut transform = Transform::default();
            transform.matrix11 = Fixed::from(output.scale_x()).0;
            transform.matrix22 = Fixed::from(output.scale_y()).0;
            transform.matrix33 = Fixed::from(1.).0;

            if handle_no_reply_error(
                set_crtc_transform(&self.conn, crtc_id, transform, "bilinear".as_bytes(), &[]),
                "set CRTC transform",
            ) {
                return false;
            }

            if handle_reply_error(
                self.update_crtc(
                    crtc_id,
                    output.x(),
                    output.y(),
                    output.mode().map_or(0, |m| m.id()),
                    output.randr_rotation(),
                    &[output.id()],
                ),
                "update CRTC",
            ) {
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
        let enabled = outputs
            .iter::<Output>()
            .map(Result::unwrap)
            .filter(Output::enabled)
            .collect::<Vec<_>>();
        let bounds = Rect::bounds(enabled.iter().map(|o| o.rect()).collect());
        let width = self
            .screen_size_range
            .min_width
            .max(self.screen_size_range.max_width.min(bounds.width()));
        let height = self
            .screen_size_range
            .min_height
            .max(self.screen_size_range.max_height.min(bounds.height()));

        let mut mm_dim = [0, 0];
        if enabled.len() == 1 {
            let o = outputs.first().unwrap();
            if let (Ok(w), Ok(h)) = (u16::try_from(o.mm_width()), u16::try_from(o.mm_height())) {
                mm_dim = match o.rotation() {
                    Rotation::Left | Rotation::Right => [h, w],
                    _ => [w, h],
                }
            }
        }

        if mm_dim[0] == 0 || mm_dim[1] == 0 {
            let ppi = primary.map_or(PPI_DEFAULT, |p| match p.rotation() {
                Rotation::Left | Rotation::Right => {
                    let mut ppi = p.ppi();
                    ppi.reverse();
                    ppi
                }
                _ => p.ppi(),
            });
            debug!("Using PPI {:.2}x{:.2}", ppi[0], ppi[1]);
            mm_dim = [
                ((MM_PER_INCH * f64::from(width)) / ppi[0]).ceil() as u16,
                ((MM_PER_INCH * f64::from(height)) / ppi[1]).ceil() as u16,
            ];
        }

        ScreenSize { width, height, mwidth: mm_dim[0], mheight: mm_dim[1] }
    }

    fn get_valid_empty_crtc(&self, output: &Output) -> Option<CrtcId> {
        let outputs = self.outputs.borrow();
        let Some(output_info) = outputs.get(&output.id()) else {
            error!("Unknown output {}", output.id());
            return None;
        };
        for crtc_id in &output_info.crtcs {
            if let Some(crtc_info) = self.crtcs.borrow().get(crtc_id) {
                if crtc_info.outputs.is_empty()
                    && crtc_info.possible.contains(&output.id())
                    && crtc_info.rotations.contains(output.randr_rotation())
                {
                    return Some(*crtc_id);
                }
            }
        }
        error!("Failed to get empty CRTC for output {}", output.id());
        None
    }

    fn disable_crtc(&self, crtc: CrtcId) -> Result<SetConfig, ReplyError> {
        self.update_crtc(crtc, 0, 0, 0, RRotation::ROTATE0, &[])
    }

    fn update_crtc(
        &self,
        crtc: CrtcId,
        x: i16,
        y: i16,
        mode: ModeId,
        rotation: RRotation,
        outputs: &[OutputId],
    ) -> Result<SetConfig, ReplyError> {
        if outputs.len() > 1 {
            error!("Attaching multiple outputs to one CRTC is not supported yet");
            return Ok(SetConfig::FAILED);
        }

        if mode == 0 && !outputs.is_empty() || mode > 0 && outputs.is_empty() {
            error!("Output must be set if mode is set and vice versa");
            return Ok(SetConfig::FAILED);
        }

        match rotation & 0xf {
            RRotation::ROTATE0
            | RRotation::ROTATE90
            | RRotation::ROTATE180
            | RRotation::ROTATE270 => (),
            _ => {
                error!("Invalid rotation: multiple rotation bits set");
                return Ok(SetConfig::FAILED);
            }
        }

        let crtcs = self.crtcs.borrow();
        let Some(crtc_info) = crtcs.get(&crtc) else {
            error!("Unknown CRTC: {crtc}");
            return Ok(SetConfig::FAILED);
        };

        if mode > 0 && !self.modes.borrow().contains_key(&mode) {
            error!("Unknown mode: {mode}");
            return Ok(SetConfig::FAILED);
        }

        if !outputs.is_empty() && mode > 0 {
            if let Some(output_info) = self.outputs.borrow().get(&outputs[0]) {
                if !crtc_info.possible.contains(&outputs[0]) || !output_info.crtcs.contains(&crtc) {
                    error!("Cannot attach output {} to CRTC {crtc}", outputs[0]);
                    return Ok(SetConfig::FAILED);
                }
                if !crtc_info.rotations.contains(rotation) {
                    error!("Rotation {rotation:#?} not valid for CRTC {crtc}");
                    return Ok(SetConfig::FAILED);
                }
                if !output_info.modes.contains(&mode) {
                    error!("Mode {mode} not valid for output {}", outputs[0]);
                    return Ok(SetConfig::FAILED);
                }
            } else {
                error!("Unknown output: {}", outputs[0]);
                return Ok(SetConfig::FAILED);
            }
        }

        if outputs.is_empty() {
            debug!("Disabling crtc {crtc}");
        } else {
            debug!(
                "Attaching output {} to CTRC {} at position +{}+{} with mode {}",
                outputs[0], crtc, x, y, mode
            );
        }

        Ok(set_crtc_config(
            &self.conn,
            crtc,
            CURRENT_TIME,
            CURRENT_TIME,
            x,
            y,
            mode,
            rotation,
            outputs,
        )?
        .reply()?
        .status)
    }

    pub fn revert(&self, snapshot: Snapshot) {
        debug!("Reverting changes");

        for crtc_id in self.crtcs.borrow().keys() {
            handle_reply_error(self.disable_crtc(*crtc_id), &format!("disable CRTC {crtc_id}"));
        }

        let ScreenSize { width, height, mwidth, mheight } = snapshot.screen_size;
        debug!("Reverting screen size to {width}x{height} px, {mwidth}x{mheight} mm");
        handle_no_reply_error(
            set_screen_size(
                &self.conn,
                snapshot.root,
                width,
                height,
                mwidth.into(),
                mheight.into(),
            ),
            &format!("revert screen size to {width}x{height} px, {mwidth}x{mheight} mm"),
        );

        for (crtc_id, crtc_info) in snapshot.crtcs {
            if crtc_info.mode == 0 {
                continue;
            }

            let mut mode = 0;
            if self.modes.borrow().contains_key(&crtc_info.mode) {
                mode = crtc_info.mode;
            } else {
                warn!(
                    "Mode {} is no longer available (trying to select next best mode)",
                    crtc_info.mode
                );
                let outputs = self.outputs.borrow();
                if let Some(m) = crtc_info
                    .outputs
                    .first()
                    .and_then(|output_id| outputs.get(output_id))
                    .and_then(|output_info| output_info.modes.first())
                {
                    debug!("Selecting replacement mode {m}");
                    mode = *m;
                }
            }

            if mode > 0 {
                handle_no_reply_error(
                    set_crtc_transform(
                        &self.conn,
                        crtc_id,
                        snapshot.transforms[&crtc_id],
                        "bilinear".as_bytes(),
                        &[],
                    ),
                    "set CRTC transform",
                );
                handle_reply_error(
                    self.update_crtc(
                        crtc_id,
                        crtc_info.x,
                        crtc_info.y,
                        mode,
                        crtc_info.rotation,
                        &crtc_info.outputs,
                    ),
                    "revert CRTC",
                );
            } else {
                error!("No mode for CRTC {crtc_id}");
            }
        }
    }
}

pub fn check() -> Result<(), Box<dyn Error>> {
    if let Ok((conn, _)) = x11rb::connect(DISPLAY) {
        let extension = query_extension(&conn, randr::X11_EXTENSION_NAME.as_bytes())?.reply()?;
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
    let (conn, screen_num) = x11rb::connect(DISPLAY)?;
    let root = conn.setup().roots[screen_num].root;

    conn.randr_select_input(
        root,
        NotifyMask::SCREEN_CHANGE | NotifyMask::CRTC_CHANGE | NotifyMask::OUTPUT_CHANGE,
    )?
    .check()?;

    let handle = thread::spawn(move || {
        while let Ok(event) = conn.wait_for_event() {
            sender.send_blocking(event).expect("channel should be open");
        }
    });

    Ok(handle)
}

fn get_edid(conn: &RustConnection, output: OutputId) -> Result<Edid, Box<dyn Error>> {
    let name = "EDID";
    let property = intern_atom(conn, true, name.as_bytes())?.reply()?.atom;
    if property == AtomEnum::NONE.into() {
        return Err(format!("No property named: {name}").into());
    }
    Ok(get_output_property(conn, output, property, AtomEnum::INTEGER, 0, 256, false, false)?
        .reply()?
        .data)
}

fn get_monitor_name(edid: &[u8]) -> Option<String> {
    if edid.len() >= 128 {
        let version = edid[0x12];
        let revision = edid[0x13];
        if version == 1 && (revision == 3 || revision == 4) {
            let mut i = 0x48;
            while i <= 0x6C {
                // This 18 byte descriptor is a used as a display descriptor with a tag 0xFC
                // (display product name).
                if edid[i..(i + 3)] == [0, 0, 0] && edid[i + 3] == 0xFC && edid[i + 4] == 0 {
                    return Some(
                        String::from_utf8_lossy(&edid[(i + 5)..(i + 18)]).trim_end().to_owned(),
                    );
                }
                i += 18;
            }
        }
    }
    None
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

fn request_transforms<'a, Conn: RequestConnection>(
    conn: &'a Conn,
    res: &ScreenResources,
) -> Result<HashMap<CrtcId, Cookie<'a, Conn, GetCrtcTransformReply>>, ConnectionError> {
    let mut cookies = HashMap::new();
    for crtc in &res.crtcs {
        cookies.insert(*crtc, get_crtc_transform(conn, *crtc)?);
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

fn get_transforms(
    cookies: HashMap<CrtcId, Cookie<impl RequestConnection, GetCrtcTransformReply>>,
) -> Result<HashMap<CrtcId, Transform>, ReplyError> {
    let mut crtcs = HashMap::new();
    for (crtc, c) in cookies {
        crtcs.insert(crtc, c.reply()?.current_transform);
    }
    Ok(crtcs)
}

#[cfg(debug_assertions)]
#[allow(clippy::use_debug)]
fn log_crtcs(
    crtcs: &HashMap<CrtcId, CrtcInfo>,
    modes: &HashMap<ModeId, ModeInfo>,
    transforms: &HashMap<CrtcId, Transform>,
) {
    for (crtc_id, crtc) in crtcs {
        debug!("{:-^40}", format!(" CRTC {crtc_id} "));
        debug!("XID:       {crtc_id}");
        debug!("Pos:       +{}+{}", crtc.x, crtc.y);
        debug!("Dim:       {}x{}", crtc.width, crtc.height);
        if crtc.mode > 0 {
            debug!("Mode:      {}: {}", crtc.mode, Mode::from(modes[&crtc.mode]));
        }
        debug!("Outputs:   {:?}", crtc.outputs);
        debug!("Rot:       {:#?}", crtc.rotation);
        debug!("Possible:  {:?}", crtc.possible);
        log_transform(&transforms[crtc_id]);
    }
}

#[cfg(debug_assertions)]
#[allow(clippy::use_debug)]
fn log_transform(transform: &Transform) {
    let Transform {
        matrix11: a11,
        matrix12: a12,
        matrix13: a13,
        matrix21: a21,
        matrix22: a22,
        matrix23: a23,
        matrix31: a31,
        matrix32: a32,
        matrix33: a33,
    } = transform;
    debug!("Transform: | {a11:^5} {a12:^5} {a13:^5} |");
    debug!("           | {a21:^5} {a22:^5} {a23:^5} |");
    debug!("           | {a31:^5} {a32:^5} {a33:^5} |");
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

pub fn gen_xrandr_command(outputs: &Outputs) -> String {
    let mut cmd = "xrandr".to_owned();
    for (i, output) in outputs.iter::<Output>().map(Result::unwrap).enumerate() {
        let pad = if i == 0 { 1 } else { 7 };
        let nl = if u32::try_from(i).unwrap() < outputs.n_items() - 1 { " \\\n" } else { "" };
        cmd += &format!("{:>pad$}--output {}", "", &output.name());
        if let Some(mode) = output.mode() {
            cmd += &format!(" --mode {}x{}", mode.width(), mode.height());
            cmd += &format!(" --rate {:.2}", mode.refresh());
            cmd += &format!(" --pos {}x{}", output.x(), output.y());
            cmd += &format!(" --rotate {}", output.rotation().xrandr());
            cmd += &format!(" --reflect {}", output.reflection().xrandr());
            if nearly_eq(output.scale_x(), output.scale_y()) {
                cmd += &format!(" --scale {:.2}", output.scale_x());
            } else {
                cmd += &format!(" --scale {:.2}x{:.2}", output.scale_x(), output.scale_y());
            }
            if output.primary() {
                cmd += " --primary";
                cmd += &format!(" --dpi {}", &output.name());
            }
            cmd += nl;
        } else {
            cmd += &format!(" --off{nl}");
        }
    }
    cmd
}

struct Fixed(i32);

impl From<Fixed> for f64 {
    fn from(f: Fixed) -> Self { f64::from(f.0) / 65536. }
}

impl From<f64> for Fixed {
    fn from(f: f64) -> Self { Self((f * 65536.) as i32) }
}
