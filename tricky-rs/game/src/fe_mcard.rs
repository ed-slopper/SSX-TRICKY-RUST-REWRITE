//! The front end's memory card overlay (board row G9y4) on the game's bytes: `cFEStateMCOverlay`, the front end
//! state that shows the memory card manager's (`cMCOverlayManager`, the save code's) progress: its dialog, message,
//! prompt and slot list (`FEMemCard_*`), the buttons a state asks for, the pad's buttons as the manager wants them,
//! the save and load of the career through the card's buffer, and the switch on the manager's state that puts the
//! right words on screen. Every call out goes through [`FeEnv`]; the functions of the row that call each other, and
//! the widgets' own (`fe_wtext`, `fe_wentry`), are called directly. Checked in the function runner
//! (`tests/fe_mcard.rs`).
//!
//! The overlay (o): +0 the operation (0 the career, 1 the options), +4 whether it runs quietly, +8 whether the card
//! has two slots' worth of buttons, +0x38 the slot, +0x4c / +0x50 the fade, +0x54 the manager's state, +0x21c the
//! chosen slot, +0x224 the card (its table at +0), +0x428 its second table, +0x42c the front end state it shows (its
//! table at +0x438, its root group at +0x464), +0x480.. its flags, +0x4a0 the load's read position, +0x4a4 the
//! options' buffer (0x27d4 bytes), +0x2c70 the buffer's sum, +0x2c78 the dialog, +0x2c7c the message, +0x2c80 the
//! prompt.

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_venuesel::LOC_STRING;
use crate::fe_wentry::{button_layout, SET_WORLD_POS};
use crate::fe_wtext::{set_scale, set_string_id, set_wide_text};
use crate::pf::Mem;
use crate::ps2f::F;

/// The overlay's tables; the career
pub const STATE_TABLE: u32 = 0x3b12b8;
pub const SECOND_TABLE: u32 = 0x3b1398;
pub const CAREER: u32 = 0x32b390;
/// The memory card manager (the save code): its constructor and destructor, its set-ups for a save or a load, its
/// buffer, its update, draw and steps, the slot label, the message text and the slot moves
pub const MGR_CTOR: u32 = 0x1ce490;
pub const MGR_DTOR: u32 = 0x1ce580;
pub const MGR_START: u32 = 0x1cee68;
pub const MGR_END: u32 = 0x1ceeb0;
pub const MGR_SAVE_SET: u32 = 0x1ce8b0;
pub const MGR_SAVE: u32 = 0x1ce778;
pub const MGR_LOAD_SET: u32 = 0x1ce730;
pub const MGR_LOAD: u32 = 0x1ce5b0;
pub const MGR_BEGIN: u32 = 0x1ce6b8;
pub const MGR_STEP: u32 = 0x1ce9f8;
pub const MGR_DRAW: u32 = 0x1d11b0;
pub const MGR_READ: u32 = 0x1d1fd0;
pub const MGR_FINISH: u32 = 0x1d2078;
pub const MGR_TEXT: u32 = 0x1d2fe8;
pub const MGR_SLOT_LABEL: u32 = 0x1d2d50;
pub const MGR_NEXT: u32 = 0x1d21c8;
pub const MGR_PREV: u32 = 0x1d2200;
pub const MGR_SLOT_NEXT: u32 = 0x1d2278;
pub const MGR_SLOT_PREV: u32 = 0x1d2230;
pub const MGR_CLEAR: u32 = 0x1d0ec8;
pub const MGR_SHOW: u32 = 0x1cee28;
/// The front end state's own: its set-up (0x2715c8), destructor (0x2717a0), buttons (0x2718f0: the bits), a button
/// by its bit (0x271848), a group shown on top (0x275468), its leaving (0x275888)
pub const STATE_SETUP: u32 = 0x2715c8;
pub const STATE_DTOR: u32 = 0x2717a0;
pub const STATE_BUTTONS: u32 = 0x2718f0;
pub const STATE_BUTTON: u32 = 0x271848;
pub const STATE_SHOW_GROUP: u32 = 0x275468;
pub const STATE_LEAVE: u32 = 0x275888;
/// The career's: the start and end of a career save (0x166800, 0x166840), its options' save (0x161a98), its load
/// (0x166870), the player's progress kept (0x167060); the sum (0x2ce04c) and copy (0x2ce114); the fill (0x2cc808);
/// a group's widget by id (0x2ac0c0) and the n-th (0x2ac048); the wide count (0x17cc48), copy (0x17cc80) and compare
/// (0x2fb220); the renderer's brightness (0x1e0eb8 / 0x1e0db8) and the front end's music (0x275a70)
pub const CAREER_SAVE_BEGIN: u32 = 0x166800;
pub const CAREER_SAVE_END: u32 = 0x166840;
pub const OPTIONS_SAVE: u32 = 0x161a98;
pub const CAREER_LOAD: u32 = 0x166870;
pub const CAREER_KEEP: u32 = 0x167060;
pub const SUM: u32 = 0x2ce04c;
pub const COPY: u32 = 0x2ce114;
pub const FILL: u32 = 0x2cc808;
pub const FIND: u32 = 0x2ac0c0;
pub const NTH: u32 = 0x2ac048;
pub const WSTR_COUNT: u32 = 0x17cc48;
pub const WSTR_COPY: u32 = 0x17cc80;
pub const MEMCMP: u32 = 0x2fb220;
pub const RENDER_GET: u32 = 0x1e0eb8;
pub const RENDER_SET: u32 = 0x1e0db8;
pub const FE_MUSIC: u32 = 0x275a70;
/// The pad: enter (0x26cde8), escape (0x26cdf8), accept, back, up, down, the key 0x63
pub const INPUT_ENTER: u32 = 0x26cde8;
pub const INPUT_ESCAPE: u32 = 0x26cdf8;
pub const INPUT_ACCEPT: u32 = 0x26ce08;
pub const INPUT_BACK: u32 = 0x26ce30;
pub const INPUT_UP: u32 = 0x26ce78;
pub const INPUT_DOWN: u32 = 0x26ce88;
pub const INPUT_KEY63: u32 = 0x26ce68;
/// The last loaded career block's address (0x32ea8c: the career's +0x36fc, the block `finish` sums)
pub const LOAD_BLOCK: u32 = 0x32ea8c;

