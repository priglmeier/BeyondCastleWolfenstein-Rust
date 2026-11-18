//! Sound: PC speaker effects (timed with the original's I/O-port delay
//! loops), Sound Blaster samples, the guards' voices, and the alarm bell.
//! The alarm/voice loops also poll the keyboard for the fire key, exactly
//! as often as the original.

use super::*;

const DEFAULT_PITCH: u8 = 211;

#[derive(Clone, Copy)]
pub(super) enum SbSeg {
    One,
    Two,
}

impl<H: Host> Game<H> {
    /// speaker_toggle
    pub(super) fn speaker_toggle(&mut self) {
        if self.b(SOUND_ENABLED) != 0 {
            self.host.speaker_toggle();
        }
    }

    /// playSample: CX samples from sample segment `seg` at offset BX.
    pub(super) fn play_sample(&mut self, seg: SbSeg, bx: u16, count: u16) {
        let mut cx = count.wrapping_sub(1);
        if (bx as u32) + (cx as u32) > 0xFFFF {
            // the DMA page-boundary guard
            cx = bx.wrapping_add(cx).wrapping_sub(1);
        }
        let img = match seg {
            SbSeg::One => SB1_IMAGE,
            SbSeg::Two => SB2_IMAGE,
        };
        let start = (bx as usize).min(img.len());
        let end = (bx as usize + cx as usize + 1).min(img.len());
        let pitch = self.b(PITCH);
        if self.b(SOUND_ENABLED) != 0 {
            self.host.sb_play(&img[start..end], pitch);
        } else {
            self.host.sb_play(&[], pitch);
        }
    }

    /// footstep
    pub(super) fn footstep(&mut self) {
        if !self.sb_present() {
            self.speaker_short();
            return;
        }
        if self.b(ALARM_SOUNDING) == 0xFF {
            return;
        }
        let off = SB1_HEIL.wrapping_add(self.w(CLICK_SOUND));
        self.play_sample(SbSeg::One, off, 46);
    }

    /// playClick: a lock tumbler.
    pub(super) fn play_click(&mut self) {
        if !self.sb_present() {
            self.do_speaker();
            return;
        }
        self.play_sample(SbSeg::One, SB1_HEIL + 400, 46);
    }

    fn do_speaker(&mut self) {
        self.r.set_al(25);
        self.speaker_low();
        self.r.set_al(64);
        self.speaker_low();
    }

    /// speaker_high: AL = pitch mask
    pub(super) fn speaker_high(&mut self) {
        let ax = self.r.al() as u16;
        let (sax, sbx) = (self.r.ax, self.r.bx);
        self.loc_641(ax, 0);
        self.r.ax = sax;
        self.r.bx = sbx;
    }

    /// speaker_low: AL = pitch mask
    pub(super) fn speaker_low(&mut self) {
        let ax = self.r.al() as u16;
        let (sax, sbx) = (self.r.ax, self.r.bx);
        self.loc_641(ax, ax + 1);
        self.r.ax = sax;
        self.r.bx = sbx;
    }

    /// Nine speaker pulses with pseudo-random periods (the original's
    /// shift-register noise generator).
    fn loc_641(&mut self, ax: u16, bx: u16) {
        self.setw(V4791, ax);
        self.setw(V4793, bx);
        for _ in 0..9 {
            let a = (self.w(V478F) & self.w(V4791)) | self.w(V4793);
            let cx = ((a as u8) as u16) << 1;
            // each loop pass is two `in al,80h` (about 2 us)
            self.host.io_delay(cx as u32 * 2);
            self.speaker_toggle();
            let mut ax = self.w(V478D);
            let d = self.w(V478D).rotate_right(1);
            self.setw(V478D, d);
            let f = self.w(V478F).rotate_right(1);
            self.setw(V478F, f);
            ax = ax.rotate_right(1);
            if (ax & f) & 0x8000 != 0 {
                ax |= 0x10;
            }
            // `test ax,v478F / jno` : TEST always clears OF, never taken
            ax ^= self.w(V478D);
            self.setw(V478D, ax);
            self.r.ax = ax;
        }
        self.r.cx = 0;
    }

    /// speaker_short: a footstep click.
    pub(super) fn speaker_short(&mut self) {
        self.speaker_toggle();
        let al = self.b(SPEAKER_DELAY);
        self.speaker_delay(al);
        self.speaker_toggle();
        let d = self.b(SPEAKER_DELAY) << 1;
        self.setb(SPEAKER_DELAY, d);
        self.speaker_delay(d);
        self.speaker_toggle();
    }

    /// speaker_delay: a CPU-speed loop of AL*AL iterations.
    fn speaker_delay(&mut self, al: u8) {
        let n = if al == 0 { 256u32 } else { al as u32 };
        self.host.io_delay(n * n * 3);
    }

    /// crashSound
    pub(super) fn crash_sound(&mut self) {
        if self.sb_present() {
            self.play_sample(SbSeg::Two, SB2_CRASH, 1200);
            return;
        }
        self.speaker_noise();
    }

