//! Audio output through ALSA, loaded at run time with dlopen so the program
//! still starts (silently) on a system without libasound.

use super::{Mixer, Shared, SAMPLE_RATE};
use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString};
use std::sync::atomic::Ordering;
use std::sync::Arc;

extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}
const RTLD_NOW: c_int = 2;

type Pcm = *mut c_void;
type OpenFn = unsafe extern "C" fn(*mut Pcm, *const c_char, c_int, c_int) -> c_int;
type SetParamsFn = unsafe extern "C" fn(Pcm, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int;
type WriteiFn = unsafe extern "C" fn(Pcm, *const c_void, c_ulong) -> c_long;
type RecoverFn = unsafe extern "C" fn(Pcm, c_int, c_int) -> c_int;
type DelayFn = unsafe extern "C" fn(Pcm, *mut c_long) -> c_int;

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_FLOAT_LE: c_int = 14;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;

unsafe fn sym<T>(lib: *mut c_void, name: &str) -> Option<T> {
    let n = CString::new(name).unwrap();
    let p = dlsym(lib, n.as_ptr());
    if p.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy(&p))
    }
}

/// Start the audio thread. Returns false if no audio device is available.
pub fn start(sh: Arc<Shared>) -> bool {
    unsafe {
        let mut lib = std::ptr::null_mut();
        for name in ["libasound.so.2", "libasound.so"] {
            let n = CString::new(name).unwrap();
            lib = dlopen(n.as_ptr(), RTLD_NOW);
            if !lib.is_null() {
                break;
            }
        }
        if lib.is_null() {
            eprintln!("bcw: libasound not found, no sound");
            return false;
        }
        let (Some(open), Some(set_params), Some(writei), Some(recover), Some(delay)) = (
            sym::<OpenFn>(lib, "snd_pcm_open"),
            sym::<SetParamsFn>(lib, "snd_pcm_set_params"),
            sym::<WriteiFn>(lib, "snd_pcm_writei"),
            sym::<RecoverFn>(lib, "snd_pcm_recover"),
            sym::<DelayFn>(lib, "snd_pcm_delay"),
        ) else {
            eprintln!("bcw: libasound is missing functions, no sound");
            return false;
        };
        let mut pcm: Pcm = std::ptr::null_mut();
        let dev = CString::new("default").unwrap();
        if open(&mut pcm, dev.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0) < 0 {
            eprintln!("bcw: cannot open audio device, no sound");
            return false;
        }
        if set_params(
            pcm,
            SND_PCM_FORMAT_FLOAT_LE,
            SND_PCM_ACCESS_RW_INTERLEAVED,
            1,
            SAMPLE_RATE,
            1,
            40_000,
        ) < 0
        {
            eprintln!("bcw: cannot configure audio device, no sound");
            return false;
        }
        let pcm_addr = pcm as usize;
        std::thread::Builder::new()
            .name("bcw-audio".into())
            .spawn(move || {
                let pcm = pcm_addr as Pcm;
                let mut mixer = Mixer::new(&sh);
                let mut buf = vec![0f32; 441];
                while !sh.quit.load(Ordering::SeqCst) && !sh.game_over.load(Ordering::SeqCst) {
                    let mut d: c_long = 0;
                    let buffered = if delay(pcm, &mut d) == 0 {
                        d as f64 * 1e6 / SAMPLE_RATE as f64
                    } else {
                        0.0
                    };
                    mixer.resync(&sh, buffered);
                    sh.render_audio(&mut mixer, &mut buf);
                    let mut off = 0usize;
                    while off < buf.len() {
                        let n = writei(
                            pcm,
                            buf[off..].as_ptr() as *const c_void,
                            (buf.len() - off) as c_ulong,
                        );
                        if n < 0 {
                            if recover(pcm, n as c_int, 1) < 0 {
                                return;
                            }
                        } else {
                            off += n as usize;
                        }
                    }
                }
            })
            .is_ok()
    }
}