const ONE: u32 = 0x3f80_0000;
const HALF: F = F(0.5);

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn wf(m: &mut Mem, at: u32, v: F) {
    m.w32(at, v.0.to_bits());
}

/// `cvt.s.w`: toward zero
fn fl(v: u32) -> F {
    let v = v as i32;
    let mut f = v as f32;
    if (f as f64).abs() > (v as f64).abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    F(f)
}

fn lt(a: F, b: F) -> bool {
    a.ee() < b.ee()
}

fn q(m: &Mem, at: u32) -> [F; 4] {
    let a = at & !0xf;
    [rf(m, a), rf(m, a + 4), rf(m, a + 8), rf(m, a + 12)]
}

fn put(m: &mut Mem, at: u32, v: [F; 4]) {
    let a = at & !0xf;
    for (k, x) in v.iter().enumerate() {
        wf(m, a + 4 * k as u32, *x);
    }
}

fn app(m: &Mem) -> u32 {
    m.r32(GAPP)
}

/// A virtual call through a table at +0 (the card's, the sound player's, the front end's)
fn vcall0(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// A virtual call through a table kept at `obj + table`
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, table: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj + table);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

fn loc(m: &mut Mem, e: &mut dyn FeEnv, id: u32) -> u32 {
    let l = m.r32(app(m) + 0x50);
    e.call(m, LOC_STRING, &[l, id])
}

/// The root group's widget with that id
fn widget(m: &mut Mem, e: &mut dyn FeEnv, o: u32, id: u32) -> u32 {
    let root = m.r32(o + 0x464);
    e.call(m, FIND, &[root + 0x110, id])
}

/// The menu font's height plus 4, the text widgets' line height
fn line_height(m: &Mem) -> F {
    let font = m.r32(app(m) + 0x60);
    fl(m.r32(font + 0xc)) + F(4.0)
}

/// The offset that takes `w` from its position (+0x80) to `at`, its w 1, written at `out`
fn offset_to(m: &mut Mem, w: u32, at: [F; 4], out: u32) {
    let p = q(m, w + 0x80);
    put(m, out, [at[0] - p[0], at[1] - p[1], at[2] - p[2], F(1.0)]);
}

// ---- construction

/// `cFEStateMCOverlay::cFEStateMCOverlay` 0x28f260(o, op, saving): the manager made, the front end state set up,
/// its flags cleared and tables set, the manager started; then by the operation: a save (the options' buffer of
/// 0x27d4 set up first, +0x490 0x1c4) or a load (the career's 0x80000 or the options' 0x27d4 set up, +0x490 0x176 /
/// 0x177)
pub fn ctor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, op: u32, saving: u32) -> u32 {
    e.call(m, MGR_CTOR, &[o, op, saving]);
    e.call(m, STATE_SETUP, &[o + 0x42c]);
    for at in [0x484u32, 0x480, 0x488, 0x4a0, 0x2c78, 0x2c7c, 0x494, 0x498, 0x49c] {
        m.w32(o + at, 0);
    }
    m.w32(o + 0x438, STATE_TABLE);
    m.w32(o + 0x428, SECOND_TABLE);
    e.call(m, MGR_START, &[o]);
    m.w32(o + 0x42c + 0x18, 0);
    if saving != 0 {
        if op == 1 {
            e.call(m, MGR_SAVE_SET, &[o, 0x27d4]);
            m.w32(o + 0x490, 0x1c4);
        }
        e.call(m, MGR_SAVE, &[o, op]);
    } else {
        if op == 0 || op == 1 {
            let (size, id) = if op == 0 { (0x80000, 0x176) } else { (0x27d4, 0x177) };
            e.call(m, MGR_LOAD_SET, &[o, size]);
            m.w32(o + 0x490, id);
        }
        e.call(m, MGR_LOAD, &[o, op]);
    }
    o
}

/// `cFEStateMCOverlay::~cFEStateMCOverlay` 0x28f380(o, flags): its tables, the manager ended, the front end state's
/// destructor, then the manager's
pub fn dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) -> u32 {
    m.w32(o + 0x438, STATE_TABLE);
    m.w32(o + 0x428, SECOND_TABLE);
    e.call(m, MGR_END, &[o]);
    e.call(m, STATE_DTOR, &[o + 0x42c, 0]);
    e.call(m, MGR_DTOR, &[o, flags])
}

// ---- the pad and the operation's steps

