//! The pause menu's stack of pages and its main page (board row G9a): `PauseMenu_Resume` 0x1ac410,
//! `PauseMenu_PopLevel` 0x1ac580, `PauseMenu_BuildMainPage` 0x1ac678 and `PauseMenu_UpdateMainPage` 0x1ace00.
//!
//! The menu object `p` keeps a stack of page records at +0x7e0 (the top's index at +0x7e8; each is a 0x138-byte
//! record from +0x438). A record has two member-function slots: enter (+0x128 adjust, +0x12a index, +0x12c pointer or
//! table offset) and leave (+0x130, +0x132, +0x134); a positive index picks an eight-byte entry (adjust, pointer) from a
//! table in `p`, otherwise the slot is the pointer itself. The main page is the record at +0x438 whose items are 0x18
//! bytes from +0x454 (the kind first) with the selection at +0x44c and the count at +0x450. Every other thing is a
//! call by address ([`LoadEnv`]).

use crate::speechlib::Mem;
use crate::speechload::LoadEnv;

pub const LOC_GET: u32 = 0x2c5160;
pub const GET_BUTTONS: u32 = 0x1a8688;
pub const SOUND_MOVE: u32 = 0x1a8648;
pub const SOUND_BACK: u32 = 0x1a8658;
pub const SOUND_SELECT: u32 = 0x1a8668;
pub const DIALOG_SET: u32 = 0x1aa090;
pub const GET_TRICKBOOK: u32 = 0x163f18;
pub const COUNT_PAGES: u32 = 0x164060;
pub const PAGE_FIRST: u32 = 0x164110;
pub const SYNC_AUDIO: u32 = 0x161928;
pub const HIDE_ITEM: u32 = 0x1a7860;
pub const RESUME_ALL: u32 = 0x2153f8;
pub const PLAY_PAUSE: u32 = 0x215458;
pub const STOP_ALL: u32 = 0x215488;
pub const STOP_SONG: u32 = 0x225240;

const GAPP: u32 = 0x337c58;
const SYS: u32 = 0x343050;
const CAREER: u32 = 0x32b390;

fn sext16(v: u32) -> u32 {
    v as u16 as i16 as i32 as u32
}

/// The target and the `this` adjust of a member slot of record `rec` (`adj`, `idx`, `tbl` the offsets of its three
/// fields), or `None` when the slot is empty.
fn slot(m: &mut dyn Mem, p: u32, rec: u32, adj: u32, idx: u32, tbl: u32) -> Option<(u32, u32)> {
    let n = m.r16(rec.wrapping_add(idx)) as i16 as i32;
    if n == 0 {
        return None;
    }
    if n > 0 {
        let base = m.r32(p.wrapping_add(sext16(m.r16(rec.wrapping_add(tbl)) as u32)));
        let at = base.wrapping_add((n as u32).wrapping_mul(8)).wrapping_sub(8);
        let (lo, hi) = (m.r32(at), m.r32(at.wrapping_add(4)));
        Some((hi, sext16(lo).wrapping_add(sext16(m.r16(rec.wrapping_add(adj)) as u32))))
    } else {
        Some((m.r32(rec.wrapping_add(tbl)), sext16(m.r16(rec.wrapping_add(adj)) as u32)))
    }
}

/// 0x1ac410: push page `page`, leaving the one on top first.
pub fn resume(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32, page: u32) {
    let mut top = m.r32(p + 0x7e8) as i32;
    if top >= 0 {
        let rec = m.r32(p.wrapping_add(0x7e0).wrapping_add((top as u32).wrapping_mul(4)));
        if let Some((t, a)) = slot(m, p, rec, 0x130, 0x132, 0x134) {
            e.call(m, t, &[p.wrapping_add(a), 0]);
            top = m.r32(p + 0x7e8) as i32;
        }
    }
    let top = top.wrapping_add(1);
    m.w32(p + 0x7e8, top as u32);
    m.w32(p.wrapping_add(0x7e0).wrapping_add((top as u32).wrapping_mul(4)), p.wrapping_add(page.wrapping_mul(0x138)).wrapping_add(0x438));
    if top == 0 {
        let s = m.r32(SYS);
        e.call(m, RESUME_ALL, &[s]);
        let s = m.r32(SYS);
        e.call(m, PLAY_PAUSE, &[s]);
    }
    let top = m.r32(p + 0x7e8);
    let rec = m.r32(p.wrapping_add(0x7e0).wrapping_add(top.wrapping_mul(4)));
    m.w32(rec.wrapping_add(0x14), 0);
    if let Some((t, a)) = slot(m, p, rec, 0x128, 0x12a, 0x12c) {
        e.call(m, t, &[p.wrapping_add(a)]);
    }
}

/// 0x1ac580: leave the top page and pop it.
pub fn pop_level(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32) {
    let mut top = m.r32(p + 0x7e8);
    let rec = m.r32(p.wrapping_add(0x7e0).wrapping_add(top.wrapping_mul(4)));
    if let Some((t, a)) = slot(m, p, rec, 0x130, 0x132, 0x134) {
        e.call(m, t, &[p.wrapping_add(a), 1]);
        top = m.r32(p + 0x7e8);
    }
    let top = top.wrapping_sub(1) as i32;
    m.w32(p + 0x7e8, top as u32);
    if top >= 0 {
        let s = m.r32(SYS);
        e.call(m, STOP_SONG, &[s]);
        return;
    }
    let g = m.r32(GAPP);
    let race = m.r32(g.wrapping_add(0x730));
    let hud = m.r32(race.wrapping_add(0x38));
    e.call(m, HIDE_ITEM, &[hud, 6, 0xffff_ffff]);
    let g = m.r32(GAPP);
    let race = m.r32(g.wrapping_add(0x730));
    if m.r32(race.wrapping_add(0x54)) != 1 {
        let s = m.r32(SYS);
        e.call(m, STOP_ALL, &[s]);
    }
}