    /// fireGunSound
    pub(super) fn fire_gun_sound(&mut self) {
        if self.sb_present() {
            self.play_sample(SbSeg::Two, SB2_GUN, 6980);
            return;
        }
        self.speaker_noise();
    }

    /// speakerCrash / speakerGun: the same burst of noise.
    fn speaker_noise(&mut self) {
        let (sax, sbx) = (self.r.ax, self.r.bx);
        for (al, low) in [
            (0x1F, true),
            (0x1F, false),
            (0x3F, true),
            (0x3F, false),
            (0x7F, true),
            (0x7F, false),
            (0x7F, false),
        ] {
            self.r.set_al(al);
            if low {
                self.speaker_low()
            } else {
                self.speaker_high()
            }
        }
        if self.b(CPU_TYPE) == 0 {
            self.r.set_al(0xFF);
            self.speaker_low();
            self.r.set_al(0xFF);
            self.speaker_high();
        }
        self.r.set_bl(0xE0);
        self.sub_140(2);
        self.r.ax = sax;
        self.r.bx = sbx;
    }

    /// sub_140: a CPU-speed toggling loop (a buzz on slow machines).
    fn sub_140(&mut self, al: u8) {
        self.setb(V4795, al);
        let mut bl = self.r.bl();
        self.setb(V4796, bl);
        let mut cl: u8 = 0xFF;
        loop {
            bl = bl.wrapping_sub(1);
            if bl == 0 {
                self.speaker_toggle();
                bl = 0;
            }
            self.host.io_delay(1);
            cl = cl.wrapping_sub(1);
            if cl != 0 {
                continue;
            }
            if self.decb(V4795) == 0 {
                break;
            }
        }
        self.r.set_bl(bl);
        self.r.set_cl(cl);
    }

    /// blast: the bomb goes off.
    pub(super) fn blast(&mut self) {
        let mut bx: u16 = 10;
        loop {
            self.r.bx = bx;
            self.crash_sound();
            self.r.ax = bx;
            self.screen_garbage();
            bx -= 1;
            if bx == 0 {
                break;
            }
        }
        self.r.bx = 0;
    }

    /// changePitch: randomise the death scream.
    fn change_pitch(&mut self) {
        let ax = self.r.ax;
        let al = self.get_rand();
        let up = al & 0x10 == 0;
        let n = al & 0x0F;
        let mut p = self.b(PITCH).wrapping_add(n);
        if !up {
            p = p.wrapping_sub(n).wrapping_sub(n);
        }
        self.setb(PITCH, p);
        self.r.ax = ax;
    }

    /// guardAlert (sub_142): play the next queued voice / message.
    pub(super) fn guard_alert(&mut self) {
        let di;
        loop {
            let bx = self.w(V4789);
            self.r.bx = bx;
            if bx == self.w(V4787) {
                return;
            }
            let al = self.b(V4764 + bx);
            self.r.set_al(al);
            let nb = (bx + 1) & 0x1F;
            self.r.bx = nb;
            self.setw(V4789, nb);
            if al & 0x80 != 0 {
                let d = (al & 0x7F) as i8 as i16 as u16;
                self.r.ax = d;
                self.r.di = d;
                let v = self.b(V4750 + d);
                self.setb(SPEAKER_DELAY2, v);
                continue;
            }
            di = al as i8 as i16 as u16;
            self.r.ax = di;
            self.r.di = di;
            break;
        }
        self.r.set_bh(0);
        let bl = self.b(V479B + di);
        self.r.set_bl(bl);
        if bl != 0 {
            let bx = self.r.bx;
            self.r.dx = 0;
            self.r.di = 0x1CC0;
            self.r.cx = 0x128;
            self.put_sprite();
            self.r.dx = 0x1708;
            self.locate();
            self.r.bx = bx;
            self.print_msg(); // "Stop !", "Heil !", ...
            self.r.di = di;
            self.host.present(&self.st);
        }
        // loc_625
        if self.sb_present() {
            let (bx, dx) = (self.r.bx, self.r.dx);
            if di == 0 {
                self.change_pitch();
            }
            let seg = if di <= 4 { SbSeg::One } else { SbSeg::Two };
            let d2 = di << 1;
            self.r.di = d2;
            let off = self.w(SB_VOICE_TABLE + d2);
            let n = self.w(SB_VOICE_TABLE_SIZE + d2);
            self.play_sample(seg, off, n);
            self.setb(SAMPLE_PLAYING, 0xFF);
            self.setb(PITCH, DEFAULT_PITCH);
            let mut cx: u16 = 4000;
            loop {
                if self.host.sb_done() {
                    self.setb(SAMPLE_PLAYING, 0);
                    break;
                }
                self.host.tick_wait(&self.st);
                cx -= 1;
                if cx == 0 {
                    break;
                }
            }
            self.r.cx = cx;
            self.setb(SAMPLE_PLAYING, 0);
            // (the original pops these swapped)
            self.r.bx = dx;
            self.r.dx = bx;
        } else {
            // speakerOnly: 1-bit voice data in the data segment
            let d2 = di << 1;
            let mut p = self.w(VOICE_TABLE + d2);
            self.r.si = self.w(SPEAKER_DELAY2);
            let mut ch = self.r.ch();
            loop {
                p = p.wrapping_add(1);
                let cl = self.ds[p as usize];
                let dl = cl;
                let cx = (((ch as u16) << 8) | cl as u16).wrapping_add(2) >> 1;
                // each pass: 60 x `in al,80h` (~60 us)
                self.host.io_delay(cx as u32 * 60);
                ch = 0;
                self.r.set_dl(dl);
                if dl == 0xFF {
                    break;
                }
                if dl == 0xFE {
                    continue;
                }
                if self.b(SOUND_ENABLED) == 0 {
                    continue;
                }
                self.host.speaker_toggle();
            }
            self.r.di = p;
            self.r.cx = 0;
        }
        // loc_631: swallow extra fire-key presses
        loop {
            self.r.set_ah(1);
            match self.int16_peek() {
                None => return,
                Some(k) => {
                    if k as u8 != b'l' {
                        return;
                    }
                }
            }
            self.r.set_ah(0);
            self.int16_get();
        }
    }