/// `cFEStateMCOverlay` vf2 0x290de0(o): the buttons the manager wants from either pad (the front end's +0x74 and
/// +0x78): while not quiet, the key 0x63 (0x40) in states 7, 23 and 28, enter (none) in 9, back or accept (0x20 /
/// 0x800, none with escape or enter) in 10; else accept 0x800, back 0x20, up 0x20001, down 0x40002, or none
pub fn pad_buttons(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    let fe = m.r32(app(m) + 0x734);
    let (p2, p1) = (m.r32(fe + 0x78), m.r32(fe + 0x74));
    let either = |m: &mut Mem, e: &mut dyn FeEnv, f: u32| e.call(m, f, &[p1]) != 0 || e.call(m, f, &[p2]) != 0;
    if m.r32(o + 4) == 0 {
        match m.r32(o + 0x54).wrapping_sub(7) {
            0 | 16 | 21 => {
                if either(m, e, INPUT_KEY63) {
                    return 0x40;
                }
            }
            2 => {
                if either(m, e, INPUT_ENTER) {
                    return 0;
                }
            }
            3 => {
                if either(m, e, INPUT_BACK) {
                    return if either(m, e, INPUT_ESCAPE) { 0 } else { 0x20 };
                }
                if either(m, e, INPUT_ACCEPT) {
                    return if either(m, e, INPUT_ENTER) { 0 } else { 0x800 };
                }
            }
            _ => {}
        }
    }
    if either(m, e, INPUT_ACCEPT) {
        return 0x800;
    }
    if either(m, e, INPUT_BACK) {
        return 0x20;
    }
    if either(m, e, INPUT_UP) {
        return 0x20001;
    }
    if either(m, e, INPUT_DOWN) {
        return 0x40002;
    }
    0
}

/// `cFEStateMCOverlay::Draw` 0x290fd0(o): the manager's draw
pub fn draw(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    e.call(m, MGR_DRAW, &[o])
}

/// `cFEStateMCOverlay` vf7 0x291178 / vf11 0x291450(o): for the career, the career's save begun / ended
pub fn career_save_mark(m: &mut Mem, e: &mut dyn FeEnv, o: u32, end: bool) {
    if m.r32(o) == 0 {
        e.call(m, if end { CAREER_SAVE_END } else { CAREER_SAVE_BEGIN }, &[CAREER]);
    }
}

/// `cFEStateMCOverlay` vf8 0x291198(o): the manager's operation begun: the career from the last loaded block (0x920
/// bytes, reading) or the options' buffer (cleared, 0x27d4 bytes); 1 for anything else
pub fn begin(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    match m.r32(o) {
        0 => {
            let block = m.r32(LOAD_BLOCK);
            m.w32(o + 0x488, 1);
            m.w32(o + 0x4a0, block);
            let slot = m.r32(o + 0x38);
            e.call(m, MGR_BEGIN, &[o, slot, block, 0x920])
        }
        1 => {
            e.call(m, FILL, &[o + 0x4a4, 0, 0x27d4]);
            let slot = m.r32(o + 0x38);
            e.call(m, MGR_BEGIN, &[o, slot, o + 0x4a4, 0x27d4])
        }
        _ => 1,
    }
}

/// `cFEStateMCOverlay` vf9 0x291240(o, n): for the career, the next `n` bytes (at most 0x4000) read in from the
/// read position; first the header's 0x920 bytes, whose sum (0x2ce04c) must match the word it carries (+0x334,
/// cleared); 0 on a bad sum or for the options
pub fn read_more(m: &mut Mem, e: &mut dyn FeEnv, o: u32, n: u32) -> u32 {
    if m.r32(o) != 0 {
        return 0;
    }
    if m.r32(o + 0x488) != 0 {
        let v = m.r32(o + 0x4a0);
        m.w32(o + 0x488, 0);
        let want = m.r32(v + 0x334);
        m.w32(v + 0x334, 0);
        let got = e.call(m, SUM, &[v, 0x920]);
        let p = m.r32(o + 0x4a0).wrapping_add(0x920);
        m.w32(o + 0x4a0, p);
        if got != want {
            return 0;
        }
    }
    let mut n = n as i32;
    if n > 0 {
        if n > 0x4000 {
            n = 0x4000;
        }
        let at = m.r32(o + 0x4a0);
        e.call(m, MGR_READ, &[o, at, n as u32]);
        let p = m.r32(o + 0x4a0).wrapping_add(n as u32);
        m.w32(o + 0x4a0, p);
    }
    1
}

/// `cFEStateMCOverlay` vf10 0x2912e8(o): the operation finished: for the career, the loaded block's bytes summed
/// from 0xdeadbeef must match its word, then the player kept (the front end's +5 less one) and the career loaded
/// (+0x498); for the options, the buffer's sum must match, then it is copied into the career's settings, the sound
/// set from them (the front end's sound player, +0x138) and the options saved (+0x49c); 1 for anything else
pub fn finish(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    match m.r32(o) {
        0 => {
            let base = m.r32(CAREER + 0x36fc);
            let head = base.wrapping_add(0x7fff8);
            let n = m.r32(head) as i32;
            if n <= 0 {
                return 0;
            }
            let mut sum = 0xdead_beefu32;
            for i in 0..n as u32 {
                sum = sum.wrapping_add(m.r8(base + 0x920 + i) as i8 as i32 as u32);
            }
            if sum != m.r32(head + 4) {
                return 0;
            }
            let fe = m.r32(app(m) + 0x734);
            let p = (m.r8(fe + 5) as i8 as i32).wrapping_sub(1) as u32;
            m.w32(CAREER + 0x325c, p);
            e.call(m, CAREER_LOAD, &[CAREER]);
            m.w32(o + 0x498, 1);
            1
        }
        1 => {
            let want = m.r32(o + 0x2c70);
            m.w32(o + 0x2c70, 0);
            if e.call(m, SUM, &[o + 0x4a4, 0x27d4]) != want {
                return 0;
            }
            e.call(m, COPY, &[CAREER + 8, o + 0x4a4, 0x27d4]);
            let mode = m.r32(CAREER + 8 + 0x1668);
            let fe = m.r32(app(m) + 0x734);
            let snd = m.r32(fe + 0xa4);
            vcall0(m, e, snd, 0x138, &[mode]);
            e.call(m, OPTIONS_SAVE, &[CAREER]);
            m.w32(o + 0x49c, 1);
            1
        }
        _ => 1,
    }
}

