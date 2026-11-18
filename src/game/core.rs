//! Start-up, title screen, keyboard input, random numbers, text output and
//! the disk/save routines.

use super::*;

pub const ESCAPE: u8 = 0x1B;
pub const CR: u8 = 0x0D;
const NOSB: u16 = 0x01;

impl<H: Host> Game<H> {
    // ------------------------------------------------------------------
    // start-up

    /// checkRequirements: DOS version, memory, CPU type, Windows detection.
    /// On a modern machine every check passes: cpuType stays 0 ("fast CPU"),
    /// and the IN_WIN flag stays clear.
    pub(super) fn check_requirements(&mut self) {
        // memAlloc results; the values are irrelevant (buffers are Vecs).
        self.setw(BUF_SEG900, 0x4000);
        self.setw(BUF_SEG_D00, 0x4400);
    }

    /// checkCommandLine: "-nosb" and "-quiet".
    pub(super) fn check_command_line(&mut self, o: &Options) {
        if o.no_sb {
            let f = self.w(FLAGS) | NOSB;
            self.setw(FLAGS, f);
        }
        if o.quiet {
            self.setb(SOUND_ENABLED, 0);
        }
    }

    /// sbLocate (+ moveSamples): the port always has a "Sound Blaster"
    /// unless -nosb was given.
    pub(super) fn sb_locate(&mut self) {
        if self.w(FLAGS) & NOSB != 0 {
            return;
        }
        self.setw(SBBASE, 0x220);
        self.st.sb = true;
        // sbLocate plays a 50-sample blip to discover the IRQ and waits.
        for _ in 0..3 {
            self.host.tick_wait(&self.st);
        }
        self.setb(SAMPLE_PLAYING, 0);
    }

    pub(super) fn sb_present(&self) -> bool {
        self.w(SBBASE) != 0xFFFF
    }

    /// exitProgram
    pub(super) fn exit_program(&mut self) {
        let st = &mut self.st;
        st.bios.set_mode(&mut st.vram, 3);
        self.host.present(&self.st);
    }

    /// startGame
    pub(super) fn start_game(&mut self) {
        self.copy_sectors_900();
        self.copy_sectors_d00();
        self.host.key_flush();
        self.r.set_al(CR);
        self.set_mode(4);
        jump(Jump::Title);
    }

    pub(super) fn set_mode(&mut self, m: u8) {
        let st = &mut self.st;
        st.bios.set_mode(&mut st.vram, m);
        self.host.present(&self.st);
    }

    /// loc_411: title screen. Waits ~5.5 s for Return, otherwise runs the demo.
    pub(super) fn loc_411(&mut self) {
        let st = &mut self.st;
        st.vram.copy_from_slice(&LOGO_IMAGE[..0x4000]);
        self.host.present(&self.st);
        self.r.cx = 100;
        let mut key = 0;
        loop {
            let k = self.get_intro_key();
            if k == CR {
                key = CR;
                break;
            }
            self.host.tick_wait(&self.st);
            self.r.cx = self.r.cx.wrapping_sub(1);
            if self.r.cx == 0 {
                break;
            }
        }
        self.r.set_al(key);
        self.loc_8();
    }

    /// getIntroKey: INT 21h AH=06h DL=FFh
    pub(super) fn get_intro_key(&mut self) -> u8 {
        let k = self.host.key_dos(&self.st).unwrap_or(0);
        self.r.set_al(k);
        k
    }

    /// loc_8: start a game (AL=CR) or the demo (AL=0).
    pub(super) fn loc_8(&mut self) {
        let al = self.r.al();
        self.setb(START_KEY, al);
        // the bunker data reloaded from the "disk" copy
        self.restore_sectors_d00();
        self.set_mode(4);
        // F000:FFFE machine ID byte: not a PCjr
        if self.rom[0x7FFE] == 0xFD {
            self.incb(CPU_TYPE);
        }
        self.new_game_prep();
        let al = self.b(ROOM_INDEX);
        self.load_room_data(al);
        self.sub_79();
        self.refresh_screen();
        self.main_loop();
    }

