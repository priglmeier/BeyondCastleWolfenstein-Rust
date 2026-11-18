//! The small slice of IBM PC BIOS behaviour the game relies on: CGA mode 4
//! (320x200, 4 colours), the cursor, teletype text output using the 8x8 ROM
//! font, palette selection, and the "ROM" bytes at F800:0000 that the game
//! reads as a source of randomness and screen noise.
//!
//! This file is shared verbatim with the reference test harness, so both
//! produce byte-identical video memory.

/// IBM PC 8x8 character set, first 128 glyphs (as found at F000:FA6E).
pub static FONT_8X8: &[u8; 1024] = include_bytes!("../../assets/font8x8.bin");

pub const VRAM_SIZE: usize = 0x4000;

#[derive(Clone, Debug)]
pub struct Bios {
    pub mode: u8,
    pub row: u8,
    pub col: u8,
    /// CGA colour-select register (port 3D9h): bits 0-3 background,
    /// bit 4 intensity, bit 5 palette (0 = green/red/brown, 1 = cyan/magenta/white).
    pub color_select: u8,
}

impl Default for Bios {
    fn default() -> Self {
        Bios {
            mode: 3,
            row: 0,
            col: 0,
            color_select: 0x30,
        }
    }
}

impl Bios {
    /// INT 10h AH=00h
    pub fn set_mode(&mut self, vram: &mut [u8], mode: u8) {
        self.mode = mode & 0x7F;
        self.row = 0;
        self.col = 0;
        if mode & 0x80 == 0 {
            for b in vram.iter_mut() {
                *b = 0;
            }
        }
        if self.mode == 4 || self.mode == 5 {
            self.color_select = 0x30;
        }
    }

    /// INT 10h AH=02h
    pub fn set_cursor(&mut self, row: u8, col: u8) {
        self.row = row;
        self.col = col;
    }

    /// INT 10h AH=0Bh
    pub fn set_palette(&mut self, bh: u8, bl: u8) {
        if bh == 0 {
            self.color_select = (self.color_select & 0xE0) | (bl & 0x1F);
        } else {
            self.color_select = (self.color_select & !0x20) | if bl & 1 != 0 { 0x20 } else { 0 };
        }
    }

    fn put_pixel(vram: &mut [u8], x: usize, y: usize, color: u8) {
        let mut off = (y >> 1) * 80 + (x >> 2);
        if y & 1 != 0 {
            off += 0x2000;
        }
        let shift = 2 * (3 - (x & 3));
        let old = vram[off];
        vram[off] = if color & 0x80 != 0 {
            old ^ ((color & 3) << shift)
        } else {
            (old & !(3 << shift)) | ((color & 3) << shift)
        };
    }

    /// Draw one 8x8 glyph at a character cell (mode 4 only).
    pub fn write_char(vram: &mut [u8], col: u8, row: u8, ch: u8, attr: u8) {
        let back = attr & 0x80;
        let x0 = col as usize * 8;
        let y0 = row as usize * 8;
        for h in 0..8 {
            let bits = if ch < 128 {
                FONT_8X8[ch as usize * 8 + h]
            } else {
                0
            };
            for b in 0..8 {
                let on = bits & (0x80 >> b) != 0;
                Self::put_pixel(vram, x0 + b, y0 + h, if on { attr } else { back });
            }
        }
    }

    /// Scroll the whole 40x25 graphics screen up one text row, filling with 0.
    fn scroll_up(vram: &mut [u8]) {
        for bank in [0usize, 0x2000] {
            // 4 interleaved scan lines per bank make up one text row (80 bytes each)
            vram.copy_within(bank + 320..bank + 8000, bank);
            for b in &mut vram[bank + 8000 - 320..bank + 8000] {
                *b = 0;
            }
        }
    }

    /// INT 10h AH=0Eh, graphics modes.
    pub fn teletype(&mut self, vram: &mut [u8], ch: u8, attr: u8) {
        let (mut row, mut col) = (self.row, self.col);
        match ch {
            7 => return,
            8 => {
                if col > 0 {
                    col -= 1
                }
            }
            b'\r' => col = 0,
            b'\n' => row += 1,
            _ => {
                if self.mode == 4 || self.mode == 5 {
                    Self::write_char(vram, col, row, ch, attr);
                }
                col += 1;
            }
        }
        if col == 40 {
            col = 0;
            row += 1;
        }
        if row == 25 {
            if self.mode == 4 || self.mode == 5 {
                Self::scroll_up(vram);
            }
            row -= 1;
        }
        self.row = row;
        self.col = col;
    }
}

/// 64 KiB stand-in for the PC's ROM area at F800:0000. The game reads words
/// from here as its random number source and XORs it onto the screen as
/// "noise" when you walk into a wall. Real BIOS ROMs differ from machine to
/// machine; this image is a fixed pseudo-random fill with the 8x8 font at its
/// real location (F000:FA6E = F800:7A6E).
pub fn rom_f800() -> Vec<u8> {
    let mut rom = vec![0u8; 0x10000];
    let mut x: u32 = 0x1F2E_3D4C;
    for b in rom.iter_mut() {
        // xorshift32
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *b = (x >> 11) as u8;
    }
    rom[0x7A6E..0x7A6E + 1024].copy_from_slice(&FONT_8X8[..]);
    rom[0x7F53] = 0xCF; // IRET used as the default interrupt handler
    rom[0x7FFE] = 0xFC; // machine ID: PC/AT (not a PCjr)
    rom
}