/// `cFEStateMCOverlay` vf3 0x291470(o): for the options, the manager's finish (0x1d2078); 1 for the others
pub fn vf3(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    if m.r32(o) == 1 { e.call(m, MGR_FINISH, &[o]) } else { 1 }
}

/// `cFEStateMCOverlay` vf4 0x291498: 0; vf6 0x2914a0, vf23 0x293210 and vf26 0x293218: nothing
pub fn zero() -> u32 {
    0
}
pub fn nothing() {}

/// `cFEStateMCOverlay` vf5 0x2914a8(o): for the options, the options saved; 1 for the others
pub fn vf5(m: &mut Mem, e: &mut dyn FeEnv, o: u32) -> u32 {
    if m.r32(o) == 1 { e.call(m, OPTIONS_SAVE, &[CAREER]) } else { 1 }
}

// ---- the slots

/// `cFEStateMCOverlay` vf44 0x2914d0(o, k; `buf` 0x100 bytes): slot `k`'s line: the state's buttons 0x140, the
/// line enabled when the slot has a save (+0x174 and +0x168 of its 0x58-byte record), and the chosen slot's button
/// (0x40) the same; its label (0x1d2d50) set; the list laid out
pub fn slot_line(m: &mut Mem, e: &mut dyn FeEnv, o: u32, k: u32, buf: u32) {
    e.call(m, FILL, &[buf, 0, 0x100]);
    let list = widget(m, e, o, 0);
    e.call(m, STATE_BUTTONS, &[o + 0x42c, 0x140]);
    let line = e.call(m, NTH, &[list + 0x118, k]);
    let button = e.call(m, STATE_BUTTON, &[o + 0x42c, 0x40]);
    let rec = o.wrapping_add(k.wrapping_mul(0x58));
    let on = (m.r32(rec + 0x174) != 0 && m.r32(rec + 0x168) != 0) as u32;
    vcall(m, e, line, 0x100, &[on]);
    if k == m.r32(o + 0x21c) {
        vcall(m, e, button, 0x100, &[on]);
    }
    e.call(m, MGR_SLOT_LABEL, &[o, k, buf, 0x80]);
    vcall(m, e, line, 0x148, &[buf]);
    vcall(m, e, list, 0x140, &[]);
}

/// `cFEStateMCOverlay` vf47 0x291648 / vf48 0x291688(o): the manager's next / previous, then its own +0x168 (the
/// second table) with the chosen slot
pub fn step(m: &mut Mem, e: &mut dyn FeEnv, o: u32, next: bool) {
    e.call(m, if next { MGR_NEXT } else { MGR_PREV }, &[o]);
    let k = m.r32(o + 0x21c);
    vcall_at(m, e, o, 0x428, 0x168, &[k]);
}

/// `cFEStateMCOverlay` vf49 0x2916c8(o): the slot list hidden and left, the info (0xb) enabled and shown, the
/// state's buttons 0xb00 (two slots) or 0x500
pub fn vf49(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let list = widget(m, e, o, 0);
    vcall(m, e, list, 0x28, &[0]);
    vcall(m, e, list, 0xf0, &[0]);
    let info = widget(m, e, o, 0xb);
    vcall(m, e, info, 0x100, &[1]);
    vcall(m, e, info, 0x28, &[1]);
    let bits = if m.r32(o + 8) != 0 { 0xb00 } else { 0x500 };
    e.call(m, STATE_BUTTONS, &[o + 0x42c, bits]);
}

/// `cFEStateMCOverlay` vf50 0x292918 / vf51 0x292940(o): the manager's next / previous slot, then the slot list
/// filled
pub fn slot_step(m: &mut Mem, e: &mut dyn FeEnv, o: u32, next: bool, buf: u32) {
    e.call(m, if next { MGR_SLOT_NEXT } else { MGR_SLOT_PREV }, &[o]);
    fill_slot_list(m, e, o, buf);
}

/// Two wide strings the same (0x2fb220 over the second's length)
fn same_text(m: &mut Mem, e: &mut dyn FeEnv, a: u32, id: u32) -> bool {
    let s0 = loc(m, e, id);
    let s = loc(m, e, id);
    let n = e.call(m, WSTR_COUNT, &[s]);
    e.call(m, MEMCMP, &[a, s0, n << 1]) == 0
}

