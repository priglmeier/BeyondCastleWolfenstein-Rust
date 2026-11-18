//! Guards: drawing, the per-turn AI (someProc), path finding (sub_129),
//! line-of-sight (colDetect), talking to / bribing guards, shooting, and the
//! SS men arriving when the alarm rings. Object slots are 16 bytes each at
//! v597+80h..v597+F0h: +0 type, +1 cell, +2 direction, +3 target, ...

use super::*;

impl<H: Host> Game<H> {
    pub(super) fn clear_health(&mut self) {
        self.setb(V4D4, 0);
        self.setb(V4FF, 0);
        self.setb(HEALTH + 1, 0);
        self.set_health();
    }

    pub(super) fn set_health(&mut self) {
        let al = self.b(V4D4);
        self.setb(V4D3, al);
        let al = self.b(V4FF);
        self.r.set_al(al);
        self.setb(HEALTH, al);
    }

    /// sub_141: queue a sound / message event.
    pub(super) fn sub_141(&mut self) {
        let al = self.r.al();
        let bx = self.w(V4787);
        self.setb(V4764 + bx, al);
        self.setw(V4787, (bx + 1) & 0x1F);
    }

    /// sub_141 with AL
    pub(super) fn queue(&mut self, al: u8) {
        self.r.set_al(al);
        self.sub_141();
    }

    /// sub_153: AL = voice-delay selector for object SI (83h..86h).
    pub(super) fn sub_153(&mut self) {
        let si = self.r.si;
        let al = (((si as u8) >> 4) & 3).wrapping_add(3) | 0x80;
        self.r.ax = (si & 0xFF00) | al as u16;
    }

