//! Linux window through Xlib (works on X11 and on Wayland via XWayland).

use super::{bios_key, cga_to_rgb, scale_into, Shared, Special};
use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

type Display = c_void;
type Window = c_ulong;
type Atom = c_ulong;
type Gc = *mut c_void;
type Visual = c_void;

#[repr(C)]
struct XImage {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut c_char,
    // remaining fields are not touched
    _rest: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XKeyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    event: Window,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    message_type: Atom,
    format: c_int,
    data: [c_long; 5],
}

#[repr(C)]
union XEvent {
    type_: c_int,
    key: XKeyEvent,
    configure: XConfigureEvent,
    client: XClientMessageEvent,
    pad: [c_long; 24],
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XDefaultScreen(d: *mut Display) -> c_int;
    fn XRootWindow(d: *mut Display, screen: c_int) -> Window;
    fn XBlackPixel(d: *mut Display, screen: c_int) -> c_ulong;
    fn XCreateSimpleWindow(
        d: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        border_w: c_uint,
        border: c_ulong,
        bg: c_ulong,
    ) -> Window;
    fn XStoreName(d: *mut Display, w: Window, name: *const c_char) -> c_int;
    fn XSelectInput(d: *mut Display, w: Window, mask: c_long) -> c_int;
    fn XMapWindow(d: *mut Display, w: Window) -> c_int;
    fn XInternAtom(d: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XSetWMProtocols(d: *mut Display, w: Window, protocols: *mut Atom, count: c_int) -> c_int;
    fn XPending(d: *mut Display) -> c_int;
    fn XNextEvent(d: *mut Display, ev: *mut XEvent) -> c_int;
    fn XLookupString(
        ev: *mut XKeyEvent,
        buf: *mut c_char,
        len: c_int,
        keysym: *mut c_ulong,
        status: *mut c_void,
    ) -> c_int;
    fn XDefaultGC(d: *mut Display, screen: c_int) -> Gc;
    fn XDefaultVisual(d: *mut Display, screen: c_int) -> *mut Visual;
    fn XDefaultDepth(d: *mut Display, screen: c_int) -> c_int;
    fn XCreateImage(
        d: *mut Display,
        visual: *mut Visual,
        depth: c_uint,
        format: c_int,
        offset: c_int,
        data: *mut c_char,
        width: c_uint,
        height: c_uint,
        bitmap_pad: c_int,
        bytes_per_line: c_int,
    ) -> *mut XImage;
    fn XPutImage(
        d: *mut Display,
        w: Window,
        gc: Gc,
        img: *mut XImage,
        sx: c_int,
        sy: c_int,
        dx: c_int,
        dy: c_int,
        w_: c_uint,
        h: c_uint,
    ) -> c_int;
    fn XFlush(d: *mut Display) -> c_int;
    fn XCloseDisplay(d: *mut Display) -> c_int;
}

const KEY_PRESS: c_int = 2;
const EXPOSE: c_int = 12;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;
const KEY_PRESS_MASK: c_long = 1 << 0;
const EXPOSURE_MASK: c_long = 1 << 15;
const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;
const ZPIXMAP: c_int = 2;

pub fn run(sh: Arc<Shared>, title: &str) -> Result<(), String> {
    unsafe {
        let d = XOpenDisplay(std::ptr::null());
        if d.is_null() {
            return Err("cannot open the X display (is DISPLAY set?)".into());
        }
        let scr = XDefaultScreen(d);
        let depth = XDefaultDepth(d, scr);
        if depth < 24 {
            return Err(format!(
                "needs a 24/32-bit display, this one is {}-bit",
                depth
            ));
        }
        let (mut w, mut h) = (960usize, 720usize);
        let win = XCreateSimpleWindow(
            d,
            XRootWindow(d, scr),
            0,
            0,
            w as c_uint,
            h as c_uint,
            0,
            0,
            XBlackPixel(d, scr),
        );
        let t = CString::new(title).unwrap();
        XStoreName(d, win, t.as_ptr());
        XSelectInput(
            d,
            win,
            KEY_PRESS_MASK | EXPOSURE_MASK | STRUCTURE_NOTIFY_MASK,
        );
        let wm_delete_name = CString::new("WM_DELETE_WINDOW").unwrap();
        let mut wm_delete = XInternAtom(d, wm_delete_name.as_ptr(), 0);
        XSetWMProtocols(d, win, &mut wm_delete, 1);
        XMapWindow(d, win);
        let gc = XDefaultGC(d, scr);
        let visual = XDefaultVisual(d, scr);

        let mut src = vec![0u32; 320 * 200];
        let mut dst: Vec<u32> = vec![0; w * h];
        let mut img = XCreateImage(
            d,
            visual,
            depth as c_uint,
            ZPIXMAP,
            0,
            dst.as_mut_ptr() as *mut c_char,
            w as c_uint,
            h as c_uint,
            32,
            0,
        );
        let mut last_serial = u64::MAX;
        let mut dirty = true;
        let mut ev: XEvent = std::mem::zeroed();

        loop {
            while XPending(d) > 0 {
                XNextEvent(d, &mut ev);
                match ev.type_ {
                    KEY_PRESS => {
                        let mut buf = [0 as c_char; 8];
                        let mut keysym: c_ulong = 0;
                        let n = XLookupString(
                            &mut ev.key,
                            buf.as_mut_ptr(),
                            8,
                            &mut keysym,
                            std::ptr::null_mut(),
                        );
                        let special = match keysym {
                            0xFF52 | 0xFF97 => Special::Up,
                            0xFF54 | 0xFF99 => Special::Down,
                            0xFF51 | 0xFF96 => Special::Left,
                            0xFF53 | 0xFF98 => Special::Right,
                            _ => Special::None,
                        };
                        let ascii = if keysym == 0xFF8D {
                            13
                        } else if n > 0 {
                            buf[0] as u8
                        } else {
                            0
                        };
                        if let Some(k) = bios_key(ascii, special) {
                            sh.push_key(k);
                        }
                    }
                    CONFIGURE_NOTIFY => {
                        let (nw, nh) = (
                            ev.configure.width.max(1) as usize,
                            ev.configure.height.max(1) as usize,
                        );
                        if nw != w || nh != h {
                            w = nw;
                            h = nh;
                            // the old XImage only points into our buffer; leak the small header
                            (*img).data = std::ptr::null_mut();
                            dst = vec![0; w * h];
                            img = XCreateImage(
                                d,
                                visual,
                                depth as c_uint,
                                ZPIXMAP,
                                0,
                                dst.as_mut_ptr() as *mut c_char,
                                w as c_uint,
                                h as c_uint,
                                32,
                                0,
                            );
                            dirty = true;
                        }
                    }
                    EXPOSE => dirty = true,
                    CLIENT_MESSAGE => {
                        if ev.client.data[0] as Atom == wm_delete {
                            sh.request_quit();
                        }
                    }
                    _ => {}
                }
            }
            if sh.quit.load(Ordering::SeqCst) || sh.game_over.load(Ordering::SeqCst) {
                break;
            }
            {
                let fr = sh.frame.lock().unwrap();
                if fr.serial != last_serial {
                    last_serial = fr.serial;
                    cga_to_rgb(&fr, &mut src);
                    dirty = true;
                }
            }
            if dirty {
                scale_into(&src, &mut dst, w, h);
                XPutImage(d, win, gc, img, 0, 0, 0, 0, w as c_uint, h as c_uint);
                XFlush(d);
                dirty = false;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        (*img).data = std::ptr::null_mut();
        XCloseDisplay(d);
        Ok(())
    }
}
