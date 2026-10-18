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

// ---- the small helpers ported for row G9y7

/// The calls out: the scripted camera's part 0x26ac30, the list's add (0x2ac140), the render operations'
/// destructors (0x269558, 0x2694c8, 0x269440), the state's vf12 (0x2717e8), the sprite set's rectangle (0x1f9e50),
/// the world group (0x2720f8), the two menu lists (0x274000, 0x274108), the state's buttons (0x2718f0), the dialog
/// thing (0x273f18)
pub const SCRIPTED_PART_26AC30: u32 = 0x26ac30;
pub const LIST_ADD: u32 = 0x2ac140;
pub const RENDER_OP_DTOR: u32 = 0x269558;
pub const TEXT_OP_DTOR: u32 = 0x2694c8;
pub const UPDATE_OP_DTOR: u32 = 0x269440;
pub const STATE_VF12: u32 = 0x2717e8;
pub const SPRITE_SET_RECT: u32 = 0x1f9e50;
pub const WORLD_GROUP: u32 = 0x2720f8;
pub const CREATE_LIST: u32 = 0x274000;
pub const CREATE_LIST2: u32 = 0x274108;
pub const STATE_BUTTONS: u32 = 0x2718f0;
pub const CREATE_DIALOG: u32 = 0x273f18;
/// The tables: the state's and its world's (and the world's allocation name), the render, text and update
/// operations', and the render's matrix
pub const STATE_VTABLE: u32 = 0x3aea90;
pub const STATE_WORLD_VTABLE: u32 = 0x3b58d0;
pub const STATE_WORLD_NAME: u32 = 0x3ae548;
pub const RENDER_OP_VTABLE: u32 = 0x3aa010;
pub const TEXT_OP_VTABLE: u32 = 0x3a9ff8;
pub const UPDATE_OP_VTABLE: u32 = 0x3a9fe0;
pub const RENDER_MATRIX: u32 = 0x346440;

fn lt(a: F, b: F) -> bool {
    a.ee() < b.ee()
}

/// A venue's shot from a table of ten (none for the others, which only keep the venue)
fn venue_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32, shots: [u32; 10]) -> u32 {
    let k = k as u8 as i8 as i32 as u32;
    let r = if k < 10 { camera_play_shot(m, e, c, shots[k as usize]) } else { 0 };
    m.w8(c + 0x185, k as u8);
    r
}

/// `FE3DCamera_PanToVenue` 0x26c090(c, k): the pan to venue `k` (a signed byte; 0x86, then 0x8f down to 0x87), the
/// venue kept (+0x185)
pub fn pan_to_venue(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32) -> u32 {
    venue_shot(m, e, c, k, [0x86, 0x8f, 0x8e, 0x8d, 0x8c, 0x8b, 0x8a, 0x89, 0x88, 0x87])
}

/// `FE3DCamera_PlayVenueShot` 0x26c1a8(c, k): venue `k`'s shot (0x7c + k), the venue kept (+0x185)
pub fn play_venue_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32, k: u32) -> u32 {
    venue_shot(m, e, c, k, [0x7c, 0x7d, 0x7e, 0x7f, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85])
}

/// `VenueSel_PlayCurrentShot` 0x26c2c0(c): the kept venue's (+0x185, signed) own shot (0x90 + k); 0x3b0000 left in
/// the answer for any other
pub fn play_current_shot(m: &mut Mem, e: &mut dyn FeEnv, c: u32) -> u32 {
    let k = m.r8(c + 0x185) as i8 as i32 as u32;
    if k < 10 { camera_play_shot(m, e, c, 0x90 + k) } else { 0x3b_0000 }
}

/// 0x26c430(c, a): the scripted camera's part (+0xc0) handed on to 0x26ac30
pub fn camera_part_26c430(m: &mut Mem, e: &mut dyn FeEnv, c: u32, a: u32) -> u32 {
    e.call(m, SCRIPTED_PART_26AC30, &[c + 0xc0, a])
}

/// A list operation (at `op`) handed to the world's +0x10 (its table at +0x20), then destroyed (2)
fn world_apply(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32, dtor: u32) {
    let vt = m.r32(w + 0x20);
    let adj = m.r16(vt.wrapping_add(0x10)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(0x14));
    e.call(m, f, &[w.wrapping_add(adj), op]);
    e.call(m, dtor, &[op, 2]);
}

