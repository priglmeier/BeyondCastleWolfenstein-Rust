//! Windows window (Win32 + GDI) and audio (waveOut). Plain FFI, no crates.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use super::{bios_key, cga_to_rgb, Mixer, Shared, Special, SAMPLE_RATE};
use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

type HANDLE = *mut c_void;
type HWND = HANDLE;
type HDC = HANDLE;
type HINSTANCE = HANDLE;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;
type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: WNDPROC,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HANDLE,
    hCursor: HANDLE,
    hbrBackground: HANDLE,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
    lPrivate: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PAINTSTRUCT {
    hdc: HDC,
    fErase: i32,
    rcPaint: RECT,
    fRestore: i32,
    fIncUpdate: i32,
    rgbReserved: [u8; 32],
}

#[repr(C)]
struct BITMAPINFOHEADER {
    biSize: u32,
    biWidth: i32,
    biHeight: i32,
    biPlanes: u16,
    biBitCount: u16,
    biCompression: u32,
    biSizeImage: u32,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: u32,
    biClrImportant: u32,
}

#[repr(C)]
struct BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER,
    bmiColors: [u32; 1],
}

#[repr(C)]
struct WAVEFORMATEX {
    wFormatTag: u16,
    nChannels: u16,
    nSamplesPerSec: u32,
    nAvgBytesPerSec: u32,
    nBlockAlign: u16,
    wBitsPerSample: u16,
    cbSize: u16,
}

