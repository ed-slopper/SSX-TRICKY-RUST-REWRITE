//! The memory card operation starts and the "save before exit" menus (board row G9n): `MemCardOp_StartSave` 0x1ce778,
//! `MemCardOp_StartLoad` 0x1ce5b0, `NextEventOverlay_SaveBeforeExitA` 0x1bbb30 / `B` 0x1bc8a0 and
//! `Results_UpdateSaveBeforeExit` 0x1b32c0. Every callee (the overlay's operation functions of [`crate::mcoverlay`], the
//! dialogs, the HUD, the career) is a [`LoadEnv`] call by address; the two virtual calls (the owner's at 0x1c4 of its
//! second vtable `o+0x428`, the card system's at 0x84) are made through the vtables in memory.

use crate::speechlib::Mem;
use crate::speechload::LoadEnv;

pub const LOC_GET: u32 = 0x2c5160;
pub const GET_BUTTONS: u32 = 0x1a8688;
pub const SOUND_MOVE: u32 = 0x1a8648;
pub const SOUND_BACK: u32 = 0x1a8658;
pub const SOUND_SELECT: u32 = 0x1a8668;
pub const DIALOG_SET: u32 = 0x1aa090;
pub const SYNC_AUDIO: u32 = 0x161928;
pub const HUD_HIDE: u32 = 0x1a7860;
pub const HUD_SHOW: u32 = 0x1a7848;
pub const OVERLAY_TICK: u32 = 0x1a8bd8;
pub const OVERLAY_ASK: u32 = 0x1aac08;
pub const OP_UPDATE: u32 = 0x1ceee0;
pub const OP_IS_DONE: u32 = 0x1d21a0;
pub const OP_RESULT: u32 = 0x1d21c0;
pub const OP_FINISH: u32 = 0x1ceeb0;
pub const OP_PREPARE: u32 = 0x1cee68;
pub const OP_BEGIN: u32 = 0x1ce8b0;
pub const OP_START_SAVE: u32 = 0x1ce778;
pub const OP_RESTART: u32 = 0x1d2140;
pub const STR_A: u32 = 0x1d2f60;
pub const STR_B: u32 = 0x1d2fa0;
pub const STR_C: u32 = 0x1d2fc8;
pub const SPRINTF: u32 = 0x2fca28;
pub const CAREER_A: u32 = 0x161838;
pub const CAREER_B: u32 = 0x161a98;
pub const START_RACE: u32 = 0x113bb0;
pub const CAREER_C: u32 = 0x164288;
pub const CAREER_D: u32 = 0x161840;
pub const CAREER_E: u32 = 0x166580;
pub const CAREER_F: u32 = 0x164368;
pub const CAREER_G: u32 = 0x1635b0;
pub const HUD_GET_A: u32 = 0x1a7d48;
pub const HUD_GET_B: u32 = 0x1a7ce8;
pub const RACE_A: u32 = 0x113be0;
pub const RACE_B: u32 = 0x113bc0;
pub const RACE_C: u32 = 0x113bd0;
pub const LIST_TICK: u32 = 0x1ab148;

pub const GAPP: u32 = 0x337c58;
pub const CAREER: u32 = 0x32b390;
const FMT: u32 = 0x391ef0;