    /// sub_104: every guard forgets it was alerted.
    pub(super) fn sub_104(&mut self) {
        let mut si: u16 = 0x80;
        loop {
            let al = self.b(V597 + si);
            self.r.set_al(al);
            if al == 0x10 || al == 0x50 {
                let v = (self.b(V59D + si) & 0x7F) | 1;
                self.setb(V59D + si, v);
                self.setb(V5A3 + si, 0);
            }
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
    }

    pub(super) fn sub_130(&mut self) {
        let mut si: u16 = 0x80;
        loop {
            let al = self.b(V597 + si);
            self.r.set_al(al);
            if al == 0x10 || al == 0x50 {
                self.setb(V59D + si, 0x80);
            }
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
    }

    /// sub_120: raise the alarm; every guard is alerted.
    pub(super) fn sub_120(&mut self) {
        let si0 = self.r.si;
        self.setb(V4F3, 1);
        self.setb(V59D + si0, 1);
        self.setb(V503, 0);
        self.setb(V18D1, 175);
        let mut si: u16 = 0x80;
        loop {
            let al = self.b(V597 + si);
            self.r.set_al(al);
            if al == 0x10 {
                self.setb(V59D + si, 1);
            }
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
    }

    // ------------------------------------------------------------------
    // drawing a guard

    /// v3a84: guard drawn by refreshScreen.
    pub(super) fn v3a84(&mut self) {
        let si = self.r.si;
        if self.b(V4EE) == 0 {
            self.setb(V5A1 + si, 0);
            self.setb(V59A + si, 0);
            if self.b(V5A3 + si) == 0 {
                self.r.set_al(0);
                self.sub_129();
            }
            let al = self.b(ROOM_INDEX);
            self.r.set_al(al);
            if al == self.b(V4DC) && self.b(V4D1) == 0 {
                self.loc_571();
                return;
            }
        }
        // loc_449
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        self.get_screen_pos();
        let al = self.b(V4E5).wrapping_sub(bl);
        self.r.set_al(al);
        if al == 0 || al == 8 || al == 0xF8 {
            self.setb(V597 + si, 0);
            return;
        }
        self.loc_451();
    }

    /// sub_110 / loc_451
    pub(super) fn loc_451(&mut self) {
        let si = self.r.si;
        self.r.set_bh(0);
        self.r.ax = si;
        let bx = self.r.bx;
        self.setb(V54F + bx, si as u8);
        self.setb(V557 + bx, si as u8);
        let bl = self.b(V599 + si);
        self.r.set_bl(bl);
        let al = self.b(V70B + self.r.bx);
        self.r.set_al(al);
        self.setb(V10AB, al);
        self.loc_452();
    }

    fn loc_452(&mut self) {
        let si = self.r.si;
        self.r.dx = self.r.dx.wrapping_add(2);
        self.r.di = self.r.di.wrapping_add(0x140);
        let dx = self.r.dx;
        self.setw(XPOS, dx);
        let bl = self.b(V599 + si);
        self.r.set_bl(bl);
        let al = if bl == 2 { 0x75 } else { 0x77 };
        self.setb(V10A7, al);
        self.r.set_bh(0);
        let mut al = self.b(V44E9 + self.r.bx);
        if self.b(ROOM_INDEX) == 0x3C && si == 0x80 {
            self.r.set_bl(0);
            al = 0x4B; // the Fuehrer himself
        }
        self.setb(V10A3, al);
        self.r.set_bh(0);
        if self.b(ROOM_INDEX) != 0x3C {
            let bl = self.b(V59A + si);
            self.r.set_bl(bl);
        }
        let bx = self.r.bx;
        let mut al = self.b(V6DF + bx);
        let mut ah = 0x20u8;
        if self.b(V3808) != 0 {
            self.decb(V3808);
            al = ah;
            ah = 0x64;
            self.setb(V10A4, al);
        }
        self.setb(V10A2, ah);
        self.setb(V10A6, al);
        let al = self.b(V6EA + bx);
        self.setb(V10A8, al);
        let ax = (self.r.di << 1).wrapping_add(self.r.dx);
        self.r.ax = ax;
        self.setw(V5A4 + si, ax);
        if self.b(V599 + si) != 0 {
            self.setb(SPEAKER_DELAY, 2);
            self.setw(CLICK_SOUND, 0x400);
            self.footstep();
        }
        self.r.si = 0x1E;
        self.put_gfx();
        self.r.si = si;
    }

    /// sub_135: erase a guard's sprite.
    pub(super) fn sub_135(&mut self) {
        let si = self.r.si;
        let mut di = self.w(V5A4 + si);
        self.r.dx = di & 0x7F;
        di >>= 1;
        di &= 0xFFC0;
        self.r.di = di;
        self.r.cx = 0x303;
        self.put_sprite();
    }

    // ------------------------------------------------------------------
    // the guard's turn

    /// someProc: per-turn update of a guard (type 10h).
    pub(super) fn some_proc(&mut self) {
        self.sub_135();
        let si = self.r.si;
        if self.b(V5A3 + si) != 0 {
            self.sub_124();
            self.loc_471();
            return;
        }
        let v = self.incb(V5A1 + si);
        if v & 1 == 0 {
            self.loc_472();
            return;
        }
        // loc_459: look around
        self.sub_115();
        self.sub_119();
        let al = self.b(V3802);
        self.r.set_al(al);
        let mut to_461 = false;
        if al != 0 {
            self.xlat_dir();
            if self.r.al() != self.b(V599 + si) {
                to_461 = true;
            }
        }
        if !to_461 {
            // loc_460
            self.sub_111();
            if !self.zf {
                self.sub_119();
                if self.cf {
                    self.loc_466();
                    return;
                }
                self.loc_462();
                return;
            }
        }
        // loc_461
        let al = self.r.di as u8;
        self.r.ax = self.r.di;
        self.setb(V5A6 + si, al);
        let v = self.b(V59D + si) & !2;
        self.setb(V59D + si, v);
        self.loc_477();
    }

    fn loc_462(&mut self) {
        let si = self.r.si;
        if self.b(V4F3) == 0 {
            let al = self.b(V59D + si);
            if al & 1 != 0 && al & 2 == 0 {
                self.setb(V3806, 0);
                let mut al = 0x11u8;
                self.setb(V381B, al);
                loop {
                    // loc_464
                    al &= 0x0F;
                    self.r.set_al(al);
                    let bl = self.b(V598 + si);
                    self.r.set_bl(bl);
                    self.col_detect();
                    let a = self.b(V3806);
                    self.r.set_al(a);
                    if a != 0 {
                        let a = a.wrapping_sub(8);
                        self.setb(V59B + 1 + si, a);
                        let a = self.b(V598 + si);
                        self.setb(V59B + si, a);
                        let a = self.b(V3807);
                        self.r.set_al(a);
                        self.setb(V599 + si, a);
                        let v = self.b(V59D + si) | 2;
                        self.setb(V59D + si, v);
                        self.loc_466();
                        return;
                    }
                    // loc_463
                    let v = self.b(V381B);
                    self.setb(V381B, v << 1);
                    if v & 0x80 != 0 {
                        break;
                    }
                    al = v << 1;
                    self.r.set_al(al);
                }
            }
        }
        self.loc_465();
    }

    fn loc_465(&mut self) {
        let si = self.r.si;
        let al = self.b(V598 + si);
        self.r.set_al(al);
        if al == self.b(V59B + 1 + si) || al == self.b(V59B + si) {
            self.loc_467();
            return;
        }
        self.loc_466();
    }

    fn loc_466(&mut self) {
        self.sub_127();
        if !self.cf {
            self.loc_471();
            return;
        }
        self.sub_118();
        if !self.cf {
            self.loc_471();
            return;
        }
        let si = self.r.si;
        let v = self.b(V59D + si) & !2;
        self.setb(V59D + si, v);
        self.loc_467();
    }

    fn loc_467(&mut self) {
        let si = self.r.si;
        let al = self.b(V59D + si);
        if al & 1 == 0 {
            // loc_470
            self.r.set_al(0);
            self.sub_129();
            self.loc_471();
            return;
        }
        if al & 2 != 0 {
            self.sub_120();
        }
        // loc_468
        self.r.set_al(0xFF);
        self.sub_129();
        self.sub_111();
        let zf = self.zf;
        self.r.cx = self.r.di;
        if !zf {
            self.r.set_cl(0);
            self.get_rand();
            if self.sf {
                let cl = self.b(V599 + si);
                self.r.set_cl(cl);
            }
        }
        let cl = self.r.cl();
        self.setb(V5A6 + si, cl);
        self.setb(V59A + si, cl);
        if zf {
            self.shooting_guard();
        }
        self.loc_471();
    }

    fn loc_471(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.r.set_bh(0);
        let bl = self.b(V599 + si);
        self.r.set_bl(bl);
        let al = self.b(V6AF + self.r.bx) << 2;
        self.r.set_al(al);
        let dl = self.r.dl().wrapping_add(al);
        self.r.set_dl(dl);
        self.r.bx <<= 1;
        self.r.di = self.r.di.wrapping_add(self.w(V6C7 + self.r.bx));
        self.r.bx >>= 1;
        let al = self.b(V716 + self.r.bx);
        self.r.set_al(al);
        self.setb(V10AB, al);
        self.loc_452();
    }

    fn loc_472(&mut self) {
        let si = self.r.si;
        self.sub_127();
        let mut to_475 = !self.cf;
        if !to_475 {
            let al = self.r.al();
            let to_474;
            if al == 0xFF {
                let v = self.b(V59D + si);
                self.r.set_al(v);
                if v & 0x80 != 0 {
                    to_474 = true;
                } else {
                    self.r.set_al(v >> 1);
                    if v & 1 == 0 {
                        self.loc_477();
                        return;
                    }
                    self.caught();
                }
            } else {
                // loc_473
                self.sub_118();
                if !self.cf {
                    to_475 = true;
                    to_474 = false;
                } else {
                    to_474 = true;
                }
            }
            if to_474 && !to_475 {
                self.sub_114();
            }
        }
        if to_475 {
            // loc_475: step forward
            let bx = self.r.bx;
            let bl = self.b(V598 + si);
            self.setb(V54F + bl as u16, 0);
            self.setb(V557 + bl as u16, 0);
            self.r.bx = bx;
            self.setb(V598 + si, bx as u8);
        }
        // loc_476
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.loc_451();
    }

    fn loc_477(&mut self) {
        let si = self.r.si;
        let mut to_479 = self.b(V59D + si) & 1 != 0;
        if !to_479 {
            let mut to_478 = self.b(V4F8) != 0 || self.b(DOOR_OPEN_ATTEMPTS) != 0;
            if !to_478 {
                let al = self.b(GUN_DIR);
                self.r.set_al(al);
                if al == 0 {
                    // loc_483: you put your gun away in front of a guard
                    if self.b(V59D + si) & 0x80 != 0 {
                        self.loc_465();
                        return;
                    }
                    // loc_484
                    self.setb(V599 + si, 0);
                    self.setb(V381E, 0);
                    self.setb(V380C, 0);
                    self.setb(V381D, 0);
                    self.setb(V5A3 + si, 0x80);
                    let al = self.b(V598 + si);
                    self.setb(V59B + 1 + si, al);
                    self.sub_153();
                    self.sub_141();
                    self.queue(1);
                    self.sub_130();
                    self.loc_471();
                    return;
                }
                if self.b(V4D9) != 0 {
                    to_478 = true;
                } else {
                    self.xlat_dir();
                    if self.r.al() == self.b(V5A6 + si) {
                        to_478 = true;
                    } else if self.b(V59D + si) & 0x80 != 0 {
                        self.loc_466();
                        return;
                    } else {
                        to_478 = true;
                    }
                }
            }
            if to_478 {
                self.sub_153();
                self.sub_141();
                self.queue(5);
                self.sub_104();
                to_479 = true;
            }
        }
        if to_479 {
            if self.b(V59E + si) == 0 {
                self.setb(V59A + si, 0);
            } else {
                // loc_480
                let al = self.b(V3803).wrapping_add(self.b(V4E5));
                self.setb(V59B + 1 + si, al);
                let al = self.b(V5A6 + si);
                self.r.set_al(al);
                self.setb(V599 + si, al);
                if al == self.b(V59A + si) {
                    self.shooting_guard();
                } else {
                    self.setb(V59A + si, al);
                }
            }
        }
        self.loc_466();
    }

    // ------------------------------------------------------------------
    // looking and moving

    /// sub_111: can the guard see the player? ZF set = yes (DI = direction).
    pub(super) fn sub_111(&mut self) {
        let si = self.r.si;
        self.setb(V3804, 0);
        let al = self.b(V599 + si);
        self.r.set_al(al);
        self.sub_113();
        if self.zf {
            return;
        }
        let mut bx = V4494;
        self.get_rand();
        if self.sf {
            bx = V44A0;
        }
        self.r.bx = bx;
        let al = self.b(V599 + si);
        let al = self.b(bx.wrapping_add(al as u16));
        self.r.set_al(al);
        self.sub_113();
        if self.zf {
            return;
        }
        if self.b(V59D + si) & 1 == 0 {
            self.r.set_al(1);
            self.zf = false;
            self.sf = false;
            return;
        }
        let al = self.b(V599 + si);
        self.r.set_al(al);
        self.xlat_dir();
        self.sub_113();
    }

    fn sub_113(&mut self) {
        let bl = self.b(V598 + self.r.si);
        self.r.set_bl(bl);
        self.sub_122();
        self.zf = self.r.al() == 0x40;
    }

    /// colDetect? (sub_121): look from cell BL in direction AL. Returns
    /// AL = 40h if the player is in sight, an object number, or 0 (wall).
    pub(super) fn col_detect(&mut self) {
        let al = self.r.al();
        if al != 2 {
            let bl = self.r.bl().wrapping_add(8);
            self.r.set_bl(bl);
        }
        self.loc_501();
    }

    /// sub_122: as colDetect but the +8 row offset applies to direction 1.
    pub(super) fn sub_122(&mut self) {
        let al = self.r.al();
        if al == 1 {
            let bl = self.r.bl().wrapping_add(8);
            self.r.set_bl(bl);
        }
        self.loc_501();
    }

    fn loc_501(&mut self) {
        let al = self.r.al();
        self.setb(V3801, al);
        let bl = self.r.bl();
        self.setb(V3800, bl);
        loop {
            // loc_502
            let al = self.b(V3801);
            let bl = self.b(V3800);
            self.r.ax = al as u16;
            self.r.set_bl(bl);
            self.r.di = self.r.ax;
            self.sub_123();
            if self.cf {
                self.r.set_al(0);
                self.zf = true;
                self.sf = false;
                return;
            }
            // loc_503
            let bl = self.r.bl();
            self.setb(V3800, bl);
            let d = bl.wrapping_sub(self.b(V4E5));
            self.r.set_bl(d);
            if d == 0 || d == 8 {
                self.setb(V3803, d);
                let bl = self.b(V3800);
                self.r.set_bl(bl);
                let al = self.b(V3801);
                self.r.ax = al as u16;
                self.r.di = self.r.ax;
                self.r.set_al(0x40);
                self.zf = false;
                self.sf = false;
                return;
            }
            // loc_505
            let bl = self.b(V3800);
            self.r.bx = bl as u16;
            let al = self.b(V54F + self.r.bx);
            self.r.set_al(al);
            if al == 0 {
                continue;
            }
            self.r.set_bl(al);
            let cl = self.b(V3800);
            let ch = self.b(V3801);
            self.r.set_cl(cl);
            self.r.set_ch(ch);
            let t = self.b(V597 + self.r.bx);
            self.r.set_al(t);
            if t == 0x30 {
                self.setb(V3806, cl);
                self.setb(V3807, ch);
                continue;
            }
            if t == 0x40 {
                self.setb(V3804, cl);
                self.setb(V3805, ch);
                continue;
            }
            // loc_508
            let bl = self.b(V3800);
            self.r.bx = bl as u16;
            let al = self.b(V54F + self.r.bx);
            self.r.ax = al as u16;
            self.zf = al == 0;
            self.sf = al & 0x80 != 0;
            return;
        }
    }

    /// sub_123: step from BL in direction DI. CF set if a wall is in the way.
    pub(super) fn sub_123(&mut self) {
        self.r.set_bh(0);
        let di = self.r.di;
        if self.b(V597 + self.r.bx) & self.b(V697 + di) != 0 {
            self.cf = true;
            return;
        }
        self.r.ax = di;
        self.sub_24();
        if self.r.dl() >= 8 || self.r.dh() >= 9 {
            self.cf = true;
            return;
        }
        let al = self.b(V597 + self.r.bx) & self.b(V6A3 + di);
        self.r.set_al(al);
        self.cf = al != 0;
    }

    fn sub_114(&mut self) {
        let si = self.r.si;
        let al = self.b(V599 + si);
        self.r.set_al(al);
        self.xlat_dir();
        let al = self.r.al();
        self.setb(V599 + si, al);
        let ax = self.w(V59B + si).swap_bytes();
        self.r.ax = ax;
        self.setw(V59B + si, ax);
    }

    /// sub_115: is the player next to the guard? CF clear = yes.
    pub(super) fn sub_115(&mut self) {
        let si = self.r.si;
        self.setb(V3804, 0);
        self.setw(V3855, 0xFFFF);
        loop {
            let n = self.w(V3855).wrapping_add(1);
            self.setw(V3855, n);
            self.r.bx = n;
            let al = self.b(V448B + n);
            self.r.set_al(al);
            self.setb(V3802, al);
            if al == 0 {
                self.cf = self.b(V3804) == 0;
                return;
            }
            self.r.ax = al as u16;
            self.r.di = self.r.ax;
            let mut bl = self.b(V598 + si);
            if self.r.di == 1 {
                bl = bl.wrapping_add(8);
            }
            self.r.set_bl(bl);
            self.sub_123();
            if self.cf {
                continue;
            }
            let bx = self.r.bx;
            self.r.set_bh(0);
            let bl = self.b(V54F + self.r.bx);
            self.r.set_bl(bl);
            let al = self.b(V597 + self.r.bx);
            self.r.set_al(al);
            if al == 0x40 {
                let al = self.b(V598 + self.r.bx);
                self.r.set_al(al);
                self.setb(V3804, al);
            }
            self.r.bx = bx;
            if self.b(V3802) == 2 {
                let bl = self.r.bl().wrapping_sub(8);
                self.r.set_bl(bl);
            }
            if self.r.bl() != self.b(V4E5) {
                continue;
            }
            self.cf = false;
            return;
        }
    }

    /// sub_118: AL = type of object DI in the way. CF clear if passable.
    pub(super) fn sub_118(&mut self) {
        let al = self.r.al();
        let si = self.r.si;
        if al == 0x40 {
            if self.b(V59D + si) & 1 == 0 {
                self.sub_104();
            }
        } else if al != 0x30 {
            self.cf = true;
            return;
        }
        self.r.ax = si;
        let di = self.r.di;
        self.setb(V59A + di, si as u8);
        self.cf = false;
    }

    /// sub_119: the guard noticed a body.
    fn sub_119(&mut self) {
        let si = self.r.si;
        let bl = self.b(V3804);
        self.r.set_bl(bl);
        if bl == 0 || self.b(V59D + si) & 1 != 0 {
            self.cf = false;
            return;
        }
        self.setb(V59B + 1 + si, bl);
        let al = self.b(V3805);
        self.setb(V599 + si, al);
        self.sub_104();
        self.queue(5);
        self.cf = true;
    }

    /// sub_124: a guard that has stopped you ("Your pass !").
    pub(super) fn sub_124(&mut self) {
        let si = self.r.si;
        if self.b(V5A3 + si) & 0x80 != 0 {
            // loc_510
            self.sub_115();
            let mut to_511 = self.cf;
            if !to_511 {
                let bl = self.b(V598 + si);
                self.r.set_bl(bl);
                let al = self.b(V3802);
                self.r.ax = al as u16;
                self.r.di = self.r.ax;
                self.sub_123();
                to_511 = self.cf;
                if !to_511 {
                    let al = self.b(V3802);
                    self.setb(V5A6 + si, al);
                    self.setb(V5A3 + si, 1);
                    let bl = self.b(SKILL_LEVEL);
                    self.r.bx = bl as u16;
                    let al = self.b(V385C + self.r.bx);
                    self.setb(V5A2 + si, al);
                    self.setb(V4C6, 0xFF);
                    self.setb(V380B, 0);
                    self.setw(V3809, si);
                    self.queue(4);
                    self.incb(V5A1 + si);
                    return;
                }
            }
            // loc_511
            if self.b(GUN_DIR) != 0 {
                self.loc_513();
                return;
            }
            if self.b(V381D) == 0 {
                self.incb(V381D);
                self.queue(3);
                self.sub_126();
                let bl = self.b(SKILL_LEVEL);
                self.r.bx = bl as u16;
                let al = self.r.al().wrapping_add(self.b(V3857 + self.r.bx));
                self.r.set_al(al);
                self.setb(V5A2 + si, al);
            }
            if self.decb(V5A2 + si) == 0 {
                self.loc_513();
                return;
            }
            self.incb(V5A1 + si);
            return;
        }
        // loc_516
        self.sub_115();
        if self.cf || self.b(GUN_DIR) != 0 {
            self.loc_513();
            return;
        }
        if self.b(V381E) == 0 {
            if self.decb(V5A2 + si) == 0 {
                self.loc_518();
                return;
            }
            self.incb(V5A1 + si);
            return;
        }
        // loc_517: you showed a pass
        let bl = self.b(V4EB);
        self.r.bx = (bl as u16).wrapping_sub(1);
        let al = self.b(V381E) & self.b(KEYZ + self.r.bx);
        self.r.set_al(al);
        if al != 0 {
            // loc_514: the right pass
            self.queue(2);
            self.setb(V59D + si, 0x80);
            self.setb(V5A3 + si, 0);
            self.setb(V4C6, 0);
            self.setb(V59A + si, 0);
            self.setb(V3808, 1);
            self.r.set_al(0);
            self.sub_129();
            self.incb(V5A1 + si);
            return;
        }
        let pass = self.b(V381E);
        let v = self.b(V5A0 + si) | pass;
        self.setb(V5A0 + si, v);
        let p = self.b(PASSES) ^ pass;
        self.setb(PASSES, p);
        self.setb(V381E, 0);
        self.r.set_al(0);
        self.print_passes();
        self.r.si = si;
        self.loc_518();
    }

    fn loc_513(&mut self) {
        self.sub_104();
        self.setb(V4C6, 0);
        self.queue(5);
        self.r.set_al(0xFF);
        self.sub_129();
        let si = self.r.si;
        self.incb(V5A1 + si);
    }

    fn loc_518(&mut self) {
        let si = self.r.si;
        if self.b(V380C) != 0 {
            self.loc_513();
            return;
        }
        self.incb(V380C);
        let bl = self.b(SKILL_LEVEL);
        self.r.bx = bl as u16;
        let al = self.b(V3861 + self.r.bx);
        self.setb(V5A2 + si, al);
        let r = self.get_rand();
        if r >= self.b(V3867 + self.r.bx) {
            let al = self.b(V5A6 + si);
            self.r.set_al(al);
            self.setb(V59A + si, al);
        }
        self.queue(4);
        self.incb(V5A1 + si);
    }

    /// bribeGuard (sub_125): BL = 0..4 show pass number, 5 = money.
    pub(super) fn bribe_guard(&mut self) {
        let si = self.w(V3809);
        self.r.si = si;
        let bl = self.r.bl();
        let al: u8;
        if bl == 5 {
            if self.b(MARKS) == 0 {
                self.setb(V381E, 0x80);
                self.setb(V380C, 0x80);
            } else {
                let mut a = self.get_rand();
                if a == 0 {
                    a = 19;
                }
                loop {
                    let bl = self.b(SKILL_LEVEL);
                    self.r.bx = bl as u16;
                    if a < self.b(V386C + self.r.bx) {
                        break;
                    }
                    a >>= 1;
                    if a == 0 {
                        a = 19;
                    }
                }
                let ah = a;
                let m = self.b(MARKS);
                if m < ah {
                    self.setb(MARKS, 0);
                    self.setb(V381E, 0x80);
                    self.setb(V380C, 0x80);
                } else {
                    self.setb(MARKS, m - ah);
                    let mut a2 = self.b(V59F + si).wrapping_add(ah);
                    if a2 >= 0x65 {
                        a2 = ah;
                    }
                    self.setb(V59F + si, a2);
                    let bl = self.b(V4EB);
                    self.r.bx = (bl as u16).wrapping_sub(1);
                    let k = self.b(KEYZ + self.r.bx);
                    self.setb(V381E, k);
                    self.setb(V380B, k);
                }
                self.r.set_ah(ah);
            }
            al = 0x24; // '$'
        } else {
            let a = self.b(PASSES) & self.b(V381F + self.r.bx);
            self.r.set_al(a);
            if a == 0 {
                return;
            }
            self.setb(V381E, a);
            al = bl.wrapping_add(0x14);
        }
        // loc_529
        self.r.set_al(al);
        let ax = self.r.ax;
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.get_screen_pos();
        self.r.di = self.r.di.wrapping_add(0x280);
        self.r.dx = self.r.dx.wrapping_add(4);
        self.r.ax = ax;
        self.draw2screen();
        self.r.set_al(0x13);
        self.draw2screen();
    }

    /// loc_531: actionBranch for a guard when leaving the room.
    pub(super) fn loc_531(&mut self) {
        let si = self.r.si;
        if self.b(V59D + si) & 1 != 0 {
            self.sub_126();
            if self.r.al() < 5 {
                self.sub_153();
                self.sub_141();
                self.queue(3);
            }
        }
        self.setb(V5A3 + si, 0);
        self.setb(V59D + si, 0);
    }

    /// sub_126: AL = rough distance from guard SI to the player.
    pub(super) fn sub_126(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        self.decode_bcd();
        let g = self.r.dx;
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.decode_bcd();
        self.r.ax = g;
        let mut dl = self.r.dl().wrapping_sub(g as u8);
        if dl & 0x80 != 0 {
            dl ^= 0xFF;
        }
        let mut dh = self.r.dh().wrapping_sub((g >> 8) as u8);
        if dh & 0x80 != 0 {
            dh ^= 0xFF;
        }
        self.r.set_dl(dl);
        self.r.set_dh(dh);
        self.r.set_al(dl.wrapping_add(dh));
    }

    /// sub_127: can guard SI step in its direction? CF clear = free (BL =
    /// new cell). CF set with AL = 0 (wall), FFh (the player) or the type
    /// of the object in the way (DI = object).
    pub(super) fn sub_127(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.bx = bl as u16;
        let al = self.b(V599 + si);
        self.r.ax = al as u16;
        self.r.di = self.r.ax;
        let fail = |g: &mut Self| {
            g.r.set_al(0);
            g.cf = true;
        };
        if al == 0 {
            fail(self);
            return;
        }
        let (bx, di) = (self.r.bx, self.r.di);
        if self.b(V597 + bx) & self.b(V766 + di) != 0 || self.b(V59F + bx) & self.b(V75A + di) != 0
        {
            fail(self);
            return;
        }
        let al = self.b(V599 + si);
        self.r.set_al(al);
        self.sub_24();
        if self.r.dl() >= 8 || self.r.dh() >= 8 {
            fail(self);
            return;
        }
        let al = self.b(V599 + si);
        self.r.ax = al as u16;
        self.r.di = self.r.ax;
        self.r.set_bh(0);
        let (bx, di) = (self.r.bx, self.r.di);
        if self.b(V597 + bx) & self.b(V742 + di) != 0 || self.b(V59F + bx) & self.b(V74E + di) != 0
        {
            fail(self);
            return;
        }
        let d = self.r.bl().wrapping_sub(self.b(V4E5));
        if d == 0 || d == 8 || d == 0xF8 {
            self.r.set_al(0xFF);
            self.cf = true;
            return;
        }
        self.r.set_ah(0);
        let al = self.b(V54F + bx);
        self.r.set_al(al);
        if al != 0 && self.r.ax != si {
            self.loc_537();
            return;
        }
        let al = self.b(V557 + bx);
        self.r.set_al(al);
        if al == 0 || self.r.ax == si {
            self.cf = false;
            return;
        }
        self.loc_537();
    }

    fn loc_537(&mut self) {
        self.r.di = self.r.ax;
        let al = self.b(V597 + self.r.di);
        self.r.set_al(al);
        self.cf = true;
    }

    /// sub_129: path finding. AL = 0 (wander) or FFh (hunt the player).
    /// Tries walking straight in every direction and picks a route.
    pub(super) fn sub_129(&mut self) {
        let si = self.r.si;
        let al = self.r.al();
        self.setb(V3810, al);
        for v in [V3811, V3812, V3815, V3814, V3817, V3819, V381A] {
            self.setb(v, 0);
        }
        let al = self.b(V598 + si);
        for v in [V380F, V3813, V3816, V3818] {
            self.setb(v, al);
        }
        let r = self.get_rand() & 7;
        self.r.ax = r as u16;
        self.r.bx = self.r.ax;
        let mut al = self.b(V448B + self.r.bx);
        self.setb(V380E, al);
        loop {
            // loc_543
            self.r.set_al(al);
            self.setb(V380D, al);
            self.setb(V599 + si, al);
            let a = self.b(V380F);
            self.setb(V598 + si, a);
            // loc_544
            loop {
                self.sub_127();
                if !self.cf {
                    self.loc_545();
                    continue;
                }
                // loc_547
                let a = self.r.al();
                if a == 0xFF {
                    if self.b(V59D + si) & 0x80 == 0 {
                        let bl = self.r.bl();
                        self.setb(V59B + 1 + si, bl);
                        self.loc_548();
                        return;
                    }
                } else if a == 0x40 {
                    if self.b(V59D + si) & 1 != 0 {
                        self.loc_545();
                        continue;
                    }
                    let bl = self.r.bl();
                    self.setb(V59B + 1 + si, bl);
                    self.sub_104();
                    self.queue(5);
                    self.loc_548();
                    return;
                } else if a == 0x30 {
                    self.loc_545();
                    continue;
                }
                // loc_551
                let a12 = self.b(V3812);
                let a11 = self.b(V3811);
                if a12 < a11 {
                    self.setb(V3812, a11);
                    self.setb(V3811, 0);
                    let a = self.b(V598 + si);
                    self.setb(V3813, a);
                    let cl = self.b(V599 + si);
                    self.r.set_cl(cl);
                    self.setb(V3814, cl);
                    if self.b(V381A) == 0 {
                        self.setb(V3818, a);
                        self.setb(V3819, cl);
                        self.incb(V381A);
                    }
                }
                break;
            }
            // loc_542
            let bl = self.b(V380D);
            self.r.bx = bl as u16;
            al = self.b(V4494 + self.r.bx);
            self.r.set_al(al);
            if al == self.b(V380E) {
                break;
            }
        }
        // loc_553
        if self.b(V3810) != 0 {
            let a = self.b(V3817);
            if a != 0 {
                self.setb(V599 + si, a);
                let a = self.b(V3816);
                self.setb(V59B + 1 + si, a);
            } else {
                let a = self.b(V3818);
                self.setb(V59B + 1 + si, a);
                let a = self.b(V3819);
                self.setb(V599 + si, a);
            }
            self.loc_548();
            return;
        }
        // loc_556
        let a = self.b(V3813);
        self.setb(V59B + 1 + si, a);
        let start = self.b(V380F);
        self.setb(V598 + si, start);
        let dir = self.b(V3814);
        self.r.set_al(dir);
        self.xlat_dir();
        let a = self.r.al();
        self.setb(V599 + si, a);
        loop {
            self.sub_127();
            if self.cf {
                break;
            }
            let bl = self.r.bl();
            self.setb(V598 + si, bl);
        }
        let a = self.b(V598 + si);
        self.setb(V59B + si, a);
        self.setb(V599 + si, dir);
        self.r.ax = (self.r.ax & 0xFF00) | start as u16;
        self.setb(V598 + si, start);
    }

    fn loc_545(&mut self) {
        let si = self.r.si;
        let bl = self.r.bl();
        self.setb(V598 + si, bl);
        self.r.set_bh(0);
        let bx = self.r.bx;
        if self.b(V3810) != 0 {
            let a = self.b(V3815);
            if a < self.b(V505 + bx) {
                let a = self.b(V505 + bx);
                self.setb(V3815, a);
                let a = self.b(V598 + si);
                self.setb(V3816, a);
                let a = self.b(V599 + si);
                self.setb(V3817, a);
            }
        }
        self.incb(V3811);
    }

    fn loc_548(&mut self) {
        let si = self.r.si;
        let a = self.b(V380F);
        self.r.set_al(a);
        self.setb(V598 + si, a);
        self.setb(V59B + si, a);
    }

    // ------------------------------------------------------------------
    // shots and deaths

    /// shootingGuard (sub_131): guard SI fires at you.
    pub(super) fn shooting_guard(&mut self) {
        let si = self.r.si;
        if self.b(V59E + si) == 0 {
            // loc_564: out of bullets
            self.setb(V59A + si, 0);
            self.r.set_al(0x1D);
            self.speaker_high();
            self.sub_153();
            self.sub_141();
            let mut s: u16 = 0x80;
            loop {
                self.r.si = s;
                let al = self.b(V597 + s);
                self.r.set_al(al);
                if al == 0x10 {
                    self.sub_153();
                    self.sub_141();
                    self.queue(1);
                    self.r.si = si;
                    return;
                }
                s += 0x10;
                if s == 0x100 {
                    break;
                }
            }
            self.r.si = si;
            self.queue(5);
            return;
        }
        self.decb(V59E + si);
        let bl = self.b(V598 + si);
        self.r.bx = bl as u16;
        self.get_screen_pos();
        let bl = self.b(V59A + si);
        self.r.bx = bl as u16;
        let dl = self.r.dl().wrapping_add(self.b(V8B6 + self.r.bx));
        self.r.set_dl(dl);
        self.r.bx <<= 1;
        self.r.di = self.r.di.wrapping_add(self.w(V8C2 + self.r.bx));
        self.modify_sprite();
        let (di, dx) = (self.r.di, self.r.dx);
        self.r.set_al(0x71);
        self.host.tick_mark();
        self.draw2screen();
        self.r.dx = dx;
        self.r.di = di;
        self.fire_gun_sound();
        self.host.tick_wait_mark(&self.st);
        self.r.set_al(0x1B);
        self.draw2screen();
        let al = self.b(V59A + si);
        self.r.set_al(al);
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        self.col_detect();
        if self.r.al() != 0x40 {
            return;
        }
        // in the line of fire
        let al = self.get_rand().wrapping_add(0x10);
        self.r.set_al(al);
        let bl = self.b(SKILL_LEVEL);
        self.r.bx = bl as u16;
        if al >= self.b(V439E + self.r.bx) {
            return;
        }
        self.adjust_health();
        if self.cf {
            self.captured();
        }
    }

    /// adjustHealth (sub_132): you were hit. CF set = third hit.
    fn adjust_health(&mut self) {
        let bl = self.b(HEALTH + 1);
        self.r.bx = bl as u16;
        if bl >= 2 {
            self.cf = true;
            return;
        }
        self.incb(HEALTH + 1);
        let bx = self.r.bx;
        let al = self.b(V4487 + bx);
        self.setb(HEALTH, al);
        self.setb(V4FF, al);
        let al = self.b(V4489 + bx);
        self.r.set_al(al);
        self.setb(V4D3, al);
        self.setb(V4D4, al);
        self.r.bx = if bl == 0 { 0x52 } else { 0x53 };
        self.locate_and_print();
        self.cf = false;
    }

    /// captured (loc_617)
    fn captured(&mut self) -> ! {
        self.setb(CAP_COUNTER, 4);
        loop {
            self.screen_garbage();
            self.screen_garbage();
            self.r.set_al(0xFF);
            self.speaker_low();
            self.r.set_al(0xFF);
            self.speaker_low();
            if self.decb(CAP_COUNTER) == 0 {
                break;
            }
        }
        self.caught();
    }

    /// deadGuy: you shot a guard.
    pub(super) fn dead_guy(&mut self) {
        self.sub_153();
        self.sub_141();
        self.get_rand();
        let al = if self.sf { 0 } else { 6 };
        self.asdfg(al);
    }

    /// sub_134: stabbed.
    pub(super) fn sub_134(&mut self) {
        self.asdfg(6);
    }

    fn asdfg(&mut self, al: u8) {
        self.queue(al);
        if self.b(V599 + self.r.si) == 0xFF {
            return;
        }
        self.loc_571();
    }

    /// loc_571: guard SI becomes a body.
    pub(super) fn loc_571(&mut self) {
        let si = self.r.si;
        self.setb(V597 + si, 0x40);
        self.setb(V59A + si, 0);
        let bl = self.b(V598 + si);
        self.r.bx = bl as u16;
        {
            let i = V54F + self.r.bx;
            self.setb(i, 0);
        }
        self.sub_135();
        let bl = self.b(V598 + si).wrapping_add(8);
        self.r.set_bl(bl);
        self.setb(V598 + si, bl);
        self.loc_121();
    }

    // ------------------------------------------------------------------
    // type 50h: the officers at their desks

    pub(super) fn v450b(&mut self) {
        let si = self.r.si;
        self.setb(V59D + si, 0);
        let v = self.b(V59B + si);
        if v < 5 {
            if v >= 3 {
                return;
            }
            self.setb(V59B + si, 0);
        }
        self.loc_531();
    }

    fn loc_573(&mut self) {
        let si = self.r.si;
        if self.b(SKILL_LEVEL) >= 3 {
            self.setw(V3852, si);
            self.setb(V384F, 0xFF);
            self.sub_105();
            let si = self.w(V3852);
            self.r.si = si;
            if !self.cf {
                self.setb(V59B + si, 3);
            }
        }
        self.loc_595();
    }

    /// loc_575: you shot an officer.
    pub(super) fn loc_575(&mut self) {
        let si = self.r.si;
        if self.b(V59B + si) >= 3 {
            return;
        }
        self.dead_guy();
        let si = self.r.si;
        self.setb(V59B + si, 4);
        self.loc_595();
    }

    /// v455a: per-turn update of an officer.
    pub(super) fn v455a(&mut self) {
        let si = self.r.si;
        let al = self.b(V59B + si);
        self.r.set_al(al);
        if al >= 3 {
            if al < 5 {
                return;
            }
            // loc_577
            if self.b(V4F3) == 0 && self.b(V59D + si) & 1 != 0 {
                self.sub_120();
            }
            if self.w(V3850) == 0 {
                self.setb(V59B + si, 5);
                if self.get_rand() >= 0xC4 {
                    self.setw(V3850, 2);
                    self.setb(V3808, 1);
                }
                self.loc_595();
                return;
            }
            let n = self.w(V3850).wrapping_sub(1);
            self.setw(V3850, n);
            let al = if n == 0 {
                self.incb(V59B + si);
                0x87
            } else {
                0x82
            };
            self.queue(al);
            self.queue(2);
            self.loc_595();
            return;
        }
        // loc_583
        if self.b(V5A3 + si) != 0 {
            let saved = self.b(V59B + si);
            self.sub_124();
            let si = self.r.si;
            self.setb(V59B + si, saved);
            self.r.set_al(saved);
            self.setb(V599 + si, 0xFF);
            if self.b(V59D + si) & 1 != 0 {
                self.loc_590();
                return;
            }
            if self.b(V3808) != 0 {
                self.setb(V59B + si, 1);
                self.setb(V3808, 0);
                if self.b(V380B) != 0 {
                    self.print_secret_message();
                }
            }
            self.loc_595();
            return;
        }
        // loc_586
        self.setb(V3804, 0);
        self.setw(V3855, 0);
        loop {
            // loc_587
            let bx = self.w(V3855);
            self.r.bx = bx;
            self.setw(V3855, bx.wrapping_add(1));
            let al = self.b(V448B + bx);
            self.r.set_al(al);
            if al == 0 {
                // loc_592
                if self.b(V59B + si) == 2 {
                    self.loc_593();
                    return;
                }
                if self.b(V3804) != 0 {
                    self.loc_590();
                    return;
                }
                self.loc_595();
                return;
            }
            self.setb(V3854, al);
            let bl = self.b(V598 + si);
            self.r.set_bl(bl);
            self.sub_122();
            if self.r.al() == 0x40 {
                break;
            }
        }
        let al = self.b(GUN_DIR);
        self.r.set_al(al);
        let mut to_589 = al == 0;
        if !to_589 {
            if self.b(V4D9) != 0 {
                self.loc_593();
                return;
            }
            self.xlat_dir();
            if self.r.al() != self.b(V3854) {
                to_589 = true;
            } else {
                if self.b(V59B + si) != 2 {
                    self.setb(V59D + si, 1);
                    self.setb(V59B + si, 2);
                    self.queue(5);
                    self.sub_104();
                }
                self.loc_595();
                return;
            }
        }
        // loc_589
        let _ = to_589;
        if self.b(V59B + si) == 2 {
            self.loc_593();
            return;
        }
        if self.b(V59D + si) & 1 != 0 || self.b(V3804) != 0 {
            self.loc_590();
            return;
        }
        // loc_594
        if self.b(V59D + si) & 0x80 == 0 {
            self.setb(V381E, 0);
            self.setb(V380C, 0);
            self.setb(V381D, 0);
            self.setb(V5A3 + si, 0x80);
            self.queue(1);
            self.sub_130();
            self.setb(V59B + si, 1);
        }
        self.loc_595();
    }

    fn loc_590(&mut self) {
        if self.b(V4F3) == 0 {
            self.sub_104();
            self.sub_120();
        }
        self.loc_573();
    }

    fn loc_593(&mut self) {
        let si = self.r.si;
        self.setb(V59B + si, 0);
        self.loc_590();
    }

    fn loc_595(&mut self) {
        let si = self.r.si;
        self.sub_28();
        self.r.si = si;
        if self.b(V59B + si) == 1 {
            self.decb(V59B + si);
        }
    }

    /// printSecretMessage (sub_136): what a bribed officer tells you.
    fn print_secret_message(&mut self) {
        self.r.bx = 2;
        self.locate_and_print();
        let al = (self.get_rand() & 7) << 1;
        self.r.ax = al as u16;
        self.r.di = self.r.ax;
        let target = self.w(V46DA + self.r.di);
        self.call_code(target);
    }

    pub(super) fn v46ea(&mut self) {
        let bl = self.b(V4EB);
        self.r.bx = (bl as u16).wrapping_sub(1);
        let mut al = self.b(KEYZ + self.r.bx);
        let mut bx: u16 = 0;
        loop {
            bx += 1;
            let c = al & 1 != 0;
            al >>= 1;
            if c {
                break;
            }
        }
        self.r.set_al(al);
        self.r.dx = bx;
        self.r.bx = 0x56;
        self.print_msg(); // "PASS n"
    }

    pub(super) fn loc_598(&mut self) {
        let al = loop {
            let a = self.get_rand() & 7;
            if a != 0 {
                break a;
            }
        };
        self.r.ax = al as u16;
        let mut bx = al as u16;
        let mut cx: u16 = 3;
        loop {
            let mut dl = self.b(V4C7 + bx) & 0x0F;
            if dl >= 0x0A {
                dl &= 7;
            }
            self.r.dx = dl as u16;
            self.r.bx = 0x21;
            self.print_msg();
            bx += 1;
            cx -= 1;
            if cx == 0 {
                break;
            }
        }
        self.r.bx = bx;
        self.r.cx = 0;
    }

    pub(super) fn v4730(&mut self) {
        self.r.dx = if self.b(V4C7) & 1 != 0 { 1 } else { 2 };
        self.r.bx = 0x57;
        self.print_msg(); // "Circuit breaker n"
    }

    pub(super) fn v4741(&mut self) {
        self.r.bx = (self.r.di >> 1).wrapping_add(0x55);
        self.print_msg();
    }

    // ------------------------------------------------------------------
    // reinforcements

    pub(super) fn sub_105(&mut self) {
        let si = self.r.si;
        self.sub_106();
        self.r.si = si;
    }

    /// sub_106: bring a new guard into the room. CF set = none came.
    fn sub_106(&mut self) {
        self.setw(V384B, 0);
        self.setw(V384D, 0);
        let mut si: u16 = 0x80;
        let mut spawn_at: Option<u16> = None;
        loop {
            let al = self.b(V597 + si);
            self.r.set_al(al);
            if al == 0 {
                spawn_at = Some(si);
                break;
            }
            if al == 0x40 {
                if self.w(V384B) == 0 {
                    self.setw(V384B, si);
                }
                let a = self.b(V59A + si);
                self.r.set_al(a);
                if a != 0 && a != 0xFF {
                    self.r.bx = a as u16;
                    if self.b(V597 + self.r.bx) == 0x40 {
                        self.setw(V384B, si);
                        self.setw(V384D, si);
                    }
                }
            }
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
        let si = match spawn_at {
            Some(s) => s,
            None => {
                // loc_429: replace a body
                let bx = self.w(V384B);
                self.r.bx = bx;
                if bx == 0 {
                    self.r.si = si;
                    self.loc_432();
                    return;
                }
                if self.w(V384D) == 0 {
                    let al = self.b(V598 + bx);
                    self.r.ax = al as u16;
                    self.r.di = self.r.ax;
                    {
                        let i = V54F + self.r.di;
                        self.setb(i, 0);
                    }
                }
                bx
            }
        };
        self.r.si = si;
        // loc_431
        self.sub_108();
        if self.r.bl() & 0x80 != 0 {
            self.loc_432();
            return;
        }
        self.sub_107();
        if self.cf {
            let bl = self.r.bl().wrapping_add(self.b(V3837));
            self.r.set_bl(bl);
            self.sub_107();
            if self.cf {
                self.loc_432();
                return;
            }
        }
        // loc_433
        for di in 0..0x10u16 {
            let al = self.b(V383B + di);
            self.setb(V597 + si + di, al);
        }
        self.r.di = 0x10;
        let bl = self.r.bl();
        self.setb(V598 + si, bl);
        self.setb(V59B + si, bl);
        self.r.set_al(0xFF);
        self.sub_129();
        let al = self.b(V599 + si);
        self.setb(V59A + si, al);
        self.sub_111();
        if self.zf {
            let al = self.r.di as u8;
            self.r.ax = self.r.di;
            self.setb(V5A6 + si, al);
            self.setb(V59A + si, al);
        }
        // loc_435
        let bl = self.b(V598 + si);
        self.r.bx = bl as u16;
        self.get_screen_pos();
        self.loc_451();
        self.queue(5);
        let mut al = (self.get_rand() & 0x0F).wrapping_add(0x28);
        if self.b(START_KEY) != 0 {
            al = al.wrapping_add(0x14);
        }
        self.r.set_al(al);
        self.setb(V503, al);
        self.cf = false;
    }

    fn loc_432(&mut self) {
        self.setb(V503, 0x40);
        self.cf = true;
    }

    /// sub_107: is cell BL free for a new guard? CF set = no.
    fn sub_107(&mut self) {
        let bl = self.r.bl();
        let d = bl.wrapping_sub(self.b(V4E5));
        self.r.set_al(d);
        if d == 0 || d == 8 || d == 0xF8 {
            self.cf = true;
            return;
        }
        let bx = self.r.bx;
        let al = self.b(V54F + bx);
        self.r.set_al(al);
        if al != 0 {
            self.r.ax = al as u16;
            self.r.di = self.r.ax;
            let t = self.b(V597 + self.r.di);
            self.r.set_al(t);
            self.sub_118();
            if self.cf {
                return;
            }
        }
        let al = self.b(V557 + bx);
        self.r.set_al(al);
        if al == 0 {
            self.cf = false;
            return;
        }
        self.r.ax = al as u16;
        self.r.di = self.r.ax;
        let t = self.b(V597 + self.r.di);
        self.r.set_al(t);
        self.sub_118();
        // CF as returned: set = blocked
    }

    /// sub_108 + sub_109: pick the door a new guard comes in through.
    fn sub_108(&mut self) {
        if self.b(V384F) != 0 {
            let di = self.w(V3852);
            self.r.di = di;
            self.setb(V3837, 0);
            let bl = self.b(V598 + di).wrapping_sub(1);
            self.r.bx = bl as u16;
            return;
        }
        let mut al = self.get_rand() & 7;
        if al >= 4 {
            al >>= 1;
        }
        self.r.set_al(al);
        self.r.bx = al as u16;
        let bx = self.r.bx;
        let v = self.b(V382F + bx);
        self.setb(V3838, v);
        let v = self.b(V3827 + bx);
        self.setb(V383A, v);
        let v = self.b(V382B + bx);
        self.setb(V3839, v);
        let v = self.b(V3833 + bx);
        self.r.set_al(v);
        self.setb(V3837, v);
        loop {
            // loc_442
            let bl = self.b(V383A);
            self.r.bx = bl as u16;
            let al = self.b(V597 + self.r.bx) & self.b(V3838);
            self.r.set_al(al);
            if al == 0 {
                return;
            }
            let al = self.b(V3839);
            self.r.set_al(al);
            self.sub_24();
            if self.r.dl() >= 8 || self.r.dh() >= 8 {
                break;
            }
            let bl = self.r.bl();
            self.setb(V383A, bl);
        }
        // loc_444
        let al = self.b(V2ED2);
        self.r.set_al(al);
        self.setb(V3837, al);
        let bl = self.b(V4D5);
        self.r.set_bl(bl);
        let mut use_502 = true;
        if bl & 0x80 == 0 {
            let mut hit = bl == self.b(V502);
            if !hit {
                self.r.bx = self.r.bx.wrapping_add(1);
                hit = self.r.bl() == self.b(V502);
            }
            if hit && self.b(V4D2) & 0x80 != 0 {
                use_502 = false;
            }
        }
        if use_502 {
            let bl = self.b(V502);
            self.r.set_bl(bl);
        }
    }
}