#[repr(C)]
struct WAVEHDR {
    lpData: *mut i8,
    dwBufferLength: u32,
    dwBytesRecorded: u32,
    dwUser: usize,
    dwFlags: u32,
    dwLoops: u32,
    lpNext: *mut WAVEHDR,
    reserved: usize,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: HANDLE,
        inst: HINSTANCE,
        param: *mut c_void,
    ) -> HWND;
    fn DefWindowProcW(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT;
    fn ShowWindow(h: HWND, cmd: i32) -> i32;
    fn PeekMessageW(msg: *mut MSG, h: HWND, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(msg: *const MSG) -> i32;
    fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    fn GetClientRect(h: HWND, r: *mut RECT) -> i32;
    fn GetDC(h: HWND) -> HDC;
    fn ReleaseDC(h: HWND, dc: HDC) -> i32;
    fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> i32;
    fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> HANDLE;
    fn AdjustWindowRect(r: *mut RECT, style: u32, menu: i32) -> i32;
    fn DestroyWindow(h: HWND) -> i32;
    fn MessageBoxW(h: HWND, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn StretchDIBits(
        dc: HDC,
        xd: i32,
        yd: i32,
        wd: i32,
        hd: i32,
        xs: i32,
        ys: i32,
        ws: i32,
        hs: i32,
        bits: *const c_void,
        bmi: *const BITMAPINFO,
        usage: u32,
        rop: u32,
    ) -> i32;
    fn SetStretchBltMode(dc: HDC, mode: i32) -> i32;
    fn PatBlt(dc: HDC, x: i32, y: i32, w: i32, h: i32, rop: u32) -> i32;
    fn GetStockObject(i: i32) -> HANDLE;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
}

#[link(name = "winmm")]
extern "system" {
    fn waveOutOpen(
        h: *mut HANDLE,
        dev: u32,
        fmt: *const WAVEFORMATEX,
        cb: usize,
        inst: usize,
        flags: u32,
    ) -> u32;
    fn waveOutPrepareHeader(h: HANDLE, hdr: *mut WAVEHDR, size: u32) -> u32;
    fn waveOutWrite(h: HANDLE, hdr: *mut WAVEHDR, size: u32) -> u32;
}

const WM_DESTROY: u32 = 0x0002;
const WM_SIZE: u32 = 0x0005;
const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;
const PM_REMOVE: u32 = 1;
const SW_SHOW: i32 = 5;
const IDC_ARROW: usize = 32512;
const BLACK_BRUSH: i32 = 4;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;
const BLACKNESS: u32 = 0x0000_0042;
const COLORONCOLOR: i32 = 3;
const WAVE_MAPPER: u32 = 0xFFFF_FFFF;
const WAVE_FORMAT_PCM: u16 = 1;
const WHDR_DONE: u32 = 1;

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
static mut PIXELS: [u32; 320 * 200] = [0; 320 * 200];

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn paint(h: HWND, dc: HDC) {
    let mut r = RECT::default();
    GetClientRect(h, &mut r);
    let (w, hh) = (r.right - r.left, r.bottom - r.top);
    let (mut vw, mut vh) = (w, w * 3 / 4);
    if vh > hh {
        vh = hh;
        vw = hh * 4 / 3;
    }
    let (ox, oy) = ((w - vw) / 2, (hh - vh) / 2);
    PatBlt(dc, 0, 0, w, oy, BLACKNESS);
    PatBlt(dc, 0, oy + vh, w, hh - oy - vh, BLACKNESS);
    PatBlt(dc, 0, oy, ox, vh, BLACKNESS);
    PatBlt(dc, ox + vw, oy, w - ox - vw, vh, BLACKNESS);
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: 320,
            biHeight: -200,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [0],
    };
    SetStretchBltMode(dc, COLORONCOLOR);
    StretchDIBits(
        dc,
        ox,
        oy,
        vw,
        vh,
        0,
        0,
        320,
        200,
        std::ptr::addr_of!(PIXELS) as *const c_void,
        &bmi,
        DIB_RGB_COLORS,
        SRCCOPY,
    );
}

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let sh = SHARED.get();
    match msg {
        WM_CHAR => {
            if let Some(sh) = sh {
                let c = wp as u32;
                if c < 128 {
                    if let Some(k) = bios_key(c as u8, Special::None) {
                        sh.push_key(k);
                    }
                }
            }
            0
        }
        WM_KEYDOWN => {
            let sp = match wp {
                0x25 => Special::Left,
                0x26 => Special::Up,
                0x27 => Special::Right,
                0x28 => Special::Down,
                _ => Special::None,
            };
            if sp != Special::None {
                if let Some(sh) = sh {
                    if let Some(k) = bios_key(0, sp) {
                        sh.push_key(k);
                    }
                }
                return 0;
            }
            DefWindowProcW(h, msg, wp, lp)
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(h, &mut ps);
            paint(h, dc);
            EndPaint(h, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_SIZE => 0,
        WM_CLOSE => {
            if let Some(sh) = sh {
                sh.request_quit();
            }
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}

/// Start the waveOut audio thread.
pub fn start_audio(sh: Arc<Shared>) -> bool {
    std::thread::Builder::new()
        .name("bcw-audio".into())
        .spawn(move || unsafe {
            let fmt = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM,
                nChannels: 1,
                nSamplesPerSec: SAMPLE_RATE,
                nAvgBytesPerSec: SAMPLE_RATE * 2,
                nBlockAlign: 2,
                wBitsPerSample: 16,
                cbSize: 0,
            };
            let mut wo: HANDLE = std::ptr::null_mut();
            if waveOutOpen(&mut wo, WAVE_MAPPER, &fmt, 0, 0, 0) != 0 {
                return;
            }
            const NBUF: usize = 4;
            const FRAMES: usize = 441;
            let mut data: Vec<Vec<i16>> = (0..NBUF).map(|_| vec![0i16; FRAMES]).collect();
            let mut hdrs: Vec<WAVEHDR> = (0..NBUF)
                .map(|i| WAVEHDR {
                    lpData: data[i].as_mut_ptr() as *mut i8,
                    dwBufferLength: (FRAMES * 2) as u32,
                    dwBytesRecorded: 0,
                    dwUser: 0,
                    dwFlags: 0,
                    dwLoops: 0,
                    lpNext: std::ptr::null_mut(),
                    reserved: 0,
                })
                .collect();
            let hsize = std::mem::size_of::<WAVEHDR>() as u32;
            for hd in hdrs.iter_mut() {
                waveOutPrepareHeader(wo, hd, hsize);
                hd.dwFlags |= WHDR_DONE;
            }
            let mut mixer = Mixer::new(&sh);
            let mut fbuf = vec![0f32; FRAMES];
            let mut i = 0usize;
            while !sh.quit.load(Ordering::SeqCst) && !sh.game_over.load(Ordering::SeqCst) {
                let flags = std::ptr::read_volatile(&hdrs[i].dwFlags);
                if flags & WHDR_DONE == 0 {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                let queued = (0..NBUF)
                    .filter(|&j| std::ptr::read_volatile(&hdrs[j].dwFlags) & WHDR_DONE == 0)
                    .count();
                mixer.resync(
                    &sh,
                    queued as f64 * FRAMES as f64 * 1e6 / SAMPLE_RATE as f64,
                );
                sh.render_audio(&mut mixer, &mut fbuf);
                for (d, s) in data[i].iter_mut().zip(fbuf.iter()) {
                    *d = (s * 32000.0) as i16;
                }
                hdrs[i].dwFlags &= !WHDR_DONE;
                waveOutWrite(wo, &mut hdrs[i], hsize);
                i = (i + 1) % NBUF;
            }
        })
        .is_ok()
}

pub fn run(sh: Arc<Shared>, title: &str) -> Result<(), String> {
    let _ = SHARED.set(sh.clone());
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide("BCWWindow");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16),
            hbrBackground: GetStockObject(BLACK_BRUSH),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            return Err("RegisterClass failed".into());
        }
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 960,
            bottom: 720,
        };
        AdjustWindowRect(&mut r, WS_OVERLAPPEDWINDOW, 0);
        let name = wide(title);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            name.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            r.right - r.left,
            r.bottom - r.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            inst,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            return Err("CreateWindow failed".into());
        }
        ShowWindow(hwnd, SW_SHOW);
        let mut last_serial = u64::MAX;
        let mut msg: MSG = std::mem::zeroed();
        loop {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if sh.quit.load(Ordering::SeqCst) || sh.game_over.load(Ordering::SeqCst) {
                break;
            }
            let changed = {
                let fr = sh.frame.lock().unwrap();
                if fr.serial != last_serial {
                    last_serial = fr.serial;
                    cga_to_rgb(&fr, &mut *std::ptr::addr_of_mut!(PIXELS));
                    true
                } else {
                    false
                }
            };
            if changed {
                let dc = GetDC(hwnd);
                paint(hwnd, dc);
                ReleaseDC(hwnd, dc);
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        DestroyWindow(hwnd);
        Ok(())
    }
}

pub fn error_box(msg: &str) {
    let t = wide(msg);
    let c = wide("Beyond Castle Wolfenstein");
    unsafe {
        MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), 0x10);
    }
}