/// 0x26feb8(w; `buf` 0x48 bytes): the world rendered: the render's matrix (0x346440, 0x40 bytes) copied, a render
/// operation pointing at the copy handed to the world, then destroyed, and the copy's free on flag (2: kept)
pub fn render_all(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    for k in 0..16 {
        let v = m.r32(RENDER_MATRIX + 4 * k);
        m.w32(buf + 4 * k, v);
    }
    m.w32(buf + 0x40, RENDER_OP_VTABLE);
    m.w32(buf + 0x44, buf);
    world_apply(m, e, w, buf + 0x40, RENDER_OP_DTOR);
    free_on_flag(m, e, buf, 2);
}

/// `FE3DTextList_RenderAll` 0x26ff40(w; `buf` 4 bytes): a text render operation handed to the world, then destroyed
pub fn text_render_all(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    m.w32(buf, TEXT_OP_VTABLE);
    world_apply(m, e, w, buf, TEXT_OP_DTOR);
}

/// `FE3D_UpdateAll` 0x26ff88(w; `buf` 4 bytes): an update operation handed to the world, then destroyed
pub fn update_all(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) {
    m.w32(buf, UPDATE_OP_VTABLE);
    world_apply(m, e, w, buf, UPDATE_OP_DTOR);
}

/// `cFE3DState::cFE3DState` 0x2715c8(s): its links (+0 -1, +4 and +8 itself), its words (+0x10 2, +0x18..+0x20
/// and +0x28 1, the rest cleared, +0x2c and +0x2e bytes), its table; its world (+0x34: a thing of 0xd0 bytes with
/// the world's table and its list at +0xa0) added to the front end's list (+0x80); +0x2d -1
pub fn state_ctor(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    m.w32(s, u32::MAX);
    m.w32(s + 4, s);
    m.w32(s + 8, s);
    for (at, v) in [(0x14u32, 0u32), (0x18, 1), (0x1c, 1), (0x20, 1), (0x24, 0), (0x28, 1)] {
        m.w32(s + at, v);
    }
    m.w8(s + 0x2c, 0);
    m.w8(s + 0x2e, 0);
    for at in [0x30u32, 0x34, 0x38, 0x3c, 0x40, 0x44, 0x48, 0x50] {
        m.w32(s + at, 0);
    }
    m.w32(s + 0xc, STATE_VTABLE);
    m.w32(s + 0x10, 2);
    let w = e.call(m, ALLOC, &[STATE_WORLD_NAME, 0xd0, 0]);
    if w != 0 {
        e.call(m, FILL, &[w, 0xdead_c0de, 0xd0]);
    }
    // the thing: its links, flags and vectors (as `fe_widgets::widget_init` begins), then the world's table
    m.w32(w, u32::MAX);
    m.w32(w.wrapping_add(4), w);
    m.w32(w.wrapping_add(8), w);
    m.w32(w.wrapping_add(0x10), 0);
    m.w32(w.wrapping_add(0x14), 1);
    for at in [0x18u32, 0x1c, 0x20, 0x24, 0x2c, 0x30] {
        m.w32(w.wrapping_add(at), 0);
    }
    m.w32(w.wrapping_add(0x28), 1);
    for k in 0..8 {
        m.w32(w.wrapping_add(0x40 + 4 * k), 0);
    }
    for k in 0..4 {
        m.w32(w.wrapping_add(0x60 + 4 * k), ONE);
    }
    m.w32(w.wrapping_add(0x70), 0);
    m.w32(w.wrapping_add(0x74), 0);
    for (k, v) in [0, 0, 0, ONE].into_iter().enumerate() {
        m.w32(w.wrapping_add(0x80 + 4 * k as u32), v);
    }
    for k in 0..3 {
        m.w32(w.wrapping_add(0x90 + 4 * k), 0);
    }
    m.w32(w.wrapping_add(0xc), STATE_WORLD_VTABLE);
    e.call(m, crate::fe_widgets::LIST_INIT, &[w.wrapping_add(0xa0)]);
    m.w32(s + 0x34, w);
    let fe = m.r32(m.r32(GAPP) + 0x734);
    e.call(m, LIST_ADD, &[fe + 0x80, w]);
    m.w8(s + 0x2d, 0xff);
    s
}

/// `cFE3DState::~cFE3DState` 0x2717a0(s, flags): its table, its vf12 (0x2717e8), then the node's destructor
pub fn state_base_dtor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flags: u32) {
    m.w32(s + 0xc, STATE_VTABLE);
    e.call(m, STATE_VF12, &[s]);
    crate::fe_wgroup::node_dtor(m, e, s, flags);
}

/// `FE_GetSpriteRect` 0x2759c8(s, k): sprite `k`'s rectangle in the app's sprite set (+0x728)
pub fn sprite_rect(m: &mut Mem, e: &mut dyn FeEnv, k: u32) -> u32 {
    let set = m.r32(m.r32(GAPP) + 0x728);
    e.call(m, SPRITE_SET_RECT, &[set, k])
}

