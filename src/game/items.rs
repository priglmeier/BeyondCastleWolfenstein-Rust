//! Searching bodies, closets and their contents, locked doors, the alarm
//! box, and picking up / dropping inventory.

use super::*;

impl<H: Host> Game<H> {
    /// loc_156: a closet was shot.
    pub(super) fn loc_156(&mut self) {
        let si = self.r.si;
        let al = self.b(V598 + si).wrapping_add(1);
        self.r.set_al(al);
        if al != self.b(V4E5) {
            return;
        }
        let dl = self.b(V599 + si);
        self.r.set_dl(dl);
        if dl == 4 || dl == 0x0F {
            // shot the powder kegs or the bomb
            self.setb(V18D1, 31);
            self.blast();
            self.cls();
            self.r.dx = 0x0C02;
            self.locate();
            self.r.bx = 0x5C;
            self.print_msg();
            self.loc_634();
        }
        if self.b(V59B + si) == 0 && dl == 0x0A {
            // the alarm controls
            let al = if self.get_rand() < 0xAF {
                self.setb(V18D1, 175);
                1
            } else {
                self.setb(V18D1, 0);
                0
            };
            self.r.set_al(al);
            self.setb(V59D + si, al);
            self.setb(V4F3, al);
            return;
        }
        if self.get_rand() >= 0x24 {
            return;
        }
        self.loc_170();
    }

    /// searchResults: searching a body finished.
    pub(super) fn search_results(&mut self) {
        let si = self.r.si;
        let bl = self.b(si + V598);
        self.r.set_bl(bl);
        let cl = self.b(si + V597);
        self.r.set_cl(cl);
        if cl != 0x40 {
            // loc_169
            if cl != 0x20 {
                return;
            }
            if self.b(ALARM_BOX_ARMED) != 0 {
                self.alarmboxstuff();
                return;
            }
            if self.b(V59B + 1 + si) == 0 {
                self.loc_170();
            } else {
                self.loc_265();
            }
            return;
        }
        self.r.bx = 2;
        self.locate_and_print();
        self.setb(NUM_ITEMS_FOUND, 0);
        let al = self.b(si + V59E);
        self.r.set_al(al);
        if al != 0 && al > self.b(BULLETS) {
            let old = self.b(BULLETS);
            self.setb(BULLETS, al);
            self.setb(V59E + si, old);
            self.r.set_al(old);
            self.incb(NUM_ITEMS_FOUND);
            self.r.dx = self.b(BULLETS) as u16;
            self.r.bx = 0x1B;
            self.print_msg();
        }
        // loc_164: passes
        let al = self.b(V5A0 + si) & 0x1F;
        self.r.set_al(al);
        if al != 0 {
            let al = al | self.b(PASSES);
            if al != self.b(PASSES) {
                let ah = al ^ self.b(PASSES);
                self.setb(PASSES, al);
                let v = self.b(V5A0 + si) ^ ah;
                self.setb(V5A0 + si, v);
                self.r.set_al(al);
                self.r.set_ah(ah);
                let ax = self.r.ax;
                self.multiple_items_found();
                self.r.ax = ax;
                self.count_bits();
                self.r.bx = 0x1C;
                self.print_msg();
                self.incb(NUM_ITEMS_FOUND);
            }
        }
        // loc_165: money
        let dl = self.b(V59F + si);
        self.r.set_dl(dl);
        if dl != 0 {
            self.r.set_dh(0);
            let dx = self.r.dx;
            self.multiple_items_found();
            self.r.dx = dx;
            self.r.bx = 0x1E;
            self.print_msg();
            self.incb(NUM_ITEMS_FOUND);
            self.add_marks();
            let dl = self.r.dl();
            self.setb(V59F + si, dl);
        }
        if self.b(NUM_ITEMS_FOUND) == 0 {
            self.r.bx = 0x1F;
            self.print_msg();
        }
        self.print_passes();
        self.loc_193();
    }

    fn loc_193(&mut self) {
        self.setb(V4F0, 0);
        self.setw(V4F1, 0);
    }

    fn loc_170(&mut self) {
        let si = self.r.si;
        self.setb(V59B + si, 0);
        self.open_closet();
    }

