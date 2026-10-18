//! The rest of the front end's own code in 0x26c000..0x2aab58 (board row G9y6) on the game's bytes: the cameras',
//! animations' and geometry's destructors and small virtuals, the states' destructors and the screen numbers they
//! answer, the text and icon entries' small virtuals, the options pages' refreshes (sound, game, display), the
//! dialog's set-up (`FE_CreateDialog`), two state helpers, and a few unnamed small ones. Every call out goes through
//! [`FeEnv`]; the functions of the row that call each other, and the widgets' own (`fe_wgroup`, `fe_wentry`), are
//! called directly. Checked in the function runner (`tests/fe_misc.rs`).

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_wentry::{entry_dtor, SET_WORLD_POS};
use crate::fe_wgroup::{
    list_dtor, node_vf1, widget_place, widget_set_value, FREE, LIST_DELETE_ALL, TWEEN_RUN,
};
use crate::fe_widget::THING_DTOR;
use crate::fe_widgets::TEXT_ENTRY_VTABLE;
use crate::pf::Mem;
use crate::ps2f::F;

/// The tables
pub const CAMERA_COMBINED_VTABLE: u32 = 0x3ab558;
pub const CAMERA_SCRIPTED_VTABLE: u32 = 0x3ab648;
pub const CAMERA_VTABLE: u32 = 0x3ab738;
pub const ANIM_TRANSLATE_VTABLE: u32 = 0x3ab828;
pub const ANIM_ROTATE_VTABLE: u32 = 0x3ab910;
pub const ANIM_VTABLE: u32 = 0x3ab9f8;
pub const GEOMETRY_VTABLE: u32 = 0x3abbc8;
pub const TEXT_ICON_VTABLE: u32 = 0x3ae690;
pub const ICON_TEXT_VTABLE: u32 = 0x3b1560;
pub const ICON_ENTRY_VTABLE: u32 = 0x3b16b8;
/// The states with their tables (each destructor sets it and goes on to the state's, 0x2717a0) and the screen
/// numbers their vf13 answers
pub const STATE_DTORS: [(u32, u32); 14] = [
    (0x28e108, 0x3afe70),
    (0x28e1a8, 0x3aff50),
    (0x28e210, 0x3b0030),
    (0x28e2b0, 0x3b0110),
    (0x28e478, 0x3b02d0),
    (0x28e520, 0x3b03b0),
    (0x28e5c8, 0x3b0490),
    (0x29d7b0, 0x3b2d30),
    (0x29d868, 0x3b2e10),
    (0x29d910, 0x3b2ef0),
    (0x29d9b8, 0x3b2fd0),
    (0x29da60, 0x3b30b0),
    (0x29db08, 0x3b3190),
    (0x29dc88, 0x3b3350),
];
pub const SCREEN_NUMBERS: [(u32, u32); 12] = [
    (0x276430, 0xf),
    (0x28e420, 7),
    (0x28e4c8, 5),
    (0x28e570, 6),
    (0x28e5e0, 4),
    (0x29d810, 0xa),
    (0x29d8b8, 8),
    (0x29d960, 9),
    (0x29da08, 2),
    (0x29dab0, 3),
    (0x29db58, 1),
    (0x29dca0, 0),
];
/// The virtuals that do nothing, or answer 0 or 1
pub const NOTHINGS: [u32; 9] = [0x2763a0, 0x2763a8, 0x2763b0, 0x276418, 0x276420, 0x276428, 0x276460, 0x29a248, 0x29a250];
pub const ZEROS: [u32; 1] = [0x2764a0];
pub const ONES: [u32; 12] = [
    0x26c598, 0x275d30, 0x275d38, 0x275d40, 0x275d48, 0x275db8, 0x275dc0, 0x293290, 0x293298, 0x2932a0, 0x293310,
    0x293318,
];
/// The calls out: the camera's shot (0x26b2f0), the scripted camera's own destructor part (0x26ac50), the thing's
/// constructor (0x267f58) and its float set-up (0x268808), the state's destructor (0x2717a0) and its helpers
/// (0x2763f0 a widget added, 0x2759c8 a sprite's rectangle, 0x274df0), a group's widget by id (0x2ac0c0) and the n-th
/// (0x2ac048), the heap (0x2ccf70) and its fill (0x2cc808), the display brightness (0x17b970)
pub const CAMERA_SHOT: u32 = 0x26b2f0;
pub const SCRIPTED_PART_DTOR: u32 = 0x26ac50;
pub const THING_CTOR: u32 = 0x267f58;
pub const THING_FLOAT: u32 = 0x268808;
pub const STATE_DTOR: u32 = 0x2717a0;
pub const STATE_ADD: u32 = 0x2763f0;
pub const STATE_RECT: u32 = 0x2759c8;
pub const STATE_274DF0: u32 = 0x274df0;
pub const FIND: u32 = 0x2ac0c0;
pub const NTH: u32 = 0x2ac048;
pub const ALLOC: u32 = 0x2ccf70;
pub const FILL: u32 = 0x2cc808;
pub const BRIGHTNESS: u32 = 0x17b970;
/// The dialog's allocation name and the item table the state helper counts (17 of 12 bytes)
pub const DIALOG_NAME: u32 = 0x3ae5d0;
pub const ITEMS: u32 = 0x347f20;
pub const CAREER: u32 = 0x32b390;