/// `FEMemCard_FillSlotList` 0x292968(o; `buf` 0x80 bytes): the slot's name (the card's +0x70) and the state's buttons
/// by what the card says: with two slots' buttons (0x200 and 0x800), the first enabled when the card is ready
/// (+0x178, +0x190 and +0x160 all), else by the name; with one (0x400), by the name: none for the empty-slot name
/// (0x15f), the first only when the card says the slot can't be used (+0x1c8), the second unless it is the
/// corrupt name (0x148)
pub fn fill_slot_list(m: &mut Mem, e: &mut dyn FeEnv, o: u32, buf: u32) {
    let card = m.r32(o + 0x224);
    let slot = m.r32(o + 0x38);
    vcall0(m, e, card, 0x70, &[slot, buf]);
    let mut second = 0;
    let first;
    let ready;
    if m.r32(o + 8) != 0 {
        first = e.call(m, STATE_BUTTON, &[o + 0x42c, 0x200]);
        second = e.call(m, STATE_BUTTON, &[o + 0x42c, 0x800]);
        let card = m.r32(o + 0x224);
        ready = vcall0(m, e, card, 0x178, &[]) == 0 || {
            let card = m.r32(o + 0x224);
            vcall0(m, e, card, 0x190, &[]) == 0
        } || {
            let card = m.r32(o + 0x224);
            vcall0(m, e, card, 0x160, &[]) != 0
        };
        if ready {
            vcall(m, e, first, 0x100, &[1]);
            vcall(m, e, second, 0x100, &[0]);
            return;
        }
    } else {
        first = e.call(m, STATE_BUTTON, &[o + 0x42c, 0x400]);
    }
    let card = m.r32(o + 0x224);
    if vcall0(m, e, card, 0x170, &[]) == 0 {
        vcall(m, e, first, 0x100, &[0]);
        if second != 0 || m.r32(o + 8) != 0 {
            vcall(m, e, second, 0x100, &[0]);
        }
        return;
    }
    let unusable = same_text(m, e, buf, 0x15f) || {
        let card = m.r32(o + 0x224);
        let slot = m.r32(o + 0x38);
        vcall0(m, e, card, 0x1c8, &[slot]) == 0
    };
    if unusable {
        let on = (m.r32(o + 8) != 0) as u32;
        vcall(m, e, first, 0x100, &[on]);
        if second != 0 {
            vcall(m, e, second, 0x100, &[0]);
        }
        return;
    }
    let corrupt = same_text(m, e, buf, 0x148);
    vcall(m, e, first, 0x100, &[(!corrupt) as u32]);
    if second != 0 {
        vcall(m, e, second, 0x100, &[1]);
    }
}

/// `FEMemCard_ShowEmptySlots` 0x292d80(o): each slot's line (the first only for the options) given the empty-slot
/// name (0x15f, its +0x148), the list laid out
pub fn show_empty_slots(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let list = widget(m, e, o, 0xb);
    for i in 0..4 {
        let line = e.call(m, NTH, &[list + 0x118, i]);
        let text = m.r32(line + 0x114);
        let vt = m.r32(text + 0xc);
        let adj = m.r16(vt + 0x148) as i16 as u32;
        let s = loc(m, e, 0x15f);
        let f = m.r32(vt + 0x14c);
        e.call(m, f, &[text.wrapping_add(adj), s]);
        if m.r32(o) == 1 {
            break;
        }
    }
    vcall(m, e, list, 0x140, &[]);
}

// ---- the dialog, message and prompt

/// `FEMemCard_ShowLocDialog` 0x291da8(o, id, words, buttons): the dialog with the id's string
pub fn show_loc_dialog(m: &mut Mem, e: &mut dyn FeEnv, o: u32, id: u32, words: u32, buttons: u32, buf: u32) {
    let s = loc(m, e, id);
    show_dialog(m, e, o, s, words, buttons, buf);
}

/// `FEMemCard_ShowDialog` 0x291e00(o, text, words, buttons; `buf` 0x60 bytes): the dialog (+0x2c78)'s text set and
/// centred in it, the dialog shown and enabled; its two buttons (0x13, 0x14) hidden, then laid out and shown by the
/// bits of `buttons` (each a sprite and a string); the two placed side by side under it (or the one centred); the
/// state's buttons disabled, the message widgets shown by `words`, the dialog on top
pub fn show_dialog(m: &mut Mem, e: &mut dyn FeEnv, o: u32, text: u32, words: u32, buttons: u32, buf: u32) {
    let d = m.r32(o + 0x2c78);
    let t = e.call(m, NTH, &[d + 0x110, 0]);
    set_wide_text(m, e, t, text);
    m.w32(t + 0x118, 0);
    m.w32(t + 0x12c + 0x20, 1);
    m.w32(t + 0x12c + 0x24, 0);
    let h = line_height(m);
    set_scale(m, e, t, h);
    vcall(m, e, t, 0x130, &[buf]);
    let d = m.r32(o + 0x2c78);
    vcall(m, e, d, 0x130, &[buf + 0x10]);
    let d = m.r32(o + 0x2c78);
    let pd = q(m, d + 0x80);
    let y = pd[1] + rf(m, buf + 0x14) * HALF - rf(m, buf + 4) * HALF;
    let at = buf + 0x50;
    m.w32(at, 0);
    wf(m, at + 4, y);
    m.w32(at + 8, 0);
    m.w32(at + 0xc, ONE);
    e.call(m, SET_WORLD_POS, &[t, at]);
    vcall(m, e, t, 0x140, &[]);
    vcall(m, e, t, 0x28, &[1]);
    let d = m.r32(o + 0x2c78);
    vcall(m, e, d, 0x100, &[1]);
    let d = m.r32(o + 0x2c78);
    vcall(m, e, d, 0x28, &[1]);
    let d = m.r32(o + 0x2c78);
    let b1 = e.call(m, FIND, &[d + 0x110, 0x13]);
    vcall(m, e, b1, 0x28, &[0]);
    let d = m.r32(o + 0x2c78);
    let b2 = e.call(m, FIND, &[d + 0x110, 0x14]);
    vcall(m, e, b2, 0x28, &[0]);
    for (bit, sprite, s) in [(0x10u32, 0x51u32, 0x155u32), (1, 0x51, 0x1ba), (4, 0x51, 0x155), (8, 0x51, 0x157), (0x20, 0x50, 0x162), (0x40, 0x51, 0x1d6)] {
        if buttons & bit != 0 {
            button_layout(m, e, b1, sprite, s, buf + 0x60);
            vcall(m, e, b1, 0x28, &[1]);
        }
    }
    if buttons & 2 != 0 {
        if m.r32(b1 + 0x14) == 0 {
            button_layout(m, e, b1, 0x50, 0x154, buf + 0x60);
            vcall(m, e, b1, 0x28, &[1]);
        } else {
            let sprite = if m.r32(o + 0x54) == 10 { 0x51 } else { 0x50 };
            button_layout(m, e, b2, sprite, 0x154, buf + 0x60);
            vcall(m, e, b2, 0x28, &[1]);
        }
    }
    if buttons & 0x80 != 0 {
        button_layout(m, e, b2, 0x50, 0x188, buf + 0x60);
        vcall(m, e, b2, 0x28, &[1]);
    }
    vcall(m, e, b1, 0x130, &[buf + 0x30]);
    let white = |m: &mut Mem| {
        for k in 0..4 {
            m.w32(at + 4 * k, ONE);
        }
    };
    let y = pd[1] + F(225.0) - F(60.0);
    if m.r32(b2 + 0x14) != 0 {
        let x = F(320.0) - (rf(m, buf + 0x30) + F(50.0));
        put(m, at, [x, y, F(0.0), F(1.0)]);
        m.w32(at + 8, 0);
        e.call(m, SET_WORLD_POS, &[b1, at]);
        white(m);
        vcall(m, e, b1, 0x50, &[at]);
        white(m);
        vcall(m, e, b2, 0x50, &[at]);
        vcall(m, e, b2, 0x130, &[buf + 0x40]);
        put(m, at, [F(370.0), y, F(0.0), F(1.0)]);
        m.w32(at + 8, 0);
        e.call(m, SET_WORLD_POS, &[b2, at]);
    } else {
        white(m);
        vcall(m, e, b1, 0x50, &[at]);
        let x = F(320.0) - rf(m, buf + 0x30) * HALF;
        put(m, at, [x, y, F(0.0), F(1.0)]);
        m.w32(at + 8, 0);
        e.call(m, SET_WORLD_POS, &[b1, at]);
    }
    enable_buttons(m, e, o, 0);
    show_message_widgets(m, e, o, words);
    let d = m.r32(o + 0x2c78);
    e.call(m, STATE_SHOW_GROUP, &[o + 0x42c, d]);
}

