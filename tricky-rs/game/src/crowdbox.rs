//! The crowd boxes' sound bookkeeping (board row G10m): `CrowdSoundList_Construct` 0x21eca8 / `_Add` 0x21ed30,
//! `List_PickRandom` 0x21ed98, `CrowdBox_Init` 0x21ee70, `CrowdBox_StopSlot` 0x2201f8, `_StopAll` 0x220338, `_Arm`
//! 0x2206c8, `_Stop` 0x2206e0, `Crowd_StopCheer` 0x220720 and `CrowdSys_UpdateBoxes` 0x220968. The calls out are
//! [`LoadEnv`] calls by address; a call that takes a float in `f12` gets its bits as the argument after the ints.
//!
//! A box is 0x190 bytes: +0 the box's sound (-1 none), +0x10 .. +0x1c a list of cheer files ([`list_construct`]), +0xd4 the
//! slots in use (of fifteen from +0x20, three words each: in use, a value, the sound entry), +0xd8, three sound
//! entries of 0x30 bytes from +0xe0 (id, voice, use count), +0x170 the voices playing, +0x174 the cheer's voice, +0x178,
//! +0x17c, +0x180 the cheer state (0 off, 1 armed, 2 playing), +0x184, +0x188 the cheer level, +0x18c.

use crate::speechlib::Mem;
use crate::speechload::LoadEnv;

pub const OPEN_FILE: u32 = 0x2c8068;
pub const RAND: u32 = 0x23d4a8;
pub const VOICE_STOP: u32 = 0x20c2a0;
pub const BOX_UPDATE: u32 = 0x21efb0;
const AUDIO: u32 = 0x343120;
const QUARTER: u32 = 0x3e80_0000;

/// 0x21eca8: an empty list (the count and five files).
pub fn list_construct(m: &mut dyn Mem, l: u32) {
    for k in 0..6 {
        m.w32(l + k * 4, 0);
    }
}

/// 0x21ed30: add the file `name` (opened by `OPEN_FILE`) when there is room; 1 when it was added.
pub fn list_add(m: &mut dyn Mem, e: &mut dyn LoadEnv, l: u32, name: u32, mode: u32) -> u32 {
    if (m.r32(l) as i32) < 5 {
        let h = e.call(m, OPEN_FILE, &[name, mode]);
        let n = m.r32(l);
        m.w32(l.wrapping_add(n.wrapping_shl(2)).wrapping_add(4), h);
        if h != 0 {
            m.w32(l, n.wrapping_add(1));
            return 1;
        }
    }
    0
}

/// 0x21ed98: a random file of the list, 0 when it is empty.
pub fn list_pick_random(m: &mut dyn Mem, e: &mut dyn LoadEnv, l: u32) -> u32 {
    if m.r32(l) == 0 {
        return 0;
    }
    let r = e.call(m, RAND, &[]);
    let n = m.r32(l);
    m.r32(l.wrapping_add((r % n).wrapping_shl(2)).wrapping_add(4))
}

/// 0x21ee70.
pub fn box_init(m: &mut dyn Mem, b: u32) {
    m.w32(b, 0xffff_ffff);
    for k in [0x10, 0x14, 0x18, 0x1c] {
        m.w32(b + k, 0);
    }
    m.w32(b + 0xd8, 0xffff_ffff);
    m.w32(b + 0xd4, 0);
    m.w32(b + 0x184, 3);
    m.w32(b + 0x174, 0xffff_ffff);
    for k in [0x170, 0x178, 0x17c, 0x180, 0x188, 0x18c] {
        m.w32(b + k, 0);
    }
    for s in 0..15 {
        for k in [0x20, 0x24, 0x28] {
            m.w32(b + k + s * 0xc, 0);
        }
    }
    for s in 0..3 {
        m.w32(b + 0xe0 + s * 0x30, 0xffff_ffff);
        m.w32(b + 0xe4 + s * 0x30, 0xffff_ffff);
        m.w32(b + 0xe8 + s * 0x30, 0);
        m.w32(b + 0x100 + s * 0x30, s);
    }
}