const ONE: u32 = 0x3f80_0000;

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn wf(m: &mut Mem, at: u32, v: F) {
    m.w32(at, v.0.to_bits());
}

/// A virtual call through a table at +0 (the sound player's)
fn vcall0(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

// ---- the cameras, animations and geometry

/// `FE3DCamera_PlayShot` 0x26c370(c, a): its shot (+0xb0) pointed at its own (+0xc0), then played (handed `a`)
pub fn camera_play_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, a: u32) -> u32 {
    m.w32(c + 0xb0, c + 0xc0);
    e.call(m, CAMERA_SHOT, &[c + 0xc0, a])
}

/// `cFE3DCameraCombined::~cFE3DCameraCombined` 0x26c450(c, flags): its table, the node's vf1, its scripted part
/// (+0xc0) destroyed, then the camera's destructor
pub fn camera_combined_dtor(m: &mut Mem, e: &mut dyn FeEnv, c: u32, flags: u32) -> u32 {
    m.w32(c + 0xc, CAMERA_COMBINED_VTABLE);
    node_vf1(m, e, c);
    camera_scripted_dtor(m, e, c + 0xc0, 2);
    camera_dtor(m, e, c, flags)
}

/// `cFE3DCameraScripted` vf28 0x26c4f0: +0xb8
pub fn camera_scripted_vf28(m: &Mem, c: u32) -> u32 {
    m.r32(c + 0xb8)
}

/// `cFE3DCameraScripted::~cFE3DCameraScripted` 0x26c500(c, flags): its table, its own part (0x26ac50), then the
/// camera's destructor
pub fn camera_scripted_dtor(m: &mut Mem, e: &mut dyn FeEnv, c: u32, flags: u32) -> u32 {
    m.w32(c + 0xc, CAMERA_SCRIPTED_VTABLE);
    e.call(m, SCRIPTED_PART_DTOR, &[c]);
    camera_dtor(m, e, c, flags)
}

/// `cFE3DCamera::~cFE3DCamera` 0x26c5d0(c, flags): its table, the node's vf1, then the thing's destructor (0x2acd10)
pub fn camera_dtor(m: &mut Mem, e: &mut dyn FeEnv, c: u32, flags: u32) -> u32 {
    m.w32(c + 0xc, CAMERA_VTABLE);
    node_vf1(m, e, c);
    e.call(m, THING_DTOR, &[c, flags])
}

/// `cFE3DAnimTranslate` 0x26c680 / `cFE3DAnimRotate` 0x26c820 destructors: their table, then the animation's
pub fn anim_kind_dtor(m: &mut Mem, e: &mut dyn FeEnv, a: u32, table: u32, flags: u32) {
    m.w32(a + 0xc, table);
    anim_dtor(m, e, a, ANIM_VTABLE, flags);
}