    /// quitNReboot (ESC during a game): save and wait for space.
    pub(super) fn quit_n_reboot(&mut self) {
        self.setb(RESUME_GAME, 0xFF);
        let al = self.b(ROOM_INDEX);
        self.save_room_data(al);
        self.save_variables();
        self.save_game_file();
        self.r.ax = 4;
        self.set_mode(4);
        self.r.bx = 102;
        self.print_msg();
        loop {
            let k = self.host.key_get(&self.st);
            self.r.ax = k;
            if k as u8 == b' ' {
                break;
            }
        }
        jump(Jump::Title);
    }

    /// endOfGame (loc_404): final screen when the mission is complete.
    pub(super) fn end_of_game(&mut self) -> ! {
        if self.b(RESUME_GAME) == 0x3C {
            let st = &mut self.st;
            st.vram.copy_from_slice(
                &LOGO_IMAGE[LOGO_FINAL_SCREEN as usize..LOGO_FINAL_SCREEN as usize + 0x4000],
            );
            self.host.present(&self.st);
            self.r.cx = 100;
            loop {
                let k = self.get_intro_key();
                if k == CR {
                    break;
                }
                self.host.tick_wait(&self.st);
                self.r.cx = self.r.cx.wrapping_sub(1);
                if self.r.cx == 0 {
                    break;
                }
            }
            let al = self.b(SKILL_LEVEL).wrapping_add(1);
            if al < 5 {
                self.setb(SKILL_LEVEL, al);
            }
        }
        // loc_407
        self.cls();
        self.r.dx = 0;
        self.locate();
        self.r.bx = 0x68;
        if self.b(RESUME_GAME) != 0x3C {
            self.r.bx += 2;
        }
        let bx = self.r.bx;
        self.print_msg();
        self.r.bx = bx + 1;
        self.print_msg();
        loop {
            let k = self.host.key_get(&self.st);
            self.r.ax = k;
            if k as u8 == 0x0D {
                break;
            }
        }
        jump(Jump::StartGame);
    }

    /// caught: "They got you !"
    pub(super) fn caught(&mut self) -> ! {
        self.cls();
        self.r.dx = 0x0C0E;
        self.locate();
        self.r.bx = 0x5D;
        self.print_msg();
        self.loc_634()
    }

    /// loc_634: pause, then back to the title screen.
    pub(super) fn loc_634(&mut self) -> ! {
        self.r.bx = 50;
        loop {
            self.host.tick_wait(&self.st);
            self.r.bx = self.r.bx.wrapping_sub(1);
            if self.r.bx == 0 {
                break;
            }
        }
        jump(Jump::Title);
    }

    // ------------------------------------------------------------------
    // keyboard

    /// INT 16h AH=01h. Returns ZF semantics: Some(key) means ZF=0.
    pub(super) fn int16_peek(&mut self) -> Option<u16> {
        let k = self.host.key_peek(&self.st);
        if let Some(k) = k {
            self.r.ax = k;
            self.zf = false;
        } else {
            self.zf = true;
        }
        k
    }

    /// INT 16h AH=00h
    pub(super) fn int16_get(&mut self) -> u16 {
        let k = self.host.key_get(&self.st);
        self.r.ax = k;
        k
    }

    /// getakey: drain the keyboard buffer.
    pub(super) fn getakey(&mut self) {
        loop {
            self.r.set_ah(1);
            if self.int16_peek().is_none() {
                return;
            }
            self.r.set_ah(0);
            self.int16_get();
        }
    }