/// `FEMemCard_HideDialog` 0x2923b8(o): the dialog hidden and left, the message widgets shown, the state's buttons
/// enabled
pub fn hide_dialog(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let d = m.r32(o + 0x2c78);
    vcall(m, e, d, 0x28, &[0]);
    let d = m.r32(o + 0x2c78);
    vcall(m, e, d, 0xf0, &[0]);
    show_message_widgets(m, e, o, 1);
    enable_buttons(m, e, o, 1);
}

/// `FEMemCard_ShowMessageWidgets` 0x292428(o, v): the info (0xb), message (0x15) and detail (0x16) groups shown or
/// hidden
pub fn show_message_widgets(m: &mut Mem, e: &mut dyn FeEnv, o: u32, v: u32) {
    for id in [0xb, 0x15, 0x16] {
        let w = widget(m, e, o, id);
        vcall(m, e, w, 0x28, &[v]);
    }
}

/// `FEMemCard_EnableButtons` 0x2924d0(o, v): the state's buttons enabled (1) or not (0): 0x100, then 0x800 and 0x200
/// with two slots' buttons (+8, read after the first), else 0x400
pub fn enable_buttons(m: &mut Mem, e: &mut dyn FeEnv, o: u32, v: u32) {
    let on = (v != 0) as u32;
    let w = e.call(m, STATE_BUTTON, &[o + 0x42c, 0x100]);
    vcall(m, e, w, 0x100, &[on]);
    let bits: &[u32] = if m.r32(o + 8) != 0 { &[0x800, 0x200] } else { &[0x400] };
    for &bit in bits {
        let w = e.call(m, STATE_BUTTON, &[o + 0x42c, bit]);
        vcall(m, e, w, 0x100, &[on]);
    }
}

/// `FEMemCard_ShowPrompt` 0x292638(o; `buf` 0x60 bytes): the prompt (+0x2c80) given the string for the card's state
/// (+0x1a0: 0x15e, else 0xdeb), its line height set and placed centred under the message, 5 below it; shown
pub fn show_prompt(m: &mut Mem, e: &mut dyn FeEnv, o: u32, buf: u32) {
    let card = m.r32(o + 0x224);
    let id = if vcall0(m, e, card, 0x1a0, &[]) != 0 { 0x15e } else { 0xdeb };
    let p = m.r32(o + 0x2c80);
    set_string_id(m, e, p, id);
    let msg = m.r32(o + 0x2c7c);
    vcall(m, e, msg, 0x130, &[buf]);
    let msg = m.r32(o + 0x2c7c);
    let pm = q(m, msg + 0x80);
    let mh = rf(m, buf + 4);
    let p = m.r32(o + 0x2c80);
    let mut pp = q(m, p + 0x80);
    vcall(m, e, p, 0x130, &[buf + 0x30]);
    let p = m.r32(o + 0x2c80);
    let h = line_height(m);
    set_scale(m, e, p, h);
    pp[1] = pm[1] + mh + F(5.0);
    pp[0] = F(320.0) - rf(m, buf + 0x30) * HALF;
    let p = m.r32(o + 0x2c80);
    offset_to(m, p, pp, buf + 0x40);
    vcall(m, e, p, 0x70, &[buf + 0x40]);
    let p = m.r32(o + 0x2c80);
    vcall(m, e, p, 0x28, &[1]);
}

/// `FEMemCard_HidePrompt` 0x2927a8(o): the prompt hidden
pub fn hide_prompt(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let p = m.r32(o + 0x2c80);
    vcall(m, e, p, 0x28, &[0]);
}

/// `FEMemCard_ShowLocMessage` 0x2927d8(o, id): the message with the id's string
pub fn show_loc_message(m: &mut Mem, e: &mut dyn FeEnv, o: u32, id: u32) {
    let s = loc(m, e, id);
    show_message(m, e, o, s);
}