/// `FE3DGroup_Add` 0x2762e0(g, w): the widget added to the group's list (+0x110)
pub fn group_add(m: &mut Mem, e: &mut dyn FeEnv, g: u32, w: u32) -> u32 {
    e.call(m, LIST_ADD, &[g + 0x110, w])
}

/// `FE3D_BuildDialogScreen` 0x274208 / `FE3D_BuildDialogScreen2` 0x274278(s, title, buttons; f12): the world group
/// (title, buttons), the dialog thing (30 frames, f12), the menu list (0x274000 / 0x274108, in the root group +0x38)
/// added to the root group, then the state's buttons
pub fn dialog_screen(m: &mut Mem, e: &mut dyn FeEnv, s: u32, title: u32, buttons: u32, f12: F, second: bool) -> u32 {
    e.call(m, WORLD_GROUP, &[s, title, buttons]);
    e.call(m, CREATE_DIALOG, &[s, 0x1e, f12.0.to_bits()]);
    let root = m.r32(s + 0x38);
    e.call(m, if second { CREATE_LIST2 } else { CREATE_LIST }, &[s, root]);
    let (root, list) = (m.r32(s + 0x38), m.r32(s + 0x3c));
    group_add(m, e, root, list);
    e.call(m, STATE_BUTTONS, &[s, buttons])
}

/// `FEList_SetFocused` 0x275468(s, w): the focused widget (+0x30) told it is left (+0xf0 with 0), the new one kept
/// and told (+0xf0 with 1)
pub fn set_focused(m: &mut Mem, e: &mut dyn FeEnv, s: u32, w: u32) {
    let old = m.r32(s + 0x30);
    if old != 0 {
        vcall(m, e, old, 0xf0, &[0]);
    }
    m.w32(s + 0x30, w);
    if w != 0 {
        vcall(m, e, w, 0xf0, &[1]);
    }
}

/// 0x2755e8(s; f12 the height; `buf` 0x30 bytes): the help bar (+0x44): its fourth widget measured (unused); the
/// space left (the height less sprite 5's, less 0) when more than 0: its first two widgets sized 320 by it, scaled
/// (+0x124) by it over sprite 15's height, +0x120 1 (the first also measured, unused)
pub fn help_bar_layout(m: &mut Mem, e: &mut dyn FeEnv, s: u32, h: F, buf: u32) {
    let zero = F(0.0);
    let bar = m.r32(s + 0x44);
    let w = e.call(m, NTH, &[bar + 0x110, 3]);
    vcall(m, e, w, 0x130, &[buf]);
    let r5 = sprite_rect(m, e, 5);
    let h5 = rf(m, r5.wrapping_add(8));
    let r15 = sprite_rect(m, e, 0xf);
    let d = (h - h5) - zero;
    if !lt(zero, d) {
        return;
    }
    let h15 = rf(m, r15.wrapping_add(8));
    let scale = d / h15;
    let size = |m: &mut Mem| {
        for (k, v) in [320.0f32.to_bits(), d.0.to_bits(), 0, 0].into_iter().enumerate() {
            m.w32(buf + 0x20 + 4 * k as u32, v);
        }
    };
    let bar = m.r32(s + 0x44);
    let w = e.call(m, NTH, &[bar + 0x110, 0]);
    vcall(m, e, w, 0x130, &[buf + 0x10]);
    size(m);
    vcall(m, e, w, 0x138, &[buf + 0x20]);
    wf(m, w + 0x124, scale);
    m.w32(w + 0x120, ONE);
    let bar = m.r32(s + 0x44);
    let w = e.call(m, NTH, &[bar + 0x110, 1]);
    size(m);
    vcall(m, e, w, 0x138, &[buf + 0x20]);
    wf(m, w + 0x124, scale);
    m.w32(w + 0x120, ONE);
}

// ---- the options pages' values (row G9y7)

/// A page value: the item's choice (its chosen node's index, 0x2abec0 on the value widget's +0x140) or bar value
/// (the value widget's +0x160)
#[derive(Clone, Copy)]
enum Kind {
    Choice,
    Bar,
}

/// The pages' values: (the page's list, (item, the word of the page it is kept in, the kind) in the game's order)
const PAGE1: (u32, [(u32, u32, Kind); 8]) = (
    0x80,
    [
        (0, 0x6c, Kind::Choice),
        (1, 0x70, Kind::Choice),
        (2, 0x54, Kind::Bar),
        (3, 0x58, Kind::Bar),
        (4, 0x5c, Kind::Bar),
        (5, 0x60, Kind::Bar),
        (6, 0x64, Kind::Bar),
        (7, 0x68, Kind::Choice),
    ],
);
const PAGE2: (u32, [(u32, u32, Kind); 4]) =
    (0x8c, [(0, 0x54, Kind::Choice), (2, 0x5c, Kind::Choice), (1, 0x58, Kind::Choice), (3, 0x60, Kind::Choice)]);
