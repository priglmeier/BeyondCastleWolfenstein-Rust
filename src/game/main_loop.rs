//! mainLoop and everything it jumps to: player movement, commands, the
//! bomb, and moving between rooms.

use super::*;

impl<H: Host> Game<H> {
    pub(super) fn main_loop(&mut self) -> ! {
        loop {
            self.main_loop_iter();
        }
    }

    /// One pass of `mainLoop` (returns where the original does `jmp mainLoop`).
    fn main_loop_iter(&mut self) {
        if self.b(V4D5) & 0x80 == 0 {
            self.sub_18();
            if !self.cf {
                self.loc_299();
                return;
            }
        }
        // loc_17
        self.r.set_bh(0);
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        let al = self.b(V4EE);
        self.r.set_al(al);
        self.setb(V505 + bl as u16, al);
        if self.b(V4ED) != 0 {
            // walked into a wall
            self.getakey();
            self.sub_26();
            self.print_bombtime();
            self.screen_garbage();
            self.refresh_screen();
            self.r.set_al(0);
            self.setb(V4ED, 0);
            self.setb(MOVE_DIR, 0);
            self.setb(GUN_DIR, 0);
            return;
        }
        // loc_18
        if self.incb(V4EE) == 0 {
            self.sub_76();
            self.incb(V4EF);
        }
        if self.b(V4D3) == 1 {
            self.set_health();
        }
        self.guard_alert();
        self.setb(EASYFLAG, 1);
        self.check_alarm();
        self.kybd_joystick();
        self.getakey();
        self.print_bombtime();

        let bl = self.b(MOVE_DIR);
        self.r.set_bl(bl);
        if bl != self.b(V0D) {
            self.setb(V0E, bl);
        }
        // loc_21 .. loc_27: actions in progress are cancelled by moving
        let mut cancel = false;
        'actions: {
            if self.b(V4F0) != 0 {
                if self.b(MOVE_DIR) != 0 {
                    cancel = true;
                    break 'actions;
                }
                let al = self.b(GUN_DIR);
                self.r.set_al(al);
                if al != self.b(V12) {
                    cancel = true;
                    break 'actions;
                }
                let al = self.b(BULLETS);
                self.r.set_al(al);
                if al != self.b(V13) {
                    cancel = true;
                    break 'actions;
                }
                if self.decb(V4F0) == 0 {
                    let si = self.w(V4F1);
                    self.r.si = si;
                    if si >= 0x80 {
                        self.search_results();
                        break 'actions;
                    }
                }
            }
            // loc_22
            if self.b(V4F8) != 0 {
                if self.b(MOVE_DIR) != 0 {
                    cancel = true;
                    break 'actions;
                }
                if self.decb(V4F8) != 0 {
                    break 'actions;
                }
                self.setw(BOMB_TIMER, 1000);
                self.r.bx = 0x4E; // "Done !"
                self.locate_and_print();
                break 'actions;
            }
            // loc_25
            if self.b(V504) != 0 {
                if self.b(MOVE_DIR) != 0 {
                    cancel = true;
                    break 'actions;
                }
                if self.decb(V504) != 0 {
                    break 'actions;
                }
                self.decb(AID_KITS);
                self.clear_health();
                self.r.bx = 0x4E;
                self.locate_and_print();
                break 'actions;
            }
            // loc_26
            if self.b(V4D8) != 0 {
                if self.b(MOVE_DIR) != 0 {
                    cancel = true;
                    break 'actions;
                }
                self.sub_5();
            }
        }
        if cancel {
            // loc_27
            self.r.bx = 2;
            self.locate_and_print();
            self.r.ax = 0;
            self.setb(V4F0, 0);
            self.setw(V4F1, 0);
            self.setb(V4F8, 0);
            self.setb(V504, 0);
            self.setb(V4D8, 0);
        }
        // loc_28
        let v = self.b(V4D3);
        self.setb(V4D3, v >> 1);
        if v & 1 == 0 {
            let al = self.b(V0E);
            self.r.set_al(al);
            self.setb(V0F, al);
            if al != 0 {
                self.setb(SPEAKER_DELAY, 1);
                self.setw(CLICK_SOUND, 0x400);
                self.footstep();
            }
        }
        // loc_29
        let bl = self.b(V0F);
        self.r.set_bl(bl);
        self.setb(MOVE_DIR, bl);
        self.setb(V0D, bl);
        let h = self.b(HEALTH);
        self.setb(HEALTH, h >> 1);
        if h & 1 == 0 {
            self.sub_27();
            self.r.di = self.w(V4E6);
            self.r.dx = self.w(V4E8);
            self.r.cx = 0x303;
            self.put_sprite();
            let bl = self.b(V4E5);
            self.r.set_bl(bl);
            let al = self.b(MOVE_DIR);
            self.r.ax = al as i8 as i16 as u16;
            self.r.set_bh(0);
            let si = self.r.ax;
            self.r.si = si;
            let bx = self.r.bx;
            let mut al = self.b(V597 + bx) & self.b(V766 + si);
            let blocked = if al != 0 {
                true
            } else {
                al = self.b(V59F + bx) & self.b(V75A + si);
                al != 0
            };
            self.r.set_al(al);
            if blocked {
                self.loc_47();
                return;
            }
            self.sub_25();
        }
        // loc_32
        if self.b(MOVE_DIR) == 0 {
            // loc_34
            if self.w(CTRL_KEY) != 0 {
                self.check_bribe_key();
                let mut bx = self.w(CTRL_KEY);
                self.r.bx = bx;
                if bx != 0 {
                    if bx >= 0x1A {
                        bx = 0x18;
                    }
                    bx = (bx - 0x11) << 1;
                    self.r.bx = bx;
                    let target = self.w(CTRL_ROUTINE + bx);
                    self.call_code(target);
                }
            }
        }
        // loc_36
        if self.w(CTRL_KEY) == 0x17 {
            self.holster();
        }
        if self.w(CTRL_KEY) == 0x19 {
            self.shoot();
        }
        // loc_38
        self.guard_alert();
        self.setb(EASYFLAG, 0);
        self.check_alarm();
        let al = self.b(V0D);
        self.r.set_al(al);
        self.setb(MOVE_DIR, al);
        let v = self.b(V4D3);
        self.setb(V4D3, v >> 1);
        if v & 1 != 0 {
            self.loc_49();
            return;
        }
        // loc_39
        let al = self.b(MOVE_DIR);
        self.r.set_al(al);
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.sub_24();
        let bl = self.r.bl();
        self.setb(V14, bl);
        let dx = self.r.dx;
        self.setw(V10, dx);
        let (dl, dh) = (self.r.dl(), self.r.dh());
        if dl & 0x80 != 0 {
            self.loc_305();
            return;
        }
        if dl >= 8 {
            self.loc_308();
            return;
        }
        if dh & 0x80 != 0 {
            self.loc_309();
            return;
        }
        if dh >= 8 {
            self.loc_310();
            return;
        }
        // loc_43
        self.r.set_ah(0);
        self.r.set_bh(0);
        let bx = self.r.bx;
        let mut go_45 = false;
        let al = self.b(V54F + bx);
        self.r.set_al(al);
        if al != 0 {
            self.r.si = self.r.ax;
            let al = self.b(V597 + self.r.si);
            self.r.set_al(al);
            if al != 0x30 {
                go_45 = true;
            }
        }
        let mut go_46 = false;
        if !go_45 {
            // loc_44
            let al = self.b(V557 + bx);
            self.r.set_al(al);
            if al == 0 {
                go_46 = true;
            } else {
                self.r.si = self.r.ax;
                let al = self.b(V597 + self.r.si);
                self.r.set_al(al);
                if al == 0x30 {
                    go_46 = true;
                }
            }
        }
        if !go_46 {
            // loc_45
            let al = self.r.al();
            if al != 0x40 && al != 0 {
                self.sub_58();
                if self.cf {
                    self.loc_47();
                    return;
                }
            }
        }
        // loc_46
        let bl = self.b(V14);
        self.r.bx = bl as u16;
        let al = self.b(MOVE_DIR);
        self.r.ax = al as i8 as i16 as u16;
        let si = self.r.ax;
        self.r.si = si;
        let bx = self.r.bx;
        let mut al = self.b(V597 + bx) & self.b(si + V742);
        if al == 0 {
            al = self.b(V59F + bx) & self.b(si + V74E);
            self.r.set_al(al);
            if al == 0 {
                // loc_48
                self.setb(V4E5, bl);
                if self.b(MOVE_DIR) != 0 {
                    self.setb(SPEAKER_DELAY, 1);
                    self.setw(CLICK_SOUND, 0x200);
                    self.footstep();
                }
                self.loc_49();
                return;
            }
        }
        self.r.set_al(al);
        self.loc_47();
    }

    /// loc_47: the move is blocked.
    fn loc_47(&mut self) {
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.setb(V14, bl);
        self.decode_bcd();
        let dx = self.r.dx;
        self.setw(V10, dx);
        self.setb(V4ED, 1);
    }

    /// loc_49: redraw the player.
    fn loc_49(&mut self) {
        let h = self.b(HEALTH);
        self.setb(HEALTH, h >> 1);
        if h & 1 != 0 {
            return;
        }
        self.r.di = self.w(V4E6);
        self.r.dx = self.w(V4E8);
        self.r.cx = 0x303;
        self.put_sprite();
        self.sub_26();
    }

    /// Dispatch a code address taken from one of the original jump tables.
    pub(super) fn call_code(&mut self, addr: u16) {
        match addr {
            QUIT_NREBOOT => self.quit_n_reboot(),
            Q_INVENTORY => self.q_inventory(),
            RESET_TIMER => self.reset_timer(),
            SET_BOMB => self.set_bomb(),
            FIRST_AID => self.first_aid(),
            SWITCH_WEAPONS => self.switch_weapons(),
            LOC_RET_53 => {}
            DO_SEARCH => self.do_search(),
            // vc74 (refreshScreen object drawing)
            LOC_RET_143 => {}
            V3A84 => self.v3a84(),
            VD1E => self.vd1e(),
            SOME_PROC3 => self.some_proc3(),
            LOC_121 => self.loc_121(),
            VC82 => self.vc82(),
            VD0B => self.vd0b(),
            // v1967 (actionBranch)
            DFLT_RET => self.cf = false,
            DFLT_CY_RET => self.cf = true,
            LOC_531 => self.loc_531(),
            V1ED2 => self.v1ed2(),
            V450B => self.v450b(),
            // v1da9 (sub_58)
            V4370 => self.v4370(),
            V1D83 => self.v1d83(),
            SUB_50 => self.sub_50(),
            // v1e7a (object update)
            SOME_PROC => self.some_proc(),
            SOMEPROC2 => self.some_proc2(),
            V1E88 => self.v1e88(),
            V455A => self.v455a(),
            // v2cfc (closet contents)
            ARET => {}
            V2D1C => self.v2d1c(),
            GET_MARKS => self.get_marks(),
            V2DA8 => self.v2da8(),
            CANT_GET => self.cant_get(),
            PRINT_OWNER => self.print_owner(),
            BAD_PAINTINGS => self.bad_paintings(),
            V2DFE => self.v2dfe(),
            CANT_READ => self.cant_read(),
            V2DDB => self.v2ddb(),
            V2D5B => self.v2d5b(),
            V2D85 => self.v2d85(),
            V2E1F => self.v2e1f(),
            // v2c84 (clearInventory)
            NO_DAGGERS => self.no_daggers(),
            NO_TOOLKITS => self.no_toolkits(),
            NOAIDKITS => self.no_aidkits(),
            V2CA2 => self.v2ca2(),
            // v46da (printSecretMessage)
            V46EA => self.v46ea(),
            LOC_598 => self.loc_598(),
            V4730 => self.v4730(),
            V4741 => self.v4741(),
            _ => panic!("unknown code address {:04X}", addr),
        }
    }

    // ------------------------------------------------------------------

    /// checkBribeKey: a pass number or 'm' while a guard is waiting.
    fn check_bribe_key(&mut self) {
        if self.b(V4C6) == 0 {
            return;
        }
        let mut bx: u16 = 5;
        self.r.ax = self.w(CTRL_KEY);
        loop {
            if self.r.al() == self.b(V3D + bx) {
                self.r.bx = bx;
                self.bribe_guard();
                self.setw(CTRL_KEY, 0);
                return;
            }
            bx = bx.wrapping_sub(1);
            if bx & 0x8000 != 0 {
                break;
            }
        }
        self.r.bx = bx;
    }

    /// firstAid ('f')
    fn first_aid(&mut self) {
        if self.b(AID_KITS) == 0 {
            return;
        }
        let al = loop {
            let a = self.get_rand() & 0x14;
            if a != 0 {
                break a;
            }
        };
        self.r.set_al(al);
        self.setb(V504, al);
        self.r.bx = 0x51;
        self.locate_and_print();
    }

    /// sub_5: weapon switch countdown. When it completes, the original
    /// calls into the middle of the main loop (print "Done !" and carry on
    /// at loc_28), which is where its caller continues anyway.
    fn sub_5(&mut self) {
        if self.decb(V4D8) != 0 {
            return;
        }
        let v = self.b(V4D9) ^ 0xFF;
        self.setb(V4D9, v);
        self.r.bx = 0x4E;
        self.locate_and_print();
    }

    /// fireGun
    pub(super) fn fire_gun(&mut self) {
        if self.b(BULLETS) == 0 {
            self.r.set_al(0x1D);
            self.speaker_high();
            return;
        }
        let bl = self.b(V4E5);
        self.r.bx = bl as u16;
        self.get_screen_pos();
        let bl = self.b(GUN_DIR);
        self.r.bx = bl as u16;
        let dl = self.r.dl().wrapping_add(self.b(V8B6 + self.r.bx));
        self.r.set_dl(dl);
        self.r.bx <<= 1;
        self.r.di = self.r.di.wrapping_add(self.w(V8C2 + self.r.bx));
        self.modify_sprite();
        let (di, dx) = (self.r.di, self.r.dx);
        self.host.tick_mark();
        self.r.set_al(0x71);
        self.draw2screen(); // the "bang"
        self.r.dx = dx;
        self.r.di = di;
        self.fire_gun_sound();
        self.host.tick_wait_mark(&self.st);
        self.r.set_al(0x1B);
        self.draw2screen(); // erase it
        self.sub_104();
        self.decb(BULLETS);
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        let al = self.b(GUN_DIR);
        self.r.set_al(al);
        self.col_detect();
        let al = self.r.al();
        if al & 0x80 == 0 {
            return;
        }
        self.r.set_ah(0);
        self.r.si = self.r.ax;
        if self.get_rand() < 0x50 {
            return; // missed
        }
        let si = self.r.si;
        let bl = self.b(V598 + si);
        self.r.set_bl(bl);
        let al = self.b(V597 + si);
        self.r.set_al(al);
        match al {
            0 => {}
            0x10 => self.dead_guy(),
            0x20 => self.loc_156(),
            0x50 => self.loc_575(),
            _ => {}
        }
    }

    fn holster(&mut self) {
        self.setb(GUN_DIR, 0);
    }

    /// switchWeapons (':')
    fn switch_weapons(&mut self) {
        if self.b(NUM_DAGGERS) == 0 {
            return;
        }
        self.r.bx = 0x4F;
        self.locate_and_print();
        self.setb(V4D8, 5);
    }

    /// doSearch (space and the digit / tool keys)
    fn do_search(&mut self) {
        let bl = self.b(V4E5);
        self.r.bx = bl as u16;
        self.r.set_ah(0);
        let bx = self.r.bx;
        let mut found = false;
        let al = self.b(V54F + bx);
        self.r.set_al(al);
        if al != 0 {
            self.r.si = self.r.ax;
            if self.b(V597 + self.r.si) != 0x30 {
                found = true;
            }
        }
        if !found {
            // loc_58
            let al = self.b(V557 + bx);
            self.r.set_al(al);
            if al != 0 {
                self.r.si = self.r.ax;
                if self.b(V597 + self.r.si) != 0x30 {
                    found = true;
                }
            }
        }
        if !found {
            // loc_59
            self.sub_23();
            if self.r.bx >= 0xC0 {
                return;
            }
            let al = self.b(V54F + self.r.bx);
            self.r.set_al(al);
            if al == 0 {
                return;
            }
            self.r.si = self.r.ax;
        }
        // loc_60
        let g = self.b(GUN_DIR);
        self.r.set_al(g);
        self.setb(V12, g);
        let b = self.b(BULLETS);
        self.r.set_al(b);
        self.setb(V13, b);
        self.setb(MOVE_DIR, 0);
        self.search_or_body_move();
    }

    /// shoot ('l')
    pub(super) fn shoot(&mut self) {
        if self.b(GUN_DIR) == 0 || self.b(V4D9) != 0 {
            return;
        }
        self.fire_gun();
    }

    /// setBomb ('b'): drop / pick up / arm the bomb.
    fn set_bomb(&mut self) {
        let cl = self.b(V4E5);
        let ch = self.b(ROOM_INDEX);
        self.r.set_cl(cl);
        self.r.set_ch(ch);
        self.r.set_bh(0);
        if self.b(BOMB_SET) & 0x80 != 0 {
            let bl = self.b(V4DE);
            self.r.set_bl(bl);
            let al = self.b(VF02 + self.r.bx);
            self.setb(V4DF, al);
            self.setb(V4F6, cl);
            self.setb(V4F5, ch);
            let al = self.b(V4EB);
            self.r.set_al(al);
            self.setb(V4F7, al);
            self.setb(BOMB_SET, 1);
            self.setb(V10B7, 0x20);
            self.setb(V10B9, 0x20);
            self.sub_72();
            return;
        }
        if self.b(NUM_BOMBS) == 0 {
            self.r.bx = 0x47;
            self.locate_and_print();
            return;
        }
        if self.b(V4F5) != ch || self.b(V4F6) != cl {
            return;
        }
        let bl = self.b(GUN_DIR);
        self.r.set_bl(bl);
        let al = self.b(VF02 + self.r.bx);
        self.r.set_al(al);
        if al != self.b(V4DF) {
            self.setb(GUN_DIR, 0);
        }
        self.setb(BOMB_SET, 0x80);
        self.r.set_al(0);
        self.setb(V4F5, 0);
        self.setb(V4F7, 0);
        self.setb(V4F6, 0);
        self.setb(V4DF, 0);
        self.sub_72();
    }

    /// resetTimer ('r')
    fn reset_timer(&mut self) {
        if self.b(BOMB_SET) & 0x80 != 0 {
            return;
        }
        if self.b(V4F5) != self.b(ROOM_INDEX) || self.b(V4F6) != self.b(V4E5) {
            return;
        }
        let mut al = self.get_rand() & 0x0F;
        if al == 0 {
            al = 0x14;
        }
        self.r.set_al(al);
        self.setb(V4F8, al);
        self.r.bx = 0x4D;
        self.locate_and_print();
    }

    /// sub_72 (loc_278): count free inventory slots into numBombs.
    pub(super) fn sub_72(&mut self) {
        let mut cx: u8 = 3;
        if self.b(NUM_DAGGERS) != 0 {
            cx -= 1;
        }
        if self.b(TOOLKITS) != 0 {
            cx = cx.wrapping_sub(1);
        }
        if self.b(AID_KITS) != 0 {
            cx = cx.wrapping_sub(1);
        }
        if self.b(BOMB_SET) & 0x80 != 0 {
            cx = cx.wrapping_sub(1);
        }
        self.setb(NUM_BOMBS, cx);
    }

    /// qInventory (Return)
    fn q_inventory(&mut self) {
        self.r.dx = self.b(BULLETS) as u16;
        self.r.bx = 0x27;
        self.locate_and_print();
        self.r.dx = self.b(MARKS) as u16;
        self.r.bx = 0x28;
        self.print_msg();
        self.full_inventory();
    }

    /// fullInventory
    pub(super) fn full_inventory(&mut self) {
        self.r.ax = 0;
        self.setw(V1F40, 0);
        self.setw(V1F42, 0);
        self.setb(V1F44, 0);
        self.r.dx = 0x1700;
        self.locate();
        if self.b(NUM_DAGGERS) != 0 {
            let al = self.sub_68();
            self.setb(V1F40 + 1, al);
            self.r.bx = 0x29;
            self.print_msg();
        }
        if self.b(TOOLKITS) != 0 {
            let al = self.sub_68();
            self.setb(V1F42, al);
            self.r.bx = 0x2A;
            self.print_msg();
        }
        if self.b(AID_KITS) != 0 {
            let al = self.sub_68();
            self.setb(V1F42 + 1, al);
            self.r.bx = 0x2B;
            self.print_msg();
        }
        if self.b(BOMB_SET) & 0x80 != 0 {
            let al = self.sub_68();
            self.setb(V1F44, al);
            self.r.bx = 0x2C;
            self.print_msg();
        }
    }

    fn sub_68(&mut self) -> u8 {
        let n = self.incb(V1F40);
        self.r.ax = 0x0E00 | (n | 0x30) as u16;
        self.r.bx = 3;
        self.teletype(n | 0x30, 3);
        self.r.set_al(n);
        n
    }

    // ------------------------------------------------------------------
    // the "exit" / elevator-like door animation in the room (v960-v962)

    /// sub_18. Returns CF clear when the player walked out through it.
    fn sub_18(&mut self) {
        self.setb(V4D2, 0xFF);
        if self.b(V4D6) != 0 {
            if self.b(V4D6) == 1 {
                self.sub_19();
                if self.b(V503) < 0x0A {
                    self.setb(V503, 0);
                    self.setb(V4D2, 0);
                }
            }
            self.decb(V4D6);
            self.cf = true;
            return;
        }
        // loc_92
        if self.b(V962) != 0 {
            let al = self.b(V961);
            self.r.set_al(al);
            if al < 1 {
                // loc_94
                let bl = self.b(V960);
                self.r.set_bl(bl);
                self.sub_34();
                self.setb(V962, 0);
                self.cf = true;
                return;
            }
            if al == 5 {
                let bl = self.b(V960);
                self.r.bx = bl as u16 + 1;
                self.sub_34();
            }
            self.decb(V961);
            self.sub_33();
            self.r.set_al(0x20);
            self.draw2screen();
            self.cf = true;
            return;
        }
        // loc_95
        let al = self.b(V961);
        self.r.set_al(al);
        let mut to_98 = al >= 7;
        if !to_98 {
            if al == 0 {
                let bl = self.b(V4D5);
                self.r.bx = bl as u16 + 8;
                if self.r.bl() == self.b(V4E5) {
                    to_98 = true;
                } else if self.b(V54F + self.r.bx) != 0 {
                    self.sub_20();
                    self.cf = true;
                    return;
                } else {
                    let bl = self.b(V960);
                    self.r.set_bl(bl);
                    self.sub_35();
                }
            } else if al == 4 {
                // loc_96
                self.r.bx = 9;
                let bl = 9u8.wrapping_add(self.b(V4D5));
                self.r.set_bl(bl);
                if bl == self.b(V4E5) {
                    to_98 = true;
                } else if self.b(V54F + self.r.bx) != 0 {
                    self.sub_20();
                    self.cf = true;
                    return;
                } else {
                    let bl = self.b(V960);
                    self.r.set_bl(bl);
                    self.r.bx = self.r.bx.wrapping_add(1);
                    self.sub_35();
                }
            }
            if !to_98 {
                // loc_97
                self.incb(V961);
                self.sub_33();
                self.cf = true;
                return;
            }
        }
        // loc_98
        let bl = self.b(V4D5);
        self.r.set_bl(bl);
        if bl == self.b(V4E5) {
            self.sub_20();
            self.cf = false;
            return;
        }
        self.r.bx = self.r.bx.wrapping_add(1);
        if self.r.bl() == self.b(V4E5) {
            self.sub_20();
            self.cf = false;
            return;
        }
        self.sub_20();
        self.cf = true;
    }

    fn sub_19(&mut self) {
        self.sub_36();
        let bl = self.b(V5E4);
        self.r.set_bl(bl);
        let al = if bl < self.b(ROOM_INDEX) { 0x6A } else { 0x7B };
        self.r.set_al(al);
        self.draw2screen();
    }

    fn sub_20(&mut self) {
        self.sub_36();
        self.r.set_al(0x55);
        self.draw2screen();
        self.setb(V962, 1);
        self.setb(V4D6, 0x0F);
    }

    pub(super) fn sub_34(&mut self) {
        self.r.set_bh(0);
        let bx = self.r.bx;
        let v = self.b(V597 + bx) & !0x20;
        self.setb(V597 + bx, v);
        let v = self.b(V58F + bx) & !0x40;
        self.setb(V58F + bx, v);
    }

    pub(super) fn sub_35(&mut self) {
        self.r.set_bh(0);
        let bx = self.r.bx;
        let v = self.b(V597 + bx) | 0x20;
        self.setb(V597 + bx, v);
        let v = self.b(V58F + bx) | 0x40;
        self.setb(V58F + bx, v);
    }

    pub(super) fn sub_36(&mut self) {
        let bl = self.b(V4D5);
        self.r.bx = bl as u16;
        self.get_screen_pos();
        self.r.dx = self.r.dx.wrapping_add(2);
    }

    // ------------------------------------------------------------------
    // leaving a room

    /// loc_298: taking the stairs (reached from inside sub_58).
    pub(super) fn loc_298(&mut self) -> ! {
        let si = self.r.si;
        let al = self.b(V5A0 + si).wrapping_add(1);
        self.setb(V4E5, al);
        let al = self.b(V5A1 + si);
        self.actionbranch();
        self.save_room_data(al);
        self.r.set_al(0xFF);
        self.loc_312(0xFF);
        jump(Jump::MainLoop);
    }

    /// loc_299: walked out through the room's exit door.
    fn loc_299(&mut self) {
        self.setb(V2ED2, 1);
        let al = self.b(V4E5).wrapping_sub(self.b(V4D5));
        self.setb(V2ED3, al);
        if al != 0 {
            self.setb(V2ED2, 0xFF);
        }
        let al = self.b(ROOM_INDEX);
        self.setb(V2ED1, al);
        self.actionbranch();
        let al = self.b(V5E4);
        self.save_room_data(al);
        let mut bx: u16 = 0x3F;
        loop {
            if self.b(V597 + bx) & 0x10 != 0 {
                break;
            }
            bx = bx.wrapping_sub(1);
            if bx & 0x8000 != 0 {
                bx = 0x24;
                break;
            }
        }
        self.r.bx = bx;
        let al = (bx as u8).wrapping_add(self.b(V2ED3));
        self.setb(V4E5, al);
        self.sub_75();
        self.loc_311();
    }

    fn sub_75(&mut self) {
        let al = self.b(V2ED1);
        if al != self.b(V4F5) {
            return;
        }
        let al = self.b(V4F6).wrapping_sub(self.b(V4D5));
        if al != 0 {
            if al != 1 {
                return;
            }
            self.r.bx = self.r.bx.wrapping_add(1);
        }
        self.setb(V4F6, al);
        let al = self.b(ROOM_INDEX);
        self.setb(V4F5, al);
    }

    /// loc_305: walked off the left edge.
    fn loc_305(&mut self) {
        self.actionbranch();
        let al = self.b(V5E0);
        self.save_room_data(al);
        self.setb(V2ED2, 0);
        self.r.bx = 7;
        self.loc_306(2);
    }

    fn loc_306(&mut self, ah: u8) {
        let mut bx = self.r.bx;
        while self.b(V597 + bx) & ah != 0 {
            bx = bx.wrapping_add(8);
        }
        self.r.bx = bx;
        self.setb(V4E5, bx as u8);
        self.loc_311();
    }

    /// loc_308: right edge.
    fn loc_308(&mut self) {
        self.actionbranch();
        let al = self.b(V5E1);
        self.save_room_data(al);
        self.setb(V2ED2, 0);
        self.r.bx = 0;
        self.loc_306(1);
    }

    /// loc_309: top edge.
    fn loc_309(&mut self) {
        self.actionbranch();
        let bl = self.b(V4E5);
        self.r.set_bl(bl);
        self.sub_77(4);
        let al = self.b(V5E2);
        self.save_room_data(al);
        self.r.bx = 0x40;
        self.sub_78(8);
        let v = self.b(V4E5) | 0x38;
        self.setb(V4E5, v);
        self.loc_311();
    }

    /// loc_310: bottom edge.
    fn loc_310(&mut self) {
        self.actionbranch();
        let bl = self.b(V4E5).wrapping_add(8);
        self.r.set_bl(bl);
        self.sub_77(8);
        let al = self.b(V5E3);
        self.save_room_data(al);
        self.r.bx = 0;
        self.sub_78(4);
        self.loc_311();
    }

    fn loc_311(&mut self) {
        let al = self.b(V4E5);
        self.loc_312(al);
    }

    /// loc_312 .. loc_319: enter the new room.
    fn loc_312(&mut self, al: u8) {
        self.setb(V502, al);
        self.setb(MOVE_DIR, 0);
        self.setw(V4EE, 0);
        self.setb(V2ED3, 0);
        let al = self.b(ROOM_INDEX);
        if al == self.b(V4DC) && self.b(V2ED4) == 0 {
            let mut bx: u16 = 0x12;
            loop {
                self.setb(V597 + bx, 0);
                bx += 1;
                if bx & 7 < 7 {
                    continue;
                }
                bx += 2;
                if bx < 0x25 {
                    continue;
                }
                break;
            }
            self.r.bx = bx;
            self.r.cx = bx & 7;
            self.incb(V2ED4);
        }
        // loc_314
        self.sub_79();
        self.refresh_screen();
        self.setb(ALARM_SOUNDING, 0);
        self.setb(V773, 0xFF);
        self.setb(V18D1, 0);
        self.setb(V4C6, 0);
        if self.b(V4F3) != 0 {
            let v = self.b(V4F3) | 0x80;
            self.setb(V4F3, v);
            self.sub_104();
        }
        self.sub_76();
        let mut bx: u16 = 1;
        let al = self.b(ROOM_INDEX);
        if al >= 0x19 {
            bx += 1;
            if al >= 0x31 {
                bx += 1;
            }
        }
        self.r.bx = bx;
        self.setb(V4EB, bx as u8);
        if al != 1 {
            return;
        }
        // back in room 1 (the exit) after setting the bomb or blowing it up
        let mut al = self.b(V4DC);
        if al == 0 {
            al = self.b(V4F5);
            if al == 0 {
                return;
            }
        }
        if al == 0x3C {
            self.setb(RESUME_GAME, al);
        }
        self.end_of_game();
    }

    pub(super) fn sub_76(&mut self) {
        for si in 0..0x40u16 {
            self.setb(V505 + si, 0);
        }
        self.r.si = 0xFFFF;
        self.r.set_al(0);
    }

    fn sub_77(&mut self, ah: u8) {
        self.r.set_bh(0);
        let bx = self.r.bx;
        let mut dl: u8 = 0;
        let mut dh: u8 = 1;
        self.r.cx = bx & 7;
        let md = self.b(MOVE_DIR) & 0x0C;
        let inc = if self.r.cx == 7 || self.b(V598 + bx) & ah != 0 {
            // loc_321
            md == 0
        } else {
            md != 0
        };
        if inc {
            dl += 1;
            dh = 0xFF;
        }
        self.r.dx = ((dh as u16) << 8) | dl as u16;
        self.r.set_al(md);
        self.setb(V4E5, dl);
        self.setb(V2ED2, dh);
    }

    fn sub_78(&mut self, ah: u8) {
        self.r.set_bh(0);
        let mut bx = self.r.bx;
        let cx = bx + 7;
        self.r.cx = cx;
        loop {
            if self.b(V597 + bx) & ah == 0 {
                break;
            }
            bx += 1;
            if bx == cx {
                bx = 3;
                break;
            }
        }
        bx &= 7;
        self.r.bx = bx;
        let v = self.b(V4E5).wrapping_add(bx as u8);
        self.setb(V4E5, v);
    }

    /// sub_79: close off the edges of the room that lead nowhere.
    pub(super) fn sub_79(&mut self) {
        if self.b(V5E2) == 0 {
            self.r.bx = 0;
            self.sub_81(4);
        }
        if self.b(V5E3) == 0 {
            self.r.bx = 0x40;
            self.sub_81(8);
        }
        if self.b(V5E0) == 0 {
            self.r.bx = 0;
            self.sub_80(1);
        }
        if self.b(V5E1) == 0 {
            self.r.bx = 7;
            self.sub_80(2);
        }
        if self.b(V5E4) == 0 {
            let mut bx: u16 = 0x47;
            loop {
                let v = self.b(bx + V597) & !0x10;
                self.setb(bx + V597, v);
                bx = bx.wrapping_sub(1);
                if bx & 0x8000 != 0 {
                    break;
                }
            }
            self.r.bx = bx;
        }
    }

    fn sub_80(&mut self, ah: u8) {
        let mut bx = self.r.bx;
        loop {
            let v = self.b(V597 + bx) | ah;
            self.setb(V597 + bx, v);
            bx += 8;
            if bx >= 0x50 {
                break;
            }
        }
        self.r.bx = bx;
    }

    fn sub_81(&mut self, ah: u8) {
        let mut bx = self.r.bx;
        loop {
            let v = self.b(V597 + bx) | ah;
            self.setb(V597 + bx, v);
            bx += 1;
            if bx & 7 == 0 {
                break;
            }
        }
        self.r.bx = bx;
        self.r.cx = 0;
    }

    // ------------------------------------------------------------------
    // the bomb

    /// printBombtime (sub_60): bomb countdown, then the per-turn update of
    /// every object in the room (guards etc.), then loc_8999.
    pub(super) fn print_bombtime(&mut self) {
        if self.b(BOMB_SET) != 0 {
            let t = self.w(BOMB_TIMER).wrapping_sub(1);
            self.setw(BOMB_TIMER, t);
            if t == 0 {
                self.r.dx = 0;
                self.locate();
                self.r.bx = 0x50;
                self.print_msg();
                self.detonate_bomb();
                if self.cf {
                    self.loc_634();
                }
            } else {
                self.r.dx = 0x200;
                self.locate();
                self.r.bx = 0x48;
                self.print_msg();
                let mut bx: u16 = 0x49;
                let dx = self.w(BOMB_TIMER);
                self.r.dx = dx;
                if dx < 1000 {
                    bx += 1;
                }
                if dx < 100 {
                    bx += 1;
                }
                if dx < 10 {
                    bx += 1;
                }
                self.r.bx = bx;
                self.print_msg();
            }
        }
        // loc_999
        let mut si: u16 = 0x80;
        loop {
            self.r.si = si;
            let bl = self.b(V598 + si);
            self.r.bx = bl as u16;
            let al = self.b(V597 + si) >> 3;
            self.r.ax = al as u16;
            let di = al as u16;
            self.r.di = di;
            if di > 12 {
                panic!("Fatal error! (object type {:02X})", al << 3);
            }
            let target = self.w(V1E7A + di);
            self.call_code(target);
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
        self.r.si = si;
        self.loc_8999();
    }

    /// loc_8999: while the alarm rings, more guards may come in.
    fn loc_8999(&mut self) {
        let v = self.b(V4F3);
        if v != 0 && v & 0x80 == 0 {
            self.setb(V18D1, 175);
            if self.b(ROOM_INDEX) != 1 && self.b(V503) == 0 {
                self.setb(V384F, 0);
                self.sub_105();
            }
        }
        self.decb(V503);
    }

    /// detonateBomb (sub_59)
    pub(super) fn detonate_bomb(&mut self) {
        self.setb(V4F3, 0);
        self.sub_144();
        let al = self.b(V4F5);
        self.r.set_al(al);
        if al != 0 && al != self.b(ROOM_INDEX) {
            // blastPass
            self.blast();
            let al = self.b(V4F5);
            self.setb(V4DC, al);
            self.r.set_al(0);
            self.setb(V18D1, 0);
            self.setb(BOMB_SET, 0);
            self.setb(V4F5, 0);
            self.setb(V4F6, 0);
            self.setb(V4F7, 0);
            self.setb(V4F3, 1);
            self.sub_104();
            self.cf = false;
            return;
        }
        // blastFail
        self.blast();
        self.cls();
        self.r.dx = 0x0C04;
        self.locate();
        self.r.bx = 0x5C;
        self.print_msg();
        self.cf = true;
    }

    fn sub_144(&mut self) {
        let mut al = self.b(V4F5).wrapping_sub(self.b(ROOM_INDEX));
        if al & 0x80 != 0 {
            al = (al ^ 0xFF).wrapping_add(1);
        }
        let mut bx: u16 = 0;
        if al >= 25 {
            bx += 1;
            if al >= 49 {
                bx += 1;
            }
        }
        self.r.bx = bx;
        let al = self.b(V4798 + bx);
        self.r.set_al(al);
        self.setb(V18D1, al);
    }
}
