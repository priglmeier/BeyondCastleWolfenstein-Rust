//! Screen drawing. The game draws everything with 8x8 "glyphs" (16 bytes
//! of 2-bit CGA pixels each) from the table at cs:v11A4, strung together
//! into small pictures by byte strings listed in gfxTable.

use super::*;

impl<H: Host> Game<H> {
    /// cls: clear all of video memory.
    pub(super) fn cls(&mut self) {
        for b in self.st.vram.iter_mut() {
            *b = 0;
        }
        self.r.di = 0x4000;
        self.r.ax = 0;
        self.r.cx = 0;
        self.host.present(&self.st);
    }

    /// putSprite?: clear a block. CH = text rows, CL = words per scan line,
    /// DI + DX = start offset.
    pub(super) fn put_sprite(&mut self) {
        self.r.ax = 0;
        let mut bl = self.r.ch() << 3;
        let words = self.r.cl() as u16;
        let mut di = self.r.di.wrapping_add(self.r.dx);
        loop {
            for i in 0..words {
                self.st.set_vw(di.wrapping_add(i * 2), 0);
            }
            di = di.wrapping_add(0x2000);
            if di >= 0x4000 {
                di = di.wrapping_sub(0x3FB0);
            }
            bl = bl.wrapping_sub(1);
            if bl == 0 {
                break;
            }
        }
        self.r.di = di;
        self.r.cx = words;
        self.r.set_bl(0);
    }

    /// modifySprite: save the 8x8 cell at DI+DX into glyph 1Bh (so it can
    /// be put back after a gunshot flash).
    pub(super) fn modify_sprite(&mut self) {
        let di = self.r.di.wrapping_add(self.r.dx);
        let offs = [0u16, 0x2000, 0x50, 0x2050, 0xA0, 0x20A0, 0xF0, 0x20F0];
        let dst = [V1274, V1276, V1278, V127A, V127C, V127E, V1280, V1282];
        for i in 0..8 {
            let v = self.st.vw(di.wrapping_add(offs[i]));
            self.setw(dst[i], v);
            self.r.ax = v;
        }
    }

    /// draw2Screen: glyph AL at DI+DX, then DX += 2 (wrapping to the next
    /// cell row at the right edge). Glyph 0Dh means "next line".
    pub(super) fn draw2screen(&mut self) {
        let al = self.r.al();
        if al != 0x0D {
            let di = self.r.di.wrapping_add(self.r.dx);
            let mut ax = (al as u16) << 4;
            ax = ax.wrapping_add(V11A4).wrapping_sub(0xE0);
            self.r.ax = ax;
            let src = ax;
            let offs = [0u16, 0x2000, 0x50, 0x2050, 0xA0, 0x20A0, 0xF0, 0x20F0];
            for i in 0..8u16 {
                let v = self.w(src + i * 2);
                self.st.set_vw(di.wrapping_add(offs[i as usize]), v);
            }
            self.r.dx = self.r.dx.wrapping_add(2);
            if self.r.dx != 0x50 {
                self.cf = false;
                return;
            }
        }
        // loc_150
        self.r.dx = self.w(XPOS);
        self.r.di = self.r.di.wrapping_add(320);
        self.cf = false;
    }

    /// put_gfx: draw picture number SI (index into gfxTable).
    pub(super) fn put_gfx(&mut self) {
        let mut si = self.w(GFX_TABLE + self.r.si);
        loop {
            let al = self.b(si);
            si = si.wrapping_add(1);
            self.r.set_al(al);
            if al == 0 {
                break;
            }
            self.r.si = si;
            self.draw2screen();
        }
        self.r.si = si;
    }