    /// checkAlarm (sub_138): ring the alarm bell if it is on, otherwise
    /// just wait a beat. Either way the fire key is checked.
    pub(super) fn check_alarm(&mut self) {
        self.setb(SHOOT_FLAG, 0);
        let v = self.b(V4F3);
        if v == 0 || v & 0x80 != 0 {
            self.no_alarm();
            return;
        }
        self.setb(ALARM_SOUNDING, 0xFF);
        let mut dx: u16 = 0x0A00;
        if self.b(CPU_TYPE) != 0 {
            dx >>= 1;
        }
        let mut bx: u16 = 0;
        if self.sb_present() {
            self.sb_alarm();
            self.r.bx = bx;
            self.r.dx = dx;
            self.test_fire_button();
            self.r.bx = bx;
            self.r.dx = dx;
            return;
        }
        loop {
            // loc_605 (speakerAlarm)
            self.host.io_delay(40);
            bx = bx.wrapping_add(1);
            if (bx as u8) >= self.b(V18D1) {
                self.speaker_toggle();
                bx = 0;
            }
            let dl = (dx as u8).wrapping_sub(1);
            dx = (dx & 0xFF00) | dl as u16;
            if dl != 0 {
                continue;
            }
            self.r.bx = bx;
            self.r.dx = dx;
            self.test_fire_button();
            let dh = ((dx >> 8) as u8).wrapping_sub(1);
            dx = (dx & 0x00FF) | ((dh as u16) << 8);
            if dh != 0 {
                continue;
            }
            break;
        }
        self.r.bx = bx;
        self.r.dx = dx;
        self.host.tick_wait(&self.st);
        self.host.tick_wait(&self.st);
        if self.b(V18D1) != 0 {
            self.decb(V18D1);
        }
    }

    fn no_alarm(&mut self) {
        let mut si: u16 = 0x80;
        let mut bx: u16 = 0;
        loop {
            let t = self.b(V597 + si);
            if t != 0x10 && t == 0x50 {
                bx += 2;
            }
            si += 0x10;
            if si == 0x100 {
                break;
            }
        }
        self.r.si = si;
        self.r.bx = bx;
        // (the original's timeout table lookup is overridden by "mov cx,500h")
        let mut ch: u8 = 5;
        loop {
            self.host.tick_wait(&self.st);
            self.r.cx = (ch as u16) << 8;
            self.test_fire_button();
            ch -= 1;
            if ch == 0 {
                break;
            }
        }
        self.r.cx = 0;
    }

    fn sb_alarm(&mut self) {
        let (ax, bx, cx, dx) = (self.r.ax, self.r.bx, self.r.cx, self.r.dx);
        self.host.tick_wait(&self.st);
        self.host.tick_wait(&self.st);
        self.play_sample(SbSeg::Two, SB2_ALARM + 400, 6850);
        self.host.tick_wait(&self.st);
        self.host.tick_wait(&self.st);
        self.r.ax = ax;
        self.r.bx = bx;
        self.r.cx = cx;
        self.r.dx = dx;
    }

    /// testFireButton: 'l' fires even during the alarm / waits.
    pub(super) fn test_fire_button(&mut self) {
        if self.b(SHOOT_FLAG) != 0 {
            return;
        }
        // (joystick button only if one was detected; never here)
        self.r.set_ah(1);
        match self.int16_peek() {
            None => return,
            Some(k) => {
                if k as u8 != 0x6C {
                    return;
                }
            }
        }
        self.r.set_ah(0);
        self.int16_get();
        self.shoot();
        self.incb(SHOOT_FLAG);
    }
}
