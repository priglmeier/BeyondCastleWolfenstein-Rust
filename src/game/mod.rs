//! Beyond Castle Wolfenstein game logic, translated routine by routine from
//! the public-domain 8086 source (BCW.ASM, Silas Warner / jeff leyda).
//!
//! The original code is register-oriented spaghetti: routines pass values
//! in registers and flags, jump into the middle of each other, and keep all
//! state in a few hundred bytes of variables inside the code segment. To stay
//! faithful the port keeps that model:
//!
//! * `cs` holds the original code-segment image. Every game variable and
//!   table lives at its original offset (see `sym.rs`), so indexed accesses
//!   such as `v597[si]` behave exactly as before.
//! * `ds` holds the data segment (bunker maps, demo input, speaker voices);
//!   the "900h" disk buffer the game uses for all 64 rooms is `ds[3000h..]`.
//! * `r` is a small register file (AX BX CX DX SI DI) and `cf`/`zf`/`sf`
//!   carry the flag results that routines return to their callers.
//!
//! Everything that touched hardware (CGA memory, keyboard, PC speaker,
//! Sound Blaster, DOS files and clock) goes through the [`Host`] trait.

#![allow(clippy::all)]

mod core;
mod draw;
mod guards;
mod items;
mod main_loop;
mod menu;
mod sound;
pub mod sym;

use crate::pc::bios::{Bios, VRAM_SIZE};
pub use sym::*;

pub static CODE_IMAGE: &[u8] = include_bytes!("../../assets/code.bin");
pub static DATA_IMAGE: &[u8] = include_bytes!("../../assets/data.bin");
pub static LOGO_IMAGE: &[u8] = include_bytes!("../../assets/logo.bin");
pub static SB1_IMAGE: &[u8] = include_bytes!("../../assets/sb1.bin");
pub static SB2_IMAGE: &[u8] = include_bytes!("../../assets/sb2.bin");

/// Offset of the 900h disk buffer (all room data) inside the data segment.
pub const SEG900: usize = 0x3000;

/// Byte ranges of the code segment whose contents depend on the host
/// environment (DOS segment numbers, file handles, timer scratch) rather than
/// on the game. They are excluded from state comparisons.
pub static HOST_SPECIFIC: &[(u16, u16)] = &[
    (BUF_SEG900, 2),
    (BUF_SEG_D00, 2),
    (FILEHANDLE, 6),
    (MS, 1),
    (SAMPLE_SEG, 2),
    (OLD_ISR3SEG, 12),
    (ISR_FLAG, 2),
];

/// Ways the original code abandons its call stack with a far `jmp`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Jump {
    /// `jmp loc_411`: back to the title screen / demo.
    Title,
    /// `jmp startGame`: re-copy pristine data, then title screen.
    StartGame,
    /// `jmp mainLoop` from inside a nested routine.
    MainLoop,
    /// `jmp exitProgram`.
    Exit,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Regs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
}

macro_rules! byte_regs {
    ($($lo:ident $hi:ident $set_lo:ident $set_hi:ident $w:ident),*) => {
        impl Regs {
            $(
            #[inline] pub fn $lo(&self) -> u8 { self.$w as u8 }
            #[inline] pub fn $hi(&self) -> u8 { (self.$w >> 8) as u8 }
            #[inline] pub fn $set_lo(&mut self, v: u8) { self.$w = (self.$w & 0xFF00) | v as u16 }
            #[inline] pub fn $set_hi(&mut self, v: u8) { self.$w = (self.$w & 0x00FF) | ((v as u16) << 8) }
            )*
        }
    };
}
byte_regs!(al ah set_al set_ah ax, bl bh set_bl set_bh bx, cl ch set_cl set_ch cx, dl dh set_dl set_dh dx);

/// Everything the game owns: memory images, registers and BIOS video state.
pub struct State {
    pub cs: Vec<u8>,
    pub ds: Vec<u8>,
    pub vram: Vec<u8>,
    pub rom: Vec<u8>,
    pub buf900: Vec<u8>,
    pub buf_d00: Vec<u8>,
    pub bios: Bios,
    pub r: Regs,
    pub cf: bool,
    pub zf: bool,
    pub sf: bool,
    /// Sound Blaster present (`sbBase != -1` in the original).
    pub sb: bool,
}

impl State {
    pub fn new() -> State {
        let mut cs = vec![0u8; 0x10000];
        cs[..CODE_IMAGE.len()].copy_from_slice(CODE_IMAGE);
        let mut ds = vec![0u8; 0x10000];
        ds[..DATA_IMAGE.len()].copy_from_slice(DATA_IMAGE);
        State {
            cs,
            ds,
            vram: vec![0u8; VRAM_SIZE],
            rom: crate::pc::bios::rom_f800(),
            buf900: vec![0u8; 0x4000],
            buf_d00: vec![0u8; 0x3000],
            bios: Bios::default(),
            r: Regs::default(),
            cf: false,
            zf: false,
            sf: false,
            sb: false,
        }
    }

    // ---- code-segment variable access (all offsets wrap like 16-bit pointers)
    #[inline]
    pub fn b(&self, off: u16) -> u8 {
        self.cs[off as usize]
    }
    #[inline]
    pub fn setb(&mut self, off: u16, v: u8) {
        self.cs[off as usize] = v;
    }
    #[inline]
    pub fn w(&self, off: u16) -> u16 {
        self.cs[off as usize] as u16 | ((self.cs[off.wrapping_add(1) as usize] as u16) << 8)
    }
    #[inline]
    pub fn setw(&mut self, off: u16, v: u16) {
        self.cs[off as usize] = v as u8;
        self.cs[off.wrapping_add(1) as usize] = (v >> 8) as u8;
    }
    #[inline]
    pub fn incb(&mut self, off: u16) -> u8 {
        let v = self.b(off).wrapping_add(1);
        self.setb(off, v);
        v
    }
    #[inline]
    pub fn decb(&mut self, off: u16) -> u8 {
        let v = self.b(off).wrapping_sub(1);
        self.setb(off, v);
        v
    }