fn sext16(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// A virtual call: `obj`'s vtable pointer is at `vt_at`, the adjust at `off - 4` and the function at `off`.
fn vcall(m: &mut dyn Mem, e: &mut dyn LoadEnv, obj: u32, vt: u32, off: u32, a1: u32) {
    let adj = sext16(m.r16(vt.wrapping_add(off).wrapping_sub(4)));
    let f = m.r32(vt.wrapping_add(off));
    e.call(m, f, &[obj.wrapping_add(adj), a1]);
}

fn start_op(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, o: u32, mode: u32, save: bool) {
    m.w32(o, mode);
    let vt = m.r32(o + 0x428);
    vcall(m, e, o, vt, 0x1c4, 1);
    if save {
        m.w32(o + 8, 1);
    }
    let first = m.r32(o) == 0;
    if !save {
        m.w32(o + 8, 0);
    }
    let card = m.r32(o + 0x224);
    let (state, a1, third_fn, buf) = if first { (4, 0x8_0000, STR_B, sp + 0x10) } else { (1, 0x27d4, STR_C, sp + 0x20) };
    m.w32(o + 0x2c, state);
    let cvt = m.r32(card);
    vcall(m, e, card, cvt, 0x84, a1);
    e.call(m, STR_A, &[sp, 0x10]);
    e.call(m, third_fn, &[buf, 8]);
    e.call(m, SPRINTF, &[o + 0x98, FMT, sp, buf]);
}

/// 0x1ce778: start saving, `mode` 0 for the first card operation.
pub fn start_save(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, o: u32, mode: u32) {
    start_op(m, e, sp, o, mode, true);
}

/// 0x1ce5b0.
pub fn start_load(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, o: u32, mode: u32) {
    start_op(m, e, sp, o, mode, false);
}

fn loc(m: &mut dyn Mem, e: &mut dyn LoadEnv, id: u32) -> u32 {
    let g = m.r32(GAPP);
    let l = m.r32(g.wrapping_add(0x50));
    e.call(m, LOC_GET, &[l, id])
}

/// The dialog: four texts (the first with a flag set, "save changes?") or three (the plain notice).
fn dialog(m: &mut dyn Mem, e: &mut dyn LoadEnv, ov: u32, ids: &[u32]) {
    let t: Vec<u32> = ids.iter().map(|i| loc(m, e, *i)).collect();
    let o = m.r32(ov.wrapping_add(0x42c));
    if t.len() == 4 {
        e.call(m, DIALOG_SET, &[o, t[0], 0, t[1], t[2], t[3]]);
    } else {
        e.call(m, DIALOG_SET, &[o, t[0], 0, t[1], 0, t[2]]);
    }
}

fn hud(m: &mut dyn Mem) -> u32 {
    let g = m.r32(GAPP);
    let race = m.r32(g.wrapping_add(0x730));
    m.r32(race.wrapping_add(0x38))
}

fn race(m: &mut dyn Mem) -> u32 {
    let g = m.r32(GAPP);
    m.r32(g.wrapping_add(0x730))
}

/// The saving dialog the three menus share: ask whether to save when the profile has something to (`flag` stays set
/// on the owner), else the plain notice.
fn save_dialog(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32, set_flag: bool) {
    if e.call(m, SYNC_AUDIO, &[CAREER]) != 0 {
        if set_flag {
            m.w32(p + 0x430, 1);
        }
        dialog(m, e, p, &[0x1bc, 0x1d6, 0x188, 0x154]);
    } else {
        dialog(m, e, p, &[0x146, 0x1d6, 0x188]);
    }
}

/// The overlay's answer to the card dialog, shared by the three: the state at +0x430 (2 while a card operation runs)
/// and the operation tick; `None` when the menu has nothing more to do this tick (the operation is still running),
/// else whether the operation finished with a success (`s5`) and the overlay.
fn operation_tick(m: &mut dyn Mem, e: &mut dyn LoadEnv, p: u32) -> Option<bool> {
    let ov = m.r32(p + 0x42c);
    e.call(m, OVERLAY_TICK, &[ov]);
    let mut done = false;
    if m.r32(p + 0x430) == 2 {
        e.call(m, OP_UPDATE, &[p]);
        if e.call(m, OP_IS_DONE, &[p]) == 0 {
            return None;
        }
        let r = e.call(m, OP_RESULT, &[p]);
        m.w32(p + 0x430, 0);
        done = r != 0;
        e.call(m, OP_FINISH, &[p]);
    }
    Some(done)
}

/// What a dialog answered (0x1aac08): the yes branch restarts the card operation with the "overwrite" prompt.
fn restart_operation(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, p: u32) {
    m.w32(p + 0x430, 2);
    m.w32(p + 0x434, 0x1c4);
    e.call(m, OP_PREPARE, &[p]);
    e.call(m, OP_BEGIN, &[p, 0x27d4]);
    start_save(m, e, sp.wrapping_sub(0x60), p, 1);
    e.call(m, OP_RESTART, &[p]);
}

/// 0x1bbb30; `sp` is its own frame (the nested `start_save` has the 0x60 bytes below it).
pub fn save_before_exit_a(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, p: u32) {
    let mut s5 = match operation_tick(m, e, p) {
        None => return,
        Some(b) => b,
    };
    let ov = m.r32(p + 0x42c);
    if m.r32(ov.wrapping_add(0x328)) != 0 {
        let v = e.call(m, OVERLAY_ASK, &[ov, 0xffff_ffff]);
        if v == 0 {
            return;
        }
        if v == 2 {
            m.w32(p + 0x430, 0);
        } else if v != 1 || m.r32(p + 0x430) != 1 {
            s5 = true;
        } else {
            restart_operation(m, e, sp, p);
        }
    } else {
        let b = e.call(m, GET_BUTTONS, &[ov, 0xffff_ffff]);
        if b & 0x2_0001 != 0 {
            let o = m.r32(p + 0x42c);
            e.call(m, SOUND_MOVE, &[o]);
            let v = (m.r32(p + 0x558) as i32).wrapping_sub(1);
            m.w32(p + 0x558, v as u32);
            let mut a0 = m.r32(p + 0x438) as i32;
            if v < 0 {
                m.w32(p + 0x558, (a0 - 1) as u32);
            }
            a0 = m.r32(p + 0x438) as i32;
            if m.r32(p + 0x558) as i32 == a0 - 2 {
                m.w32(p + 0x558, (a0 - 3) as u32);
            }
        }
        if b & 0x4_0002 != 0 {
            let o = m.r32(p + 0x42c);
            e.call(m, SOUND_MOVE, &[o]);
            let v = (m.r32(p + 0x558) as i32).wrapping_add(1);
            let a0 = m.r32(p + 0x438) as i32;
            m.w32(p + 0x558, v as u32);
            if v >= a0 {
                m.w32(p + 0x558, 0);
            }
            if m.r32(p + 0x558) as i32 == a0 - 2 {
                m.w32(p + 0x558, (a0 - 1) as u32);
            }
        }
        let mut back = b & 0x20 != 0;
        if b & 0x800 != 0 {
            let a0 = m.r32(p + 0x438) as i32;
            let sel = m.r32(p + 0x558) as i32;
            if sel == a0 - 1 {
                save_dialog(m, e, p, true);
                back = false;
            } else if sel < a0 - 2 {
                let at = p.wrapping_add((sel as u32).wrapping_mul(4));
                if m.r32(at.wrapping_add(0x4b0)) != 0 {
                    let v = m.r32(at.wrapping_add(0x440));
                    m.w32(CAREER + 0x3814, v);
                    m.w32(CAREER + 0x3260, 4);
                    e.call(m, CAREER_A, &[CAREER]);
                    let h = hud(m);
                    e.call(m, HUD_HIDE, &[h, 0x10, 0xffff_ffff]);
                    let r = race(m);
                    m.w32(r + 0x54, 1);
                    m.w32(CAREER + 0x3708, 1);
                    finish_a(m, e, p, s5);
                    return;
                }
            }
        }
        if back {
            let h = hud(m);
            e.call(m, HUD_HIDE, &[h, 0x10, 0xffff_ffff]);
            let id = if m.r32(p + 0x55c) != 0 { 1 } else { 0x11 };
            let hh = hud(m);
            e.call(m, HUD_SHOW, &[hh, id, 0xffff_ffff]);
        }
    }
    finish_a(m, e, p, s5);
}

fn finish_a(m: &mut dyn Mem, e: &mut dyn LoadEnv, _p: u32, s5: bool) {
    if s5 {
        let h = hud(m);
        e.call(m, HUD_HIDE, &[h, 0x10, 0xffff_ffff]);
        e.call(m, CAREER_B, &[CAREER]);
        let r = race(m);
        let l = m.r32(r.wrapping_add(0x1c));
        e.call(m, START_RACE, &[l]);
        m.w32(CAREER + 0x3260, 2);
    }
}

/// 0x1bc8a0: the same menu with five items (the selection at +0x460) and its own set of choices.
pub fn save_before_exit_b(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, p: u32) {
    let mut s5 = match operation_tick(m, e, p) {
        None => return,
        Some(b) => b,
    };
    let ov = m.r32(p + 0x42c);
    if m.r32(ov.wrapping_add(0x328)) != 0 {
        let v = e.call(m, OVERLAY_ASK, &[ov, 0xffff_ffff]);
        if v == 0 {
            return;
        }
        if v == 2 {
            m.w32(p + 0x430, 0);
        } else if v != 1 || m.r32(p + 0x430) != 1 {
            s5 = true;
        } else {
            restart_operation(m, e, sp, p);
        }
    } else {
        let b = e.call(m, GET_BUTTONS, &[ov, 0xffff_ffff]);
        if b != 0 {
            if b & 0x2_0001 != 0 {
                let o = m.r32(p + 0x42c);
                e.call(m, SOUND_MOVE, &[o]);
                let v = (m.r32(p + 0x460) as i32).wrapping_sub(1);
                m.w32(p + 0x460, v as u32);
                if v < 0 {
                    m.w32(p + 0x460, 4);
                }
            } else if b & 0x4_0002 != 0 {
                let o = m.r32(p + 0x42c);
                e.call(m, SOUND_MOVE, &[o]);
                let v = (m.r32(p + 0x460) as i32).wrapping_add(1);
                m.w32(p + 0x460, v as u32);
                if v >= 5 {
                    m.w32(p + 0x460, 0);
                }
            } else if b & 0x800 != 0 {
                let idx = m.r32(p + 0x460);
                if idx < 5 {
                    let sel = |m: &mut dyn Mem, e: &mut dyn LoadEnv| {
                        let o = m.r32(p + 0x42c);
                        e.call(m, SOUND_SELECT, &[o]);
                    };
                    match idx {
                        0 => {
                            sel(m, e);
                            let mode = m.r32(CAREER + 0x2afc);
                            let v = m.r32(CAREER + 0x16b8);
                            m.w32(CAREER + 0x3818, if mode == 4 || mode == 5 { 4 } else { 2 });
                            m.w32(CAREER + 0x381c, v);
                            s5 = true;
                        }
                        1 => {
                            sel(m, e);
                            let mode = m.r32(CAREER + 0x2afc);
                            m.w32(CAREER + 0x3818, if mode == 4 || mode == 5 { 5 } else { 3 });
                            let r = e.call(m, CAREER_C, &[CAREER]);
                            m.w32(CAREER + 0x381c, r);
                            s5 = true;
                        }
                        2 => {
                            if m.r32(p + 0x454) != 0 {
                                sel(m, e);
                                s5 = true;
                                m.w32(CAREER + 0x3818, 7);
                                let r = e.call(m, CAREER_C, &[CAREER]);
                                m.w32(CAREER + 0x381c, r);
                            }
                        }
                        3 => {
                            sel(m, e);
                            dialog(m, e, p, &[0x169, 0x1d6, 0x188]);
                        }
                        _ => {
                            sel(m, e);
                            save_dialog(m, e, p, true);
                        }
                    }
                }
            } else if b & 0x20 != 0 && m.r32(p + 0x464) != 0 {
                let o = m.r32(p + 0x42c);
                e.call(m, SOUND_BACK, &[o]);
                let h = hud(m);
                e.call(m, HUD_HIDE, &[h, 0x11, 0xffff_ffff]);
                let h = hud(m);
                e.call(m, HUD_SHOW, &[h, 1, 0xffff_ffff]);
            }
        }
    }
    if s5 {
        let h = hud(m);
        e.call(m, HUD_HIDE, &[h, 0x11, 0xffff_ffff]);
        let a0 = m.r32(p + 0x460);
        if a0 == 3 || a0 == 4 {
            e.call(m, CAREER_B, &[CAREER]);
            let r = race(m);
            let l = m.r32(r.wrapping_add(0x1c));
            e.call(m, START_RACE, &[l]);
            m.w32(CAREER + 0x3260, if a0 == 3 { 0x10 } else { 2 });
        } else {
            let h = hud(m);
            e.call(m, HUD_SHOW, &[h, 0x10, 0xffff_ffff]);
        }
    }
}

fn results_finish(m: &mut dyn Mem, e: &mut dyn LoadEnv) {
    e.call(m, CAREER_B, &[CAREER]);
    e.call(m, CAREER_D, &[CAREER]);
    m.w32(CAREER + 0x3260, 2);
    let r = race(m);
    m.w32(r.wrapping_add(0x54), 1);
}

/// The two pointers a choice leaves on the HUD: `a` first, then `b` (the pause menu and the results page order them
/// differently), each returned by an accessor and flagged at +0x464 / +0x55c.
fn hud_flags(m: &mut dyn Mem, e: &mut dyn LoadEnv, first: u32, v464: u32, show: u32) {
    let h = hud(m);
    let q = e.call(m, first, &[h]);
    m.w32(q.wrapping_add(if first == HUD_GET_B { 0x464 } else { 0x55c }), if first == HUD_GET_B { v464 } else { 0 });
    let h = hud(m);
    let second = if first == HUD_GET_B { HUD_GET_A } else { HUD_GET_B };
    let q = e.call(m, second, &[h]);
    m.w32(q.wrapping_add(if second == HUD_GET_B { 0x464 } else { 0x55c }), if second == HUD_GET_B { v464 } else { 0 });
    let h = hud(m);
    e.call(m, HUD_SHOW, &[h, show, 0xffff_ffff]);
}

/// 0x1b32c0: the results page's "save before exit" list; `o` holds the list at +0, the menu at +4 and a count at +0x10.
pub fn results_update_save_before_exit(m: &mut dyn Mem, e: &mut dyn LoadEnv, sp: u32, o: u32) {
    let list = m.r32(o);
    e.call(m, LIST_TICK, &[list]);
    let base = race(m);
    let base = m.r32(base.wrapping_add(0x1c));
    if m.r32(base.wrapping_add(0x4f8)) == 0 {
        let c = m.r32(o + 0x10);
        let list = m.r32(o);
        m.w32(list + 0x34, c);
    } else {
        let list = m.r32(o);
        m.w32(list + 0x34, 0);
    }
    let p = m.r32(o + 4);
    let state = m.r32(p + 0x430);
    if state == 2 {
        e.call(m, OP_UPDATE, &[p]);
        let p = m.r32(o + 4);
        if e.call(m, OP_IS_DONE, &[p]) == 0 {
            return;
        }
        let p = m.r32(o + 4);
        if e.call(m, OP_RESULT, &[p]) != 0 {
            e.call(m, CAREER_B, &[CAREER]);
            e.call(m, CAREER_D, &[CAREER]);
            m.w32(CAREER + 0x3260, state);
            let r = race(m);
            m.w32(r.wrapping_add(0x54), 1);
        } else {
            let p = m.r32(o + 4);
            m.w32(p + 0x430, 0);
        }
        let p = m.r32(o + 4);
        e.call(m, OP_FINISH, &[p]);
        return;
    }
    let ov = m.r32(p + 0x42c);
    if m.r32(ov.wrapping_add(0x328)) != 0 {
        let v = e.call(m, OVERLAY_ASK, &[ov, 0xffff_ffff]);
        if v == 0 || (v == 3 && m.r32(m.r32(o + 4) + 0x430) != 1) {
            let list = m.r32(o);
            m.w32(list + 0x38, 0);
            return;
        }
        if v == 2 {
            let p = m.r32(o + 4);
            let list = m.r32(o);
            m.w32(p + 0x430, 0);
            m.w32(list + 0x38, 0);
            return;
        }
        let p = m.r32(o + 4);
        if v == 1 && m.r32(p + 0x430) == 1 {
            m.w32(p + 0x430, 2);
            m.w32(p + 0x434, 0x1c4);
            e.call(m, OP_PREPARE, &[p]);
            let p = m.r32(o + 4);
            e.call(m, OP_BEGIN, &[p, 0x27d4]);
            let p = m.r32(o + 4);
            start_save(m, e, sp.wrapping_sub(0x60), p, 1);
            let p = m.r32(o + 4);
            e.call(m, OP_RESTART, &[p]);
            return;
        }
        results_finish(m, e);
        return;
    }
    let b = e.call(m, GET_BUTTONS, &[ov, 0xffff_ffff]);
    if b == 0 {
        return;
    }
    let list = m.r32(o);
    let count = m.r32(list + 0x34) as i32;
    if count <= 0 {
        return;
    }
    let snd = |m: &mut dyn Mem, e: &mut dyn LoadEnv, f: u32| {
        let p = m.r32(o + 4);
        let ov = m.r32(p + 0x42c);
        e.call(m, f, &[ov]);
    };
    if b & 0x800 != 0 {
        snd(m, e, SOUND_SELECT);
        let list = m.r32(o);
        let idx = m.r32(list + 0x38);
        let kind = m.r32(list.wrapping_add(idx.wrapping_shl(3)).wrapping_add(0xc));
        match kind {
            0 => {
                let h = hud(m);
                e.call(m, HUD_HIDE, &[h, 1, 0xffff_ffff]);
                let mode = m.r32(CAREER + 0x2afc);
                if mode == 1 {
                    m.w32(CAREER + 0x3818, 1);
                    let r = e.call(m, CAREER_C, &[CAREER]);
                    m.w32(CAREER + 0x381c, r);
                    let h = hud(m);
                    let q = e.call(m, HUD_GET_A, &[h]);
                    m.w32(q.wrapping_add(0x55c), 1);
                    let h = hud(m);
                    e.call(m, HUD_SHOW, &[h, 0x10, 0xffff_ffff]);
                } else if mode == 5 {
                    let v = e.call(m, CAREER_E, &[CAREER, 0]);
                    if (e.call(m, CAREER_F, &[CAREER, v]) as i32) > 0 {
                        hud_flags(m, e, HUD_GET_B, 0, 0xf);
                    } else {
                        hud_flags(m, e, HUD_GET_B, 1, 0x11);
                    }
                } else if mode == 4 {
                    if e.call(m, CAREER_G, &[CAREER]) != 2 {
                        let r = race(m);
                        let l = m.r32(r.wrapping_add(0x1c));
                        e.call(m, START_RACE, &[l]);
                    } else {
                        let v = e.call(m, CAREER_E, &[CAREER, 0]);
                        if (e.call(m, CAREER_F, &[CAREER, v]) as i32) > 0 {
                            hud_flags(m, e, HUD_GET_A, 0, 0xf);
                        } else {
                            hud_flags(m, e, HUD_GET_B, 1, 0x11);
                        }
                    }
                } else {
                    hud_flags(m, e, HUD_GET_B, 1, 0x11);
                }
            }
            1 | 2 | 5 => {
                let f = match kind {
                    1 => RACE_A,
                    2 => RACE_B,
                    _ => RACE_C,
                };
                let r = race(m);
                let l = m.r32(r.wrapping_add(0x1c));
                e.call(m, f, &[l]);
            }
            3 => {
                let g = e.call(m, SYNC_AUDIO, &[CAREER]);
                let p = m.r32(o + 4);
                if g != 0 {
                    m.w32(p + 0x430, 1);
                    dialog(m, e, p, &[0x1bc, 0x1d6, 0x188, 0x154]);
                } else {
                    m.w32(p + 0x430, 0);
                    dialog(m, e, p, &[0x146, 0x1d6, 0x188]);
                }
            }
            4 => {
                let h = hud(m);
                e.call(m, HUD_HIDE, &[h, 1, 0xffff_ffff]);
                let h = hud(m);
                e.call(m, HUD_SHOW, &[h, 0xb, 0xffff_ffff]);
            }
            _ => {}
        }
    } else if b & 0x2_0001 != 0 {
        snd(m, e, SOUND_MOVE);
        let list = m.r32(o);
        let v = (m.r32(list + 0x38) as i32).wrapping_sub(1);
        m.w32(list + 0x38, v as u32);
        if v < 0 {
            let c = m.r32(list + 0x34);
            m.w32(list + 0x38, c.wrapping_sub(1));
        }
    } else if b & 0x4_0002 != 0 {
        snd(m, e, SOUND_MOVE);
        let list = m.r32(o);
        let v = (m.r32(list + 0x38) as i32).wrapping_add(1);
        let c = m.r32(list + 0x34) as i32;
        m.w32(list + 0x38, v.wrapping_rem(c) as u32);
    }
}
