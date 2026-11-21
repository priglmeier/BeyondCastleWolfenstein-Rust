//! Real-time host: runs the game on its own thread and connects it to a
//! window, the keyboard and the sound card. The window/audio back ends are
//! plain FFI (Xlib + ALSA on Linux, Win32 + waveOut on Windows), so the
//! program has no external dependencies.

use crate::game::{jump, GameFile, Host, Jump, State};
use crate::pc::bios::VRAM_SIZE;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[cfg(all(unix, not(target_os = "macos")))]
pub mod alsa;
#[cfg(any(windows, feature = "wincheck"))]
pub mod win32;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod x11;

pub const SAMPLE_RATE: u32 = 44100;
/// Audio is scheduled this far behind the game clock so that events the
/// game produces in bursts (busy-wait loops) still come out evenly.
const AUDIO_LATENCY_US: f64 = 60_000.0;

/// What the window shows.
pub struct Frame {
    pub vram: Vec<u8>,
    pub color_select: u8,
    pub text_mode: bool,
    pub message: String,
    pub serial: u64,
}

struct SbVoice {
    data: Vec<u8>,
    rate: f64,
    start_us: f64,
}

/// Audio events, in microseconds since `Shared::epoch`.
#[derive(Default)]
pub struct AudioState {
    speaker: VecDeque<f64>,
    voices: Vec<SbVoice>,
}

pub struct Shared {
    pub frame: Mutex<Frame>,
    pub keys: Mutex<VecDeque<u16>>,
    pub key_cv: Condvar,
    pub quit: AtomicBool,
    pub game_over: AtomicBool,
    pub audio: Mutex<AudioState>,
    pub epoch: Instant,
}

impl Shared {
    pub fn new() -> Arc<Shared> {
        Arc::new(Shared {
            frame: Mutex::new(Frame {
                vram: vec![0; VRAM_SIZE],
                color_select: 0x30,
                text_mode: false,
                message: String::new(),
                serial: 0,
            }),
            keys: Mutex::new(VecDeque::new()),
            key_cv: Condvar::new(),
            quit: AtomicBool::new(false),
            game_over: AtomicBool::new(false),
            audio: Mutex::new(AudioState::default()),
            epoch: Instant::now(),
        })
    }

    pub fn now_us(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64() * 1e6
    }

