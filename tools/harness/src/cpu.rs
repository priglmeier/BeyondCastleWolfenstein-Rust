// Minimal but complete 8086 interpreter used as a reference oracle.
// Behaves like a 286 for the two quirks cpuDetect probes (shift-count masking
// and PUSH SP), so the game takes its "modern CPU" code paths.

pub const CF: u16 = 0x0001;
pub const PF: u16 = 0x0004;
pub const AF: u16 = 0x0010;
pub const ZF: u16 = 0x0040;
pub const SF: u16 = 0x0080;
pub const TF: u16 = 0x0100;
pub const IF: u16 = 0x0200;
pub const DF: u16 = 0x0400;
pub const OF: u16 = 0x0800;

pub const AX: usize = 0;
pub const CX: usize = 1;
pub const DX: usize = 2;
pub const BX: usize = 3;
pub const SP: usize = 4;
pub const BP: usize = 5;
pub const SI: usize = 6;
pub const DI: usize = 7;

pub const ES: usize = 0;
pub const CS: usize = 1;
pub const SS: usize = 2;
pub const DS: usize = 3;

pub trait Io {
    fn port_in(&mut self, cpu: &mut Cpu, port: u16) -> u8;
    fn port_out(&mut self, cpu: &mut Cpu, port: u16, val: u8);
    /// Return true if the interrupt was handled at high level (no IVT dispatch).
    fn int(&mut self, cpu: &mut Cpu, n: u8) -> bool;
}

pub struct Cpu {
    pub r: [u16; 8],
    pub s: [u16; 4],
    pub ip: u16,
    pub flags: u16,
    pub mem: Vec<u8>,
    pub halted: bool,
    pub icount: u64,
    // decode state
    seg_ovr: Option<usize>,
    rep: u8, // 0 none, 0xF2 repne, 0xF3 rep/repe
    // modrm
    mod_: u8,
    reg: u8,
    rm: u8,
    ea: u32,
    ea_seg: usize,
    ea_off: u16,
}

