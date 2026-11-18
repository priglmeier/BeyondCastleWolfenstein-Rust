// Reference harness: runs the original assembled BCW.EXE on a small 8086
// interpreter with just enough PC (BIOS/DOS/CGA/speaker/Sound Blaster)
// emulation. Used to produce state traces that the Rust port must match.

mod cpu;
#[path = "../../../src/pc/bios.rs"]
mod bios;

use cpu::*;
use std::collections::VecDeque;
use std::io::Write;

const LOAD_SEG: u16 = 0x1000;
const PSP_SEG: u16 = 0x0FF0;
const SB_BASE: u16 = 0x220;

struct Script {
    keys: VecDeque<(u32, u16)>, // (polls to wait before key appears, key)
    wait: u32,
}

struct Machine {
    bios: bios::Bios,
    kbd: VecDeque<u16>,
    script: Script,
    polls: u64,
    t_us: f64,
    port61: u8,
    // sound blaster
    sb_enabled: bool,
    sb_out: VecDeque<u8>,
    sb_cmd: Vec<u8>,
    sb_rate_tc: u8,
    irq_due: Option<f64>,
    files: Vec<Option<(String, Vec<u8>, usize, bool)>>,
    dir: String,
    alloc_next: u16,
    log: std::io::BufWriter<std::fs::File>,
    dump_at: Option<u64>,
    dump_path: String,
    max_polls: u64,
    finished: bool,
    exit_code: Option<u8>,
    segs: Segs,
    trace_ints: bool,
}

#[derive(Clone, Copy)]
struct Segs {
    code: u16,
    data: u16,
}