    /// printPasses: the column of pass numbers on the left.
    pub(super) fn print_passes(&mut self) {
        self.r.di = 0x0C80;
        let entry_ah = self.r.ah();
        let mut al = self.b(PASSES);
        self.setb(PASS_ROW, 0x14);
        loop {
            let carry = al & 1 != 0;
            al >>= 1;
            let mut bx: u16 = 0x2020;
            if carry {
                bx = 0x1300 | self.b(PASS_ROW) as u16;
            }
            self.r.bx = bx;
            self.r.set_al(bx as u8);
            self.r.dx = 0;
            self.draw2screen();
            self.r.bx = bx;
            self.r.set_al((bx >> 8) as u8);
            self.draw2screen();
            self.r.di = self.r.di.wrapping_add(0x280);
            self.r.set_al(al);
            if self.incb(PASS_ROW) == 0x19 {
                break;
            }
        }
        self.r.set_ah(entry_ah);
    }

    /// refreshScreen (sub_21): redraw the whole room.
    pub(super) fn refresh_screen(&mut self) {
        self.cls();
        self.print_passes();
        self.set_health();
        self.setb(V4D5, 0xFF);
        let mut bx: u16 = 0;
        loop {
            self.r.bx = bx;
            self.draw_screen();
            self.setb(V54F + bx, 0);
            bx += 1;
            if bx == 0x48 {
                break;
            }
        }
        self.r.bx = bx;
        let mut si: u16 = 0x80;
        loop {
            self.r.si = si;
            let al = self.b(V597 + si) & 0xF0;
            self.r.set_al(al);
            if al != 0 {
                let bl = self.b(V598 + si);
                self.r.set_bl(bl);
                self.sub_22();
            }
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
        self.r.si = si;
        self.sub_26();
        self.host.present(&self.st);
    }

    /// sub_22: draw object SI of type AL through the vC74 table.
    fn sub_22(&mut self) {
        let ax = (self.r.al() as i8 as i16 as u16) >> 3;
        self.r.ax = ax;
        self.r.set_bh(0);
        self.r.di = ax;
        let si = self.r.si;
        let target = self.w(VC74 + ax);
        self.call_code(target);
        self.r.si = si;
    }

    /// sub_23: the cell the gun points at (BX = C0h if blocked/holstered).
    pub(super) fn sub_23(&mut self) {
        let bl = self.b(V4E5);
        self.r.bx = bl as u16;
        let al = self.b(GUN_DIR);
        self.r.ax = al as i8 as i16 as u16;
        self.r.di = self.r.ax;
        if al == 0 {
            self.r.set_bl(0xC0);
            return;
        }
        if al != 2 {
            self.r.bx = self.r.bx.wrapping_add(8);
        }
        let cl = self.b(V597 + self.r.bx) & self.b(V697 + self.r.di);
        self.r.set_cl(cl);
        if cl != 0 {
            self.r.set_bl(0xC0);
            return;
        }
        self.sub_24();
    }

    /// sub_24: AL = direction, BL = cell -> DL/DH moved column/row, BL = new cell.
    pub(super) fn sub_24(&mut self) {
        self.decode_bcd();
        self.r.ax = self.r.al() as i8 as i16 as u16;
        self.r.bx = self.r.ax;
        let bx = self.r.bx;
        let dl = self.r.dl().wrapping_add(self.b(V6AF + bx));
        let dh = self.r.dh().wrapping_add(self.b(V6BB + bx));
        self.r.set_dl(dl);
        self.r.set_dh(dh);
        self.r.set_bl((dh << 3) | dl);
    }

    /// sub_25: player position shifted half a step (walking animation).
    pub(super) fn sub_25(&mut self) {
        self.r.set_bh(0);
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.r.set_bh(0);
        let bl = self.b(MOVE_DIR);
        self.r.set_bl(bl);
        let al = self.b(V6AF + self.r.bx) << 2;
        self.r.set_al(al);
        let dl = self.r.dl().wrapping_add(al);
        self.r.set_dl(dl);
        self.r.bx <<= 1;
        self.r.di = self.r.di.wrapping_add(self.w(V6C7 + self.r.bx));
        self.loc_110();
    }

    /// sub_26: draw the player.
    pub(super) fn sub_26(&mut self) {
        self.r.set_bh(0);
        let bl = self.b(MOVE_DIR);
        self.r.set_bl(bl);
        let al = self.b(V70B + self.r.bx);
        self.r.set_al(al);
        self.setb(V10B8, al);
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.loc_110();
    }

    fn loc_110(&mut self) {
        self.r.dx = self.r.dx.wrapping_add(2);
        self.r.di = self.r.di.wrapping_add(0x140);
        let (dx, di) = (self.r.dx, self.r.di);
        self.setw(XPOS, dx);
        self.setw(V4E6, di);
        self.setw(V4E8, dx);
        self.r.set_bh(0);
        let bl = self.b(GUN_DIR);
        self.r.set_bl(bl);
        let bx = self.r.bx;
        let (mut al, mut ah) = (self.b(V6DF + bx), self.b(V6EA + bx));
        if self.b(V4D9) != 0 {
            al = self.b(V6F5 + bx);
            ah = self.b(V700 + bx);
        }
        self.r.set_al(al);
        self.r.set_ah(ah);
        self.setb(V10B3, al);
        self.setb(V10B5, ah);
        if self.b(BOMB_SET) & 0x80 != 0 {
            if self.r.bx == 0 {
                let bl = self.b(V4DE);
                self.r.set_bl(bl);
            }
            let bl = self.r.bl();
            self.setb(V4DE, bl);
            let bx = self.r.bx;
            let al = self.b(V72C + bx);
            self.setb(V10B7, al);
            let al = self.b(V737 + bx);
            self.r.set_al(al);
            self.setb(V10B9, al);
        }
        self.r.si = 0x1A;
        self.put_gfx();
        // a body being dragged is drawn with the player
        let bl = self.b(V4E5);
        self.r.bx = bl as u16;
        let si = self.w(V54F + self.r.bx) & 0xFF;
        self.r.si = si;
        if si != 0 && self.b(V597 + si) == 0x40 {
            self.sub_50();
        }
        let bl = self.b(V4E5);
        self.r.bx = bl as u16;
        let si = self.w(V557 + self.r.bx) & 0xFF;
        self.r.si = si;
        if si != 0 && self.b(V597 + si) == 0x40 {
            self.sub_50();
        }
        // the placed bomb
        if self.b(BOMB_SET) & 1 == 0 {
            return;
        }
        let al = self.b(V4F5);
        self.r.set_al(al);
        if self.b(ROOM_INDEX) != al {
            return;
        }
        let bl = self.b(V4F6);
        self.r.bx = bl as u16;
        self.get_screen_pos();
        if self.b(V4DF) == 0 {
            self.r.dx = self.r.dx.wrapping_add(4);
        }
        self.r.dx = self.r.dx.wrapping_add(2);
        self.r.di = self.r.di.wrapping_add(0x3C0);
        self.r.set_al(0x46);
        self.draw2screen();
    }

    /// sub_27: player's legs for the current direction.
    pub(super) fn sub_27(&mut self) {
        let bl = self.b(MOVE_DIR);
        self.r.set_bl(bl);
        let al = if bl == 2 { 0x75 } else { 0x77 };
        self.setb(V10B4, al);
        self.r.set_bh(0);
        let bx = self.r.bx;
        let al = self.b(V721 + bx);
        self.setb(V10B0, al);
        let al = self.b(V716 + bx);
        self.r.set_al(al);
        self.setb(V10B8, al);
    }

    // ---- object drawing (vC74 table targets)

    /// vc82: type 50h (the Fuehrer's desk?).
    pub(super) fn vc82(&mut self) {
        let al = self.b(V4DC);
        self.r.set_al(al);
        if self.b(ROOM_INDEX) != al {
            self.sub_28();
        } else {
            self.loc_571();
        }
    }

    /// sub_28: a multi-cell object (desk / table) at SI.
    pub(super) fn sub_28(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.r.di = self.r.di.wrapping_add(0x140);
        let saved_bx = self.r.bx;
        let mut bx = self.r.bx;
        let mut cx: u16 = 1;
        let al = self.b(V59B + si);
        self.r.set_al(al);
        if al >= 5 {
            cx = 6;
            bx = bx.wrapping_sub(1);
        }
        self.r.ax = si;
        let al = si as u8;
        loop {
            self.setb(V54F + bx, al);
            self.setb(V557 + bx, al);
            bx = bx.wrapping_add(1);
            cx -= 1;
            if cx == 0 {
                break;
            }
        }
        self.r.cx = 0;
        self.r.bx = saved_bx;
        let al = self.b(V59B + si);
        self.r.ax = al as u16;
        self.r.si = ((al as u16) << 1).wrapping_add(0x0C);
        self.put_gfx();
    }

    /// loc_121: a body.
    pub(super) fn loc_121(&mut self) {
        self.sub_29();
        self.loc_122();
    }

    pub(super) fn loc_122(&mut self) {
        self.sub_30();
        self.r.si = 0x0A;
        self.put_gfx();
    }

    pub(super) fn sub_29(&mut self) {
        let bx = self.r.bx;
        let si = self.r.si;
        let al = self.b(V54F + bx);
        self.r.set_al(al);
        if al != 0 {
            self.r.set_ah(0);
            if self.r.ax != si {
                self.r.di = self.r.ax;
                let t = self.b(V597 + self.r.di);
                self.r.set_al(t);
                if t != 0x10 {
                    self.r.ax = si;
                    self.setb(V59A + si, si as u8);
                }
            }
        }
        self.r.ax = si;
        self.setb(V54F + bx, si as u8);
    }

    pub(super) fn sub_30(&mut self) {
        let bl = self.b(V598 + self.r.si);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.r.di = self.r.di.wrapping_add(0x140);
        self.r.dx = self.r.dx.wrapping_add(2);
        let dx = self.r.dx;
        self.setw(XPOS, dx);
    }

    /// vd0b: type 60h.
    pub(super) fn vd0b(&mut self) {
        let si = self.r.si;
        self.r.ax = si;
        let bx = self.r.bx;
        self.setb(CTRL_KEY + 1 + bx, si as u8);
        self.setb(V556 + bx, si as u8);
        self.get_screen_pos();
        self.r.si = 8;
        self.put_gfx();
    }

    /// vd1e: type 20h, a closet (clears the status line first).
    pub(super) fn vd1e(&mut self) {
        let bx = self.r.bx;
        self.r.bx = 2;
        self.locate_and_print();
        self.r.bx = bx;
        self.loc_124();
    }

    /// sub_31 / loc_124: draw a closet (open or shut) and mark its cell.
    pub(super) fn loc_124(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.bx = bl as u16;
        let al = self.b(V4DC);
        self.r.set_al(al);
        if self.b(ROOM_INDEX) == al {
            self.setb(V597 + si, 0);
            return;
        }
        self.r.ax = si;
        let bx = self.r.bx;
        self.setb(V54F + bx, si as u8);
        self.setb(V557 + bx, si as u8);
        let al = self.b(V59B + si);
        self.r.set_al(al);
        self.r.si = if al != 0 { 2 } else { 0 };
        self.get_screen_pos();
        self.r.dx = self.r.dx.wrapping_add(2);
        let dx = self.r.dx;
        self.setw(XPOS, dx);
        self.put_gfx();
    }

    /// drawScreen (sub_32): walls and doors of cell BX.
    pub(super) fn draw_screen(&mut self) {
        self.get_screen_pos();
        let bx = self.r.bx;
        let cl = self.b(bx + V597);
        self.r.set_cl(cl);
        self.setb(V964, cl);
        if self.b(V964) & 4 != 0 {
            let dx = self.r.dx;
            self.r.si = 4;
            self.put_gfx();
            self.r.dx = dx;
        }
        if self.b(V964) & 8 != 0 {
            let (di, dx) = (self.r.di, self.r.dx);
            self.r.di = self.r.di.wrapping_add(0x280);
            self.r.si = 4;
            self.put_gfx();
            self.r.dx = dx;
            self.r.di = di;
        }
        if self.b(V964) & 1 != 0 {
            let (di, dx) = (self.r.di, self.r.dx);
            self.r.si = 6;
            self.put_gfx();
            self.r.dx = dx;
            self.r.di = di;
        }
        if self.b(V964) & 2 != 0 {
            let (di, dx, xp) = (self.r.di, self.r.dx, self.w(XPOS));
            self.r.dx = self.r.dx.wrapping_add(8);
            let d = self.r.dx;
            self.setw(XPOS, d);
            self.r.si = 6;
            self.put_gfx();
            self.setw(XPOS, xp);
            self.r.dx = dx;
            self.r.di = di;
        }
        self.r.bx = bx;
        if self.b(V964) & 0x10 == 0 {
            return;
        }
        // the cell with the room's exit door
        self.setb(V4D5, bx as u8);
        if self.w(V4EE) == 0 {
            self.setb(V4D6, 0);
            self.setb(V962, 1);
            self.setb(V961, 7);
            self.r.bx = bx.wrapping_add(0x10);
            let bl = self.r.bl();
            self.setb(V960, bl);
            self.sub_35();
            self.r.bx = self.r.bx.wrapping_add(1);
            self.sub_35();
        }
        self.sub_36();
        self.r.set_al(0x55);
        self.draw2screen();
        self.sub_33();
    }

    /// sub_33: the door's open-ness bars.
    pub(super) fn sub_33(&mut self) {
        let bl = self.b(V4D5);
        self.r.bx = (bl as u16).wrapping_add(0x10);
        self.get_screen_pos();
        self.r.dx = self.r.dx.wrapping_add(2);
        let bl = self.b(V961);
        self.r.set_bl(bl);
        if bl == 0 {
            return;
        }
        loop {
            self.r.set_al(0x76);
            let bx = self.r.bx;
            self.draw2screen();
            self.r.bx = bx.wrapping_sub(1);
            if self.r.bx == 0 {
                break;
            }
        }
    }

    /// someProc3 (type 30h objects, drawn and updated each turn).
    pub(super) fn some_proc3(&mut self) {
        let si = self.r.si;
        let al = self.b(V4DC);
        self.r.set_al(al);
        if al != self.b(ROOM_INDEX) {
            self.sub_whatever();
            self.r.set_al(0x7E);
            if self.b(V4F3) != 0 {
                self.r.ax = self.r.ax.wrapping_add(1);
                if self.b(V599 + si) & 1 != 0 {
                    self.r.set_al(0x20);
                }
            }
            self.draw2screen();
            let bl = self.b(V59A + si);
            self.r.bx = bl as u16;
            let mut skip = false;
            if bl != 0 {
                let al = self.b(V598 + self.r.bx).wrapping_sub(self.b(V598 + si));
                self.r.set_al(al);
                if al == 0 || al == 0xF8 {
                    skip = true;
                } else {
                    self.setb(V59A + si, 0);
                }
            }
            if !skip {
                // loc_88
                self.r.ax = si;
                let bl = self.b(V598 + si);
                self.r.bx = bl as u16;
                {
                    let i = V54F + self.r.bx;
                    self.setb(i, si as u8);
                }
            }
        }
        // loc_89
        self.incb(V599 + si);
    }

    fn sub_whatever(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.bx = bl as u16;
        self.get_screen_pos();
        self.r.dx = self.r.dx.wrapping_add(2);
        if self.b(V598 + si) & 4 != 0 {
            self.r.dx = self.r.dx.wrapping_add(4);
        }
    }

    /// screenGarbage: flash ROM noise over the screen (hit a wall / explosion).
    pub(super) fn screen_garbage(&mut self) {
        let mut di: u16 = 0x2000;
        for _ in 0..0x1000 {
            let ax = self.rom[di as usize] as u16 | ((self.rom[di as usize + 1] as u16) << 8);
            let v = self.st.vw(di) ^ ax;
            self.st.set_vw(di, v);
            self.r.ax = ax;
            di += 2;
        }
        self.r.di = di;
        self.r.cx = 0;
        self.host.present(&self.st);
        self.r.set_al(0xFF);
        self.crash_sound();
        if self.b(CPU_TYPE) == 0 {
            self.host.tick_wait(&self.st);
        }
        self.host.present(&self.st);
    }
}