    /// alarmboxstuff (loc_171)
    fn alarmboxstuff(&mut self) {
        let al = self.b(V18D3);
        self.r.set_al(al);
        self.setb(V4F3, al);
        if al == 0 {
            self.setb(V18D1, 0);
            self.r.bx = 5;
            self.locate_and_print(); // "Alarm is off !"
        } else {
            self.r.bx = 6;
            self.locate_and_print(); // "Alarm box is jammed !"
            self.setb(V18D1, 175);
        }
        self.loc_173();
    }

    fn loc_173(&mut self) {
        self.setb(ALARM_BOX_ARMED, 0);
        let al = self.b(V18D2);
        self.r.set_al(al);
        let si = self.r.si;
        self.setb(V59D + si, al);
    }

    /// loc_174: working on the alarm box.
    fn loc_174(&mut self) {
        let si = self.r.si;
        if self.b(V59D + si) != 0 || self.b(V18D2) != 0 {
            // loc_175
            self.r.bx = 6;
            self.print_msg();
            self.loc_173();
            return;
        }
        let ck = self.w(CTRL_KEY);
        let carry: bool;
        let al_code: u8;
        if ck == 0x1F || ck == 0x20 {
            // loc_179: circuit breaker 1 or 2
            self.r.ax = ck;
            let al = self.r.al() ^ self.b(V4C7);
            carry = al & 1 != 0;
            self.r.set_al(al >> 1);
            self.loc_180(carry);
            return;
        }
        if self.b(TOOLKITS) != 0 && ck == 0x1A {
            // loc_185: tools
            let al = loop {
                let a = self.get_rand();
                if a != 0 {
                    break a;
                }
            };
            self.loc_180(al < 0xC8);
            return;
        }
        if ck == 0x1C {
            // loc_186: 'u'
            self.r.bx = 0x46;
            self.locate_and_print();
            self.r.bx = 0x55;
            self.print_msg();
            return;
        }
        if self.b(NUM_KEYS) != 0 && ck == 0x1B {
            // loc_183: keys
            let bl = self.b(V4EB);
            self.r.bx = bl as u16;
            let al = self.b(KEYZ + self.r.bx) & self.b(NUM_KEYS);
            self.r.set_al(al);
            self.r.cx = if al == 0 { 1 } else { 0 };
            let cl = self.r.cl();
            self.setb(V18D3, cl);
            al_code = 2;
            self.loc_182(al_code);
            return;
        }
        self.cf = false;
    }

    fn loc_180(&mut self, carry: bool) {
        self.r.set_cl(0);
        if carry {
            self.r.cx = self.r.cx.wrapping_add(1);
        }
        let cl = self.r.cl();
        self.setb(V18D3, cl);
        self.setb(V18D2, cl);
        self.loc_182(6);
    }

    fn loc_182(&mut self, al: u8) {
        self.r.set_al(al);
        self.sub_73();
        self.setb(ALARM_BOX_ARMED, 0xFF);
        self.r.bx = 0x54;
        self.locate_and_print(); // "Resetting alarm..."
        self.cf = true;
    }