/// `cFE3DAnim::~cFE3DAnim` 0x26c9a0 / `cFE3DGeometry::~cFE3DGeometry` 0x26cc40: their table, then the thing's
pub fn anim_dtor(m: &mut Mem, e: &mut dyn FeEnv, a: u32, table: u32, flags: u32) {
    m.w32(a + 0xc, table);
    e.call(m, THING_DTOR, &[a, flags]);
}

/// `Vec4_ScaleCopy13` 0x26cd98(out, v; s): every lane times `s`; `out`
pub fn vec_scale(m: &mut Mem, out: u32, v: u32, s: F) -> u32 {
    let x: Vec<F> = (0..4).map(|k| rf(m, (v & !0xf) + 4 * k) * s).collect();
    for (k, y) in x.iter().enumerate() {
        wf(m, (out & !0xf) + 4 * k as u32, *y);
    }
    out
}

/// `FEInput_IsEnter` 0x26cde8 / `FEInput_IsEscape` 0x26cdf8(key): the key is 13 / 27
pub fn input_is(m: &Mem, key: u32, code: u32) -> u32 {
    (m.r32(key) == code) as u32
}

/// 0x270218(x, flags): freed on flag 1
pub fn free_on_flag(m: &mut Mem, e: &mut dyn FeEnv, x: u32, flags: u32) {
    if flags & 1 != 0 {
        e.call(m, FREE, &[x, 1]);
    }
}

// ---- the states

/// `FE_CreateDialog` 0x273f18(s, frames; f12; `buf`): a thing of 0x170 bytes (filled with 0xdeadc0de) made for the
/// state, its float set, run in on track 1 from nowhere over `frames`, its +0x20 with 0, kept at +0x38 and raised
/// (+0x70 2), added to the state
pub fn create_dialog(m: &mut Mem, e: &mut dyn FeEnv, s: u32, frames: u32, v: F, buf: u32) -> u32 {
    let o = e.call(m, ALLOC, &[DIALOG_NAME, 0x170, 0]);
    if o != 0 {
        e.call(m, FILL, &[o, 0xdead_c0de, 0x170]);
    }
    let o = e.call(m, THING_CTOR, &[o, s, 0]);
    e.call(m, THING_FLOAT, &[o, v.0.to_bits()]);
    for k in 0..4 {
        m.w32(buf + 4 * k, 0);
    }
    e.call(m, TWEEN_RUN, &[o, 1, buf, frames, 0]);
    vcall(m, e, o, 0x20, &[0]);
    m.w32(s + 0x38, o);
    m.w32(o + 0x70, 2);
    e.call(m, STATE_ADD, &[s, o])
}

/// 0x2754d8(s): of the 17 items (0x347f20, 12 bytes each: an id, a group), how many are in the state's list
/// (+0x48's +0x118), counting each group (+4) once; 0 without a list
pub fn count_groups(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    if m.r32(s + 0x48) == 0 {
        return 0;
    }
    let mut n = 0;
    for i in 0..0x11u32 {
        let list = m.r32(s + 0x48) + 0x118;
        let id = m.r32(ITEMS + i * 0xc);
        if e.call(m, FIND, &[list, id]) == 0 {
            continue;
        }
        let group = m.r32(ITEMS + i * 0xc + 4);
        if (0..i).all(|j| m.r32(ITEMS + j * 0xc + 4) != group) {
            n += 1;
        }
    }
    n
}