/// `FEMemCard_ShowMessage` 0x292810(o, text): the message (+0x2c7c) given the text, centred, laid out and shown, its
/// line height set; the state's buttons disabled unless quiet
pub fn show_message(m: &mut Mem, e: &mut dyn FeEnv, o: u32, text: u32) {
    let t = m.r32(o + 0x2c7c);
    set_wide_text(m, e, t, text);
    let t = m.r32(o + 0x2c7c);
    m.w32(t + 0x12c + 0x20, 1);
    m.w32(t + 0x12c + 0x24, 0);
    m.w32(t + 0x118, 0);
    vcall(m, e, t, 0x140, &[]);
    let t = m.r32(o + 0x2c7c);
    vcall(m, e, t, 0x28, &[1]);
    let t = m.r32(o + 0x2c7c);
    let h = line_height(m);
    set_scale(m, e, t, h);
    if m.r32(o + 4) == 0 {
        enable_buttons(m, e, o, 0);
    }
}

/// `FEMemCard_HideMessage` 0x2928c0(o): the message hidden; the state's buttons enabled unless quiet
pub fn hide_message(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    let t = m.r32(o + 0x2c7c);
    vcall(m, e, t, 0x28, &[0]);
    if m.r32(o + 4) == 0 {
        enable_buttons(m, e, o, 1);
    }
}

/// `FEMemCard_LayoutMessage` 0x292e70(o; `buf` 0x140 bytes): the message group (0x15) given the card's text for the
/// chosen slot (+0x1e0), centred at the root group's y + 60 and shown; the detail group (0x16) given the card's
/// detail (+0x1b0) as a wide text, centred under it (7 below) and shown. The positions' z and w come from what the
/// buffer held (the game leaves them unset)
pub fn layout_message(m: &mut Mem, e: &mut dyn FeEnv, o: u32, buf: u32) {
    let g15 = widget(m, e, o, 0x15);
    let card = m.r32(o + 0x224);
    let k = m.r32(o + 0x21c);
    vcall0(m, e, card, 0x1e0, &[k, buf + 0x20, 0x80]);
    vcall(m, e, g15, 0x148, &[buf + 0x20]);
    vcall(m, e, g15, 0x130, &[buf]);
    let (w1, h1) = (rf(m, buf), rf(m, buf + 4));
    let root = m.r32(o + 0x464);
    let pos = buf + 0x10;
    wf(m, pos, F(320.0) - w1 * HALF);
    let y = rf(m, root + 0x84) + F(60.0);
    wf(m, pos + 4, y);
    let p = q(m, pos);
    offset_to(m, g15, p, buf + 0x120);
    vcall(m, e, g15, 0x70, &[buf + 0x120]);
    vcall(m, e, g15, 0x28, &[1]);
    let g16 = widget(m, e, o, 0x16);
    let card = m.r32(o + 0x224);
    vcall0(m, e, card, 0x1b0, &[buf + 0x20]);
    set_wide_text(m, e, g16, buf + 0x20);
    vcall(m, e, g16, 0x130, &[buf]);
    let root = m.r32(o + 0x464);
    wf(m, pos, F(320.0) - rf(m, buf) * HALF);
    let y = rf(m, root + 0x84) + F(60.0) + h1 + F(7.0);
    wf(m, pos + 4, y);
    let p = q(m, pos);
    offset_to(m, g16, p, buf + 0x120);
    vcall(m, e, g16, 0x70, &[buf + 0x120]);
    vcall(m, e, g16, 0x28, &[1]);
}

/// `FEMemCard_HideInfoGroups` 0x2930a8(o): the message and detail groups hidden
pub fn hide_info_groups(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    for id in [0x15, 0x16] {
        let w = widget(m, e, o, id);
        vcall(m, e, w, 0x28, &[0]);
    }
}

/// `cFEStateMCOverlay` vf1 0x292c70(o): the manager cleared, the dialog, message and info hidden, the slot list
/// shown on top, the info hidden
pub fn vf1(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    e.call(m, MGR_CLEAR, &[o]);
    hide_dialog(m, e, o);
    hide_message(m, e, o);
    hide_info_groups(m, e, o);
    let list = widget(m, e, o, 0);
    vcall(m, e, list, 0x28, &[1]);
    e.call(m, STATE_SHOW_GROUP, &[o + 0x42c, list]);
    let info = widget(m, e, o, 0xb);
    vcall(m, e, info, 0x28, &[0]);
}

/// `cFEStateMCOverlay` vf55 0x292d40(o): the manager's show, then (unless quiet) the empty slots
pub fn vf55(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    e.call(m, MGR_SHOW, &[o]);
    if m.r32(o + 4) == 0 {
        show_empty_slots(m, e, o);
    }
}

// ---- the manager's state on screen

/// The front end state's own +0x28 and +0x40 (its table at +0x438)
fn state_calls(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    vcall_at(m, e, o + 0x42c, 0xc, 0x28, &[]);
    vcall_at(m, e, o + 0x42c, 0xc, 0x40, &[]);
}

/// The manager's text for its state (0x1d2fe8, 0x200 bytes into `buf`)
fn text(m: &mut Mem, e: &mut dyn FeEnv, o: u32, buf: u32) {
    e.call(m, MGR_TEXT, &[o, buf, 0x200]);
}

