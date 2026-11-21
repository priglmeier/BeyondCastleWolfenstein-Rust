//! Headless, deterministic host used to check the port against the original
//! program running in the reference harness. Keyboard input comes from a
//! script, delivered with exactly the same policy as the harness, and a hash
//! of the complete game state is logged at every keyboard poll.

use crate::game::{GameFile, Host, Jump, State};
use std::collections::VecDeque;
use std::io::Write;

pub struct TestHost {
    keys: VecDeque<(u32, u16)>,
    wait: u32,
    kbd: VecDeque<u16>,
    pub polls: u64,
    max_polls: u64,
    log: std::io::BufWriter<std::fs::File>,
    dir: String,
    dump_at: Option<u64>,
    dump_path: String,
}

fn fnv(data: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl TestHost {
    pub fn new(
        script: &str,
        log: &str,
        dir: &str,
        max_polls: u64,
        dump_at: Option<u64>,
        dump_path: &str,
    ) -> TestHost {
        let mut keys = VecDeque::new();
        for line in std::fs::read_to_string(script).unwrap_or_default().lines() {
            let mut it = line.split_whitespace();
            if let (Some(w), Some(k)) = (it.next(), it.next()) {
                keys.push_back((w.parse().unwrap(), u16::from_str_radix(k, 16).unwrap()));
            }
        }
        TestHost {
            keys,
            wait: 0,
            kbd: VecDeque::new(),
            polls: 0,
            max_polls,
            log: std::io::BufWriter::new(std::fs::File::create(log).unwrap()),
            dir: dir.to_string(),
            dump_at,
            dump_path: dump_path.to_string(),
        }
    }

    fn poll_event(&mut self, st: &State, kind: &str) {
        self.polls += 1;
        let snap = st.snapshot();
        writeln!(self.log, "{} {} {:016x}", self.polls, kind, fnv(&snap)).unwrap();
        if self.dump_at == Some(self.polls) {
            std::fs::write(&self.dump_path, &snap).unwrap();
        }
        if self.polls >= self.max_polls {
            self.finish();
        }
        if self.kbd.is_empty() {
            if let Some(&(w, k)) = self.keys.front() {
                if self.wait >= w {
                    self.kbd.push_back(k);
                    self.keys.pop_front();
                    self.wait = 0;
                } else {
                    self.wait += 1;
                }
            }
        }
    }

    fn finish(&mut self) -> ! {
        self.log.flush().unwrap();
        std::panic::resume_unwind(Box::new(Jump::Exit));
    }

    fn path(&self, f: GameFile) -> String {
        let n = match f {
            GameFile::SaveGame => "savegame.bcw",
            GameFile::OptionA => "optiona.dat",
            GameFile::OptionC => "optionc.dat",
            GameFile::OptionZ => "optionz.dat",
        };
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().eq_ignore_ascii_case(n) {
                    return e.path().to_string_lossy().into_owned();
                }
            }
        }
        format!("{}/{}", self.dir, n)
    }
}

impl Drop for TestHost {
    fn drop(&mut self) {
        let _ = self.log.flush();
    }
}

impl Host for TestHost {
    fn key_peek(&mut self, st: &State) -> Option<u16> {
        self.poll_event(st, "k1");
        self.kbd.front().copied()
    }
    fn key_get(&mut self, st: &State) -> u16 {
        self.poll_event(st, "k0");
        if let Some(k) = self.kbd.pop_front() {
            return k;
        }
        if let Some((_, k)) = self.keys.pop_front() {
            self.wait = 0;
            return k;
        }
        self.finish();
    }
    fn key_dos(&mut self, st: &State) -> Option<u8> {
        self.poll_event(st, "d6");
        self.kbd.pop_front().map(|k| k as u8)
    }
    fn key_flush(&mut self) {
        self.kbd.clear();
    }
    fn tick_mark(&mut self) {}
    fn tick_wait_mark(&mut self, _st: &State) {}
    fn io_delay(&mut self, _us: u32) {}
    fn speaker_toggle(&mut self) {}
    fn sb_play(&mut self, _samples: &[u8], _tc: u8) {}
    fn sb_done(&mut self) -> bool {
        true
    }
    fn file_read(&mut self, f: GameFile) -> Option<Vec<u8>> {
        std::fs::read(self.path(f)).ok()
    }
    fn file_write(&mut self, f: GameFile, data: &[u8]) {
        std::fs::write(self.path(f), data).ok();
    }
    fn text_message(&mut self, msg: &str) {
        eprintln!("{}", msg);
    }
    fn present(&mut self, _st: &State) {}
}