    /// Called by the window when a key is typed (AH = scan code, AL = ASCII).
    pub fn push_key(&self, key: u16) {
        let mut k = self.keys.lock().unwrap();
        if k.len() < 15 {
            // the BIOS buffer holds 15 keys
            k.push_back(key);
        }
        self.key_cv.notify_all();
    }

    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::SeqCst);
        self.key_cv.notify_all();
    }

    /// Fill `out` (mono, f32) with audio for the stretch starting at
    /// `t0_us` (in the audio clock, which runs AUDIO_LATENCY behind).
    pub fn render_audio(&self, mixer: &mut Mixer, out: &mut [f32]) {
        let mut a = self.audio.lock().unwrap();
        let dt = 1e6 / SAMPLE_RATE as f64;
        for s in out.iter_mut() {
            let t = mixer.t_us - AUDIO_LATENCY_US;
            while let Some(&e) = a.speaker.front() {
                if e <= t {
                    mixer.level = -mixer.level;
                    mixer.last_toggle = t;
                    a.speaker.pop_front();
                } else {
                    break;
                }
            }
            // a speaker left in one position goes quiet (it is a cone, not a DC source)
            let target = if t - mixer.last_toggle > 20_000.0 {
                0.0
            } else {
                mixer.level * 0.25
            };
            mixer.lp += (target - mixer.lp) * 0.35;
            let mut v = mixer.lp - mixer.dc;
            mixer.dc += (mixer.lp - mixer.dc) * 0.0005;
            for voice in a.voices.iter() {
                let pos = (t - voice.start_us) * voice.rate / 1e6;
                if pos >= 0.0 && (pos as usize) < voice.data.len() {
                    v += (voice.data[pos as usize] as f32 - 128.0) / 128.0 * 0.6;
                }
            }
            *s = v.clamp(-1.0, 1.0);
            mixer.t_us += dt;
        }
        let t = mixer.t_us - AUDIO_LATENCY_US;
        a.voices
            .retain(|v| t < v.start_us + v.data.len() as f64 * 1e6 / v.rate);
        // never let the speaker queue fall far behind
        while let Some(&e) = a.speaker.front() {
            if e < t - 200_000.0 {
                a.speaker.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Per-device rendering state.
pub struct Mixer {
    t_us: f64,
    level: f32,
    lp: f32,
    dc: f32,
    last_toggle: f64,
}

impl Mixer {
    pub fn new(shared: &Shared) -> Mixer {
        Mixer {
            t_us: shared.now_us(),
            level: 1.0,
            lp: 0.0,
            dc: 0.0,
            last_toggle: -1e9,
        }
    }
    /// Keep the audio clock locked to wall-clock time (devices drift).
    pub fn resync(&mut self, shared: &Shared, buffered_us: f64) {
        let want = shared.now_us() + buffered_us;
        if (self.t_us - want).abs() > 100_000.0 {
            self.t_us = want;
        }
    }
}

/// The Host the game runs against when played for real.
pub struct RealHost {
    sh: Arc<Shared>,
    /// Virtual time of busy-wait loops, in us since the epoch.
    vt: f64,
    mark_tick: u64,
    sb_end: f64,
    dir: std::path::PathBuf,
}

const TICK_US: f64 = 1e6 * 65536.0 / 1_193_182.0; // 54.9 ms

impl RealHost {
    pub fn new(sh: Arc<Shared>, dir: std::path::PathBuf) -> RealHost {
        RealHost {
            sh,
            vt: 0.0,
            mark_tick: 0,
            sb_end: 0.0,
            dir,
        }
    }

    fn check_quit(&self) {
        if self.sh.quit.load(Ordering::SeqCst) {
            jump(Jump::Exit);
        }
    }

    /// Bring virtual time up to date; if busy-loops ran ahead of real time,
    /// sleep until real time catches up.
    fn sync(&mut self) {
        let now = self.sh.now_us();
        if self.vt < now {
            self.vt = now;
        } else if self.vt > now + 15_000.0 {
            std::thread::sleep(Duration::from_micros((self.vt - now) as u64));
        }
    }

    fn tick_now(&self) -> u64 {
        (self.sh.now_us() / TICK_US) as u64
    }

    fn file_path(&self, f: GameFile) -> std::path::PathBuf {
        let name = match f {
            GameFile::SaveGame => "savegame.bcw",
            GameFile::OptionA => "optiona.dat",
            GameFile::OptionC => "optionc.dat",
            GameFile::OptionZ => "optionz.dat",
        };
        // DOS file names are case-insensitive: accept OPTIONA.DAT, OptionA.dat, ...
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().eq_ignore_ascii_case(name) {
                    return e.path();
                }
            }
        }
        self.dir.join(name)
    }
}

impl Host for RealHost {
    fn key_peek(&mut self, st: &State) -> Option<u16> {
        self.check_quit();
        self.present(st);
        self.sync();
        self.sh.keys.lock().unwrap().front().copied()
    }

    fn key_get(&mut self, st: &State) -> u16 {
        self.present(st);
        let mut k = self.sh.keys.lock().unwrap();
        loop {
            if self.sh.quit.load(Ordering::SeqCst) {
                drop(k);
                jump(Jump::Exit);
            }
            if let Some(key) = k.pop_front() {
                drop(k);
                self.vt = self.sh.now_us();
                return key;
            }
            k = self
                .sh
                .key_cv
                .wait_timeout(k, Duration::from_millis(50))
                .unwrap()
                .0;
        }
    }

    fn key_dos(&mut self, st: &State) -> Option<u8> {
        self.check_quit();
        self.present(st);
        self.sh.keys.lock().unwrap().pop_front().map(|k| k as u8)
    }

    fn key_flush(&mut self) {
        self.sh.keys.lock().unwrap().clear();
    }

    fn tick_mark(&mut self) {
        self.sync();
        self.mark_tick = self.tick_now();
    }

    fn tick_wait_mark(&mut self, st: &State) {
        self.present(st);
        self.sync();
        loop {
            self.check_quit();
            let now = self.sh.now_us();
            let t = (now / TICK_US) as u64;
            if t != self.mark_tick {
                break;
            }
            let next = (self.mark_tick + 1) as f64 * TICK_US;
            std::thread::sleep(Duration::from_micros((next - now).max(200.0) as u64));
        }
        self.vt = self.vt.max(self.sh.now_us());
    }

    fn io_delay(&mut self, us: u32) {
        self.vt += us as f64;
        if self.vt > self.sh.now_us() + 30_000.0 {
            self.sync();
        }
    }

    fn speaker_toggle(&mut self) {
        let now = self.sh.now_us();
        if self.vt < now - 50_000.0 {
            self.vt = now;
        }
        self.sh.audio.lock().unwrap().speaker.push_back(self.vt);
    }

    fn sb_play(&mut self, samples: &[u8], tc: u8) {
        let rate = 1_000_000.0 / (256.0 - tc as f64);
        let start = self.sh.now_us().max(self.vt);
        self.sb_end = start + samples.len() as f64 * 1e6 / rate;
        if !samples.is_empty() {
            let mut a = self.sh.audio.lock().unwrap();
            // a new DMA transfer replaces the one in progress
            a.voices.clear();
            a.voices.push(SbVoice {
                data: samples.to_vec(),
                rate,
                start_us: start,
            });
        }
    }

    fn sb_done(&mut self) -> bool {
        self.sh.now_us() >= self.sb_end
    }

    fn file_read(&mut self, f: GameFile) -> Option<Vec<u8>> {
        std::fs::read(self.file_path(f)).ok()
    }

    fn file_write(&mut self, f: GameFile, data: &[u8]) {
        if let Err(e) = std::fs::write(self.file_path(f), data) {
            eprintln!("bcw: could not write {:?}: {}", self.file_path(f), e);
        }
    }

    fn text_message(&mut self, msg: &str) {
        eprintln!("{}", msg);
        let mut fr = self.sh.frame.lock().unwrap();
        fr.text_mode = true;
        fr.message = msg.to_string();
        fr.serial += 1;
    }

    fn present(&mut self, st: &State) {
        let mut fr = self.sh.frame.lock().unwrap();
        if fr.vram[..] != st.vram[..] || fr.color_select != st.bios.color_select {
            fr.vram.copy_from_slice(&st.vram);
            fr.color_select = st.bios.color_select;
            fr.serial += 1;
        }
        let tm = st.bios.mode != 4 && st.bios.mode != 5;
        if tm != fr.text_mode && fr.message.is_empty() {
            fr.text_mode = tm;
            fr.serial += 1;
        }
    }
}

// ---------------------------------------------------------------------
// shared rendering

/// The 16 CGA colours (RGBI).
pub const CGA16: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xAA5500, 0xAAAAAA, 0x555555,
    0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

/// Convert CGA mode 4 memory to 320x200 0x00RRGGBB pixels.
pub fn cga_to_rgb(fr: &Frame, out: &mut [u32]) {
    let cs = fr.color_select;
    let bright = if cs & 0x10 != 0 { 8 } else { 0 };
    let pal = if cs & 0x20 != 0 {
        [3u32, 5, 7]
    } else {
        [2u32, 4, 6]
    };
    let colors = [
        CGA16[(cs & 0x0F) as usize],
        CGA16[(pal[0] + bright) as usize],
        CGA16[(pal[1] + bright) as usize],
        CGA16[(pal[2] + bright) as usize],
    ];
    for y in 0..200 {
        let base = (y >> 1) * 80 + if y & 1 != 0 { 0x2000 } else { 0 };
        for xb in 0..80 {
            let b = fr.vram[base + xb];
            for p in 0..4 {
                let c = (b >> (6 - 2 * p)) & 3;
                out[y * 320 + xb * 4 + p] = colors[c as usize];
            }
        }
    }
    if fr.text_mode {
        // show the message (if any) in the 8x8 font, white on black
        for v in out.iter_mut() {
            *v = 0;
        }
        let font = crate::pc::bios::FONT_8X8;
        let (mut cx, mut cy) = (0usize, 0usize);
        for ch in fr.message.bytes() {
            match ch {
                b'\r' => cx = 0,
                b'\n' => cy += 1,
                _ => {
                    if cx < 40 && cy < 25 && ch < 128 {
                        for r in 0..8 {
                            let bits = font[ch as usize * 8 + r];
                            for b in 0..8 {
                                if bits & (0x80 >> b) != 0 {
                                    out[(cy * 8 + r) * 320 + cx * 8 + b] = 0xAAAAAA;
                                }
                            }
                        }
                    }
                    cx += 1;
                }
            }
        }
    }
}

/// Scale the 320x200 picture into a w x h buffer, keeping the 4:3 shape
/// of a CRT (pixels 1.2x taller than wide) and centring it.
pub fn scale_into(src: &[u32], dst: &mut [u32], w: usize, h: usize) {
    for v in dst.iter_mut() {
        *v = 0;
    }
    if w == 0 || h == 0 {
        return;
    }
    let (mut vw, mut vh) = (w, w * 3 / 4);
    if vh > h {
        vh = h;
        vw = h * 4 / 3;
    }
    let ox = (w - vw) / 2;
    let oy = (h - vh) / 2;
    let xmap: Vec<usize> = (0..vw).map(|x| x * 320 / vw).collect();
    for y in 0..vh {
        let sy = y * 200 / vh;
        let srow = &src[sy * 320..sy * 320 + 320];
        let drow = &mut dst[(oy + y) * w + ox..(oy + y) * w + ox + vw];
        for (x, d) in drow.iter_mut().enumerate() {
            *d = srow[xmap[x]];
        }
    }
}

/// Map a key from the window system into a BIOS keystroke. `ascii` is the
/// translated character (0 if none), `special` names non-character keys.
#[derive(Clone, Copy, PartialEq)]
pub enum Special {
    None,
    Up,
    Down,
    Left,
    Right,
}

pub fn bios_key(ascii: u8, special: Special) -> Option<u16> {
    // The game only looks at AL; arrow keys are offered as a convenience
    // and mapped to the original w/x/a/d movement keys.
    let al = match special {
        Special::Up => b'w',
        Special::Down => b'x',
        Special::Left => b'a',
        Special::Right => b'd',
        Special::None => {
            if ascii == 0 {
                return None;
            }
            let a = ascii;
            // the game's key table is lower case
            if a.is_ascii_uppercase() {
                a.to_ascii_lowercase()
            } else {
                a
            }
        }
    };
    Some(al as u16)
}

/// Start the game thread.
pub fn spawn_game(
    sh: Arc<Shared>,
    opts: crate::game::Options,
    dir: std::path::PathBuf,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("bcw-game".into())
        .stack_size(16 << 20)
        .spawn(move || {
            let host = RealHost::new(sh.clone(), dir);
            let mut g = crate::game::Game::new(host, &opts);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| g.run()));
            if let Err(e) = res {
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown error".into()
                };
                eprintln!("bcw: game stopped: {}", msg);
            }
            sh.game_over.store(true, Ordering::SeqCst);
        })
        .expect("spawn game thread")
}

/// Debug aid: with BCW_WAV=<file> set, record the audio mix to a WAV file
/// (in real time) instead of playing it.
pub fn start_wav_recorder(sh: Arc<Shared>, path: String) {
    std::thread::spawn(move || {
        let mut mixer = Mixer::new(&sh);
        let mut pcm: Vec<i16> = Vec::new();
        let mut buf = vec![0f32; 441];
        let start = Instant::now();
        let mut done_frames = 0u64;
        let mut last_write = 0usize;
        while pcm.len() < SAMPLE_RATE as usize * 600 {
            let due = (start.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as u64;
            while done_frames + 441 <= due {
                sh.render_audio(&mut mixer, &mut buf);
                pcm.extend(buf.iter().map(|s| (s * 32000.0) as i16));
                done_frames += 441;
            }
            if pcm.len() - last_write > SAMPLE_RATE as usize * 2 {
                last_write = pcm.len();
                write_wav(&path, &pcm);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
}

fn write_wav(path: &str, pcm: &[i16]) {
    let mut out = Vec::new();
    let data_len = (pcm.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    let _ = std::fs::write(path, out);
}