    // ---- the 900h segment (room buffer) inside the data segment
    #[inline]
    pub fn s9(&self, off: u16) -> u8 {
        self.ds[SEG900 + off as usize]
    }
    #[inline]
    pub fn set_s9(&mut self, off: u16, v: u8) {
        self.ds[SEG900 + off as usize] = v;
    }

    // ---- video memory (segment B800h)
    #[inline]
    pub fn vw(&self, off: u16) -> u16 {
        let o = off as usize & 0x3FFF;
        self.vram[o] as u16 | ((self.vram[(o + 1) & 0x3FFF] as u16) << 8)
    }
    #[inline]
    pub fn set_vw(&mut self, off: u16, v: u16) {
        let o = off as usize & 0x3FFF;
        self.vram[o] = v as u8;
        self.vram[(o + 1) & 0x3FFF] = (v >> 8) as u8;
    }

    /// Bytes compared against the reference harness: the code segment with
    /// host-specific fields zeroed, the data segment, video memory, cursor.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(0x5B08 + 0x7004 + VRAM_SIZE + 2);
        let mut code = self.cs[..0x5B08].to_vec();
        for &(off, len) in HOST_SPECIFIC {
            for b in &mut code[off as usize..(off + len) as usize] {
                *b = 0;
            }
        }
        v.extend_from_slice(&code);
        v.extend_from_slice(&self.ds[..0x7004]);
        v.extend_from_slice(&self.vram);
        v.push(self.bios.row);
        v.push(self.bios.col);
        v
    }
}

/// Which file a DOS file call refers to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameFile {
    SaveGame,
    OptionA,
    OptionC,
}

/// The machine the game runs on. Every hardware/BIOS/DOS interaction of the
/// original program maps to one of these calls. The order and number of
/// keyboard polls is identical to the original, which is what lets the port
/// be checked against the original binary.
pub trait Host {
    /// INT 16h AH=01h: is a key waiting? (does not remove it)
    fn key_peek(&mut self, st: &State) -> Option<u16>;
    /// INT 16h AH=00h: wait for and remove a key.
    fn key_get(&mut self, st: &State) -> u16;
    /// INT 21h AH=06h DL=FFh: remove a key if one is waiting.
    fn key_dos(&mut self, st: &State) -> Option<u8>;
    /// INT 21h AH=0Ch: flush the keyboard buffer.
    fn key_flush(&mut self);
    /// `starttime`: remember the DOS clock (18.2 Hz ticks).
    fn tick_mark(&mut self);
    /// `delaytime`: wait until the clock has moved on since `tick_mark`.
    fn tick_wait_mark(&mut self, st: &State);
    /// `starttime` immediately followed by `delaytime`.
    fn tick_wait(&mut self, st: &State) {
        self.tick_mark();
        self.tick_wait_mark(st);
    }
    /// Busy-wait of `us` microseconds (the original's `in al,80h` loops).
    fn io_delay(&mut self, us: u32);
    /// Flip the PC speaker cone (port 61h bit 1).
    fn speaker_toggle(&mut self);
    /// Start a Sound Blaster DMA transfer of 8-bit unsigned samples.
    fn sb_play(&mut self, samples: &[u8], time_constant: u8);
    /// Has the last Sound Blaster transfer finished (end-of-DMA IRQ)?
    fn sb_done(&mut self) -> bool;
    /// Read a whole file, if it exists.
    fn file_read(&mut self, f: GameFile) -> Option<Vec<u8>>;
    /// Create/overwrite a file.
    fn file_write(&mut self, f: GameFile, data: &[u8]);
    /// The game switched to text mode and printed a message (fatal errors).
    fn text_message(&mut self, msg: &str);
    /// Video memory or palette may have changed.
    fn present(&mut self, st: &State);
}

pub struct Options {
    /// `-nosb`: use the PC speaker even for voices.
    pub no_sb: bool,
    /// `-quiet`: no sound at all.
    pub quiet: bool,
}

pub struct Game<H: Host> {
    pub st: State,
    pub host: H,
}

impl<H: Host> std::ops::Deref for Game<H> {
    type Target = State;
    fn deref(&self) -> &State {
        &self.st
    }
}
impl<H: Host> std::ops::DerefMut for Game<H> {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.st
    }
}

/// Unwind to the matching catch point (the original's far `jmp`s).
pub fn jump(j: Jump) -> ! {
    std::panic::resume_unwind(Box::new(j))
}

impl<H: Host> Game<H> {
    pub fn new(host: H, opts: &Options) -> Game<H> {
        let mut g = Game {
            st: State::new(),
            host,
        };
        g.check_requirements();
        g.check_command_line(opts);
        g
    }

    /// Run until the player quits.
    pub fn run(&mut self) {
        // main: sbLocate / moveSamples
        self.sb_locate();
        let mut next = Jump::StartGame;
        loop {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match next {
                Jump::StartGame => self.start_game(),
                Jump::Title => self.loc_411(),
                Jump::MainLoop => self.main_loop(),
                Jump::Exit => unreachable!(),
            }));
            match res {
                Ok(()) => unreachable!(),
                Err(e) => match e.downcast::<Jump>() {
                    Ok(j) => {
                        if *j == Jump::Exit {
                            self.exit_program();
                            return;
                        }
                        next = *j;
                    }
                    Err(e) => std::panic::resume_unwind(e),
                },
            }
        }
    }
}