    /// kybdJoystick: read one command, from the keyboard or (in the demo)
    /// from the recorded key list.
    pub(super) fn kybd_joystick(&mut self) {
        if self.b(START_KEY) != 0 {
            // demo mode: only ESC gets you out
            self.r.set_ah(1);
            if self.int16_peek().is_some() {
                self.r.set_ah(0);
                let k = self.int16_get();
                if k as u8 == ESCAPE {
                    jump(Jump::Title);
                }
            }
            // loc_65
            self.setw(CTRL_KEY, 0);
            let mut si = self.w(DEMO_KEY_PTR);
            self.r.bx = self.w(V4EE);
            let dw = |g: &Self, o: u16| {
                g.ds[o as usize] as u16 | ((g.ds[o.wrapping_add(1) as usize] as u16) << 8)
            };
            if self.r.bx == dw(self, si) {
                si += 2;
                let mut al = self.ds[si as usize];
                si += 1;
                while self.r.bx == dw(self, si) {
                    si = si.wrapping_add(3);
                    if si == 0 {
                        break;
                    }
                }
                self.setw(DEMO_KEY_PTR, si);
                al &= 0x7F;
                if al >= 0x40 {
                    al = al.wrapping_add(0x20);
                }
                self.r.set_al(al);
                self.r.si = si;
                self.check_key();
            } else {
                self.r.si = si;
            }
            return;
        }
        // loc_70
        self.setw(CTRL_KEY, 0);
        self.joystick_init();
        if self.b(CODE_MODIFY) == 0x72 && !self.cf {
            // the self-modified JB: apply the joystick direction
            let bl = self.r.bl();
            self.r.set_al(bl);
            self.loc_74();
        }
        // loc_71
        self.r.set_ah(1);
        if self.int16_peek().is_some() {
            self.r.set_ah(0);
            self.int16_get();
            self.check_key();
        }
    }

    /// checkKey: AL = key. Sets moveDir, gunDir or ctrlKey.
    pub(super) fn check_key(&mut self) {
        let al = self.r.al();
        let mut bx: u16 = 0x27;
        loop {
            if al == self.b(ALLOWED_KEYS + bx) {
                self.r.bx = bx;
                self.loc_74();
                return;
            }
            bx = bx.wrapping_sub(1);
            if bx & 0x8000 != 0 {
                break;
            }
        }
        self.r.bx = bx;
    }

    pub(super) fn loc_74(&mut self) {
        let bx = self.r.bx;
        if bx >= 0x11 {
            self.setw(CTRL_KEY, bx);
            return;
        }
        let cl = self.b(DIRECTIONS + bx);
        self.r.set_cl(cl);
        if cl & 0x80 != 0 {
            self.setb(GUN_DIR, cl & 0x7F);
        } else {
            self.setb(MOVE_DIR, cl);
        }
    }

    /// joystickInit with no game port fitted (port 201h reads FFh).
    pub(super) fn joystick_init(&mut self) {
        let mut bl: u8;
        // axis bits 0/1: both sides read "1"
        bl = 3; // loc_80
                // loc_82
        bl -= 1;
        if bl != 6 {
            bl -= 2;
            if bl != 5 {
                bl = 4;
            }
        }
        // loc_84: buttons not pressed
        self.r.ax = 0x1020;
        self.r.cx = 0;
        self.r.dx = 0x201;
        self.r.set_bl(bl);
        // cmp bl,cs:[v733] / mov [v733],bl
        let same = bl == self.b(V733);
        self.setb(V733, bl);
        self.cf = same;
    }

    // ------------------------------------------------------------------
    // random numbers

    /// getRand: AL = random byte, ZF/SF from AL.
    pub(super) fn get_rand(&mut self) -> u8 {
        let al;
        if self.b(START_KEY) != 0 {
            // demo: canned random numbers from the data segment
            let si = self.w(RND_PTR);
            self.setw(RND_PTR, si.wrapping_add(1));
            al = self.ds[si as usize];
            self.r.set_al(al);
        } else {
            let si = self.w(SEED);
            let ax = self.rom[si as usize] as u16
                | ((self.rom[si.wrapping_add(1) as usize] as u16) << 8);
            self.r.ax = ax;
            al = ax as u8;
            loop {
                let s = self.w(SEED).wrapping_add(1567);
                self.setw(SEED, s);
                if s != 0xFFFF {
                    break;
                }
            }
        }
        self.zf = al == 0;
        self.sf = al & 0x80 != 0;
        al
    }

    // ------------------------------------------------------------------
    // text output

    /// locate: DX = row/col
    pub(super) fn locate(&mut self) {
        let dx = self.r.dx;
        self.st.bios.set_cursor((dx >> 8) as u8, dx as u8);
        self.r.set_ah(2);
    }

    pub(super) fn teletype(&mut self, ch: u8, color: u8) {
        let st = &mut self.st;
        st.bios.teletype(&mut st.vram, ch, color);
    }