fn loc(m: &mut dyn Mem, e: &mut dyn LoadEnv, id: u32) -> u32 {
    let g = m.r32(GAPP);
    let l = m.r32(g.wrapping_add(0x50));
    e.call(m, LOC_GET, &[l, id])
}

/// 0x1ac678: the main page's six items (kind, text) and its count.
pub fn build_main_page(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32) {
    let t = loc(m, e, 0xe63);
    m.w32(p + 0x544, t);
    for (at, kind, id) in [(0x454u32, 1u32, 0x1dbu32), (0x46c, 4, 0x1e0), (0x484, 5, 0x1dd), (0x49c, 7, 0x1de), (0x4b4, 6, 0x1ef)] {
        let t = loc(m, e, id);
        m.w32(p + at, kind);
        m.w32(p + at + 0x14, t);
        m.w32(p + at + 0x10, 0);
        m.w32(p + at + 0xc, 0);
        m.w32(p + at + 8, 0);
    }
    m.w32(p + 0x450, 5);
    for k in 0..4 {
        let v = m.r32(0x38e328 + k * 4);
        m.w32(p + 0x548 + k * 4, v);
    }
}

fn dialog(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32, flag: u32, ids: &[u32]) {
    m.w32(p + 0x430, flag);
    let t: Vec<u32> = ids.iter().map(|i| loc(m, e, *i)).collect();
    let o = m.r32(p + 0x42c);
    if t.len() == 4 {
        e.call(m, DIALOG_SET, &[o, t[0], 0, t[1], t[2], t[3]]);
    } else {
        e.call(m, DIALOG_SET, &[o, t[0], 0, t[1], 0, t[2]]);
    }
}

/// 0x1ace00: one tick of the main page.
pub fn update_main_page(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32) {
    let o = m.r32(p + 0x42c);
    let b = e.call(m, GET_BUTTONS, &[o, 0xffff_ffff]);
    if b & 0x120 != 0 {
        let o = m.r32(p + 0x42c);
        e.call(m, SOUND_BACK, &[o]);
        if m.r32(p + 0x7f0) != 0 {
            m.w32(p + 0x7f0, 0);
            return;
        }
        pop_level(m, e, p);
        return;
    }
    if m.r32(p + 0x7f0) != 0 {
        let o = m.r32(p + 0x42c);
        let g = m.r32(GAPP);
        let race = m.r32(g.wrapping_add(0x730));
        let q = m.r32(race.wrapping_add(0x38));
        let who = m.r32(q.wrapping_add(0xc));
        let book = e.call(m, GET_TRICKBOOK, &[CAREER, who]);
        let n = e.call(m, COUNT_PAGES, &[book]);
        let page = if n ^ 6 != 0 { n } else { 5 };
        let _ = o;
        if b & 0x10_0008 != 0 {
            let first = e.call(m, PAGE_FIRST, &[page]) as i32;
            if first >= m.r32(p + 0x7f4) as i32 {
                return;
            }
            let o = m.r32(p + 0x42c);
            e.call(m, SOUND_MOVE, &[o]);
            let v = m.r32(p + 0x7f4).wrapping_sub(1);
            m.w32(p + 0x7f4, v);
        } else {
            if b & 0x8_0004 == 0 {
                return;
            }
            let first = (e.call(m, PAGE_FIRST, &[page.wrapping_add(1)]) as i32).wrapping_sub(1);
            if (m.r32(p + 0x7f4) as i32) >= first {
                return;
            }
            let o = m.r32(p + 0x42c);
            e.call(m, SOUND_MOVE, &[o]);
            let v = m.r32(p + 0x7f4).wrapping_add(1);
            m.w32(p + 0x7f4, v);
        }
        return;
    }
    if b & 0x2_0001 != 0 {
        let o = m.r32(p + 0x42c);
        e.call(m, SOUND_MOVE, &[o]);
        let v = (m.r32(p + 0x44c) as i32).wrapping_sub(1);
        m.w32(p + 0x44c, v as u32);
        if v < 0 {
            let c = (m.r32(p + 0x450) as i32).wrapping_sub(1);
            m.w32(p + 0x44c, c as u32);
        }
        return;
    }
    if b & 0x4_0002 != 0 {
        let o = m.r32(p + 0x42c);
        e.call(m, SOUND_MOVE, &[o]);
        let v = m.r32(p + 0x44c).wrapping_add(1);
        m.w32(p + 0x44c, v);
        if v == m.r32(p + 0x450) {
            m.w32(p + 0x44c, 0);
        }
        return;
    }
    if b & 0x800 == 0 {
        return;
    }
    let o = m.r32(p + 0x42c);
    e.call(m, SOUND_SELECT, &[o]);
    let sel = m.r32(p + 0x44c);
    let kind = m.r32(p.wrapping_add(sel.wrapping_mul(0x18)).wrapping_add(0x454));
    match kind {
        1 => pop_level(m, e, p),
        4 => {
            m.w32(p + 0x7ec, 0);
            dialog(m, e, p, 0, &[0x146, 0x1d6, 0x188]);
        }
        5 => resume(m, e, p, 1),
        6 => m.w32(p + 0x7f0, 1),
        7 => {
            m.w32(p + 0x7ec, 1);
            if e.call(m, SYNC_AUDIO, &[CAREER]) != 0 {
                dialog(m, e, p, 1, &[0x1bc, 0x1d6, 0x188, 0x154]);
            } else {
                dialog(m, e, p, 0, &[0x146, 0x1d6, 0x188]);
            }
        }
        _ => {}
    }
}
