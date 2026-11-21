//! newGamePrep: the A/B/C/D menu, loading/resuming, and the random bunker
//! generator used for a new game.

use super::core::{CR, ESCAPE};
use super::*;

impl<H: Host> Game<H> {
    pub(super) fn new_game_prep(&mut self) {
        if self.b(START_KEY) == 0 {
            // demo: the built-in bunker, canned keys and random numbers
            self.r.set_ch(0x1E);
            self.restore_sectors_900();
            self.reset_vars();
            self.setw(DEMO_KEY_PTR, D_DEMO_KEYS);
            self.setw(RND_PTR, D_DEMO_RAND);
            return;
        }
        // loc_335
        self.setb(MENU_SELECT, 0xFF);
        self.load_saved_game();
        self.r.ax = 0x201;
        self.r.bx = 4;
        self.r.cx = 0x901;
        self.r.dx = 0;
        self.restore_variables();
        if self.b(RESUME_GAME) == 0x3C {
            self.loc_347();
            return;
        }
        // loc_336: the menu
        self.r.dx = 0x0D;
        self.locate();
        let bl = self.b(SKILL_LEVEL);
        self.r.bx = bl as u16 + 0x60;
        self.print_msg();
        self.r.dx = 0;
        self.locate();
        self.r.bx = 0x5E;
        self.print_msg();
        // Option E (not in the original): only when optionz.dat is in the
        // game folder. Without it the screen is exactly the original's.
        self.custom_bunker = self.host.file_read(GameFile::OptionZ).is_some();
        if self.custom_bunker {
            self.print_text(b" E) Start a new game with your custom\n\r    bunker");
            self.r.dx = 0x1200;
            self.locate();
        }
        if self.b(RESUME_GAME) == 0xFF {
            self.r.bx = 0x5F;
            self.print_msg(); // "D) Resume game"
        }
        self.loc_337();
        loop {
            // loc_339
            loop {
                self.r.set_ah(1);
                if self.int16_peek().is_some() {
                    break;
                }
                self.get_rand(); // seed the generator while we wait
            }
            self.r.set_ah(0);
            let al = self.int16_get() as u8;
            if al == CR {
                // loc_343
                let sel = self.b(MENU_SELECT);
                if sel == 0xFF {
                    continue;
                }
                self.r.set_bl(sel);
                match sel {
                    0 => self.choice_a(),
                    1 => {
                        self.setb(SKILL_LEVEL, 0);
                        self.choice_a();
                    }
                    2 => {
                        // C: same bunker
                        self.r.set_ch(0x0D);
                        self.load_sectors();
                        self.load_option(GameFile::OptionC);
                        self.reset_vars();
                        self.setb(RESUME_GAME, 0);
                        self.set_defaults();
                    }
                    4 => {
                        // E: the custom bunker, played as designed (like C).
                        // Nothing here or in A/B ever writes optionz.dat.
                        self.r.set_ch(0x0D);
                        self.load_sectors();
                        self.load_option(GameFile::OptionZ);
                        self.reset_vars();
                        self.setb(RESUME_GAME, 0);
                        self.set_defaults();
                    }
                    _ => {
                        // D: resume
                        self.r.set_ch(9);
                        self.load_sectors();
                        self.load_saved_game();
                        self.restore_variables();
                        self.setb(RESUME_GAME, 0);
                    }
                }
                return;
            }
            if al == ESCAPE {
                jump(Jump::Exit);
            }
            if al == 3 {
                // ctrl-C: monitor type / palette
                let m = self.b(MONITOR_TYPE) ^ 0x21;
                self.setb(MONITOR_TYPE, m);
                self.st.bios.set_palette(1, m & 1);
                self.host.present(&self.st);
                self.loc_337();
                continue;
            }
            // loc_341
            let n = al.wrapping_sub(b'a');
            self.r.set_al(n);
            if n > 3 && !(n == 4 && self.custom_bunker) {
                continue;
            }
            if n == 3 && self.b(RESUME_GAME) != 0xFF {
                continue;
            }
            self.draw_selection_border();
            // With option E shown, D sits three rows lower, so the prompt
            // moves below D's box when D is there.
            let d_shown = self.b(RESUME_GAME) == 0xFF;
            self.r.dx = if self.custom_bunker && d_shown { 0x1501 } else { 0x1301 };
            self.locate();
            self.r.bx = 0x65;
            self.print_msg(); // "Press return to begin..."
            self.host.present(&self.st);
        }
    }