    /// locateAndPrint: clear the status line, then print message BX at 16h,08h.
    pub(super) fn locate_and_print(&mut self) {
        let (si, di, bx, dx) = (self.r.si, self.r.di, self.r.bx, self.r.dx);
        self.r.dx = 0;
        self.r.di = 0x1B80;
        self.r.cx = 0x228;
        self.put_sprite();
        self.r.si = si;
        self.r.di = di;
        self.r.bx = bx;
        self.r.dx = 0x1608;
        self.locate();
        self.r.bx = bx;
        self.r.dx = dx;
        self.print_msg();
    }

    /// printMsg: BL = message number, DX = number for an FFh placeholder.
    pub(super) fn print_msg(&mut self) {
        let (save_si, save_di) = (self.r.si, self.r.di);
        let mut si = MESSAGES;
        let mut bl = self.r.bl();
        if bl != 0 {
            loop {
                let len = self.b(si) as u16;
                si = si.wrapping_add(len);
                bl = bl.wrapping_sub(1);
                if bl == 0 {
                    break;
                }
            }
        }
        self.r.set_bl(bl);
        let len = self.b(si);
        si += 1;
        self.r.ax = len as u16;
        self.r.cx = (len as u16).wrapping_sub(1);
        loop {
            let al = self.b(si);
            si += 1;
            self.r.set_al(al);
            if al == 0xFF {
                let (dx, cx) = (self.r.dx, self.r.cx);
                self.print_decimal();
                self.r.dx = dx;
                self.r.cx = cx;
                self.r.bx = 3;
            } else {
                self.r.set_ah(0x0E);
                self.r.bx = 3;
                self.teletype(al, 3);
            }
            self.r.cx = self.r.cx.wrapping_sub(1);
            if self.r.cx == 0 {
                break;
            }
        }
        self.r.si = save_si;
        self.r.di = save_di;
    }

    /// printDecimal: DX in decimal, colour 1.
    pub(super) fn print_decimal(&mut self) {
        let mut bx: u16 = 8;
        self.setb(REMAINDER, 0);
        loop {
            let mut cx: u16 = 0;
            let d = self.w(DEC_LOOKUP + bx);
            while self.r.dx >= d {
                self.r.dx -= d;
                cx += 1;
            }
            if cx != 0 || self.b(REMAINDER) != 0 || bx == 0 {
                self.incb(REMAINDER);
                cx |= 0x30;
                self.r.ax = 0x0E00 | (cx & 0xFF);
                self.teletype(cx as u8, 1);
            }
            self.r.cx = cx;
            bx = bx.wrapping_sub(2);
            if bx & 0x8000 != 0 {
                break;
            }
        }
        self.r.bx = bx;
    }

    // ------------------------------------------------------------------
    // coordinates

    /// decodeBCD: BL = room cell (row*8+col) -> DL = col, DH = row.
    pub(super) fn decode_bcd(&mut self) {
        let bl = self.r.bl();
        self.r.set_dl(bl & 7);
        self.r.set_dh(bl >> 3);
    }

    /// getScreenPos: BL = cell -> DI = screen offset of the row, DX = byte
    /// column; also stored in xpos/ypos.
    pub(super) fn get_screen_pos(&mut self) {
        self.decode_bcd();
        let dl = (self.r.dl() << 2).wrapping_add(5);
        let dh = (self.r.dh() << 1).wrapping_add(2);
        let mut ax = (dh as u16) << 6;
        ax = ax.wrapping_add((dh as u16) << 8);
        self.r.ax = ax;
        self.r.di = ax;
        self.r.dx = (dl as u16) << 1;
        let dx = self.r.dx;
        self.setw(XPOS, dx);
        self.setw(YPOS, ax);
    }

    // ------------------------------------------------------------------
    // the "disk": room buffer, variables and files