/// 0x275760(s, text; `buf` 0x20 bytes): the state's title group (+0x40): its text (the first's +0x150), the icon
/// (the second) sized to the text's width plus 20 by the sprite's height (0x2759c8 with 0x21), the third put at the
/// text's width plus 20
pub fn set_title(m: &mut Mem, e: &mut dyn FeEnv, s: u32, text: u32, buf: u32) {
    let g = m.r32(s + 0x40);
    if g == 0 {
        return;
    }
    let t = e.call(m, NTH, &[g + 0x110, 0]);
    vcall(m, e, t, 0x150, &[text]);
    vcall(m, e, t, 0x130, &[buf]);
    let g = m.r32(s + 0x40);
    let icon = e.call(m, NTH, &[g + 0x110, 1]);
    let w = rf(m, buf) + F(20.0);
    let r = e.call(m, STATE_RECT, &[s, 0x21]);
    wf(m, buf + 0x10, w);
    let h = m.r32(r.wrapping_add(8));
    m.w32(buf + 0x14, h);
    m.w32(buf + 0x18, 0);
    m.w32(buf + 0x1c, 0);
    vcall(m, e, icon, 0x138, &[buf + 0x10]);
    let g = m.r32(s + 0x40);
    let third = e.call(m, NTH, &[g + 0x110, 2]);
    let x = rf(m, buf) + F(20.0);
    let y = m.r32(third + 0x84);
    m.w32(buf + 0x14, y);
    m.w32(buf + 0x1c, ONE);
    wf(m, buf + 0x10, x);
    m.w32(buf + 0x18, 0);
    e.call(m, SET_WORLD_POS, &[third, buf + 0x10]);
}

/// 0x2759f0(l, flags): two lists (at `l` and +0x24) emptied and destroyed, freed on flag 1
pub fn lists_dtor(m: &mut Mem, e: &mut dyn FeEnv, l: u32, flags: u32) {
    e.call(m, LIST_DELETE_ALL, &[l]);
    e.call(m, LIST_DELETE_ALL, &[l + 0x24]);
    list_dtor(m, e, l + 0x24, 2);
    list_dtor(m, e, l, 2);
    if flags & 1 != 0 {
        e.call(m, FREE, &[l]);
    }
}

/// A state's destructor (`STATE_DTORS`): its table, then the state's
pub fn state_dtor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, table: u32, flags: u32) -> u32 {
    m.w32(s + 0xc, table);
    e.call(m, STATE_DTOR, &[s, flags])
}

/// 0x28e020(x, at; x, y; `rect`): its +0x114 placed
pub fn place_child(m: &mut Mem, e: &mut dyn FeEnv, x: u32, at: u32, px: F, py: F, rect: u32) {
    let w = m.r32(x + 0x114);
    widget_place(m, e, w, at, px, py, rect);
}

/// 0x293120(o): the overlay's front end state (+0x42c) through 0x274df0, then its own +0x48 (its table at +0x438)
pub fn overlay_state_update(m: &mut Mem, e: &mut dyn FeEnv, o: u32) {
    e.call(m, STATE_274DF0, &[o + 0x42c]);
    let s = o + 0x42c;
    let vt = m.r32(o + 0x438);
    let adj = m.r16(vt + 0x48) as i16 as u32;
    let f = m.r32(vt + 0x4c);
    e.call(m, f, &[s.wrapping_add(adj)]);
}

/// `cFE3DStateNumPlayers` vf26 0x29ad00(s): its own +0x30 (its second register left holding the function)
pub fn num_players_vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let vt = m.r32(s + 0xc);
    let adj = m.r16(vt + 0x30) as i16 as u32;
    let f = m.r32(vt + 0x34);
    e.call(m, f, &[s.wrapping_add(adj), f])
}

// ---- the entries

/// `cFE3DTextIcon::~cFE3DTextIcon` 0x275d50: its table, then the text entry's
pub fn text_icon_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, TEXT_ICON_VTABLE);
    text_entry_dtor(m, e, w, flags)
}

/// `cFE3DWTextEntry::~cFE3DWTextEntry` 0x275dc8: its table, then the entry's
pub fn text_entry_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, TEXT_ENTRY_VTABLE);
    entry_dtor(m, e, w, flags)
}

/// `cFE3DIconText::~cFE3DIconText` 0x2932a8: its table, then the icon entry's
pub fn icon_text_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, ICON_TEXT_VTABLE);
    icon_entry_dtor(m, e, w, flags)
}

/// `cFE3DIconEntry::~cFE3DIconEntry` 0x293320: its table, then the entry's
pub fn icon_entry_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, ICON_ENTRY_VTABLE);
    entry_dtor(m, e, w, flags)
}