fn fnv(h: &mut u64, data: &[u8]) {
    for &b in data {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

impl Machine {
    fn state_bytes(&self, cpu: &Cpu) -> Vec<u8> {
        // code segment (with HLE-specific bytes masked), data segment, vram, cursor
        let cs = Cpu::lin(self.segs.code, 0) as usize;
        let ds = Cpu::lin(self.segs.data, 0) as usize;
        let mut v = Vec::with_capacity(0x5B08 + 0x7004 + 0x4000 + 2);
        let mut code = cpu.mem[cs..cs + 0x5B08].to_vec();
        for &r in RELOCS.iter() {
            let v = u16::from_le_bytes([code[r], code[r + 1]]).wrapping_sub(LOAD_SEG);
            code[r..r + 2].copy_from_slice(&v.to_le_bytes());
        }
        for &(off, len) in MASK.iter() {
            for b in &mut code[off..off + len] {
                *b = 0;
            }
        }
        v.extend_from_slice(&code);
        v.extend_from_slice(&cpu.mem[ds..ds + 0x7004]);
        v.extend_from_slice(&cpu.mem[0xB8000..0xBC000]);
        v.push(self.bios.row);
        v.push(self.bios.col);
        v
    }

    fn poll_event(&mut self, cpu: &Cpu, kind: &str) {
        self.polls += 1;
        let st = self.state_bytes(cpu);
        let mut h = 0xcbf29ce484222325u64;
        fnv(&mut h, &st);
        writeln!(self.log, "{} {} {:016x}", self.polls, kind, h).unwrap();
        if self.dump_at == Some(self.polls) {
            std::fs::write(&self.dump_path, &st).unwrap();
        }
        if self.polls >= self.max_polls {
            self.finished = true;
        }
        // deliver scripted keys
        if self.kbd.is_empty() {
            if let Some(&(w, k)) = self.script.keys.front() {
                if self.script.wait >= w {
                    self.kbd.push_back(k);
                    self.script.keys.pop_front();
                    self.script.wait = 0;
                } else {
                    self.script.wait += 1;
                }
            }
        }
    }

    fn take_key_blocking(&mut self) -> Option<u16> {
        if let Some(k) = self.kbd.pop_front() {
            return Some(k);
        }
        if let Some((_, k)) = self.script.keys.pop_front() {
            self.script.wait = 0;
            return Some(k);
        }
        None
    }

    fn read_asciiz(cpu: &Cpu, seg: u16, off: u16) -> String {
        let mut s = String::new();
        let mut o = off;
        loop {
            let b = cpu.rbs(seg, o);
            if b == 0 {
                break;
            }
            s.push(b as char);
            o = o.wrapping_add(1);
        }
        s
    }

    fn set_cf(cpu: &mut Cpu, on: bool) {
        // INT handled at high level: flags live in cpu.flags directly
        cpu.set_flag(CF, on);
    }
}

// Byte ranges (offsets in codeSeg) that legitimately differ between the
// harness and the port (segment values, DOS handles, timing scratch).
static RELOCS: &[usize] = &[0x25, 0x89A, 0x965, 0x3B2C, 0x3BB2, 0x3C09, 0x3C1E, 0x5ABD, 0x5AD4];
static MASK: &[(usize, usize)] = &[(0x66, 2), (0x68, 2), (0x53FD, 6), (0x55FB, 1), (0x5AE8, 2), (0x5913, 12), (0x591F, 2)];

impl Io for Machine {
    fn port_in(&mut self, _cpu: &mut Cpu, port: u16) -> u8 {
        match port {
            0x80 => {
                self.t_us += 1.0;
                0xFF
            }
            0x61 => self.port61,
            0x201 => 0xFF,
            0x21 => 0x00,
            p if self.sb_enabled && p == SB_BASE + 0xE => {
                // read-buffer status; reading also acknowledges the IRQ
                if self.sb_out.is_empty() {
                    0x7F
                } else {
                    0xFF
                }
            }
            p if self.sb_enabled && p == SB_BASE + 0xA => self.sb_out.pop_front().unwrap_or(0xFF),
            p if self.sb_enabled && p == SB_BASE + 0xC => 0x7F, // write ready
            _ => 0xFF,
        }
    }

    fn port_out(&mut self, _cpu: &mut Cpu, port: u16, val: u8) {
        match port {
            0x61 => self.port61 = val,
            p if self.sb_enabled && p == SB_BASE + 6 => {
                if val == 0 {
                    self.sb_out.clear();
                    self.sb_out.push_back(0xAA);
                    self.sb_cmd.clear();
                }
            }
            p if self.sb_enabled && p == SB_BASE + 0xC => {
                self.sb_cmd.push(val);
                let c = self.sb_cmd[0];
                let need = match c {
                    0x40 => 2,
                    0x14 => 3,
                    _ => 1,
                };
                if self.sb_cmd.len() >= need {
                    match c {
                        0x40 => self.sb_rate_tc = self.sb_cmd[1],
                        0x14 => {
                            let len = self.sb_cmd[1] as u32 | ((self.sb_cmd[2] as u32) << 8);
                            let rate = 1_000_000.0 / (256.0 - self.sb_rate_tc as f64);
                            self.irq_due = Some(self.t_us + (len as f64 + 1.0) * 1e6 / rate);
                        }
                        _ => {}
                    }
                    self.sb_cmd.clear();
                }
            }
            _ => {}
        }
    }

    fn int(&mut self, cpu: &mut Cpu, n: u8) -> bool {
        let ah = cpu.ah();
        let al = cpu.al();
        if self.trace_ints {
            eprintln!("INT {:02x} AX={:04x} @ {:04x}:{:04x}", n, cpu.r[AX], cpu.s[CS], cpu.ip);
        }
        match n {
            0x10 => {
                let vram = &mut cpu.mem[0xB8000..0xBC000];
                match ah {
                    0x00 => self.bios.set_mode(vram, al),
                    0x02 => {
                        let dx = cpu.r[DX];
                        self.bios.set_cursor((dx >> 8) as u8, dx as u8)
                    }
                    0x0B => {
                        let bx = cpu.r[BX];
                        self.bios.set_palette((bx >> 8) as u8, bx as u8)
                    }
                    0x0E => {
                        let bl = cpu.r[BX] as u8;
                        self.bios.teletype(vram, al, bl)
                    }
                    0x11 => {}
                    _ => eprintln!("unhandled int10 ah={:02x}", ah),
                }
                true
            }
            0x16 => {
                match ah {
                    0x00 => {
                        self.poll_event(cpu, "k0");
                        match self.take_key_blocking() {
                            Some(k) => cpu.r[AX] = k,
                            None => {
                                self.finished = true;
                                cpu.r[AX] = 0x011B;
                            }
                        }
                    }
                    0x01 => {
                        self.poll_event(cpu, "k1");
                        if let Some(&k) = self.kbd.front() {
                            cpu.r[AX] = k;
                            cpu.set_flag(ZF, false);
                        } else {
                            cpu.set_flag(ZF, true);
                        }
                    }
                    _ => {}
                }
                true
            }
            0x21 => {
                match ah {
                    0x06 => {
                        // direct console input (DL=FF)
                        self.poll_event(cpu, "d6");
                        if let Some(k) = self.kbd.pop_front() {
                            cpu.set_al(k as u8);
                            cpu.set_flag(ZF, false);
                        } else {
                            cpu.set_al(0);
                            cpu.set_flag(ZF, true);
                        }
                    }
                    0x09 => {
                        let mut s = String::new();
                        let mut o = cpu.r[DX];
                        loop {
                            let b = cpu.rbs(cpu.s[DS], o);
                            if b == b'$' {
                                break;
                            }
                            s.push(b as char);
                            o = o.wrapping_add(1);
                        }
                        eprintln!("DOS print: {}", s);
                    }
                    0x0C => {
                        self.kbd.clear();
                    }
                    0x25 => {
                        let v = al as u32 * 4;
                        let (dx, ds) = (cpu.r[DX], cpu.s[DS]);
                        cpu.ww(v, dx);
                        cpu.ww(v + 2, ds);
                    }
                    0x35 => {
                        let v = al as u32 * 4;
                        cpu.r[BX] = cpu.rw(v);
                        cpu.s[ES] = cpu.rw(v + 2);
                    }
                    0x2C => {
                        self.t_us += 1000.0; // idle skip while polling the clock
                        let ticks = (self.t_us / 54925.4) as u64;
                        let hund = (ticks * 549254 / 100000) % 100;
                        cpu.r[DX] = hund as u16;
                        cpu.r[CX] = 0;
                    }
                    0x30 => cpu.r[AX] = 0x0005,
                    0x33 => {
                        cpu.r[BX] = 0;
                        cpu.r[DX] = 0;
                    }
                    0x3C | 0x3D => {
                        let name = Self::read_asciiz(cpu, cpu.s[DS], cpu.r[DX]).to_lowercase();
                        let path = format!("{}/{}", self.dir, name);
                        let data = if ah == 0x3C {
                            std::fs::write(&path, b"").ok();
                            Some(Vec::new())
                        } else {
                            std::fs::read(&path).ok()
                        };
                        match data {
                            Some(d) => {
                                let h = self.files.len();
                                let write = ah != 0x3C && (al & 3) == 1;
                                self.files.push(Some((path, d, 0, write)));
                                cpu.r[AX] = (h + 5) as u16;
                                Self::set_cf(cpu, false);
                            }
                            None => {
                                cpu.r[AX] = 2;
                                Self::set_cf(cpu, true);
                            }
                        }
                    }
                    0x3E => {
                        let h = cpu.r[BX] as usize;
                        if h >= 5 && h - 5 < self.files.len() {
                            if let Some((p, d, _, w)) = self.files[h - 5].take() {
                                if w {
                                    std::fs::write(p, d).ok();
                                }
                            }
                            Self::set_cf(cpu, false);
                        } else {
                            Self::set_cf(cpu, true);
                        }
                    }
                    0x3F => {
                        let h = cpu.r[BX] as usize;
                        let n = cpu.r[CX] as usize;
                        let (ds, dx) = (cpu.s[DS], cpu.r[DX]);
                        let mut got = 0usize;
                        if h >= 5 && h - 5 < self.files.len() {
                            if let Some((_, _, _, true)) = &self.files[h - 5] {
                                cpu.r[AX] = 5;
                                Self::set_cf(cpu, true);
                                return true;
                            }
                            if let Some((_, d, pos, _)) = &mut self.files[h - 5] {
                                let avail = d.len().saturating_sub(*pos).min(n);
                                let chunk: Vec<u8> = d[*pos..*pos + avail].to_vec();
                                *pos += avail;
                                got = avail;
                                for (i, b) in chunk.iter().enumerate() {
                                    cpu.wbs(ds, dx.wrapping_add(i as u16), *b);
                                }
                            }
                            cpu.r[AX] = got as u16;
                            Self::set_cf(cpu, false);
                        } else {
                            cpu.r[AX] = 6;
                            Self::set_cf(cpu, true);
                        }
                    }
                    0x40 => {
                        let h = cpu.r[BX] as usize;
                        let n = cpu.r[CX] as usize;
                        let (ds, dx) = (cpu.s[DS], cpu.r[DX]);
                        if h >= 5 && h - 5 < self.files.len() {
                            let bytes: Vec<u8> =
                                (0..n).map(|i| cpu.rbs(ds, dx.wrapping_add(i as u16))).collect();
                            if let Some((_, d, pos, w)) = &mut self.files[h - 5] {
                                if *w {
                                    if d.len() < *pos + n {
                                        d.resize(*pos + n, 0);
                                    }
                                    d[*pos..*pos + n].copy_from_slice(&bytes);
                                    *pos += n;
                                    cpu.r[AX] = n as u16;
                                } else {
                                    cpu.r[AX] = 0;
                                }
                            }
                            Self::set_cf(cpu, false);
                        } else {
                            Self::set_cf(cpu, true);
                        }
                    }
                    0x48 => {
                        let paras = cpu.r[BX];
                        cpu.r[AX] = self.alloc_next;
                        self.alloc_next += paras;
                        Self::set_cf(cpu, false);
                    }
                    0x4A => Self::set_cf(cpu, false),
                    0x4C => {
                        self.exit_code = Some(al);
                        self.finished = true;
                    }
                    0x51 | 0x62 => cpu.r[BX] = PSP_SEG,
                    _ => eprintln!("unhandled int21 ah={:02x}", ah),
                }
                true
            }
            0x2F => true,
            _ => false,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut exe = "../reference/bcw.exe".to_string();
    let mut script_path: Option<String> = None;
    let mut log_path = "trace.txt".to_string();
    let mut dir = ".".to_string();
    let mut sb = true;
    let mut dump_at = None;
    let mut dump_path = "dump.bin".to_string();
    let mut max_polls = u64::MAX;
    let mut trace_ints = false;
    let mut max_instr = u64::MAX;
    let mut cov_path: Option<String> = None;
    let mut cov = vec![0u8; 0x10000];
    let mut screens: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--exe" => {
                exe = args[i + 1].clone();
                i += 1
            }
            "--script" => {
                script_path = Some(args[i + 1].clone());
                i += 1
            }
            "--log" => {
                log_path = args[i + 1].clone();
                i += 1
            }
            "--dir" => {
                dir = args[i + 1].clone();
                i += 1
            }
            "--nosb" => sb = false,
            "--dump-at" => {
                dump_at = Some(args[i + 1].parse().unwrap());
                i += 1
            }
            "--dump" => {
                dump_path = args[i + 1].clone();
                i += 1
            }
            "--max-polls" => {
                max_polls = args[i + 1].parse().unwrap();
                i += 1
            }
            "--screens" => {
                screens = Some(args[i + 1].clone());
                i += 1
            }
            "--trace-ints" => trace_ints = true,
            "--cov" => {
                cov_path = Some(args[i + 1].clone());
                i += 1
            }
            "--max-instr" => {
                max_instr = args[i + 1].parse().unwrap();
                i += 1
            }
            a => panic!("unknown arg {}", a),
        }
        i += 1;
    }

    let mut keys = VecDeque::new();
    if let Some(p) = script_path {
        for line in std::fs::read_to_string(p).unwrap().lines() {
            let mut it = line.split_whitespace();
            if let (Some(w), Some(k)) = (it.next(), it.next()) {
                let k = u16::from_str_radix(k, 16).unwrap();
                keys.push_back((w.parse().unwrap(), k));
            }
        }
    }

    let d = std::fs::read(&exe).unwrap();
    let h = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let hdr = h(8) as usize * 16;
    let nrel = h(6) as usize;
    let rel = h(0x18) as usize;
    let mut cpu = Cpu::new();
    let base = Cpu::lin(LOAD_SEG, 0) as usize;
    let img = &d[hdr..];
    cpu.mem[base..base + img.len()].copy_from_slice(img);
    for i in 0..nrel {
        let off = h(rel + i * 4);
        let seg = h(rel + i * 4 + 2);
        let a = Cpu::lin(LOAD_SEG + seg, off);
        let v = cpu.rw(a).wrapping_add(LOAD_SEG);
        cpu.ww(a, v);
    }
    // PSP command tail
    let tail: &[u8] = if sb { b"" } else { b" -nosb" };
    let p = Cpu::lin(PSP_SEG, 0x80) as usize;
    cpu.mem[p] = tail.len() as u8;
    cpu.mem[p + 1..p + 1 + tail.len()].copy_from_slice(tail);
    cpu.mem[p + 1 + tail.len()] = 0x0D;
    // ROM + IVT pointing at an IRET
    let rom = bios::rom_f800();
    cpu.mem[0xF8000..0x108000].copy_from_slice(&rom);
    cpu.mem[0xFFF53] = 0xCF;
    for v in 0..256u32 {
        cpu.mem[(v * 4) as usize] = 0x53;
        cpu.mem[(v * 4 + 1) as usize] = 0xFF;
        cpu.mem[(v * 4 + 2) as usize] = 0x00;
        cpu.mem[(v * 4 + 3) as usize] = 0xF0;
    }
    cpu.s[CS] = LOAD_SEG + h(0x16);
    cpu.ip = h(0x14);
    cpu.s[SS] = LOAD_SEG + h(0x0E);
    cpu.r[SP] = h(0x10);
    cpu.s[DS] = PSP_SEG;
    cpu.s[ES] = PSP_SEG;
    cpu.flags = 0x0202;

    let image_paras = (img.len() as u32 + 15) / 16;
    let mut m = Machine {
        bios: bios::Bios::default(),
        kbd: VecDeque::new(),
        script: Script { keys, wait: 0 },
        polls: 0,
        t_us: 0.0,
        port61: 0,
        sb_enabled: sb,
        sb_out: VecDeque::new(),
        sb_cmd: Vec::new(),
        sb_rate_tc: 211,
        irq_due: None,
        files: Vec::new(),
        dir,
        alloc_next: ((LOAD_SEG as u32 + image_paras + 0x100 + 0xFF) & !0xFF) as u16,
        log: std::io::BufWriter::new(std::fs::File::create(&log_path).unwrap()),
        dump_at,
        dump_path,
        max_polls,
        finished: false,
        exit_code: None,
        segs: Segs { code: LOAD_SEG, data: LOAD_SEG + 0x5B1 },
        trace_ints,
    };

    let mut shot = 0;
    let mut last_shot_poll = 0;
    while !m.finished {
        if cpu.icount >= max_instr {
            eprintln!("instruction limit");
            break;
        }
        if cpu.s[CS] == LOAD_SEG {
            cov[cpu.ip as usize] = 1;
        }
        cpu.step(&mut m);
        m.t_us += 0.25;
        if let Some(t) = m.irq_due {
            if m.t_us >= t && cpu.flag(IF) {
                m.irq_due = None;
                cpu.do_int(0x0D); // IRQ5
            }
        }
        if let Some(dir) = &screens {
            if m.polls >= last_shot_poll + 2000 {
                last_shot_poll = m.polls;
                let vram = &cpu.mem[0xB8000..0xBC000];
                write_ppm(&format!("{}/shot{:04}.ppm", dir, shot), vram, m.bios.color_select);
                shot += 1;
            }
        }
    }
    m.log.flush().unwrap();
    if let Some(p) = &cov_path {
        let mut old = std::fs::read(p).unwrap_or(vec![0u8; 0x10000]);
        for i in 0..0x10000 { old[i] |= cov[i]; }
        std::fs::write(p, old).unwrap();
    }
    eprintln!(
        "done: polls={} instr={} exit={:?} t={:.1}s",
        m.polls, cpu.icount, m.exit_code, m.t_us / 1e6
    );
    if let Some(dir) = &screens {
        let vram = &cpu.mem[0xB8000..0xBC000];
        write_ppm(&format!("{}/final.ppm", dir), vram, m.bios.color_select);
    }
}

fn write_ppm(path: &str, vram: &[u8], cs: u8) {
    let pal1 = [[0, 0, 0], [85, 255, 255], [255, 85, 255], [255, 255, 255]];
    let mut out = format!("P6 320 200 255\n").into_bytes();
    for y in 0..200 {
        for x in 0..320 {
            let off = (y >> 1) * 80 + (x >> 2) + if y & 1 != 0 { 0x2000 } else { 0 };
            let c = (vram[off] >> (2 * (3 - (x & 3)))) & 3;
            let _ = cs;
            out.extend_from_slice(&pal1[c as usize]);
        }
    }
    std::fs::write(path, out).unwrap();
}
