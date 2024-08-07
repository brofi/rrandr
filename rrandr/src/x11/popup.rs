use std::collections::HashMap;
use std::error::Error;
use std::ptr;
use std::time::{Duration, Instant};

use cairo::{XCBDrawable, XCBSurface, XCBVisualType};
use config::Config;
use gio::spawn_blocking;
use glib::spawn_future_local;
use gtk::prelude::WidgetExt;
use gtk::{gio, glib, Button};
use log::{error, warn};
use x11rb::connection::Connection as XConnection;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_info, get_screen_resources_current, Mode as ModeId, ModeInfo,
    Rotation as RRotation,
};
use x11rb::protocol::xproto::{
    ConnectionExt, CreateWindowAux, EventMask, Visualtype, Window as WindowId, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::x11_utils::Serialize;
use x11rb::xcb_ffi::XCBConnection;

use super::x_error_to_string;
use crate::data::output::PPMM_DEFAULT;
use crate::draw::DrawContext;
use crate::math::Rect;

fn create_popup_window(
    conn: &impl XConnection,
    screen_num: usize,
    rect: &Rect,
) -> Result<WindowId, ReplyOrIdError> {
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
    visual_type: &mut [u8; 24],
    wid: WindowId,
    width: i32,
    height: i32,
) -> Result<XCBSurface, cairo::Error> {
    let Some(conn_ptr) = ptr::NonNull::new(conn.get_raw_xcb_connection().cast()) else {
        return Err(cairo::Error::NullPointer);
    };

    let Some(visual_ptr) = ptr::NonNull::new(visual_type.as_mut_ptr().cast()) else {
        return Err(cairo::Error::NullPointer);
    };

    XCBSurface::create(
        &cairo::XCBConnection(conn_ptr),
        &XCBDrawable(wid),
        &XCBVisualType(visual_ptr),
        width,
        height,
    )
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

#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
fn create_popup_windows(
    config: &Config,
    conn: &XCBConnection,
    screen_num: usize,
    visual_type: &mut [u8; 24],
) -> Result<HashMap<WindowId, XCBSurface>, Box<dyn Error>> {
    let mut windows = HashMap::new();
    let screen = &conn.setup().roots[screen_num];
    let res = get_screen_resources_current(&conn, screen.root)?.reply()?;
    let rr_modes: HashMap<ModeId, &ModeInfo> = res.modes.iter().map(|m| (m.id, m)).collect();

    for output in &res.outputs {
        let output_info = get_output_info(conn, *output, res.timestamp)?.reply()?;
        if output_info.crtc == 0 {
            continue;
        }

        let crtc_info = get_crtc_info(conn, output_info.crtc, res.timestamp)?.reply()?;
        if crtc_info.mode == 0 {
            continue;
        }

        let mode = rr_modes[&crtc_info.mode];
        let ppmm = if output_info.mm_width > 0 && output_info.mm_height > 0 {
            let mut ppmm = [
                f64::from(mode.width) / f64::from(output_info.mm_width),
                f64::from(mode.height) / f64::from(output_info.mm_height),
            ];
            match crtc_info.rotation {
                RRotation::ROTATE90 | RRotation::ROTATE270 => ppmm.reverse(),
                _ => (),
            }
            ppmm
        } else {
            PPMM_DEFAULT
        };

        let ratio = config.popup.ratio;
        let spacing = f64::from(config.popup.spacing);

        let spacing_x = ((spacing * ppmm[0]).round() as u16).min(crtc_info.width / 2 - 1);
        let spacing_y = ((spacing * ppmm[1]).round() as u16).min(crtc_info.height / 2 - 1);

        let width = (f64::from(crtc_info.width) * ratio)
            .round()
            .min(f64::from(crtc_info.width - (2 * spacing_x)))
            .max(1.) as u16;
        let height = (f64::from(crtc_info.height) * ratio)
            .round()
            .min(f64::from(crtc_info.height - (2 * spacing_y)))
            .max(1.) as u16;

        let x = (i32::from(crtc_info.x) + i32::from(spacing_x)).try_into().unwrap_or(i16::MAX);
        let y = (i32::from(crtc_info.y) + i32::from(crtc_info.height)
            - i32::from(spacing_y)
            - i32::from(height))
        .try_into()
        .unwrap_or(i16::MAX);

        let rect = Rect::new(x, y, width, height);
        let wid = create_popup_window(&conn, screen_num, &rect)?;
        let surface =
            create_popup_surface(conn, visual_type, wid, i32::from(width), i32::from(height))?;
        let cr = cairo::Context::new(&surface)?;
        let context = DrawContext::new(&cr, config);
        let pad = f64::from(config.popup.padding) * ppmm[1];
        let border = f64::from(config.popup.border_width) * ppmm[1];
        context.draw_popup(&rect, border, pad, &String::from_utf8_lossy(&output_info.name))?;

        surface.flush();
        windows.insert(wid, surface);
    }
    Ok(windows)
}

pub fn show_popup_windows(cfg: &Config, btn: &Button) -> Result<(), Box<dyn Error>> {
    let show_secs = cfg.popup.timeout;
    if show_secs < 0. {
        warn!("Negative show duration: {}", show_secs);
        return Ok(());
    }

    let (conn, screen_num) = XCBConnection::connect(None)?;
    let Some(visual_type) = get_root_visual_type(&conn, screen_num) else {
        return Err("Failed to get root visual type".into());
    };
    // must outlive visual pointer
    let mut visual_type = visual_type.serialize();
    let popups = create_popup_windows(cfg, &conn, screen_num, &mut visual_type)?;
    conn.flush()?;

    spawn_future_local({
        let btn = btn.clone();
        async move {
            btn.set_sensitive(false);
            if let Ok(res) =
                spawn_blocking(move || -> Result<(), ReplyError> { loop_x(&conn, show_secs) }).await
            {
                if let Err(e) = res {
                    error!("Failed to show popups:");
                    if let ReplyError::X11Error(e) = e {
                        error!("{}", x_error_to_string(&e));
                    } else {
                        error!("Cause: {e:?}");
                    }
                }
            } else {
                error!("Failed to await future");
            }
            btn.set_sensitive(true);
            for surface in popups.values() {
                if let Some(device) = surface.device() {
                    device.finish();
                }
                surface.finish();
            }
        }
    });

    Ok(())
}

fn loop_x(conn: &XCBConnection, secs: f32) -> Result<(), ReplyError> {
    let now = Instant::now();
    let secs = Duration::from_secs_f32(secs);
    while now.elapsed() < secs {
        match conn.poll_for_event()? {
            Some(Event::ButtonPress(e)) => {
                if e.detail == 1 {
                    break;
                }
            }
            Some(Event::Error(e)) => {
                return Err(e.into());
            }
            _ => (),
        }
    }
    Ok(())
}