const PAGE3: (u32, [(u32, u32, Kind); 6]) = (
    0x98,
    [
        (0, 0x58, Kind::Choice),
        (1, 0x5c, Kind::Choice),
        (2, 0x60, Kind::Choice),
        (3, 0x64, Kind::Choice),
        (4, 0x68, Kind::Choice),
        (5, 0x54, Kind::Bar),
    ],
);
/// The chosen node's index (0x2abec0)
pub const NODE_INDEX: u32 = 0x2abec0;

/// The value widget of item `k` of the page's list at `at`
fn page_value_widget(m: &mut Mem, e: &mut dyn FeEnv, page: u32, at: u32, k: u32) -> u32 {
    let list = m.r32(page + at) + 0x118;
    let item = e.call(m, FIND, &[list, k]);
    m.r32(item + 0x114)
}

fn read_page(m: &mut Mem, e: &mut dyn FeEnv, page: u32, at: u32, values: &[(u32, u32, Kind)]) {
    for &(k, off, kind) in values {
        let w = page_value_widget(m, e, page, at, k);
        let v = match kind {
            Kind::Choice => {
                let node = m.r32(w + 0x140);
                e.call(m, NODE_INDEX, &[node])
            }
            Kind::Bar => m.r32(w + 0x160),
        };
        m.w32(page + off, v);
    }
}

fn write_page(m: &mut Mem, e: &mut dyn FeEnv, page: u32, at: u32, values: &[(u32, u32, Kind)]) {
    for &(k, off, kind) in values {
        let w = page_value_widget(m, e, page, at, k);
        let v = m.r32(page + off);
        match kind {
            Kind::Choice => {
                vcall(m, e, w, 0x160, &[v]);
            }
            Kind::Bar => widget_set_value(m, e, w, F(f32::from_bits(v))),
        }
    }
}

/// `FE3DOptions_ReadPage1` 0x29e988 / `ReadPage2` 0x29ebc0 / `ReadPage3` 0x29ed50(page): the page's values kept
/// (the sound page's two choices, five bars and last choice; the game page's four choices; the display page's five
/// choices and bar)
pub fn read_page_n(m: &mut Mem, e: &mut dyn FeEnv, page: u32, n: u32) {
    match n {
        1 => read_page(m, e, page, PAGE1.0, &PAGE1.1),
        2 => read_page(m, e, page, PAGE2.0, &PAGE2.1),
        _ => read_page(m, e, page, PAGE3.0, &PAGE3.1),
    }
}

/// `FE3DOptions_WritePage1` 0x29ea90 / `WritePage2` 0x29ec70 / `WritePage3` 0x29ee30(page): the kept values put
/// back (a choice through the value widget's +0x160, a bar's value through 0x2b3950)
pub fn write_page_n(m: &mut Mem, e: &mut dyn FeEnv, page: u32, n: u32) {
    match n {
        1 => write_page(m, e, page, PAGE1.0, &PAGE1.1),
        2 => write_page(m, e, page, PAGE2.0, &PAGE2.1),
        _ => write_page(m, e, page, PAGE3.0, &PAGE3.1),
    }
}

/// The id (+0) of the chosen node of item `k`'s value widget
fn chosen_id(m: &mut Mem, e: &mut dyn FeEnv, page: u32, at: u32, k: u32) -> u32 {
    let w = page_value_widget(m, e, page, at, k);
    let node = m.r32(w + 0x140);
    m.r32(node)
}