    /// copySectors900: keep a pristine copy of the bunker buffer.
    pub(super) fn copy_sectors_900(&mut self) {
        let st = &mut self.st;
        st.buf900
            .copy_from_slice(&st.ds[SEG900 + 4..SEG900 + 4 + 0x4000]);
    }
    pub(super) fn restore_sectors_900(&mut self) {
        let st = &mut self.st;
        st.ds[SEG900 + 4..SEG900 + 4 + 0x4000].copy_from_slice(&st.buf900);
    }
    pub(super) fn copy_sectors_d00(&mut self) {
        let st = &mut self.st;
        st.buf_d00.copy_from_slice(&st.ds[..0x3000]);
    }
    pub(super) fn restore_sectors_d00(&mut self) {
        let st = &mut self.st;
        st.ds[..0x3000].copy_from_slice(&st.buf_d00);
    }

    /// restoreVariables: 240 bytes from 900:4 to numBombs...
    pub(super) fn restore_variables(&mut self) {
        for i in 0..0xF0u16 {
            let v = self.s9(i + 4);
            self.setb(NUM_BOMBS + i, v);
        }
        self.r.si = 0xF0;
        self.r.cx = 0;
    }
    pub(super) fn save_variables(&mut self) {
        for i in 0..0xF0u16 {
            let v = self.b(NUM_BOMBS + i);
            self.set_s9(i + 4, v);
        }
        self.r.si = 0xF0;
        self.r.cx = 0;
    }

    /// resetVars: restoreVariables but keep the skill level.
    pub(super) fn reset_vars(&mut self) {
        let al = self.b(SKILL_LEVEL);
        self.restore_variables();
        self.setb(SKILL_LEVEL, al);
    }

    /// saveRoomData: current room (cs:v597, 256 bytes) back to 900:room*256,
    /// then loadRoomData for room AL.
    pub(super) fn save_room_data(&mut self, al: u8) {
        let base = (self.b(ROOM_INDEX) as u16) << 8;
        for i in 0..0x100u16 {
            let v = self.b(V597 + i);
            self.set_s9(base + i, v);
        }
        self.load_room_data(al);
    }

    /// loadRoomData: room AL from 900:AL*256 into cs:v597.
    pub(super) fn load_room_data(&mut self, al: u8) {
        self.setb(ROOM_INDEX, al);
        let base = (al as u16) << 8;
        for i in 0..0x100u16 {
            let v = self.s9(base + i);
            self.setb(V597 + i, v);
        }
        self.r.ax = base;
        self.r.si = base.wrapping_add(0x100);
        self.r.di = V597 + 0x100;
        self.r.cx = 0;
    }

    /// saveGameFile: 900:0..4000h -> savegame.bcw
    pub(super) fn save_game_file(&mut self) {
        let data = self.st.ds[SEG900..SEG900 + 0x4000].to_vec();
        self.host.file_write(GameFile::SaveGame, &data);
    }

    /// loadSavedGame: savegame.bcw -> 900:0 (if it exists)
    pub(super) fn load_saved_game(&mut self) {
        if let Some(d) = self.host.file_read(GameFile::SaveGame) {
            let n = d.len().min(0x4000);
            self.st.ds[SEG900..SEG900 + n].copy_from_slice(&d[..n]);
        }
    }

    /// loadOptionA / loadOptionC: 16 KiB bunker file -> 900:4.
    /// The original quits to DOS if the file is missing.
    pub(super) fn load_option(&mut self, f: GameFile) {
        match self.host.file_read(f) {
            Some(d) => {
                let n = d.len().min(0x4000);
                self.st.ds[SEG900 + 4..SEG900 + 4 + n].copy_from_slice(&d[..n]);
            }
            None => {
                let name = if f == GameFile::OptionA {
                    "optiona.dat"
                } else {
                    "optionc.dat"
                };
                let st = &mut self.st;
                st.bios.set_mode(&mut st.vram, 3);
                self.host.text_message(&format!(
                    "Error, file: {} is missing!\r\n\r\n\
                         Copy it from the original BCW\r\n\
                         DOS release into the folder\r\n\
                         with the program (or use --dir).\r\n\r\n\
                         Press a key to exit.",
                    name
                ));
                self.host.key_get(&self.st);
                jump(Jump::Exit);
            }
        }
    }

    /// saveOptionC. The original opens the file and then calls DOS *read*
    /// instead of write (AH=3Fh), so optionc.dat is never actually updated.
    /// That behaviour is kept: nothing happens.
    pub(super) fn save_option_c(&mut self) {}
}