    /// Screen position of option `sel`'s selection box: the original's table
    /// (rows 5, 8, 11, 14), except with option E shown, where E's box takes
    /// row 14 and D's moves to row 17.
    fn menu_box_pos(&self, sel: u8) -> u16 {
        if self.custom_bunker {
            match sel {
                4 => return 0x1180,
                3 => return 0x1540,
                _ => {}
            }
        }
        self.w(V35EF + ((sel as u16) << 1))
    }

    /// Print plain text in colour 3 at the cursor (for option E's line).
    fn print_text(&mut self, text: &[u8]) {
        for &c in text {
            self.teletype(c, 3);
        }
    }

    fn loc_337(&mut self) {
        self.r.dx = 0x1700;
        self.locate();
        self.r.bx = if self.b(MONITOR_TYPE) == 1 {
            0x6C
        } else {
            0x6D
        };
        self.print_msg();
        self.host.present(&self.st);
        self.getakey();
    }

    fn choice_a(&mut self) {
        self.setb(RESUME_GAME, 0x3C);
        self.loc_347();
    }

    /// loadSectors: the floppy reads are gone; only register effects remain.
    fn load_sectors(&mut self) {
        let ch = self.r.ch().wrapping_add(4);
        self.r.ax = 0x0208;
        self.r.bx = 0x4004;
        self.r.cx = ((ch as u16) << 8) | 1;
        self.r.dx = 0;
        self.r.di = 5;
    }

    /// loc_347: build a new random bunker from optiona.dat.
    fn loc_347(&mut self) {
        self.r.set_ch(5);
        self.load_sectors();
        self.load_option(GameFile::OptionA);
        // stamp every room with its number
        let mut si: u16 = 0x150;
        while si < 0x4000 {
            self.set_s9(si, (si >> 8) as u8);
            si += 0x100;
        }
        // fewer objects on lower skill levels
        let skill = self.b(SKILL_LEVEL) as u16;
        if skill < 4 {
            let bx = self.b(V3545 + skill) as u16;
            let mut si: u16 = 0x100;
            loop {
                si = si.wrapping_add(0x80).wrapping_add(bx);
                loop {
                    self.set_s9(si, 0);
                    si = si.wrapping_add(0x10);
                    if si as u8 == 0 {
                        break;
                    }
                }
                if si >= 0x3B00 {
                    break;
                }
            }
            self.r.bx = bx;
            self.r.si = si;
        }
        self.sub_92();
        // guards: random state
        let mut si: u16 = 0x180;
        loop {
            if self.s9(si) & 0xF0 == 0x10 {
                let mut al = self.get_rand() & 0x0F;
                if al >= 0x0B {
                    al -= 6;
                }
                self.set_s9(si + 7, al);
                let al = loop {
                    let a = self.get_rand();
                    if a < 0x15 {
                        break a;
                    }
                };
                self.set_s9(si + 8, al);
                self.set_s9(si + 3, 0);
                self.set_s9(si + 6, 0);
                self.set_s9(si + 0x0C, 0);
                let al = self.get_rand() & 0x1F;
                self.set_s9(si + 9, al);
            }
            si += 0x10;
            if si as u8 == 0 {
                si += 0x80;
                if si >= 0x4000 {
                    break;
                }
            }
        }
        // closets: contents
        let bx = self.b(SKILL_LEVEL) as u16;
        let mut cl = self.b(V3549 + bx);
        let mut ch = self.b(V354E + bx);
        let mut dh = self.b(V3549 + bx);
        let mut dl: u8 = 0;
        let mut si: u16 = 0x180;
        loop {
            self.r.si = si;
            if self.s9(si) & 0xF0 == 0x20 {
                let al = match dl & 0x0F {
                    0x0A => 5,
                    0x0B => {
                        if dh != 0 {
                            dh -= 1;
                            0x0B
                        } else {
                            1
                        }
                    }
                    0x0C => {
                        if ch != 0 {
                            ch -= 1;
                            0x0C
                        } else {
                            4
                        }
                    }
                    0x0D => {
                        if cl != 0 {
                            cl -= 1;
                            0x0D
                        } else {
                            5
                        }
                    }
                    0x0F => 6,
                    n => n,
                };
                self.set_s9(si + 2, al);
                dl = dl.wrapping_add(1);
                self.r.cx = ((ch as u16) << 8) | cl as u16;
                self.r.dx = ((dh as u16) << 8) | dl as u16;
                self.r.set_al(al);
                self.sub_89();
                // sub_89 preserves CX/DX across countBits
                self.set_s9(si + 4, 0xF0);
                self.set_s9(si + 0x0B, 0x77);
                self.set_s9(si + 0x0C, 0);
                self.set_s9(si + 0x0D, 0);
                self.set_s9(si + 0x0E, 0);
            }
            si += 0x10;
            if si as u8 == 0 {
                si += 0x80;
                if si >= 0x3D00 {
                    break;
                }
            }
        }
        self.sub_92();
        let al = loop {
            let a = self.get_rand() & 0x2F;
            if a >= 0x1C {
                break a;
            }
        };
        self.r.bx = al as u16;
        self.r.si = 0x3D;
        self.sub_93();
        let al = loop {
            let a = self.get_rand() & 0x3F;
            if a < 0x3B && a >= 0x32 {
                break a;
            }
        };
        self.r.bx = al as u16;
        self.r.si = 0x3E;
        self.sub_93();
        self.sub_91();
        // link the secret passages in pairs
        self.r.di = 0;
        self.r.bx = 0;
        let mut si: u16 = 0x180;
        loop {
            if self.s9(si) == 0x20 && self.s9(si + 2) == 0x0E {
                self.r.si = si;
                self.sub_90();
            }
            si += 0x10;
            if si as u8 == 0 {
                si += 0x80;
                if si >= 0x4000 {
                    break;
                }
            }
        }
        if self.r.bl() != 0 {
            let di = self.r.di;
            self.set_s9(di + 2, 0);
        }
        // put the bomb in a closet on the middle floor
        let mut si: u16 = 0x1080;
        let mut found = false;
        loop {
            if self.s9(si) == 0x20 && self.s9(si + 2) < 0x0A {
                found = true;
                break;
            }
            si += 0x10;
            if si as u8 == 0 {
                si += 0x80;
                if si >= 0x2400 {
                    break;
                }
            }
        }
        if !found {
            si = 0x1080;
            loop {
                if self.s9(si) == 0x20 {
                    break;
                }
                si = si.wrapping_add(0x10);
                if si as u8 == 0 {
                    si = si.wrapping_add(0x80);
                }
            }
        }
        self.set_s9(si + 2, 0x0F);
        self.r.si = si;
        self.reset_vars();
        // door combinations
        let mut bx: u16 = 7;
        loop {
            let al = loop {
                let a = self.get_rand();
                if a != 0 {
                    break a;
                }
            };
            self.setb(V4C7 + bx, al);
            bx = bx.wrapping_sub(1);
            if bx & 0x8000 != 0 {
                break;
            }
        }
        self.r.bx = bx;
        self.set_defaults();
        self.restore_variables();
        self.r.set_ch(0x0D);
        self.save_option_c();
        self.setb(START_KEY, 0);
    }

