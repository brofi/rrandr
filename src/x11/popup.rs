use std::collections::HashMap;
use std::error::Error;
use std::time::{Duration, Instant};

use cairo::ffi::cairo_device_finish;
use cairo::{XCBDrawable, XCBSurface};
use gio::spawn_blocking;
use glib::spawn_future_local;
use gtk::prelude::WidgetExt;
use gtk::{gio, glib, Button};
use log::error;
use pango::{FontDescription, Weight};
use x11rb::connection::Connection as XConnection;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::protocol::randr::{
    get_crtc_info, get_output_info, get_screen_resources_current, Mode as ModeId, ModeInfo,
};
use x11rb::protocol::xproto::{
    ConnectionExt, CreateWindowAux, EventMask, Visualtype, Window as WindowId, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::x11_utils::Serialize;
use x11rb::xcb_ffi::XCBConnection;

use super::x_error_to_string;
use crate::config::Config;
use crate::draw::DrawContext;
use crate::math::Rect;

pub const POPUP_WINDOW_PAD: f64 = 20.;
const POPUP_OUTPUT_RATIO: f64 = 1. / 8.;
const POPUP_SHOW_SECS: f32 = 2.5;

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
    screen_num: usize,
    wid: WindowId,
    width: i32,
    height: i32,
) -> Result<XCBSurface, cairo::Error> {
    let cairo_conn =
        unsafe { cairo::XCBConnection::from_raw_none(conn.get_raw_xcb_connection().cast()) };
    let cairo_visual = unsafe {
        cairo::XCBVisualType::from_raw_none(
            get_root_visual_type(&conn, screen_num).unwrap().serialize().as_mut_ptr().cast(),
        )
    };
    XCBSurface::create(&cairo_conn, &XCBDrawable(wid), &cairo_visual, width, height)
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
) -> Result<HashMap<WindowId, XCBSurface>, Box<dyn Error>> {
    let mut windows = HashMap::new();
    let screen = &conn.setup().roots[screen_num];
    let res = get_screen_resources_current(&conn, screen.root)?.reply()?;
    let rr_modes: HashMap<ModeId, &ModeInfo> = res.modes.iter().map(|m| (m.id, m)).collect();
    let mut desc = FontDescription::new();
    desc.set_family("Sans");
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
            let context = DrawContext::new(cr, config.clone());
            context.draw_popup(&rect, &mut desc, &String::from_utf8_lossy(&output_info.name))?;
            surface.flush();
            windows.insert(wid, surface);
        }
    }
    Ok(windows)
}

pub fn show_popup_windows(btn: &Button) -> Result<(), Box<dyn Error>> {
    let (conn, screen_num) = XCBConnection::connect(None)?;
    let popups = create_popup_windows(&Config::default(), &conn, screen_num)?;
    conn.flush()?;

    spawn_future_local({
        let btn = btn.clone();
        async move {
            btn.set_sensitive(false);
            spawn_blocking(move || -> Result<(), ReplyError> { loop_x(&conn, POPUP_SHOW_SECS) })
                .await
                .expect("future awaited sucessfully")
                .expect("show popups finished sucessfully");
            btn.set_sensitive(true);
            for surface in popups.values() {
                unsafe { cairo_device_finish(surface.device().unwrap().to_raw_none()) };
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
                error!("{}", x_error_to_string(&e));
                return Err(e.into());
            }
            _ => (),
        }
    }
    Ok(())
}