/// `SoundOptions_Apply` 0x29f178(page): the sound page into the career's settings and the sound player (the front
/// end's +0xa4, its table at +0): the mode by the first choice's id (8, 9, 10, 11: 0..3; +0x1668, the player's
/// +0x138), the second choice's index (+0x148), the five bars (+0x158, +0x178, +0x198, +0x168, +0x1a8 with f12;
/// kept at +0x1650, +0x1658, +0x1660, +0x1654, +0x1664), the last choice on when its id is 0x12 (+0x1b8; +0x166c)
pub fn sound_apply(m: &mut Mem, e: &mut dyn FeEnv, page: u32) {
    let base = CAREER + 8;
    let fe = m.r32(m.r32(GAPP) + 0x734);
    let snd = m.r32(fe + 0xa4);
    let id = chosen_id(m, e, page, 0x80, 0);
    let mode = match id {
        8 => Some(0),
        9 => Some(1),
        10 => Some(2),
        11 => Some(3),
        _ => None,
    };
    if let Some(v) = mode {
        m.w32(base + 0x1668, v);
        vcall0(m, e, snd, 0x138, &[v]);
    }
    let w = page_value_widget(m, e, page, 0x80, 1);
    let node = m.r32(w + 0x140);
    let i = e.call(m, NODE_INDEX, &[node]);
    vcall0(m, e, snd, 0x148, &[i]);
    for (k, off, at) in [(2u32, 0x158u32, 0x1650u32), (3, 0x178, 0x1658), (4, 0x198, 0x1660), (5, 0x168, 0x1654), (6, 0x1a8, 0x1664)] {
        let w = page_value_widget(m, e, page, 0x80, k);
        let v = m.r32(w + 0x160);
        vcall0(m, e, snd, off, &[v]);
        let v = m.r32(w + 0x160);
        m.w32(base + at, v);
    }
    let on = chosen_id(m, e, page, 0x80, 7) == 0x12;
    vcall0(m, e, snd, 0x1b8, &[on as u32]);
    m.w32(base + 0x166c, on as u32);
}

/// `GameOptions_Apply` 0x29f508(page): the game page's four choices into the career's settings: +0x1670 and +0x1674
/// on for the id 5, +0x1684 and +0x1688 for the id 7
pub fn game_apply(m: &mut Mem, e: &mut dyn FeEnv, page: u32) {
    let base = CAREER + 8;
    for (k, at, on) in [(0u32, 0x1670u32, 5u32), (1, 0x1674, 5), (2, 0x1684, 7), (3, 0x1688, 7)] {
        let id = chosen_id(m, e, page, 0x8c, k);
        m.w32(base + at, (id == on) as u32);
    }
}

/// `OptionsState_Refresh` 0x29fb40(page): the sound, game and display pages refreshed
pub fn options_refresh(m: &mut Mem, e: &mut dyn FeEnv, page: u32) {
    sound_refresh(m, e, page);
    game_refresh(m, e, page);
    display_refresh(m, e, page);
}

/// The pad helpers the game page calls (0x26d1c0 with on / off, 0x26d1c8 the rumble)
pub const PAD_RUMBLE_SET: u32 = 0x26d1c0;
pub const PAD_RUMBLE: u32 = 0x26d1c8;

/// The page's two arrows (+0xc0, +0xc4): the first shown and the second hidden, or the other way round
fn arrows(m: &mut Mem, e: &mut dyn FeEnv, page: u32, first: bool) {
    let a = m.r32(page + 0xc0);
    vcall(m, e, a, 0x28, &[first as u32]);
    let b = m.r32(page + 0xc4);
    vcall(m, e, b, 0x28, &[(!first) as u32]);
}

/// 0x2a9350(page, item): unless the item is the end marker (0xabcd1234), the game page's item that shows the
/// arrows: by the item's id 0 and 2 the third (2), 1 and 3 the fourth (3); for 4..7 by the page list's chosen id
/// (+0x8c's +0x140) the same way; its chosen id 6 shows the first arrow, else the second. (Other ids read the
/// value widget of no item, the word at 0x114, as the game does)
pub fn game_line_arrows(m: &mut Mem, e: &mut dyn FeEnv, page: u32, item: u32) {
    let mut id = m.r32(item);
    if id == 0xabcd_1234 {
        return;
    }
    let list = m.r32(page + 0x8c);
    if (4..8).contains(&(id as i32)) {
        let node = m.r32(list + 0x140);
        id = m.r32(node);
    }
    let found = match id as i32 {
        0 | 2 => e.call(m, FIND, &[list + 0x118, 2]),
        1 | 3 => e.call(m, FIND, &[list + 0x118, 3]),
        _ => 0,
    };
    let w = m.r32(found.wrapping_add(0x114));
    let node = m.r32(w + 0x140);
    let chosen = m.r32(node);
    arrows(m, e, page, chosen == 6);
}