    /// openCloset (loc_187)
    pub(super) fn open_closet(&mut self) {
        let cx = self.r.cx;
        for _ in 0..5 {
            self.host.tick_wait(&self.st);
        }
        self.r.cx = cx;
        let si = self.r.si;
        let al = self.b(V599 + si);
        if al != 0x0E && al != 0x0A && self.b(V59A + si) == 0 {
            self.setb(V599 + si, 0);
        }
        self.r.bx = 2;
        self.locate_and_print();
        self.print_closet_contents();
        let si = self.r.si;
        let al = self.b(V599 + si);
        self.r.set_al(al);
        'done: {
            if al == 0 || al == 0x0A {
                break 'done;
            }
            if al != 0x0E {
                self.r.bx = 7;
                self.print_msg(); // " (U to Use)"
                break 'done;
            }
            // loc_189: a secret passage (stairs)
            let al = self.b(V4EB);
            self.r.set_al(al);
            let bl = self.b(V5A1 + si);
            self.r.set_bl(bl);
            if al == 1 {
                if bl < 0x19 {
                    break 'done;
                }
                self.r.bx = 8;
                self.print_msg(); // " downstairs !"
                break 'done;
            }
            let up = if al == 2 {
                bl < 0x19 || bl >= 0x31
            } else {
                bl < 0x31
            };
            if up {
                self.r.bx = 9;
                self.print_msg(); // " upstairs !"
            }
        }
        // loc_192
        self.loc_124();
        self.loc_193();
    }

    /// printClosetContents (sub_57)
    pub(super) fn print_closet_contents(&mut self) {
        let si = self.r.si;
        let v = self.b(V599 + si) & 0x0F;
        self.setb(V599 + si, v);
        self.r.set_bl(v.wrapping_add(0x0A));
        self.print_msg();
    }

    /// searchOrBodyMove (sub_49): space bar on a body (search / drag) or
    /// on a closet / door.
    pub(super) fn search_or_body_move(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        let cl = self.b(V597 + si);
        self.r.set_cl(cl);
        if cl == 0x40 {
            if self.w(CTRL_KEY) != 0x18 {
                return;
            }
            let al = self.b(V4E5).wrapping_sub(bl);
            self.r.set_al(al);
            if al == 0 || al == 0xF8 {
                self.print_search();
                return;
            }
            // drag the body
            let bx = self.r.bx;
            self.setb(V54F + bx, 0);
            self.sub_30();
            self.r.si = 0x1C;
            self.put_gfx();
            self.r.si = si;
            let al = self.b(GUN_DIR);
            self.r.set_al(al);
            self.xlat_dir();
            let bl = self.b(V598 + si);
            self.r.set_bl(bl);
            self.sub_24();
            let bl = self.r.bl();
            self.setb(V598 + si, bl);
            self.sub_29();
            {
                let i = V59A + self.r.si;
                self.setb(i, 0xFF);
            }
            self.loc_196();
            return;
        }
        // loc_199
        if cl != 0x20 {
            return;
        }
        if self.b(V5A4 + 1 + si) == 0 {
            let mut al = self.b(V599 + si);
            if al == 0 {
                al = 1;
            }
            self.r.set_al(al);
            self.setb(V5A4 + 1 + si, al);
        }
        if self.b(ALARM_BOX_ARMED) != 0 {
            return;
        }
        self.r.bx = self.r.bx.wrapping_add(1);
        if self.r.bl() != self.b(V4E5) {
            return;
        }
        if self.b(GUN_DIR) != 4 {
            return;
        }
        // loc_203: standing below it, facing up
        if self.b(V18D5) != 0 || self.w(CTRL_KEY) == 0x1C {
            self.loc_253();
            return;
        }
        // loc_205
        if self.b(V59B + si) == 0 {
            if self.b(V599 + si) != 0x0A {
                return;
            }
            self.loc_253();
            return;
        }
        // loc_206: a locked closet
        let mut go_207 = true;
        if self.b(DIGIT_COUNTER) == 0 {
            self.check_0thru9();
            if self.cf && self.b(DOOR_OPEN_ATTEMPTS) == 0 && self.w(CTRL_KEY) != 0x1A {
                if self.w(CTRL_KEY) != 0x18 {
                    return;
                }
                if self.b(V5A4 + 1 + si) >= 0x0A {
                    let al = self.b(V5A3 + si);
                    self.r.set_al(al);
                    if al != 0 {
                        if al & 0x80 == 0 {
                            self.r.bx = 0;
                            self.locate_and_print(); // "It's jammed !"
                            return;
                        }
                        go_207 = false;
                    }
                } else {
                    go_207 = false;
                }
            }
        }
        if go_207 {
            // loc_207
            self.unlock_door();
            if self.cf {
                return;
            }
            if self.b(DIGIT_COUNTER) < 3 {
                return;
            }
            self.setb(DIGIT_COUNTER, 0);
        }
        // loc_208: open
        self.r.set_al(1);
        self.sub_73();
        let si = self.r.si;
        self.setb(V59B + 1 + si, 0);
        self.setb(DOOR_OPEN_ATTEMPTS, 0);
        self.loc_124();
    }

    fn print_search(&mut self) {
        self.r.set_al(6);
        self.sub_73();
        self.r.bx = 0x1A;
        self.locate_and_print(); // "Searching..."
    }

    /// sub_50: draw a body lying in the cell (marks it as being dragged).
    pub(super) fn sub_50(&mut self) {
        let si = self.r.si;
        if self.b(V59A + si) == 0 {
            self.setb(V59A + si, 0xFF);
        }
        self.loc_196();
    }

    pub(super) fn loc_196(&mut self) {
        self.sub_30();
        self.r.set_al(0x61);
        self.draw2screen();
        self.r.dx = self.r.dx.wrapping_add(2);
        self.r.set_al(0x63);
        self.draw2screen();
    }

    /// unlockDoor (sub_51). CF set = not (yet) open.
    fn unlock_door(&mut self) {
        let si = self.r.si;
        if self.b(V5A4 + 1 + si) < 0x0A {
            self.cf = true;
            return;
        }
        self.r.bx = 1;
        self.locate_and_print(); // "Locked !"
        if self.b(DOOR_OPEN_ATTEMPTS) != 0 || self.w(CTRL_KEY) == 0x1A {
            self.force_open_door(); // always returns CF clear
        }
        // pickLock
        self.check_0thru9();
        if !self.cf {
            self.sub_55();
            let bl = self.b(DIGIT_COUNTER).wrapping_add(self.b(CURRENT_DIGIT));
            self.r.bx = bl as u16;
            let mut al = self.b(V4C7 + self.r.bx) & 0x0F;
            if al >= 0x0A {
                al &= 7;
            }
            al = al.wrapping_add(0x1E);
            self.r.set_al(al);
            if al == self.b(CTRL_KEY) {
                self.incb(DIGIT_COUNTER);
                self.host.tick_wait(&self.st);
                self.play_click();
                self.cf = false;
                return;
            }
        }
        // loc_214
        self.setb(DIGIT_COUNTER, 0);
        self.setb(DOOR_OPEN_ATTEMPTS, 0);
        self.cf = true;
    }

    /// forceOpenDoor (sub_52): use the tool kit.
    fn force_open_door(&mut self) {
        if self.b(TOOLKITS) != 0 && self.b(DOOR_OPEN_ATTEMPTS) == 0 {
            self.incb(DOOR_OPEN_ATTEMPTS);
            loop {
                self.get_rand();
                if !self.zf {
                    break;
                }
            }
            if self.sf {
                self.r.bx = 4;
                self.locate_and_print(); // "Still locked !"
                self.setb(DIGIT_COUNTER, 0);
            } else {
                self.r.set_al(0x0A);
                self.sub_73();
                self.r.bx = 3;
                self.locate_and_print(); // "Opening door..."
            }
        }
        self.cf = false;
    }

    fn sub_55(&mut self) {
        let al = self.b(V5A4 + 1 + self.r.si).wrapping_sub(0x0A);
        self.r.set_al(al);
        self.setb(CURRENT_DIGIT, al);
    }

    /// check0thru9: CF clear if ctrlKey is a digit key.
    fn check_0thru9(&mut self) {
        let k = self.w(CTRL_KEY);
        self.cf = !(0x1E..0x28).contains(&k);
    }

    /// sub_73: start a timed action (AL turns) on object SI.
    pub(super) fn sub_73(&mut self) {
        let al = self.r.al();
        self.setb(V4F0, al);
        let si = self.r.si;
        self.setw(V4F1, si);
    }

    /// multipleItemsFound: ", " between finds.
    fn multiple_items_found(&mut self) {
        if self.b(NUM_ITEMS_FOUND) == 0 {
            return;
        }
        self.r.bx = 0x1D;
        self.print_msg();
    }

    /// countBits: DX = number of set bits in the low 5 bits of AH.
    pub(super) fn count_bits(&mut self) {
        let mut ah = self.r.ah();
        let mut dx = 0u16;
        for _ in 0..5 {
            if ah & 1 != 0 {
                dx += 1;
            }
            ah >>= 1;
        }
        self.r.set_ah(ah);
        self.r.dx = dx;
        self.r.cx = 0;
    }

    /// addMarks: add DL marks (max 100). DL = what doesn't fit.
    /// CF set if everything fitted.
    pub(super) fn add_marks(&mut self) {
        if self.b(MARKS) >= 101 {
            self.cf = false;
            return;
        }
        let m = self.b(MARKS).wrapping_add(self.r.dl());
        self.setb(MARKS, m);
        self.r.set_dl(0);
        if m < 101 {
            self.cf = true;
            return;
        }
        self.r.set_dl(m - 100);
        self.setb(MARKS, 100);
        self.cf = false;
    }

    // ------------------------------------------------------------------
    // using a closet's contents ('u')

    fn loc_253(&mut self) {
        self.r.bx = 2;
        self.locate_and_print();
        let si = self.r.si;
        let c = self.b(V599 + si);
        if c == 0x0E {
            return;
        }
        if c == 0x0A {
            if self.b(ALARM_BOX_ARMED) == 0 {
                self.loc_174();
            }
            return;
        }
        self.sub_72();
        let mut to_258 = self.b(V18D5) != 0;
        if !to_258 {
            if self.b(V59B + si) != 0 {
                return;
            }
            if self.b(V599 + si) != 0 {
                if self.b(NUM_BOMBS) == 0 {
                    to_258 = true;
                } else {
                    self.loc_260();
                    return;
                }
            } else {
                // loc_257
                if self.b(NUM_BOMBS) >= 3 {
                    self.r.bx = 0x0A;
                    self.locate_and_print(); // "It's empty"
                    return;
                }
                to_258 = true;
            }
        }
        if to_258 {
            self.get_or_drop();
            if self.b(V18D5) != 0 && self.b(V18D4) == 0 {
                return;
            }
        }
        self.loc_260();
    }

    fn loc_260(&mut self) {
        let si = self.r.si;
        let c = self.b(V599 + si);
        if c == 0 {
            self.r.bx = 0x2D;
            self.locate_and_print(); // "Dropping..."
        } else if c == 0x0A {
            self.loc_264(0xFF);
            return;
        } else {
            self.r.bx = 0x2E;
            self.locate_and_print(); // "Getting - "
            self.print_closet_contents();
            self.r.bx = 0x2F;
            self.print_msg(); // "..."
        }
        // loc_262
        let si = self.r.si;
        let al = self.b(V599 + si);
        self.r.ax = al as u16;
        self.r.di = self.r.ax;
        let al = self.b(V2CEC + self.r.di);
        self.r.set_al(al);
        self.sub_73();
        self.loc_264(0xFF);
    }

    fn loc_264(&mut self, al: u8) {
        self.r.set_al(al);
        let si = self.r.si;
        self.setb(V59B + 1 + si, al);
    }

    /// loc_265: the timed "Getting/Dropping" action finished.
    fn loc_265(&mut self) {
        let si = self.r.si;
        if self.b(V59B + si) != 0 {
            self.loc_264(0);
            return;
        }
        if self.b(V599 + si) == 10 {
            if self.b(ALARM_BOX_ARMED) == 0 {
                return;
            }
            self.alarmboxstuff();
            self.setb(V18D4, 0);
        } else {
            // loc_268
            let al = self.b(V599 + si);
            self.r.ax = (al as u16) << 1;
            if self.b(V18D4) != 0 {
                let ax = self.r.ax;
                self.clear_inventory();
                self.r.bx = 0x4E;
                self.locate_and_print(); // "Done !"
                self.r.ax = ax;
            }
            self.r.di = self.r.ax;
            let target = self.w(V2CFC + self.r.di);
            self.call_code(target);
            let si = self.r.si;
            if self.b(V59A + si) == 0 {
                self.setb(V599 + si, 0);
            }
        }
        // loc_270
        let si = self.r.si;
        self.setw(V4F1, 0);
        self.setb(V4F0, 0);
        self.setb(V59B + 1 + si, 0);
        if self.b(V18D4) == 0 {
            return;
        }
        if self.b(V599 + si) == 0 {
            self.incb(NUM_BOMBS);
        }
        self.sub_71();
    }

    /// getOrDrop (sub_69): ask which item to drop / swap.
    fn get_or_drop(&mut self) {
        self.r.bx = 2;
        self.locate_and_print();
        let si = self.r.si;
        let al = self.b(V599 + si);
        self.r.set_al(al);
        if al == 0 {
            self.r.bx = 0x31;
            self.print_msg(); // "Drop"
        } else {
            if al < 0x0B || al == 0x0E {
                self.setb(V18D4, 0);
                return;
            }
            self.r.bx = 0x46;
            self.locate_and_print();
            self.r.bx = 0x30;
            self.print_msg(); // "Load too heavy, swap with"
        }
        // loc_274
        self.r.bx = 0x32;
        self.print_msg(); // " item (1,2,3)"
        self.full_inventory();
        self.setb(V18D5, 0xFF);
        let ck = self.w(CTRL_KEY);
        if ck < 0x1F || ck >= 0x22 {
            self.setb(V18D4, 0);
            return;
        }
        self.r.dx = ck;
        let dl = (ck as u8).wrapping_sub(0x1E);
        self.r.set_dl(dl);
        self.setb(V18D4, 0xFF);
        if dl == 0 {
            self.setb(V18D4, 0);
            return;
        }
        let mut di: u16 = 0;
        loop {
            if dl == self.b(V1F40 + 1 + di) {
                // loc_277
                let al = self.b(V2C8C + di);
                self.r.set_al(al);
                self.setb(V2AD2, al);
                self.setw(V2AD3, di);
                di <<= 1;
                self.r.di = di;
                self.setw(V2AD0, di);
                return;
            }
            di += 1;
            if di >= 4 {
                break;
            }
        }
        self.r.di = di;
        self.setb(V18D4, 0);
    }

    /// clearInventory (sub_70)
    fn clear_inventory(&mut self) {
        let di = self.w(V2AD0);
        self.r.di = di;
        self.r.set_al(0);
        let target = self.w(V2C84 + di);
        self.call_code(target);
    }

    fn sub_71(&mut self) {
        let si = self.r.si;
        let al = self.b(V2AD2);
        self.r.set_al(al);
        self.setb(V599 + si, al);
        self.setb(V18D5, 0);
        self.setb(V18D4, 0);
        self.setb(V59A + si, 1);
    }

    pub(super) fn no_daggers(&mut self) {
        let al = self.r.al();
        self.setb(V4D9, al);
        self.setb(NUM_DAGGERS, al);
    }
    pub(super) fn no_toolkits(&mut self) {
        let al = self.r.al();
        self.setb(TOOLKITS, al);
    }
    pub(super) fn no_aidkits(&mut self) {
        let al = self.r.al();
        self.setb(AID_KITS, al);
        self.setb(V504, al);
    }
    pub(super) fn v2ca2(&mut self) {
        let al = self.r.al();
        self.setb(BOMB_SET, al);
        self.setb(V10B7, 0x20);
        self.setb(V10B9, 0x20);
        self.r.dx = 0;
        self.locate();
        self.r.bx = 0x50;
        self.print_msg();
    }

    // ---- closet contents (v2cfc table)

    /// bullets
    pub(super) fn v2d1c(&mut self) {
        let si = self.r.si;
        let mut al = 10u8.wrapping_sub(self.b(BULLETS));
        if al != 0 {
            if al < self.b(V59A + si) {
                let ah = al;
                al = al.wrapping_sub(self.b(V59A + si));
                al = (al ^ 0xFF).wrapping_add(1);
                self.setb(V59A + si, al);
                al = ah;
                self.r.set_ah(ah);
            } else {
                al = self.b(V59A + si);
                self.setb(V59A + si, 0);
            }
            let b = self.b(BULLETS).wrapping_add(al);
            self.setb(BULLETS, b);
        }
        self.r.set_al(al);
        self.r.bx = 0x33;
        self.locate_and_print(); // "You have "
        self.r.dx = self.b(BULLETS) as u16;
        self.r.bx = 0x34;
        self.print_msg();
    }

    /// first aid kit
    pub(super) fn v2d5b(&mut self) {
        if self.b(AID_KITS) != 0 {
            self.pickup_item();
            return;
        }
        let mut al = self.get_rand() & 3;
        if al == 0 {
            al = 2;
        }
        self.r.set_al(al);
        self.setb(AID_KITS, al);
        let si = self.r.si;
        self.setb(V59A + si, 0);
        self.r.bx = 0x33;
        self.locate_and_print();
        self.r.bx = 0x35;
        self.print_msg();
        self.loc_289();
    }

    /// tool kit
    pub(super) fn v2d85(&mut self) {
        if self.b(TOOLKITS) != 0 {
            self.pickup_item();
            return;
        }
        self.r.bx = 0x33;
        self.locate_and_print();
        self.r.bx = 0x36;
        self.print_msg();
        self.setb(TOOLKITS, 0xFF);
        let si = self.r.si;
        self.setb(V59A + si, 0);
        self.loc_289();
    }

    /// passes
    pub(super) fn v2da8(&mut self) {
        let si = self.r.si;
        let al = self.b(PASSES) | self.b(V59A + si);
        self.r.set_al(al);
        if al == self.b(PASSES) {
            self.loc_294();
            return;
        }
        let ah = al ^ self.b(PASSES);
        let v = self.b(V59A + si) ^ ah;
        self.setb(V59A + si, v);
        self.r.set_ah(ah);
        let ax = self.r.ax;
        self.r.bx = 0x33;
        self.locate_and_print();
        self.r.ax = ax;
        self.setb(PASSES, al);
        self.r.set_ah(al);
        self.count_bits();
        self.r.bx = 0x1C;
        self.print_msg();
        self.print_passes();
    }

    /// dagger
    pub(super) fn v2ddb(&mut self) {
        if self.b(NUM_DAGGERS) != 0 {
            self.pickup_item();
            return;
        }
        self.setb(NUM_DAGGERS, 0xFF);
        let si = self.r.si;
        self.setb(V59A + si, 0);
        self.r.bx = 0x33;
        self.locate_and_print();
        self.r.bx = 0x37;
        self.print_msg();
        self.loc_289();
    }

    /// keys
    pub(super) fn v2dfe(&mut self) {
        let al = self.rand5bits();
        let k = self.b(NUM_KEYS) | al;
        self.setb(NUM_KEYS, k);
        self.r.bx = 0x33;
        self.locate_and_print();
        self.r.bx = 0x38;
        self.print_msg();
        let si = self.r.si;
        self.setb(V59A + si, 0);
    }

    pub(super) fn rand5bits(&mut self) -> u8 {
        loop {
            let a = self.get_rand() & 0x1F;
            if a != 0 {
                self.r.set_al(a);
                return a;
            }
        }
    }

    /// the bomb
    pub(super) fn v2e1f(&mut self) {
        let si = self.r.si;
        self.setb(V59A + si, 0);
        self.r.bx = 0x33;
        self.locate_and_print();
        self.r.bx = 0x39;
        self.print_msg();
        self.setb(BOMB_SET, 0x80);
        if self.w(BOMB_TIMER) == 0 {
            self.setw(BOMB_TIMER, 999);
        }
        self.loc_289();
    }

    fn loc_289(&mut self) {
        if self.b(NUM_BOMBS) != 0 {
            self.decb(NUM_BOMBS);
        }
    }

    /// pickupItem (loc_291): you already have one.
    fn pickup_item(&mut self) {
        if self.b(V18D4) != 0 && self.b(NUM_BOMBS) < 3 {
            self.incb(NUM_BOMBS);
        }
        let si = self.r.si;
        self.setb(V59A + si, 1);
        let al = self.b(V599 + si);
        self.r.set_al(al);
        self.setb(V2AD2, al);
        if self.b(V18D4) != 0 {
            let di = self.w(V2AD3);
            self.r.di = di;
            let al = if di == 3 { 0x80 } else { 1 };
            self.r.set_al(al);
            self.setb(NUM_DAGGERS + di, al);
        }
        self.loc_294();
    }

    fn loc_294(&mut self) {
        self.r.bx = 0x3A;
        self.locate_and_print(); // "You already have that !"
    }

    /// money
    pub(super) fn get_marks(&mut self) {
        let si = self.r.si;
        let dl = self.b(V59A + si);
        self.r.set_dl(dl);
        self.add_marks();
        let dl = self.r.dl();
        self.setb(V59A + si, dl);
        if self.cf {
            self.r.bx = 0x33;
            self.locate_and_print();
            self.r.bx = 0x44;
            self.print_msg(); // "the dough !"
        } else {
            self.r.bx = 0x45;
            self.locate_and_print(); // "Your wallet is full !"
        }
    }

    fn loc_296(&mut self, bx: u16) {
        self.r.bx = bx;
        self.get_rand();
        if !self.sf {
            self.r.bx += 1;
        }
        self.locate_and_print();
    }
    pub(super) fn cant_get(&mut self) {
        self.loc_296(0x3B);
    }
    pub(super) fn print_owner(&mut self) {
        self.loc_296(0x3E);
    }
    pub(super) fn bad_paintings(&mut self) {
        self.loc_296(0x40);
    }
    pub(super) fn cant_read(&mut self) {
        self.loc_296(0x42);
    }

    // ---- per-object handlers in the other tables

    /// sub_58: walking into object SI. CF set = blocked.
    pub(super) fn sub_58(&mut self) {
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        let al = self.b(V597 + si) >> 3;
        self.r.ax = al as u16;
        self.r.di = al as u16;
        let target = self.w(V1DA9 + self.r.di);
        self.call_code(target);
        self.r.si = si;
    }

    /// v1d83: a closet with a secret passage: walk through it.
    pub(super) fn v1d83(&mut self) {
        let si = self.r.si;
        let al = self.b(V5A1 + si);
        self.r.set_al(al);
        if al != self.b(V4DC) && self.b(V599 + si) == 0x0E && self.b(V59B + si) == 0 {
            self.r.ax = self.r.bx.wrapping_add(1);
            if self.r.al() == self.b(V4E5) {
                self.loc_298();
            }
        }
        self.cf = true;
    }

    /// v4370: stabbing a guard (dagger, from behind).
    pub(super) fn v4370(&mut self) {
        let si = self.r.si;
        if self.b(V4D9) != 0 {
            self.sub_23();
            if self.r.bl() < 0xC0 {
                self.r.set_bh(0);
                let al = self.b(V54F + self.r.bx);
                self.r.ax = al as u16;
                if self.r.ax == si {
                    self.r.bx = self.r.ax;
                    let al = self.b(V598 + self.r.bx);
                    self.r.set_al(al);
                    if al == self.b(V598 + si) {
                        self.sub_134();
                        self.cf = false;
                        return;
                    }
                }
            }
        }
        self.cf = true;
    }

    /// v1e88: per-turn update of a body.
    pub(super) fn v1e88(&mut self) {
        let si = self.r.si;
        let bl = self.r.bl();
        let al = self.b(si + V59A);
        self.r.set_al(al);
        if al == 0 {
            return;
        }
        if al == 0xFF {
            let d = self.b(V4E5).wrapping_sub(bl);
            self.r.set_al(d);
            if d == 0 || d == 0xF8 {
                return;
            }
            self.loc_228();
            return;
        }
        // somewhereElse
        self.r.di = self.r.ax;
        let a = self.b(self.r.bx + V54F);
        self.r.set_al(a);
        if a == 0 || self.r.ax == si {
            self.loc_228();
            return;
        }
        if self.b(self.r.di + V597) != 0x40 {
            self.loc_196();
        } else {
            self.loc_122();
        }
    }

    fn loc_228(&mut self) {
        let si = self.r.si;
        self.setb(V59A + si, 0);
        self.loc_121();
    }

    /// someProc2: per-turn update of a closet (re-locks when you leave).
    pub(super) fn some_proc2(&mut self) {
        let si = self.r.si;
        if self.b(V59B + si) != 0 {
            return;
        }
        self.r.bx = self.r.bx.wrapping_add(1);
        if self.r.bl() == self.b(V4E5) {
            return;
        }
        self.r.bx = self.r.bx.wrapping_sub(1);
        self.v1ed2();
    }

    /// v1ed2: shut a closet.
    pub(super) fn v1ed2(&mut self) {
        let si = self.r.si;
        self.setb(V59B + si, 0xF0);
        self.r.bx = 2;
        self.locate_and_print();
        let si = self.r.si;
        let al = self.b(V18D2);
        if al != 0 {
            self.setb(V59D + si, al);
        }
        self.r.set_al(0);
        self.setb(V18D5, 0);
        self.setb(V18D4, 0);
        self.setb(DIGIT_COUNTER, 0);
        self.setb(DOOR_OPEN_ATTEMPTS, 0);
        self.setb(ALARM_BOX_ARMED, 0);
        self.setb(V18D2, 0);
        self.setb(V59B + 1 + si, 0);
        self.loc_124();
    }

    /// actionBranch (sub_46): leaving the room, every object gets a say.
    pub(super) fn actionbranch(&mut self) {
        let mut si: u16 = 0x80;
        loop {
            self.r.si = si;
            let bl = self.b(V598 + si);
            self.r.set_bl(bl);
            let al = self.b(V597 + si) >> 3;
            self.r.ax = al as u16;
            self.r.di = al as u16;
            let target = self.w(V1967 + self.r.di);
            self.call_code(target);
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
        self.r.si = si;
        // loc_602
        self.guard_alert();
        self.setw(V4789, 0);
        self.setw(V4787, 0);
    }

    /// xlatDir: reverse direction AL.
    pub(super) fn xlat_dir(&mut self) {
        let al = self.r.al();
        let v = self.b(V8A6.wrapping_add(al as u16));
        self.r.set_al(v);
    }
}