#[inline]
fn parity(v: u8) -> bool {
    v.count_ones() % 2 == 0
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            r: [0; 8],
            s: [0; 4],
            ip: 0,
            flags: 0x0002,
            mem: vec![0; 0x110000],
            halted: false,
            icount: 0,
            seg_ovr: None,
            rep: 0,
            mod_: 0,
            reg: 0,
            rm: 0,
            ea: 0,
            ea_seg: DS,
            ea_off: 0,
        }
    }

    #[inline]
    pub fn lin(seg: u16, off: u16) -> u32 {
        ((seg as u32) << 4).wrapping_add(off as u32)
    }
    #[inline]
    pub fn rb(&self, a: u32) -> u8 {
        let a = if (0xBC000..0xC0000).contains(&a) { a - 0x4000 } else { a };
        self.mem[a as usize]
    }
    #[inline]
    pub fn wb(&mut self, a: u32, v: u8) {
        let a = if (0xBC000..0xC0000).contains(&a) { a - 0x4000 } else { a };
        let a = a as usize;
        if a >= 0xF0000 {
            return; // ROM
        }
        self.mem[a] = v;
    }
    #[inline]
    pub fn rw(&self, a: u32) -> u16 {
        self.rb(a) as u16 | ((self.rb(a + 1) as u16) << 8)
    }
    #[inline]
    pub fn ww(&mut self, a: u32, v: u16) {
        self.wb(a, v as u8);
        self.wb(a + 1, (v >> 8) as u8);
    }
    pub fn rbs(&self, seg: u16, off: u16) -> u8 {
        self.rb(Self::lin(seg, off))
    }
    pub fn rws(&self, seg: u16, off: u16) -> u16 {
        self.rbs(seg, off) as u16 | ((self.rbs(seg, off.wrapping_add(1)) as u16) << 8)
    }
    pub fn wbs(&mut self, seg: u16, off: u16, v: u8) {
        self.wb(Self::lin(seg, off), v)
    }
    pub fn wws(&mut self, seg: u16, off: u16, v: u16) {
        self.wbs(seg, off, v as u8);
        self.wbs(seg, off.wrapping_add(1), (v >> 8) as u8);
    }

    // 8-bit register access: 0..3 = AL CL DL BL, 4..7 = AH CH DH BH
    #[inline]
    pub fn r8(&self, i: u8) -> u8 {
        let i = i as usize;
        if i < 4 {
            self.r[i] as u8
        } else {
            (self.r[i - 4] >> 8) as u8
        }
    }
    #[inline]
    pub fn set_r8(&mut self, i: u8, v: u8) {
        let i = i as usize;
        if i < 4 {
            self.r[i] = (self.r[i] & 0xFF00) | v as u16;
        } else {
            self.r[i - 4] = (self.r[i - 4] & 0x00FF) | ((v as u16) << 8);
        }
    }
    pub fn al(&self) -> u8 {
        self.r[AX] as u8
    }
    pub fn ah(&self) -> u8 {
        (self.r[AX] >> 8) as u8
    }
    pub fn set_al(&mut self, v: u8) {
        self.set_r8(0, v)
    }
    pub fn set_ah(&mut self, v: u8) {
        self.set_r8(4, v)
    }
    pub fn set_flag(&mut self, f: u16, on: bool) {
        if on {
            self.flags |= f
        } else {
            self.flags &= !f
        }
    }
    pub fn flag(&self, f: u16) -> bool {
        self.flags & f != 0
    }

    #[inline]
    fn fetch8(&mut self) -> u8 {
        let v = self.rbs(self.s[CS], self.ip);
        self.ip = self.ip.wrapping_add(1);
        v
    }
    #[inline]
    fn fetch16(&mut self) -> u16 {
        let lo = self.fetch8() as u16;
        let hi = self.fetch8() as u16;
        lo | (hi << 8)
    }

    pub fn push(&mut self, v: u16) {
        self.r[SP] = self.r[SP].wrapping_sub(2);
        let (ss, sp) = (self.s[SS], self.r[SP]);
        self.wws(ss, sp, v);
    }
    pub fn pop(&mut self) -> u16 {
        let v = self.rws(self.s[SS], self.r[SP]);
        self.r[SP] = self.r[SP].wrapping_add(2);
        v
    }

    fn decode_modrm(&mut self) {
        let m = self.fetch8();
        self.mod_ = m >> 6;
        self.reg = (m >> 3) & 7;
        self.rm = m & 7;
        if self.mod_ == 3 {
            return;
        }
        let (mut off, mut seg): (u16, usize) = match self.rm {
            0 => (self.r[BX].wrapping_add(self.r[SI]), DS),
            1 => (self.r[BX].wrapping_add(self.r[DI]), DS),
            2 => (self.r[BP].wrapping_add(self.r[SI]), SS),
            3 => (self.r[BP].wrapping_add(self.r[DI]), SS),
            4 => (self.r[SI], DS),
            5 => (self.r[DI], DS),
            6 => (self.r[BP], SS),
            _ => (self.r[BX], DS),
        };
        match self.mod_ {
            0 => {
                if self.rm == 6 {
                    off = self.fetch16();
                    seg = DS;
                }
            }
            1 => {
                let d = self.fetch8() as i8 as i16 as u16;
                off = off.wrapping_add(d);
            }
            _ => {
                let d = self.fetch16();
                off = off.wrapping_add(d);
            }
        }
        if let Some(o) = self.seg_ovr {
            seg = o;
        }
        self.ea_seg = seg;
        self.ea_off = off;
        self.ea = Self::lin(self.s[seg], off);
    }

    fn rm8(&self) -> u8 {
        if self.mod_ == 3 {
            self.r8(self.rm)
        } else {
            self.rb(self.ea)
        }
    }
    fn set_rm8(&mut self, v: u8) {
        if self.mod_ == 3 {
            self.set_r8(self.rm, v)
        } else {
            self.wb(self.ea, v)
        }
    }
    fn rm16(&self) -> u16 {
        if self.mod_ == 3 {
            self.r[self.rm as usize]
        } else {
            self.rws(self.s[self.ea_seg], self.ea_off)
        }
    }
    fn set_rm16(&mut self, v: u16) {
        if self.mod_ == 3 {
            self.r[self.rm as usize] = v
        } else {
            let s = self.s[self.ea_seg];
            self.wws(s, self.ea_off, v)
        }
    }

    // ---- flags helpers
    fn szp8(&mut self, v: u8) {
        self.set_flag(ZF, v == 0);
        self.set_flag(SF, v & 0x80 != 0);
        self.set_flag(PF, parity(v));
    }
    fn szp16(&mut self, v: u16) {
        self.set_flag(ZF, v == 0);
        self.set_flag(SF, v & 0x8000 != 0);
        self.set_flag(PF, parity(v as u8));
    }

    fn alu8(&mut self, op: u8, a: u8, b: u8) -> u8 {
        let c = (self.flags & CF) as u8;
        let r: u8;
        match op {
            0 | 2 => {
                // add / adc
                let cin = if op == 2 { c } else { 0 };
                let s = a as u16 + b as u16 + cin as u16;
                r = s as u8;
                self.set_flag(CF, s > 0xFF);
                self.set_flag(AF, ((a & 0xF) + (b & 0xF) + cin) > 0xF);
                self.set_flag(OF, ((a ^ r) & (b ^ r) & 0x80) != 0);
                self.szp8(r);
            }
            5 | 3 | 7 => {
                // sub / sbb / cmp
                let cin = if op == 3 { c } else { 0 };
                let s = (a as i16) - (b as i16) - cin as i16;
                r = s as u8;
                self.set_flag(CF, s < 0);
                self.set_flag(AF, ((a & 0xF) as i16 - (b & 0xF) as i16 - cin as i16) < 0);
                self.set_flag(OF, ((a ^ b) & (a ^ r) & 0x80) != 0);
                self.szp8(r);
            }
            1 | 4 | 6 => {
                r = match op {
                    1 => a | b,
                    4 => a & b,
                    _ => a ^ b,
                };
                self.set_flag(CF, false);
                self.set_flag(OF, false);
                self.set_flag(AF, false);
                self.szp8(r);
            }
            _ => unreachable!(),
        }
        r
    }
    fn alu16(&mut self, op: u8, a: u16, b: u16) -> u16 {
        let c = (self.flags & CF) as u16;
        let r: u16;
        match op {
            0 | 2 => {
                let cin = if op == 2 { c } else { 0 };
                let s = a as u32 + b as u32 + cin as u32;
                r = s as u16;
                self.set_flag(CF, s > 0xFFFF);
                self.set_flag(AF, ((a & 0xF) + (b & 0xF) + cin) > 0xF);
                self.set_flag(OF, ((a ^ r) & (b ^ r) & 0x8000) != 0);
                self.szp16(r);
            }
            5 | 3 | 7 => {
                let cin = if op == 3 { c } else { 0 };
                let s = (a as i32) - (b as i32) - cin as i32;
                r = s as u16;
                self.set_flag(CF, s < 0);
                self.set_flag(AF, ((a & 0xF) as i32 - (b & 0xF) as i32 - cin as i32) < 0);
                self.set_flag(OF, ((a ^ b) & (a ^ r) & 0x8000) != 0);
                self.szp16(r);
            }
            1 | 4 | 6 => {
                r = match op {
                    1 => a | b,
                    4 => a & b,
                    _ => a ^ b,
                };
                self.set_flag(CF, false);
                self.set_flag(OF, false);
                self.set_flag(AF, false);
                self.szp16(r);
            }
            _ => unreachable!(),
        }
        r
    }
    fn inc8(&mut self, a: u8) -> u8 {
        let r = a.wrapping_add(1);
        self.set_flag(OF, a == 0x7F);
        self.set_flag(AF, (a & 0xF) == 0xF);
        self.szp8(r);
        r
    }
    fn dec8(&mut self, a: u8) -> u8 {
        let r = a.wrapping_sub(1);
        self.set_flag(OF, a == 0x80);
        self.set_flag(AF, (a & 0xF) == 0);
        self.szp8(r);
        r
    }
    fn inc16(&mut self, a: u16) -> u16 {
        let r = a.wrapping_add(1);
        self.set_flag(OF, a == 0x7FFF);
        self.set_flag(AF, (a & 0xF) == 0xF);
        self.szp16(r);
        r
    }
    fn dec16(&mut self, a: u16) -> u16 {
        let r = a.wrapping_sub(1);
        self.set_flag(OF, a == 0x8000);
        self.set_flag(AF, (a & 0xF) == 0);
        self.szp16(r);
        r
    }

    // group 2 shifts/rotates. count already masked.
    fn shift8(&mut self, op: u8, v: u8, cnt: u8) -> u8 {
        if cnt == 0 {
            return v;
        }
        let mut r = v;
        for _ in 0..cnt {
            match op {
                0 => {
                    let c = r & 0x80 != 0;
                    r = (r << 1) | c as u8;
                    self.set_flag(CF, c);
                }
                1 => {
                    let c = r & 1 != 0;
                    r = (r >> 1) | ((c as u8) << 7);
                    self.set_flag(CF, c);
                }
                2 => {
                    let c = r & 0x80 != 0;
                    r = (r << 1) | (self.flag(CF) as u8);
                    self.set_flag(CF, c);
                }
                3 => {
                    let c = r & 1 != 0;
                    r = (r >> 1) | ((self.flag(CF) as u8) << 7);
                    self.set_flag(CF, c);
                }
                4 | 6 => {
                    self.set_flag(CF, r & 0x80 != 0);
                    r <<= 1;
                }
                5 => {
                    self.set_flag(CF, r & 1 != 0);
                    r >>= 1;
                }
                _ => {
                    self.set_flag(CF, r & 1 != 0);
                    r = ((r as i8) >> 1) as u8;
                }
            }
        }
        match op {
            0 | 2 => self.set_flag(OF, ((r >> 7) & 1 != 0) != self.flag(CF)),
            1 | 3 => self.set_flag(OF, ((r ^ (r << 1)) & 0x80) != 0),
            4 | 6 => {
                self.set_flag(OF, ((r >> 7) & 1 != 0) != self.flag(CF));
                self.szp8(r);
            }
            5 => {
                self.set_flag(OF, cnt == 1 && v & 0x80 != 0);
                self.szp8(r);
            }
            _ => {
                self.set_flag(OF, false);
                self.szp8(r);
            }
        }
        r
    }
    fn shift16(&mut self, op: u8, v: u16, cnt: u8) -> u16 {
        if cnt == 0 {
            return v;
        }
        let mut r = v;
        for _ in 0..cnt {
            match op {
                0 => {
                    let c = r & 0x8000 != 0;
                    r = (r << 1) | c as u16;
                    self.set_flag(CF, c);
                }
                1 => {
                    let c = r & 1 != 0;
                    r = (r >> 1) | ((c as u16) << 15);
                    self.set_flag(CF, c);
                }
                2 => {
                    let c = r & 0x8000 != 0;
                    r = (r << 1) | (self.flag(CF) as u16);
                    self.set_flag(CF, c);
                }
                3 => {
                    let c = r & 1 != 0;
                    r = (r >> 1) | ((self.flag(CF) as u16) << 15);
                    self.set_flag(CF, c);
                }
                4 | 6 => {
                    self.set_flag(CF, r & 0x8000 != 0);
                    r <<= 1;
                }
                5 => {
                    self.set_flag(CF, r & 1 != 0);
                    r >>= 1;
                }
                _ => {
                    self.set_flag(CF, r & 1 != 0);
                    r = ((r as i16) >> 1) as u16;
                }
            }
        }
        match op {
            0 | 2 => self.set_flag(OF, ((r >> 15) & 1 != 0) != self.flag(CF)),
            1 | 3 => self.set_flag(OF, ((r ^ (r << 1)) & 0x8000) != 0),
            4 | 6 => {
                self.set_flag(OF, ((r >> 15) & 1 != 0) != self.flag(CF));
                self.szp16(r);
            }
            5 => {
                self.set_flag(OF, cnt == 1 && v & 0x8000 != 0);
                self.szp16(r);
            }
            _ => {
                self.set_flag(OF, false);
                self.szp16(r);
            }
        }
        r
    }

    fn cond(&self, c: u8) -> bool {
        let f = |x| self.flag(x);
        match c {
            0 => f(OF),
            1 => !f(OF),
            2 => f(CF),
            3 => !f(CF),
            4 => f(ZF),
            5 => !f(ZF),
            6 => f(CF) || f(ZF),
            7 => !(f(CF) || f(ZF)),
            8 => f(SF),
            9 => !f(SF),
            10 => f(PF),
            11 => !f(PF),
            12 => f(SF) != f(OF),
            13 => f(SF) == f(OF),
            14 => f(ZF) || (f(SF) != f(OF)),
            _ => !f(ZF) && (f(SF) == f(OF)),
        }
    }

    pub fn do_int(&mut self, n: u8) {
        let f = self.flags;
        self.push(f);
        self.set_flag(IF, false);
        self.set_flag(TF, false);
        let cs = self.s[CS];
        self.push(cs);
        let ip = self.ip;
        self.push(ip);
        self.ip = self.rw(n as u32 * 4);
        self.s[CS] = self.rw(n as u32 * 4 + 2);
    }

    fn str_seg(&self) -> usize {
        self.seg_ovr.unwrap_or(DS)
    }

    /// Execute one instruction (including prefixes).
    pub fn step(&mut self, io: &mut dyn Io) {
        self.seg_ovr = None;
        self.rep = 0;
        self.icount += 1;
        let start_ip = self.ip;
        let mut op;
        loop {
            op = self.fetch8();
            match op {
                0x26 => self.seg_ovr = Some(ES),
                0x2E => self.seg_ovr = Some(CS),
                0x36 => self.seg_ovr = Some(SS),
                0x3E => self.seg_ovr = Some(DS),
                0xF0 => {}
                0xF2 | 0xF3 => self.rep = op,
                _ => break,
            }
        }
        match op {
            // ALU r/m, reg and immediates: 00-3F pattern
            0x00..=0x3F if (op & 7) < 6 => {
                let aop = op >> 3;
                match op & 7 {
                    0 => {
                        self.decode_modrm();
                        let a = self.rm8();
                        let b = self.r8(self.reg);
                        let r = self.alu8(aop, a, b);
                        if aop != 7 {
                            self.set_rm8(r)
                        }
                    }
                    1 => {
                        self.decode_modrm();
                        let a = self.rm16();
                        let b = self.r[self.reg as usize];
                        let r = self.alu16(aop, a, b);
                        if aop != 7 {
                            self.set_rm16(r)
                        }
                    }
                    2 => {
                        self.decode_modrm();
                        let a = self.r8(self.reg);
                        let b = self.rm8();
                        let r = self.alu8(aop, a, b);
                        if aop != 7 {
                            self.set_r8(self.reg, r)
                        }
                    }
                    3 => {
                        self.decode_modrm();
                        let a = self.r[self.reg as usize];
                        let b = self.rm16();
                        let r = self.alu16(aop, a, b);
                        if aop != 7 {
                            self.r[self.reg as usize] = r
                        }
                    }
                    4 => {
                        let b = self.fetch8();
                        let a = self.al();
                        let r = self.alu8(aop, a, b);
                        if aop != 7 {
                            self.set_al(r)
                        }
                    }
                    _ => {
                        let b = self.fetch16();
                        let a = self.r[AX];
                        let r = self.alu16(aop, a, b);
                        if aop != 7 {
                            self.r[AX] = r
                        }
                    }
                }
            }
            0x06 | 0x0E | 0x16 | 0x1E => {
                let v = self.s[(op >> 3) as usize];
                self.push(v)
            }
            0x07 | 0x17 | 0x1F => {
                let v = self.pop();
                self.s[(op >> 3) as usize] = v
            }
            0x0F => {
                // POP CS on 8086 (cpuDetect V20 test) - emulate 8086 behaviour
                let v = self.pop();
                self.s[CS] = v;
            }
            0x27 => {
                // DAA
                let mut al = self.al();
                let old_cf = self.flag(CF);
                let old_al = al;
                if (al & 0xF) > 9 || self.flag(AF) {
                    al = al.wrapping_add(6);
                    self.set_flag(AF, true);
                } else {
                    self.set_flag(AF, false);
                }
                if old_al > 0x99 || old_cf {
                    al = al.wrapping_add(0x60);
                    self.set_flag(CF, true);
                } else {
                    self.set_flag(CF, false);
                }
                self.set_al(al);
                self.szp8(al);
            }
            0x2F => {
                let mut al = self.al();
                let old_cf = self.flag(CF);
                let old_al = al;
                if (al & 0xF) > 9 || self.flag(AF) {
                    al = al.wrapping_sub(6);
                    self.set_flag(AF, true);
                } else {
                    self.set_flag(AF, false);
                }
                if old_al > 0x99 || old_cf {
                    al = al.wrapping_sub(0x60);
                    self.set_flag(CF, true);
                } else {
                    self.set_flag(CF, false);
                }
                self.set_al(al);
                self.szp8(al);
            }
            0x37 => {
                if (self.al() & 0xF) > 9 || self.flag(AF) {
                    self.r[AX] = self.r[AX].wrapping_add(0x106);
                    self.set_flag(AF, true);
                    self.set_flag(CF, true);
                } else {
                    self.set_flag(AF, false);
                    self.set_flag(CF, false);
                }
                let al = self.al() & 0xF;
                self.set_al(al);
            }
            0x3F => {
                if (self.al() & 0xF) > 9 || self.flag(AF) {
                    let al = self.al().wrapping_sub(6);
                    self.set_al(al);
                    let ah = self.ah().wrapping_sub(1);
                    self.set_ah(ah);
                    self.set_flag(AF, true);
                    self.set_flag(CF, true);
                } else {
                    self.set_flag(AF, false);
                    self.set_flag(CF, false);
                }
                let al = self.al() & 0xF;
                self.set_al(al);
            }
            0x40..=0x47 => {
                let i = (op & 7) as usize;
                self.r[i] = self.inc16(self.r[i])
            }
            0x48..=0x4F => {
                let i = (op & 7) as usize;
                self.r[i] = self.dec16(self.r[i])
            }
            0x50..=0x57 => {
                let i = (op & 7) as usize;
                // 286 behaviour for PUSH SP: pushes old value
                let v = self.r[i];
                self.push(v)
            }
            0x58..=0x5F => {
                let v = self.pop();
                self.r[(op & 7) as usize] = v
            }
            0x60 => {
                // PUSHA (186+), only used in debug macro
                let sp = self.r[SP];
                for i in 0..8 {
                    let v = if i == SP { sp } else { self.r[i] };
                    self.push(v);
                }
            }
            0x61 => {
                for i in (0..8).rev() {
                    let v = self.pop();
                    if i != SP {
                        self.r[i] = v
                    }
                }
            }
            0x70..=0x7F => {
                let d = self.fetch8() as i8 as i16 as u16;
                if self.cond(op & 0xF) {
                    self.ip = self.ip.wrapping_add(d);
                }
            }
            0x80 | 0x82 => {
                self.decode_modrm();
                let b = self.fetch8();
                let a = self.rm8();
                let r = self.alu8(self.reg, a, b);
                if self.reg != 7 {
                    self.set_rm8(r)
                }
            }
            0x81 => {
                self.decode_modrm();
                let b = self.fetch16();
                let a = self.rm16();
                let r = self.alu16(self.reg, a, b);
                if self.reg != 7 {
                    self.set_rm16(r)
                }
            }
            0x83 => {
                self.decode_modrm();
                let b = self.fetch8() as i8 as i16 as u16;
                let a = self.rm16();
                let r = self.alu16(self.reg, a, b);
                if self.reg != 7 {
                    self.set_rm16(r)
                }
            }
            0x84 => {
                self.decode_modrm();
                let a = self.rm8();
                let b = self.r8(self.reg);
                self.alu8(4, a, b);
            }
            0x85 => {
                self.decode_modrm();
                let a = self.rm16();
                let b = self.r[self.reg as usize];
                self.alu16(4, a, b);
            }
            0x86 => {
                self.decode_modrm();
                let a = self.rm8();
                let b = self.r8(self.reg);
                self.set_rm8(b);
                self.set_r8(self.reg, a);
            }
            0x87 => {
                self.decode_modrm();
                let a = self.rm16();
                let b = self.r[self.reg as usize];
                self.set_rm16(b);
                self.r[self.reg as usize] = a;
            }
            0x88 => {
                self.decode_modrm();
                let v = self.r8(self.reg);
                self.set_rm8(v)
            }
            0x89 => {
                self.decode_modrm();
                let v = self.r[self.reg as usize];
                self.set_rm16(v)
            }
            0x8A => {
                self.decode_modrm();
                let v = self.rm8();
                self.set_r8(self.reg, v)
            }
            0x8B => {
                self.decode_modrm();
                let v = self.rm16();
                self.r[self.reg as usize] = v
            }
            0x8C => {
                self.decode_modrm();
                let v = self.s[(self.reg & 3) as usize];
                self.set_rm16(v)
            }
            0x8D => {
                self.decode_modrm();
                self.r[self.reg as usize] = self.ea_off
            }
            0x8E => {
                self.decode_modrm();
                let v = self.rm16();
                self.s[(self.reg & 3) as usize] = v
            }
            0x8F => {
                self.decode_modrm();
                let v = self.pop();
                self.set_rm16(v)
            }
            0x90 => {}
            0x91..=0x97 => {
                let i = (op & 7) as usize;
                self.r.swap(AX, i);
            }
            0x98 => self.r[AX] = self.al() as i8 as i16 as u16,
            0x99 => self.r[DX] = if self.r[AX] & 0x8000 != 0 { 0xFFFF } else { 0 },
            0x9A => {
                let off = self.fetch16();
                let seg = self.fetch16();
                let cs = self.s[CS];
                self.push(cs);
                let ip = self.ip;
                self.push(ip);
                self.ip = off;
                self.s[CS] = seg;
            }
            0x9B => {}
            0x9C => {
                let f = self.flags | 0xF002;
                self.push(f)
            }
            0x9D => {
                let f = self.pop();
                self.flags = (f & 0x0FD5) | 0x0002;
            }
            0x9E => {
                let ah = self.ah() as u16;
                self.flags = (self.flags & 0xFF00) | (ah & 0xD5) | 2;
            }
            0x9F => {
                let f = self.flags as u8;
                self.set_ah(f)
            }
            0xA0 => {
                let off = self.fetch16();
                let s = self.s[self.str_seg()];
                let v = self.rbs(s, off);
                self.set_al(v)
            }
            0xA1 => {
                let off = self.fetch16();
                let s = self.s[self.str_seg()];
                self.r[AX] = self.rws(s, off)
            }
            0xA2 => {
                let off = self.fetch16();
                let s = self.s[self.str_seg()];
                let v = self.al();
                self.wbs(s, off, v)
            }
            0xA3 => {
                let off = self.fetch16();
                let s = self.s[self.str_seg()];
                let v = self.r[AX];
                self.wws(s, off, v)
            }
            0xA4..=0xA7 | 0xAA..=0xAF => self.string_op(op),
            0xA8 => {
                let b = self.fetch8();
                let a = self.al();
                self.alu8(4, a, b);
            }
            0xA9 => {
                let b = self.fetch16();
                let a = self.r[AX];
                self.alu16(4, a, b);
            }
            0xB0..=0xB7 => {
                let v = self.fetch8();
                self.set_r8(op & 7, v)
            }
            0xB8..=0xBF => {
                let v = self.fetch16();
                self.r[(op & 7) as usize] = v
            }
            0xC2 => {
                let n = self.fetch16();
                self.ip = self.pop();
                self.r[SP] = self.r[SP].wrapping_add(n);
            }
            0xC3 => self.ip = self.pop(),
            0xC4 | 0xC5 => {
                self.decode_modrm();
                let s = self.s[self.ea_seg];
                let off = self.rws(s, self.ea_off);
                let seg = self.rws(s, self.ea_off.wrapping_add(2));
                self.r[self.reg as usize] = off;
                self.s[if op == 0xC4 { ES } else { DS }] = seg;
            }
            0xC6 => {
                self.decode_modrm();
                let v = self.fetch8();
                self.set_rm8(v)
            }
            0xC7 => {
                self.decode_modrm();
                let v = self.fetch16();
                self.set_rm16(v)
            }
            0xCA => {
                let n = self.fetch16();
                self.ip = self.pop();
                self.s[CS] = self.pop();
                self.r[SP] = self.r[SP].wrapping_add(n);
            }
            0xCB => {
                self.ip = self.pop();
                self.s[CS] = self.pop();
            }
            0xCC => self.software_int(3, io),
            0xCD => {
                let n = self.fetch8();
                self.software_int(n, io)
            }
            0xCE => {
                if self.flag(OF) {
                    self.software_int(4, io)
                }
            }
            0xCF => {
                self.ip = self.pop();
                self.s[CS] = self.pop();
                let f = self.pop();
                self.flags = (f & 0x0FD5) | 2;
            }
            0xD0..=0xD3 => {
                self.decode_modrm();
                let cnt = if op & 2 != 0 { self.r8(1) & 0x1F } else { 1 };
                if op & 1 == 0 {
                    let v = self.rm8();
                    let r = self.shift8(self.reg, v, cnt);
                    self.set_rm8(r)
                } else {
                    let v = self.rm16();
                    let r = self.shift16(self.reg, v, cnt);
                    self.set_rm16(r)
                }
            }
            0xD4 => {
                let b = self.fetch8();
                if b == 0 {
                    self.software_int(0, io);
                } else {
                    let al = self.al();
                    self.set_ah(al / b);
                    self.set_al(al % b);
                    let al = self.al();
                    self.szp8(al);
                }
            }
            0xD5 => {
                let b = self.fetch8();
                let al = self.al().wrapping_add(self.ah().wrapping_mul(b));
                self.r[AX] = al as u16;
                self.szp8(al);
            }
            0xD6 => {
                let v = if self.flag(CF) { 0xFF } else { 0 };
                self.set_al(v)
            }
            0xD7 => {
                let s = self.s[self.str_seg()];
                let off = self.r[BX].wrapping_add(self.al() as u16);
                let v = self.rbs(s, off);
                self.set_al(v)
            }
            0xD8..=0xDF => {
                self.decode_modrm();
            }
            0xE0..=0xE3 => {
                let d = self.fetch8() as i8 as i16 as u16;
                let take = match op {
                    0xE3 => self.r[CX] == 0,
                    _ => {
                        self.r[CX] = self.r[CX].wrapping_sub(1);
                        let nz = self.r[CX] != 0;
                        match op {
                            0xE0 => nz && !self.flag(ZF),
                            0xE1 => nz && self.flag(ZF),
                            _ => nz,
                        }
                    }
                };
                if take {
                    self.ip = self.ip.wrapping_add(d)
                }
            }
            0xE4 => {
                let p = self.fetch8() as u16;
                let v = io.port_in(self, p);
                self.set_al(v)
            }
            0xE5 => {
                let p = self.fetch8() as u16;
                let lo = io.port_in(self, p) as u16;
                let hi = io.port_in(self, p + 1) as u16;
                self.r[AX] = lo | (hi << 8)
            }
            0xE6 => {
                let p = self.fetch8() as u16;
                let v = self.al();
                io.port_out(self, p, v)
            }
            0xE7 => {
                let p = self.fetch8() as u16;
                let v = self.r[AX];
                io.port_out(self, p, v as u8);
                io.port_out(self, p + 1, (v >> 8) as u8)
            }
            0xE8 => {
                let d = self.fetch16();
                let ip = self.ip;
                self.push(ip);
                self.ip = self.ip.wrapping_add(d)
            }
            0xE9 => {
                let d = self.fetch16();
                self.ip = self.ip.wrapping_add(d)
            }
            0xEA => {
                let off = self.fetch16();
                let seg = self.fetch16();
                self.ip = off;
                self.s[CS] = seg;
            }
            0xEB => {
                let d = self.fetch8() as i8 as i16 as u16;
                self.ip = self.ip.wrapping_add(d)
            }
            0xEC => {
                let p = self.r[DX];
                let v = io.port_in(self, p);
                self.set_al(v)
            }
            0xED => {
                let p = self.r[DX];
                let lo = io.port_in(self, p) as u16;
                let hi = io.port_in(self, p + 1) as u16;
                self.r[AX] = lo | (hi << 8)
            }
            0xEE => {
                let p = self.r[DX];
                let v = self.al();
                io.port_out(self, p, v)
            }
            0xEF => {
                let p = self.r[DX];
                let v = self.r[AX];
                io.port_out(self, p, v as u8);
                io.port_out(self, p + 1, (v >> 8) as u8)
            }
            0xF4 => self.halted = true,
            0xF5 => self.flags ^= CF,
            0xF6 => {
                self.decode_modrm();
                let v = self.rm8();
                match self.reg {
                    0 | 1 => {
                        let b = self.fetch8();
                        self.alu8(4, v, b);
                    }
                    2 => self.set_rm8(!v),
                    3 => {
                        let r = self.alu8(5, 0, v);
                        self.set_flag(CF, v != 0);
                        self.set_rm8(r)
                    }
                    4 => {
                        let r = self.al() as u16 * v as u16;
                        self.r[AX] = r;
                        let hi = r & 0xFF00 != 0;
                        self.set_flag(CF, hi);
                        self.set_flag(OF, hi);
                    }
                    5 => {
                        let r = (self.al() as i8 as i16) * (v as i8 as i16);
                        self.r[AX] = r as u16;
                        let ext = r != (r as i8 as i16);
                        self.set_flag(CF, ext);
                        self.set_flag(OF, ext);
                    }
                    6 => {
                        if v == 0 {
                            self.software_int(0, io);
                            return;
                        }
                        let a = self.r[AX];
                        let q = a / v as u16;
                        if q > 0xFF {
                            self.software_int(0, io);
                            return;
                        }
                        self.set_al(q as u8);
                        self.set_ah((a % v as u16) as u8);
                    }
                    _ => {
                        if v == 0 {
                            self.software_int(0, io);
                            return;
                        }
                        let a = self.r[AX] as i16 as i32;
                        let d = v as i8 as i32;
                        let q = a / d;
                        if !(-128..=127).contains(&q) {
                            self.software_int(0, io);
                            return;
                        }
                        self.set_al(q as u8);
                        self.set_ah((a % d) as u8);
                    }
                }
            }
            0xF7 => {
                self.decode_modrm();
                let v = self.rm16();
                match self.reg {
                    0 | 1 => {
                        let b = self.fetch16();
                        self.alu16(4, v, b);
                    }
                    2 => self.set_rm16(!v),
                    3 => {
                        let r = self.alu16(5, 0, v);
                        self.set_flag(CF, v != 0);
                        self.set_rm16(r)
                    }
                    4 => {
                        let r = self.r[AX] as u32 * v as u32;
                        self.r[AX] = r as u16;
                        self.r[DX] = (r >> 16) as u16;
                        let hi = self.r[DX] != 0;
                        self.set_flag(CF, hi);
                        self.set_flag(OF, hi);
                    }
                    5 => {
                        let r = (self.r[AX] as i16 as i32) * (v as i16 as i32);
                        self.r[AX] = r as u16;
                        self.r[DX] = (r >> 16) as u16;
                        let ext = r != (r as i16 as i32);
                        self.set_flag(CF, ext);
                        self.set_flag(OF, ext);
                    }
                    6 => {
                        if v == 0 {
                            self.software_int(0, io);
                            return;
                        }
                        let a = ((self.r[DX] as u32) << 16) | self.r[AX] as u32;
                        let q = a / v as u32;
                        if q > 0xFFFF {
                            self.software_int(0, io);
                            return;
                        }
                        self.r[AX] = q as u16;
                        self.r[DX] = (a % v as u32) as u16;
                    }
                    _ => {
                        if v == 0 {
                            self.software_int(0, io);
                            return;
                        }
                        let a = (((self.r[DX] as u32) << 16) | self.r[AX] as u32) as i32;
                        let d = v as i16 as i32;
                        let q = a / d;
                        if !(-32768..=32767).contains(&q) {
                            self.software_int(0, io);
                            return;
                        }
                        self.r[AX] = q as u16;
                        self.r[DX] = (a % d) as u16;
                    }
                }
            }
            0xF8 => self.set_flag(CF, false),
            0xF9 => self.set_flag(CF, true),
            0xFA => self.set_flag(IF, false),
            0xFB => self.set_flag(IF, true),
            0xFC => self.set_flag(DF, false),
            0xFD => self.set_flag(DF, true),
            0xFE => {
                self.decode_modrm();
                let v = self.rm8();
                match self.reg {
                    0 => {
                        let r = self.inc8(v);
                        self.set_rm8(r)
                    }
                    1 => {
                        let r = self.dec8(v);
                        self.set_rm8(r)
                    }
                    _ => panic!("bad FE /{} at {:04x}:{:04x}", self.reg, self.s[CS], start_ip),
                }
            }
            0xFF => {
                self.decode_modrm();
                match self.reg {
                    0 => {
                        let v = self.rm16();
                        let r = self.inc16(v);
                        self.set_rm16(r)
                    }
                    1 => {
                        let v = self.rm16();
                        let r = self.dec16(v);
                        self.set_rm16(r)
                    }
                    2 => {
                        let t = self.rm16();
                        let ip = self.ip;
                        self.push(ip);
                        self.ip = t;
                    }
                    3 => {
                        let s = self.s[self.ea_seg];
                        let off = self.rws(s, self.ea_off);
                        let seg = self.rws(s, self.ea_off.wrapping_add(2));
                        let cs = self.s[CS];
                        self.push(cs);
                        let ip = self.ip;
                        self.push(ip);
                        self.ip = off;
                        self.s[CS] = seg;
                    }
                    4 => self.ip = self.rm16(),
                    5 => {
                        let s = self.s[self.ea_seg];
                        self.ip = self.rws(s, self.ea_off);
                        self.s[CS] = self.rws(s, self.ea_off.wrapping_add(2));
                    }
                    6 => {
                        let v = self.rm16();
                        self.push(v)
                    }
                    _ => panic!("bad FF /7"),
                }
            }
            _ => panic!(
                "unimplemented opcode {:02x} at {:04x}:{:04x}",
                op, self.s[CS], start_ip
            ),
        }
    }

    fn software_int(&mut self, n: u8, io: &mut dyn Io) {
        if !io.int(self, n) {
            self.do_int(n);
        }
    }

    fn string_op(&mut self, op: u8) {
        let word = op & 1 != 0;
        let delta: u16 = if word { 2 } else { 1 };
        let step = |c: &Cpu| -> u16 {
            if c.flag(DF) {
                0u16.wrapping_sub(delta)
            } else {
                delta
            }
        };
        let src_seg = self.s[self.str_seg()];
        let is_cmp = matches!(op, 0xA6 | 0xA7 | 0xAE | 0xAF);
        loop {
            if self.rep != 0 && self.r[CX] == 0 {
                break;
            }
            let d = step(self);
            match op {
                0xA4 | 0xA5 => {
                    if word {
                        let v = self.rws(src_seg, self.r[SI]);
                        let es = self.s[ES];
                        self.wws(es, self.r[DI], v);
                    } else {
                        let v = self.rbs(src_seg, self.r[SI]);
                        let es = self.s[ES];
                        self.wbs(es, self.r[DI], v);
                    }
                    self.r[SI] = self.r[SI].wrapping_add(d);
                    self.r[DI] = self.r[DI].wrapping_add(d);
                }
                0xA6 | 0xA7 => {
                    if word {
                        let a = self.rws(src_seg, self.r[SI]);
                        let b = self.rws(self.s[ES], self.r[DI]);
                        self.alu16(7, a, b);
                    } else {
                        let a = self.rbs(src_seg, self.r[SI]);
                        let b = self.rbs(self.s[ES], self.r[DI]);
                        self.alu8(7, a, b);
                    }
                    self.r[SI] = self.r[SI].wrapping_add(d);
                    self.r[DI] = self.r[DI].wrapping_add(d);
                }
                0xAA | 0xAB => {
                    let es = self.s[ES];
                    if word {
                        let v = self.r[AX];
                        self.wws(es, self.r[DI], v);
                    } else {
                        let v = self.al();
                        self.wbs(es, self.r[DI], v);
                    }
                    self.r[DI] = self.r[DI].wrapping_add(d);
                }
                0xAC | 0xAD => {
                    if word {
                        self.r[AX] = self.rws(src_seg, self.r[SI]);
                    } else {
                        let v = self.rbs(src_seg, self.r[SI]);
                        self.set_al(v);
                    }
                    self.r[SI] = self.r[SI].wrapping_add(d);
                }
                _ => {
                    // scas
                    if word {
                        let a = self.r[AX];
                        let b = self.rws(self.s[ES], self.r[DI]);
                        self.alu16(7, a, b);
                    } else {
                        let a = self.al();
                        let b = self.rbs(self.s[ES], self.r[DI]);
                        self.alu8(7, a, b);
                    }
                    self.r[DI] = self.r[DI].wrapping_add(d);
                }
            }
            if self.rep == 0 {
                break;
            }
            self.r[CX] = self.r[CX].wrapping_sub(1);
            if is_cmp {
                let z = self.flag(ZF);
                if (self.rep == 0xF3 && !z) || (self.rep == 0xF2 && z) {
                    break;
                }
            }
        }
    }
}