/// `SoundOptions_OnChange` 0x2a9720(page, a, node): by the chosen node's id: 8..11 the mode (the node's index to the
/// sound player's +0x138, then its +0x50 with 0x10), 12..17 the second choice (+0x148, the page refreshed, then +0x50
/// with 0x11), 18 and 19 the last choice (on for 0x12; +0x1b8, then +0x50 with 0x17)
pub fn sound_on_change(m: &mut Mem, e: &mut dyn FeEnv, page: u32, node: u32) {
    let fe = m.r32(m.r32(GAPP) + 0x734);
    let snd = m.r32(fe + 0xa4);
    let id = m.r32(node);
    match id {
        8..=11 => {
            let i = e.call(m, NODE_INDEX, &[node]);
            vcall0(m, e, snd, 0x138, &[i]);
            vcall0(m, e, snd, 0x50, &[0x10]);
        }
        12..=17 => {
            let i = e.call(m, NODE_INDEX, &[node]);
            vcall0(m, e, snd, 0x148, &[i]);
            sound_refresh(m, e, page);
            vcall0(m, e, snd, 0x50, &[0x11]);
        }
        18 | 19 => {
            vcall0(m, e, snd, 0x1b8, &[(id == 0x12) as u32]);
            vcall0(m, e, snd, 0x50, &[0x17]);
        }
        _ => {}
    }
}

/// 0x2a9848(page, a, node): the game page's change: by the chosen node's id: 5 the rumble on the pad that changed it
/// (the node's +0xf0's id 0: the front end's +0x74, else +0x78): on, rumbled, off; 6 the first arrow shown, 7 the
/// second
pub fn game_on_change(m: &mut Mem, e: &mut dyn FeEnv, page: u32, node: u32) {
    match m.r32(node) {
        5 => {
            let at = if m.r32(m.r32(node + 0xf0)) == 0 { 0x74 } else { 0x78 };
            let pad = |m: &Mem| m.r32(m.r32(m.r32(GAPP) + 0x734) + at);
            let p = pad(m);
            e.call(m, PAD_RUMBLE_SET, &[p, 1]);
            let p = pad(m);
            e.call(m, PAD_RUMBLE, &[p]);
            let p = pad(m);
            e.call(m, PAD_RUMBLE_SET, &[p, 0]);
        }
        6 => arrows(m, e, page, true),
        7 => arrows(m, e, page, false),
        _ => {}
    }
}

/// An unsigned int as a float (`cvt.s.w`, halved and doubled past 2^31)
fn ufl(v: u32) -> F {
    if (v as i32) >= 0 {
        fl(v)
    } else {
        let h = fl((v >> 1) | (v & 1));
        h + h
    }
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

/// `DisplayOptions_OnChange` 0x2a99a0(page, a, node): by the chosen node's id, into the career's settings: 7 / 8 the
/// first on / off (+0x1678); 9..11 the screen mode 0..2 (+0x167c): the renderer's (the app's +0x724, its table at
/// +0x10) +0x120 with it, then its +0x98 with (0, 0, the width and height from +0x58 and +0x60 as floats); 12 / 13
/// +0x169c on / off; 14 / 15 +0x16b4 off / on; 16..20 +0x16b0 the id less 14
pub fn display_on_change(m: &mut Mem, e: &mut dyn FeEnv, node: u32) {
    let base = CAREER + 8;
    let id = m.r32(node);
    match id {
        7 => m.w32(base + 0x1678, 1),
        8 => m.w32(base + 0x1678, 0),
        9..=11 => {
            let mode = id - 9;
            m.w32(base + 0x167c, mode);
            let r = m.r32(m.r32(GAPP) + 0x724);
            vcall_at(m, e, r, 0x10, 0x120, &[mode]);
            let w = ufl(vcall_at(m, e, r, 0x10, 0x58, &[]));
            let h = ufl(vcall_at(m, e, r, 0x10, 0x60, &[]));
            let z = 0.0f32.to_bits();
            vcall_at(m, e, r, 0x10, 0x98, &[z, z, w.0.to_bits(), h.0.to_bits()]);
        }
        12 => m.w32(base + 0x169c, 1),
        13 => m.w32(base + 0x169c, 0),
        14 => m.w32(base + 0x16b4, 0),
        15 => m.w32(base + 0x16b4, 1),
        16..=20 => m.w32(base + 0x16b0, id - 0xe),
        _ => {}
    }
}

/// 0x2a9c50: nothing
pub fn nothing_2a9c50() {}

/// A virtual call through a table kept at `obj + table`
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, table: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj + table);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// The help lines' string ids for each page (by the item's id) and the controller page's own string
pub const HELP_IDS: [u32; 5] = [0x3481d0, 0x3481e8, 0x348238, 0x3483f8, 0x348450];
pub const CONTROLLER_HELP: u32 = 0x3b42c8;
pub const SET_STRING: u32 = 0x2b0fb8;
/// A text's string by id (0x2b0f68)
pub const SET_STRING_ID: u32 = 0x2b0f68;