/// `cFEStateMCOverlay` vf54 0x2917b0(o, a; `buf` 0x480 bytes): the manager's step (0x1ce9f8, handed `a`), then the
/// screen for its state (+0x54): quiet, only state 1 (the front end state's +0x28 and +0x40) and 1..3's message;
/// else the dialog, message and prompt hidden first, then by state: 1 done (the career's player kept, or the
/// options' brightness and music set back); 2, 3 and 6 the message (6 with the empty slots; all without the
/// detail); 4, 5 and 22 a yes/no dialog; 7 and 28 the slot list filled from the card; 9, 10, 12, 20, 21, 23, 27 and
/// 34 dialogs with their buttons; 11, 24 and 25 a message with the prompt; 13..18 and 29..32 the same over nothing;
/// 19, 26 and 33 a fixed dialog
pub fn on_state(m: &mut Mem, e: &mut dyn FeEnv, o: u32, a: u32, buf: u32) {
    let msg = buf + 0x60;
    if m.r32(o + 4) == 1 {
        e.call(m, MGR_STEP, &[o, a]);
        let s = m.r32(o + 0x54);
        if s == 1 {
            state_calls(m, e, o);
            return;
        }
        if (s as i32) <= 0 || (s as i32) >= 4 {
            return;
        }
        text(m, e, o, msg);
        show_message(m, e, o, msg);
        return;
    }
    hide_dialog(m, e, o);
    hide_message(m, e, o);
    hide_prompt(m, e, o);
    e.call(m, MGR_STEP, &[o, a]);
    let s = m.r32(o + 0x54);
    let dialog = |m: &mut Mem, e: &mut dyn FeEnv, buttons: u32| {
        text(m, e, o, msg);
        show_dialog(m, e, o, msg, 0, buttons, buf + 0x300);
    };
    match s {
        1 => {
            state_calls(m, e, o);
            if m.r32(o + 0x498) != 0 {
                let fe = m.r32(app(m) + 0x734);
                let p = (m.r8(fe + 5) as i8 as i32).wrapping_sub(1) as u32;
                e.call(m, CAREER_KEEP, &[CAREER, p]);
                let fe = m.r32(app(m) + 0x734);
                m.w8(fe + 6, 1);
            } else if m.r32(o + 0x49c) != 0 {
                let r = m.r32(app(m) + 0x724);
                let at = buf + 0x460;
                e.call(m, RENDER_GET, &[at, r]);
                let mut v = rf(m, CAREER + 0x16b4) + F(0.5);
                if lt(v, F(0.0)) {
                    v = F(0.0);
                } else if lt(F(2.0), v) {
                    v = F(2.0);
                }
                wf(m, r + 0x67c, v);
                for k in 0..3 {
                    m.w32(buf + 0x470 + 4 * k, m.r32(at + 4 * k));
                }
                e.call(m, RENDER_SET, &[r, buf + 0x470]);
                let r = m.r32(app(m) + 0x724);
                let song = m.r32(CAREER + 0x1684);
                vcall_at(m, e, r, 0x10, 0x120, &[song]);
                let fe = m.r32(app(m) + 0x734);
                e.call(m, FE_MUSIC, &[fe + 0xa8]);
            }
        }
        2 | 3 | 6 => {
            if s == 6 {
                let w = widget(m, e, o, 0x16);
                vcall(m, e, w, 0x28, &[0]);
                show_empty_slots(m, e, o);
            } else {
                layout_message(m, e, o, buf + 0x300);
                let w = widget(m, e, o, 0x16);
                vcall(m, e, w, 0x28, &[0]);
            }
            text(m, e, o, msg);
            show_message(m, e, o, msg);
        }
        4 | 5 | 22 => dialog(m, e, 3),
        7 | 28 => {
            let list = widget(m, e, o, 0xb);
            for i in 0..4 {
                let line = e.call(m, NTH, &[list + 0x118, i]);
                let t = m.r32(line + 0x114);
                let card = m.r32(o + 0x224);
                let used = m.r32(o + 0x54) == 0x1c && vcall0(m, e, card, 0x1c0, &[i]) != 0;
                if used {
                    let s = loc(m, e, 0x15f);
                    e.call(m, WSTR_COPY, &[buf, s]);
                } else {
                    let card = m.r32(o + 0x224);
                    vcall0(m, e, card, 0x70, &[i, buf]);
                }
                vcall(m, e, t, 0x148, &[buf]);
                if m.r32(o) != 0 {
                    break;
                }
            }
            vcall(m, e, list, 0x140, &[]);
            fill_slot_list(m, e, o, buf + 0x200);
            let slot = m.r32(o + 0x38);
            vcall(m, e, list, 0x160, &[slot]);
            e.call(m, STATE_SHOW_GROUP, &[o + 0x42c, list]);
            layout_message(m, e, o, buf + 0x300);
        }
        9 => dialog(m, e, 0xc0),
        10 => dialog(m, e, 0x22),
        11 | 24 | 25 => {
            text(m, e, o, msg);
            show_message(m, e, o, msg);
            show_prompt(m, e, o, buf + 0x300);
        }
        12 | 20 | 27 | 34 => dialog(m, e, 0x10),
        13..=18 | 29..=32 => {
            text(m, e, o, msg);
            show_message(m, e, o, msg);
            show_prompt(m, e, o, buf + 0x300);
            e.call(m, STATE_SHOW_GROUP, &[o + 0x42c, 0]);
        }
        19 => show_loc_dialog(m, e, o, 0x1bf, 0, 4, buf + 0x300),
        21 => dialog(m, e, 4),
        23 => dialog(m, e, 0xa),
        26 => show_loc_dialog(m, e, o, 0x158, 0, 0x10, buf + 0x300),
        33 => {
            if m.r32(o) != 0 {
                show_loc_dialog(m, e, o, 0x173, 0, 0x10, buf + 0x300);
            }
        }
        _ => {}
    }
}