/// 0x2201f8: stop the sound in slot `slot`, fading the cheer by `fade` when it was the last one.
pub fn stop_slot(m: &mut dyn Mem, e: &mut dyn LoadEnv, b: u32, slot: i32, fade: f32) {
    if slot < 0 {
        return;
    }
    let off = (slot as u32).wrapping_mul(0xc);
    let flag = b.wrapping_add(0x20).wrapping_add(off);
    if m.r32(flag) == 0 {
        return;
    }
    let used = m.r32(b + 0xd4).wrapping_sub(1);
    let entry = m.r32(b.wrapping_add(0x28).wrapping_add(off));
    m.w32(flag, 0);
    m.w32(b + 0xd4, used);
    m.w32(b.wrapping_add(0x28).wrapping_add(off), 0);
    let rc = m.r32(entry.wrapping_add(8)).wrapping_sub(1);
    m.w32(entry.wrapping_add(8), rc);
    if rc == 0 {
        let voice = m.r32(entry.wrapping_add(4));
        if (voice as i32) >= 0 {
            let a = m.r32(AUDIO);
            e.call(m, VOICE_STOP, &[a, voice, 1, fade.to_bits()]);
        }
        m.w32(entry, 0xffff_ffff);
        m.w32(entry.wrapping_add(4), 0xffff_ffff);
        m.w32(b + 0x18c, 0);
        let v = m.r32(b + 0x170).wrapping_sub(1);
        m.w32(b + 0x170, v);
    }
    if m.r32(b + 0xd4) != 0 {
        return;
    }
    if f32::from_bits(QUARTER) < f32::from_bits(m.r32(b + 0x188)) {
        m.w32(b + 0x188, QUARTER);
    }
    let voice = m.r32(b + 0x174);
    if (voice as i32) >= 0 {
        if fade == 0.0 {
            let a = m.r32(AUDIO);
            e.call(m, VOICE_STOP, &[a, voice, 2, fade.to_bits()]);
        }
        m.w32(b + 0x174, 0xffff_ffff);
        if m.r32(b + 0x180) == 2 {
            m.w32(b + 0x180, 1);
        }
    }
    m.w32(b + 0x178, 0);
    m.w32(b, 0xffff_ffff);
}

/// 0x220338: every slot in use.
pub fn stop_all(m: &mut dyn Mem, e: &mut dyn LoadEnv, b: u32, fade: f32) {
    for s in 0..15u32 {
        if m.r32(b + 0x20 + s * 0xc) != 0 {
            stop_slot(m, e, b, s as i32, fade);
        }
    }
}

/// 0x2206c8.
pub fn arm(m: &mut dyn Mem, b: u32) {
    if m.r32(b + 0x180) == 0 {
        m.w32(b + 0x180, 1);
    }
}

/// 0x220720.
pub fn stop_cheer(m: &mut dyn Mem, e: &mut dyn LoadEnv, b: u32) {
    if f32::from_bits(QUARTER) < f32::from_bits(m.r32(b + 0x188)) {
        m.w32(b + 0x188, QUARTER);
    }
    let voice = m.r32(b + 0x174);
    m.w32(b + 0x180, 0);
    if (voice as i32) >= 0 {
        let a = m.r32(AUDIO);
        e.call(m, VOICE_STOP, &[a, voice, 2, 0]);
        m.w32(b + 0x174, 0xffff_ffff);
    }
}

/// 0x2206e0.
pub fn stop(m: &mut dyn Mem, e: &mut dyn LoadEnv, b: u32) {
    if m.r32(b + 0x180) == 2 {
        stop_cheer(m, e, b);
    }
    m.w32(b + 0x180, 0);
}

/// 0x220968: tick every box of the crowd system `s` (+0x3728 the count, +0x372c the boxes).
pub fn update_boxes(m: &mut dyn Mem, e: &mut dyn LoadEnv, s: u32) {
    if (m.r32(s + 0x3728) as i32) > 0 {
        let mut i = 0u32;
        loop {
            let a = m.r32(s + 0x372c).wrapping_add(i.wrapping_mul(0x190));
            i += 1;
            e.call(m, BOX_UPDATE, &[a]);
            if (i as i32) >= m.r32(s + 0x3728) as i32 {
                break;
            }
        }
    }
}