    /// sub_89: closet SI with contents AL gets a lock / amount.
    fn sub_89(&mut self) {
        let si = self.r.si;
        self.set_s9(si + 3, 1);
        let al = self.r.al();
        if al == 0 {
            self.set_s9(si + 3, 0);
            return;
        }
        if al == 3 {
            // passes: a random set with few enough bits for this skill level
            loop {
                let a = self.get_rand() & 0x1F;
                if a == 0 {
                    continue;
                }
                let (dx, cx) = (self.r.dx, self.r.cx);
                self.r.set_ah(a);
                self.count_bits();
                let bits = self.r.dl();
                self.r.cx = cx;
                let bl = self.b(SKILL_LEVEL);
                self.r.bx = bl as u16;
                let ok = bits < self.b(V3536 + self.r.bx);
                self.r.dx = dx;
                if !ok {
                    continue;
                }
                self.r.set_al(a);
                self.set_s9(si + 3, a);
                return;
            }
        }
        if al > 3 {
            return;
        }
        let n = al - 1;
        let di = (n as u16) * 5 + self.b(SKILL_LEVEL) as u16;
        self.r.di = di;
        let al = loop {
            let a = self.get_rand();
            if a < self.b(V353B + di) {
                break a;
            }
        };
        self.set_s9(si + 3, al);
    }

    /// sub_90: pair up secret passages.
    fn sub_90(&mut self) {
        let si = self.r.si;
        if self.r.bl() == 0 {
            let v = self.s9(si + 1) as u16 | ((self.s9(si + 8) as u16) << 8);
            self.r.bx = v;
            self.r.di = si;
            return;
        }
        let bx = self.r.bx;
        self.set_s9(si + 9, bx as u8);
        self.set_s9(si + 10, (bx >> 8) as u8);
        let di = self.r.di;
        let (b1, b8) = (self.s9(si + 1), self.s9(si + 8));
        self.set_s9(di + 9, b1);
        self.set_s9(di + 10, b8);
        self.r.bx = 0;
        self.r.di = 0;
    }

    /// sub_91: every closet records which room it is in.
    fn sub_91(&mut self) {
        let mut si: u16 = 0x180;
        let mut bx: u16 = 1;
        loop {
            if self.s9(si) == 0x20 {
                self.set_s9(si + 8, bx as u8);
            }
            si += 0x10;
            if si as u8 == 0 {
                si += 0x80;
                bx += 1;
                if si >= 0x4000 {
                    break;
                }
            }
        }
        self.r.si = si;
        self.r.bx = bx;
    }