pub fn nothing() {}

pub fn zero() -> u32 {
    0
}

pub fn one() -> u32 {
    1
}

// ---- the options pages

/// The page's list (at `at` of the page; its +0x118) item `k`'s value widget (+0x114)
fn value_widget(m: &mut Mem, e: &mut dyn FeEnv, page: u32, at: u32, k: u32) -> u32 {
    let list = m.r32(page + at) + 0x118;
    let item = e.call(m, FIND, &[list, k]);
    m.r32(item + 0x114)
}

/// The widget's +0x160 with `v`, the table read first
fn set_choice(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: u32) {
    vcall(m, e, w, 0x160, &[v]);
}

/// `SoundOptions_Refresh` 0x29ef50(page): the sound page's items from the front end's sound player (its table at
/// +0): the two choices (+0x130, +0x140), the five volumes' bars (+0x150, +0x170, +0x190, +0x160, +0x1a0), and the
/// last choice the other way round (+0x1b0)
pub fn sound_refresh(m: &mut Mem, e: &mut dyn FeEnv, page: u32) {
    let fe = m.r32(m.r32(GAPP) + 0x734);
    let snd = m.r32(fe + 0xa4);
    for (k, off) in [(0u32, 0x130u32), (1, 0x140)] {
        let list = m.r32(page + 0x80) + 0x118;
        let item = e.call(m, FIND, &[list, k]);
        let w = m.r32(item + 0x114);
        let vt = m.r32(w + 0xc);
        let adj = m.r16(vt + 0x160) as i16 as u32;
        let v = vcall0(m, e, snd, off, &[]);
        let f = m.r32(vt + 0x164);
        e.call(m, f, &[w.wrapping_add(adj), v]);
    }
    for (k, off) in [(2u32, 0x150u32), (3, 0x170), (4, 0x190), (5, 0x160), (6, 0x1a0)] {
        let list = m.r32(page + 0x80) + 0x118;
        let item = e.call(m, FIND, &[list, k]);
        let w = m.r32(item + 0x114);
        let v = F(f32::from_bits(vcall0(m, e, snd, off, &[])));
        widget_set_value(m, e, w, v);
    }
    let list = m.r32(page + 0x80) + 0x118;
    let item = e.call(m, FIND, &[list, 7]);
    let w = m.r32(item + 0x114);
    let r = vcall0(m, e, snd, 0x1b0, &[]);
    set_choice(m, e, w, (r == 0) as u32);
}

/// `GameOptions_Refresh` 0x29f430(page): the game page's four choices from the career's settings (+0x1670, +0x1674,
/// +0x1684, +0x1688)
pub fn game_refresh(m: &mut Mem, e: &mut dyn FeEnv, page: u32) {
    let base = CAREER + 8;
    for (k, at) in [(0u32, 0x1670u32), (1, 0x1674), (2, 0x1684), (3, 0x1688)] {
        let w = value_widget(m, e, page, 0x8c, k);
        let v = m.r32(base + at);
        set_choice(m, e, w, v);
    }
}

/// `DisplayOptions_Refresh` 0x29f5f0(page): the display page's choices from the career's settings (+0x1678,
/// +0x167c, +0x169c off, +0x16b4, +0x16b0 less 2) and the brightness bar (0x17b970)
pub fn display_refresh(m: &mut Mem, e: &mut dyn FeEnv, page: u32) {
    let base = CAREER + 8;
    let vals = [
        m.r32(base + 0x1678),
        m.r32(base + 0x167c),
        (m.r32(base + 0x169c) == 0) as u32,
        m.r32(base + 0x16b4),
        m.r32(base + 0x16b0).wrapping_sub(2),
    ];
    for (k, v) in vals.iter().enumerate() {
        let w = value_widget(m, e, page, 0x98, k as u32);
        set_choice(m, e, w, *v);
    }
    let w = value_widget(m, e, page, 0x98, 5);
    let app = m.r32(GAPP);
    let v = F(f32::from_bits(e.call(m, BRIGHTNESS, &[app])));
    widget_set_value(m, e, w, v);
}