/// `FE3DOptions_SetHelpText` 0x2aa998(s, item): the help line (+0x4c): 0xdce for the end marker (0xabcd1234), else
/// by the open page (+0x78, from -1): the page's string for the item's id (pages -1..3), the controller page's own
/// string (page 4) or as it was; then laid out (+0x140)
pub fn set_help_text(m: &mut Mem, e: &mut dyn FeEnv, s: u32, item: u32) {
    let id = m.r32(item);
    if id == 0xabcd_1234 {
        let t = m.r32(s + 0x4c);
        e.call(m, SET_STRING_ID, &[t, 0xdce]);
    } else {
        let k = m.r32(s + 0x78).wrapping_add(1);
        if k < 5 {
            let t = m.r32(s + 0x4c);
            let sid = m.r32(HELP_IDS[k as usize].wrapping_add(id.wrapping_mul(4)));
            e.call(m, SET_STRING_ID, &[t, sid]);
        } else if k == 5 {
            let t = m.r32(s + 0x4c);
            e.call(m, SET_STRING, &[t, CONTROLLER_HELP]);
        }
    }
    let t = m.r32(s + 0x4c);
    vcall(m, e, t, 0x140, &[]);
}

/// The controller page's three groups' layouts (count, the table of 16-byte entries: x, y, flags)
pub const CONTROLLER_LAYOUTS: [(u32, u32); 3] = [(7, 0x3482c8), (9, 0x348338), (7, 0x348258)];

/// `ControllerConfig_Layout` 0x2a9488(s; `buf` 0x20 bytes): each of the controller page's three groups (+0xc0..):
/// each widget laid out (+0x140) and placed at the group's position plus its entry's; by the entry's flags, moved
/// back (+0x70) by its width (8) or half of it (0x20), and up by its height (4) or half of it (0x40)
pub fn controller_layout(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let half = F(0.5);
    for (p, &(count, table)) in CONTROLLER_LAYOUTS.iter().enumerate() {
        let at = s + 0xc0 + 4 * p as u32;
        let g = m.r32(at);
        let (x0, y0) = (rf(m, g + 0x80), rf(m, g + 0x84));
        for i in 0..count {
            let g = m.r32(at);
            let w = e.call(m, NTH, &[g + 0x110, i]);
            vcall(m, e, w, 0x140, &[]);
            let entry = table + 16 * i;
            let x = x0 + rf(m, entry);
            let y = y0 + rf(m, entry + 4);
            for (k, v) in [x.0.to_bits(), y.0.to_bits(), 0, ONE].into_iter().enumerate() {
                m.w32(buf + 4 * k as u32, v);
            }
            e.call(m, crate::fe3dstate::WIDGET_PLACE, &[w, buf]);
            let flags = m.r32(entry + 8);
            if flags & 0x68 == 0 {
                continue;
            }
            vcall(m, e, w, 0x130, &[buf]);
            let (sw, sh) = (rf(m, buf), rf(m, buf + 4));
            let shift = |m: &mut Mem, e: &mut dyn FeEnv, dx: F, dy: F| {
                for (k, v) in [dx.0.to_bits(), dy.0.to_bits(), 0, 0].into_iter().enumerate() {
                    m.w32(buf + 0x10 + 4 * k as u32, v);
                }
                vcall(m, e, w, 0x70, &[buf + 0x10]);
            };
            let zero = F(0.0);
            if flags & 8 != 0 {
                shift(m, e, -sw, zero);
            } else if flags & 0x20 != 0 {
                shift(m, e, -sw * half, zero);
            }
            if flags & 4 != 0 {
                shift(m, e, zero, -sh);
            } else if flags & 0x40 != 0 {
                shift(m, e, zero, -sh * half);
            }
        }
    }
}

/// The scrolling text's constructor (0x2b1b20: the text, the screen, the parent, the font) and its allocation name
pub const SCROLL_TEXT_CTOR: u32 = 0x2b1b20;
pub const CREDITS_NAME: u32 = 0x3b4108;