    /// sub_92: shuffle rooms 2..39h.
    fn sub_92(&mut self) {
        let mut bx: u16 = 2;
        loop {
            let a = self.get_rand();
            let carry = bx & 1;
            self.r.cx = bx >> 1;
            let mut al = (a << 1) | carry as u8;
            al ^= 1;
            al &= 0x3F;
            if al >= 0x3A || al < 2 {
                continue;
            }
            self.r.ax = al as u16;
            self.r.si = al as u16;
            self.r.bx = bx;
            self.sub_93();
            bx += 1;
            if bx >= 0x3A {
                break;
            }
        }
        self.r.bx = bx;
    }

    /// sub_93: swap the contents of rooms SI and BX (not their exits).
    fn sub_93(&mut self) {
        let bx0 = self.r.bx;
        let mut si = self.r.si << 8;
        let mut bx = bx0 << 8;
        for (n, skip) in [(0x49u16, 6u16), (0xB1, 0)] {
            for _ in 0..n {
                let a = self.s9(si);
                let b = self.s9(bx);
                self.set_s9(bx, a);
                self.set_s9(si, b);
                si = si.wrapping_add(1);
                bx = bx.wrapping_add(1);
            }
            si = si.wrapping_add(skip);
            bx = bx.wrapping_add(skip);
        }
        self.r.si = si;
        self.r.cx = 0;
        self.r.bx = bx0;
    }

    /// setDefaults (sub_84)
    pub(super) fn set_defaults(&mut self) {
        self.assign_keys_n_passes();
        self.setb(V4E5, 36);
        self.setb(NUM_BOMBS, 3);
        self.setb(ROOM_INDEX, 1);
        self.setb(V4EB, 1);
        self.setb(MARKS, 100);
        let al = loop {
            let a = self.get_rand() & 0x1F;
            if a != 0 {
                break a;
            }
        };
        self.setb(PASSES, al);
        self.setb(BULLETS, 10);
        self.setw(V4EE, 0);
    }

    fn assign_keys_n_passes(&mut self) {
        let k = self.get_rand_bits();
        self.setb(KEYZ, k);
        let p = loop {
            let a = self.get_rand_bits();
            if a != k {
                break a;
            }
        };
        self.setb(PASSEZ, p);
        let x = loop {
            let a = self.get_rand_bits();
            if a != k && a != p {
                break a;
            }
        };
        self.setb(V4FE, x);
    }

    /// getRandBits: a random single bit 0..4 (despite the comment in the
    /// original, it only accepts 1, 2, 4, 8 or 16).
    fn get_rand_bits(&mut self) -> u8 {
        loop {
            let a = self.get_rand() & 0x1F;
            self.r.set_al(a);
            if matches!(a, 1 | 2 | 4 | 8 | 0x10) {
                return a;
            }
        }
    }

    /// drawSelectionBorder (sub_95): box around menu option AL.
    fn draw_selection_border(&mut self) {
        let al = self.r.al();
        self.r.set_bh(0);
        self.setw(XPOS, 0);
        let sel = self.b(MENU_SELECT);
        if sel != 0xFF {
            // erase the old box
            self.r.bx = (sel as u16) << 1;
            self.r.di = self.menu_box_pos(sel);
            self.r.dx = 0;
            for _ in 0..0x28 {
                self.r.set_al(0x20);
                self.draw2screen();
            }
            for _ in 0..2 {
                self.r.set_al(0x20);
                self.draw2screen();
                self.r.dx = self.r.dx.wrapping_add(0x4C);
                self.r.set_al(0x20);
                self.draw2screen();
            }
            for _ in 0..0x28 {
                self.r.set_al(0x20);
                self.draw2screen();
            }
        }
        // loc_394
        self.setb(MENU_SELECT, al);
        self.r.bx = (al as u16) << 1;
        self.r.di = self.menu_box_pos(al);
        self.r.dx = 0;
        let g = |s: &mut Self, c: u8| {
            s.r.set_al(c);
            s.draw2screen();
        };
        g(self, 0x0F);
        for _ in 0..0x26 {
            g(self, 0x11);
        }
        g(self, 0x10);
        for _ in 0..2 {
            g(self, 0x2E);
            self.r.dx = self.r.dx.wrapping_add(0x4C);
            g(self, 0x2E);
        }
        g(self, 0x12);
        for _ in 0..0x26 {
            g(self, 0x11);
        }
        g(self, 0x0E);
        self.r.cx = 0;
    }
}
