use std::borrow::Borrow;
use std::collections::HashMap;
use std::error::Error;

use gtk::prelude::ListModelExtManual;
use log::{debug, error};
use x11rb::connection::{Connection as XConnection, RequestConnection};
use x11rb::cookie::{Cookie, VoidCookie};
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_info, get_output_primary, get_output_property,
    get_screen_resources_current, get_screen_size_range, set_crtc_config, set_output_primary,
    set_screen_size, Connection, Crtc as CrtcId, GetCrtcInfoReply, GetOutputInfoReply,
    GetOutputPrimaryReply, GetScreenResourcesCurrentReply, GetScreenSizeRangeReply, Mode as ModeId,
    ModeInfo, Output as OutputId, Rotation, ScreenSize, SetConfig,
};
use x11rb::protocol::xproto::{intern_atom, AtomEnum, Window as WindowId};
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

use super::x_error_to_string;
use crate::data::mode::Mode;
use crate::data::modes::Modes;
use crate::data::output::{Output, PPI_DEFAULT};
use crate::data::outputs::Outputs;
use crate::math::{Rect, MM_PER_INCH};

pub type ScreenSizeRange = GetScreenSizeRangeReply;
type ScreenResources = GetScreenResourcesCurrentReply;
pub type OutputInfo = GetOutputInfoReply;
type CrtcInfo = GetCrtcInfoReply;
type Primary = GetOutputPrimaryReply;

pub struct Randr {
    conn: RustConnection,
    root: WindowId,
    screen_size: ScreenSize,
    screen_size_range: ScreenSizeRange,
    primary: Primary,
    crtcs: HashMap<CrtcId, CrtcInfo>,
    outputs: HashMap<OutputId, OutputInfo>,
    modes: HashMap<ModeId, ModeInfo>,
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

        Self { conn, root, screen_size, screen_size_range, primary, crtcs, outputs, modes }
    }

    pub fn screen_size_range(&self) -> ScreenSizeRange { self.screen_size_range }

    pub fn output_model(&self) -> Outputs {
        let outputs = Outputs::default();
        for (id, output_info) in self.outputs.borrow() {
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
                *id == self.primary.output,
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
        let screen_size_changed = self.screen_size.width != screen_size.width
            || self.screen_size.height != screen_size.height;

        // Disable outputs
        for output in outputs.iter::<Output>().map(Result::unwrap) {
            let crtc_id = self.outputs.borrow()[&output.id()].crtc;
            if crtc_id == 0 {
                // Output already disabled
                continue;
            }
            let crtc = &self.crtcs.borrow()[&crtc_id];
            if !output.enabled()
                || (screen_size_changed
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

        if screen_size_changed {
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
        for (crtc_id, crtc) in self.crtcs.borrow() {
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

    pub fn revert(&self) {
        debug!("Reverting changes");
        for crtc_id in self.crtcs.borrow().keys() {
            self.disable_crtc(*crtc_id).expect("disable CRTC");
        }
        debug!(
            "Reverting screen size to {}x{} px, {}x{} mm",
            self.screen_size.width,
            self.screen_size.height,
            self.screen_size.mwidth,
            self.screen_size.mheight
        );
        set_screen_size(
            &self.conn,
            self.root,
            self.screen_size.width,
            self.screen_size.height,
            self.screen_size.mwidth.into(),
            self.screen_size.mheight.into(),
        )
        .expect("revert screen size request")
        .check()
        .expect("revert screen size");
        for (crtc_id, crtc_info) in self.crtcs.borrow() {
            set_crtc_config(
                &self.conn,
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