/// `FE_BuildCredits` 0x2a8380(s; `buf` 0x10 bytes): the credits page (+0xb0) given a scrolling text (0x1c0 bytes, the
/// menu font; lines 0x85f..0xbaa at +0x198 / +0x19c) in the highlight colour, white, 300 high; the page laid out, its
/// +0x112 -1; the page's other group (+0xb4) hidden (+0x20 with 0)
pub fn build_credits(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let page = m.r32(s + 0xb0);
    let t = e.call(m, ALLOC, &[CREDITS_NAME, 0x1c0, 0]);
    if t != 0 {
        e.call(m, FILL, &[t, 0xdead_c0de, 0x1c0]);
    }
    let font = m.r32(m.r32(GAPP) + 0x60);
    let t = e.call(m, SCROLL_TEXT_CTOR, &[t, s, page, font]);
    m.w32(t + 0x19c, 0xbaa);
    m.w32(t + 0x198, 0x85f);
    let put = |m: &mut Mem, v: [u32; 4]| {
        for (k, x) in v.into_iter().enumerate() {
            m.w32(buf + 4 * k as u32, x);
        }
    };
    put(m, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
    vcall(m, e, t, 0x108, &[buf]);
    put(m, [ONE; 4]);
    vcall(m, e, t, 0x110, &[buf]);
    put(m, [0, 300.0f32.to_bits(), 0, 0]);
    vcall(m, e, t, 0x138, &[buf]);
    vcall(m, e, page, 0x148, &[t]);
    vcall(m, e, page, 0x140, &[]);
    m.w8(page + 0x112, 0xff);
    let other = m.r32(s + 0xb4);
    vcall(m, e, other, 0x20, &[0]);
}

/// The title's two arrows' set-ups (0x2adf18 the left, 0x2adf30 the right) and their allocation name
pub const ARROW_LEFT: u32 = 0x2adf18;
pub const ARROW_RIGHT: u32 = 0x2adf30;
pub const ARROW_NAME: u32 = 0x3b4110;

/// 0x2a8520(s; `buf` 0x20 bytes): the title's two arrows (icons of 0x120 bytes made in place: sized 10 × 10 by the
/// class, set up as the left / right arrow, then 9 × 9, raised (+0x70 1) and hidden), kept at +0xb8 / +0xbc and added
/// to the state
pub fn title_arrows(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    let mut r = 0;
    for (setup, at) in [(ARROW_LEFT, 0xb8u32), (ARROW_RIGHT, 0xbc)] {
        let i = e.call(m, ALLOC, &[ARROW_NAME, 0x120, 0]);
        if i != 0 {
            e.call(m, FILL, &[i, 0xdead_c0de, 0x120]);
        }
        crate::fe_widgets::icon_init(m, e, i, s, 0);
        let put = |m: &mut Mem, at: u32, v: f32| {
            for (k, x) in [v.to_bits(), v.to_bits(), 0, 0].into_iter().enumerate() {
                m.w32(at + 4 * k as u32, x);
            }
        };
        put(m, buf + 0x10, 10.0);
        crate::fe_widgets::vcall_class(m, e, crate::fe_widgets::ICON_VTABLE, i, 0x138, &[buf + 0x10]);
        e.call(m, setup, &[i]);
        put(m, buf, 9.0);
        vcall(m, e, i, 0x138, &[buf]);
        m.w32(i + 0x70, 1);
        vcall(m, e, i, 0x28, &[0]);
        m.w32(s + at, i);
        r = e.call(m, STATE_ADD, &[s, i]);
    }
    r
}

/// The single-line text's constructor (0x2b1138) and the page's texts' allocation name
pub const TEXT_CTOR: u32 = 0x2b1138;
pub const PAGE_TEXT_NAME: u32 = 0x3b4080;

/// `FE3D_BuildSingleText` 0x2a7fd8(s; `buf` 0x10 bytes): the page at +0xa4 (+0x154 cleared) given three lines (ids
/// 0..2; strings 0xde, 0xc6, 0xc7): each a single-line text (0x130 bytes, the menu font) in white with the highlight
/// colour, added; the page laid out (+0x140, its answer)
pub fn build_single_text(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) -> u32 {
    let page = m.r32(s + 0xa4);
    m.w32(page + 0x154, 0);
    let put = |m: &mut Mem, v: [u32; 4]| {
        for (k, x) in v.into_iter().enumerate() {
            m.w32(buf + 4 * k as u32, x);
        }
    };
    for (id, sid) in [(0u32, 0xdeu32), (1, 0xc6), (2, 0xc7)] {
        let t = e.call(m, ALLOC, &[PAGE_TEXT_NAME, 0x130, 0]);
        if t != 0 {
            e.call(m, FILL, &[t, 0xdead_c0de, 0x130]);
        }
        let font = m.r32(m.r32(GAPP) + 0x60);
        let t = e.call(m, TEXT_CTOR, &[t, s, page, font]);
        m.w32(t, id);
        vcall(m, e, t, 0x150, &[sid]);
        put(m, [ONE; 4]);
        vcall(m, e, t, 0x110, &[buf]);
        put(m, [ONE, 0x3f73_f7cf, 0x3f3a_9fbe, 0x3dd9_1687]);
        vcall(m, e, t, 0x108, &[buf]);
        put(m, [ONE; 4]);
        vcall(m, e, t, 0x50, &[buf]);
        vcall(m, e, page, 0x148, &[t]);
    }
    vcall(m, e, page, 0x140, &[])
}
