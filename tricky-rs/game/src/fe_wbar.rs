//! The front end's bars and pictures (board row G9x4) on the game's bytes: the progress bar (`cFE3DProgressBar` and
//! its `FE3DProgressBar_*`: a value between two limits, drawn as a filled bar with a gradient, an optional cap and two
//! arrows), the icons (`cFE3DIcon`, `cFE3DTiledIcon`), the video and options states' small virtuals, and the range's
//! unnamed helpers (setters through the stack, operation applies, constructors for the list box, single-line text and
//! the second button layout). Every call out goes through [`FeEnv`]; the functions of the row that call each other,
//! and the widget's own (`fe_widget`, `fe_wgroup`, `fe_widgets`), are called directly. Checked in the function runner
//! (`tests/fe_wbar.rs`).
//!
//! The progress bar: +0x110 and +0x120 the colours at the low and high ends, +0x130 the background, +0x140 the cap,
//! +0x150 the top and +0x154 the bottom of the range, +0x158 / +0x15c the highest / lowest the value may be set to
//! (-9999 for none), +0x160 the value, +0x164 the step, +0x168 the way it fills (0 right, 1 left, 2 up, 3 down),
//! +0x16c..+0x174 the background, end and fill sprites (the end's below 0 for none), +0x178..+0x184 the tiles'
//! repeats, +0x188 / +0x18c the arrows towards the bottom and the top.

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::fe_wentry::{sprite_rect, LIST_FIRST, SET_WORLD_POS};
use crate::fe_wgroup::{widget_place, widget_slide_in, LIST_INIT};
use crate::fe_widget::{destroy, DRAWER, DRAWER_COLOUR, TEXT_VTABLE};
use crate::fe_widgets::{vcall_class, widget_init, ICON_VTABLE, PROGRESS_VTABLE};
use crate::fe_wentry::LISTBOX_VTABLE;
use crate::pf::Mem;
use crate::ps2f::F;

/// The tables: the options and video states, the tiled icon
pub const OPTIONS_VTABLE: u32 = 0x3b42e8;
pub const VIDEO_VTABLE: u32 = 0x3b5010;
pub const TILED_VTABLE: u32 = 0x3b6e40;
/// The state's destructor (0x2717a0); the narrow copy (0x2fd14c); a list's push to the back (0x2ac140); the sprite
/// draws: a rectangle (0x1fc178: the rectangle, the flip; x, y, w, h), tiled (0x1fc728: the rectangle, the flip; x,
/// y, w, h, the repeats across and down), with four corner colours (0x1fccc8: the rectangle, the flip, the four
/// colours; x, y, w, h, the repeats)
pub const STATE_DTOR: u32 = 0x2717a0;
pub const STRCPY: u32 = 0x2fd14c;
pub const LIST_PUSH_BACK: u32 = 0x2ac140;
pub const DRAW_RECT: u32 = 0x1fc178;
pub const DRAW_TILED: u32 = 0x1fc728;
pub const DRAW_SHADED: u32 = 0x1fccc8;
/// The pad: left, right, up, down, accept
pub const INPUT_LEFT: u32 = 0x26ce98;
pub const INPUT_RIGHT: u32 = 0x26cea8;
pub const INPUT_UP: u32 = 0x26ce78;
pub const INPUT_DOWN: u32 = 0x26ce88;
pub const INPUT_ACCEPT: u32 = 0x26ce08;
/// "No limit"
pub const NO_LIMIT: F = F(-9999.0);

const ONE: u32 = 0x3f80_0000;
const HALF: F = F(0.5);

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn wf(m: &mut Mem, at: u32, v: F) {
    m.w32(at, v.0.to_bits());
}

/// `c.olt` / `c.ole` / `c.eq` as the EE does them: no NaN (an exponent of all ones is a huge value), denormals as 0
fn lt(a: F, b: F) -> bool {
    a.ee() < b.ee()
}

fn le(a: F, b: F) -> bool {
    a.ee() <= b.ee()
}

fn eq(a: F, b: F) -> bool {
    a.ee() == b.ee()
}

fn b(v: F) -> u32 {
    v.0.to_bits()
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

fn rcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![r.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

// ---- the states' small virtuals

/// `cFE3DStateOptions::~cFE3DStateOptions` 0x2aab58 / `cFE3DStateVideo::~cFE3DStateVideo` 0x2abb60 (s, flags): the
/// class's table, then the state's destructor
pub fn state_dtor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, vtable: u32, flags: u32) -> u32 {
    m.w32(s + 0xc, vtable);
    e.call(m, STATE_DTOR, &[s, flags])
}

/// `cFE3DStateVideo` vf11 0x2ab910: nothing
pub fn video_vf11() {}

/// `FE3DState_SetName` 0x2abb58(s, name): the name copied to +0x54
pub fn state_set_name(m: &mut Mem, e: &mut dyn FeEnv, s: u32, name: u32) -> u32 {
    e.call(m, STRCPY, &[s + 0x54, name])
}

/// `cFE3DStateVideo` vf13 0x2abbc0: 12
pub fn video_vf13() -> u32 {
    0xc
}

// ---- the small helpers

/// `FE3D_AddWidget` 0x2abc68(g, w): the widget put last in the group's list (+0xa0)
pub fn add_widget(m: &mut Mem, e: &mut dyn FeEnv, g: u32, w: u32) -> u32 {
    e.call(m, LIST_PUSH_BACK, &[g + 0xa0, w])
}

/// 0x2ac698(g): the group's first widget
pub fn first_widget(m: &mut Mem, e: &mut dyn FeEnv, g: u32) -> u32 {
    e.call(m, LIST_FIRST, &[g + 0xa0])
}

/// 0x2aca88(t, v): the thing's +0x90 (x, y, z) from `v`
pub fn set_90(m: &mut Mem, t: u32, v: u32) {
    for k in 0..3 {
        m.w32(t + 0x90 + 4 * k, m.r32(v + 4 * k));
    }
}

/// 0x2acaa8(t; x, y, z; `buf` 0x10 bytes): the same from three floats
pub fn set_90_xyz(m: &mut Mem, t: u32, x: F, y: F, z: F, buf: u32) {
    for (k, v) in [x, y, z].iter().enumerate() {
        wf(m, buf + 4 * k as u32, *v);
    }
    set_90(m, t, buf);
}

/// 0x2acb70(t; x, y, z, w; `buf`): its world position (0x2acb20) from four floats
pub fn set_world_xyzw(m: &mut Mem, e: &mut dyn FeEnv, t: u32, v: [F; 4], buf: u32) {
    for (k, x) in v.iter().enumerate() {
        wf(m, buf + 4 * k as u32, *x);
    }
    e.call(m, SET_WORLD_POS, &[t, buf]);
}

/// 0x2acc40(t; r, g, b, a; `buf`): its colour (+0x50) from four floats
pub fn set_colour_rgba(m: &mut Mem, e: &mut dyn FeEnv, t: u32, v: [F; 4], buf: u32) {
    for (k, x) in v.iter().enumerate() {
        wf(m, buf + 4 * k as u32, *x);
    }
    vcall(m, e, t, 0x50, &[buf]);
}

/// 0x2ad148(op, child): the child's +0x38; 0
pub fn op_skip(m: &mut Mem, e: &mut dyn FeEnv, child: u32) -> u32 {
    vcall(m, e, child, 0x38, &[]);
    0
}

/// 0x2ad1c8(op, child): the child's +0x70 with the operation's vector (+0x10); 0
pub fn op_move(m: &mut Mem, e: &mut dyn FeEnv, op: u32, child: u32) -> u32 {
    vcall(m, e, child, 0x70, &[op + 0x10]);
    0
}

/// 0x2adcb8(out, v; s): every lane times `s`; `out`
pub fn vec_scale(m: &mut Mem, out: u32, v: u32, s: F) -> u32 {
    let x = q(m, v).map(|x| x * s);
    put(m, out, x);
    out
}

/// 0x2adf88(w; `rect`): slid in at anchor 3 over 15 frames from (18, 18), then +0x114 16
pub fn slide_in_3(m: &mut Mem, e: &mut dyn FeEnv, w: u32, rect: u32) {
    widget_slide_in(m, e, w, 3, 0xf, F(18.0), F(18.0), rect);
    m.w32(w + 0x114, 0x10);
}

/// 0x2b3820 / 0x2b3860(w; v; `buf`): its size (+0x138) with the height / width set to `v` and the other kept
pub fn set_size_one(m: &mut Mem, e: &mut dyn FeEnv, w: u32, height: bool, v: F, buf: u32) {
    let (x, y) = if height { (rf(m, w + 0x100), v) } else { (v, rf(m, w + 0x104)) };
    wf(m, buf, x);
    wf(m, buf + 4, y);
    m.w32(buf + 8, 0);
    m.w32(buf + 0xc, 0);
    vcall(m, e, w, 0x138, &[buf]);
}

/// 0x2aff90(lb, w): the widget put last in the list box's list, and its first shown
pub fn listbox_add(m: &mut Mem, e: &mut dyn FeEnv, lb: u32, w: u32) {
    e.call(m, LIST_PUSH_BACK, &[lb + 0x114, w]);
    let f = e.call(m, LIST_FIRST, &[lb + 0x114]);
    m.w32(lb + 0x110, f);
}

/// 0x2b2ec0(lb): a list box with no owner or parent (its shown widget not set)
pub fn listbox_ctor_bare(m: &mut Mem, e: &mut dyn FeEnv, lb: u32) -> u32 {
    widget_init(m, e, lb, 0, 0);
    m.w32(lb + 0xc, LISTBOX_VTABLE);
    e.call(m, LIST_INIT, &[lb + 0x114]);
    lb
}

/// 0x2b32a8 / 0x2b3300(w, c): the entry's second / first child destroyed (+8 with 3) and replaced
pub fn entry_set_child(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, c: u32) {
    let old = m.r32(w + at);
    if old != 0 {
        vcall(m, e, old, 8, &[3]);
    }
    m.w32(w + at, c);
}

/// 0x2b4ab0(t): a single-line text with no owner or parent (its own words not set)
pub fn text_ctor_bare(m: &mut Mem, e: &mut dyn FeEnv, t: u32) -> u32 {
    widget_init(m, e, t, 0, 0);
    m.w32(t + 0xc, TEXT_VTABLE);
    t
}

/// 0x2ae230 (the help buttons' set-up; w, at, text; `buf` 0x40 bytes): as `FE3DButton_Layout` with the children the
/// other way round (the icon at +0x114, the label at +0x110), no first gap, and the boxes one higher than the icon
pub fn button_layout_2(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, text: u32, buf: u32) {
    let icon = m.r32(w + 0x114);
    let r = sprite_rect(m, e, at);
    let mut x = rf(m, r.wrapping_add(4));
    let r = sprite_rect(m, e, at);
    let mut y = rf(m, r.wrapping_add(8));
    let top = F(17.0);
    if lt(top, x) {
        x = top;
    }
    if lt(top, y) {
        y = top;
    }
    widget_place(m, e, icon, at, x, y, buf + 0x20);
    for k in 0..4 {
        m.w32(buf + 4 * k, ONE);
    }
    vcall(m, e, icon, 0x50, &[buf]);
    let label = m.r32(w + 0x110);
    vcall(m, e, label, 0x150, &[text]);
    m.w32(label + 0x118, 0x3f33_3333);
    m.w32(label + 0x114, 0);
    for (k, v) in [ONE, 0, 0, 0].into_iter().enumerate() {
        m.w32(buf + 4 * k as u32, v);
    }
    vcall(m, e, label, 0x50, &[buf]);
    let v = [0, b(y + F(1.0)), 0, ONE];
    for (k, x) in v.iter().enumerate() {
        m.w32(((w + 0x120) & !0xf) + 4 * k as u32, *x);
        m.w32(((w + 0x140) & !0xf) + 4 * k as u32, *x);
    }
    m.w32(w + 0x11c, 1);
    m.w32(w + 0x130, 2);
    wf(m, w + 0x118, F(4.0));
    vcall(m, e, w, 0x140, &[]);
}

// ---- the icons

/// `cFE3DIcon` vf11 0x2adce8 / `cFE3DTiledIcon` vf11 0x2adfc8 (drawn): while shown, with the renderer's state raised,
/// the sprite (+0x110, its flip +0x114) drawn at the position and size in the widget's colour, tiled by +0x120 and
/// +0x124 for the tiled icon
pub fn icon_draw(m: &mut Mem, e: &mut dyn FeEnv, w: u32, tiled: bool) {
    if m.r32(w + 0x14) == 0 {
        return;
    }
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(w + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    e.call(m, DRAWER_COLOUR, &[DRAWER, w + 0x60]);
    let sprite = m.r32(w + 0x110);
    let r = sprite_rect(m, e, sprite);
    let flip = m.r32(w + 0x114);
    let mut a = vec![r, flip, m.r32(w + 0x80), m.r32(w + 0x84), m.r32(w + 0x100), m.r32(w + 0x104)];
    if tiled {
        a.extend([m.r32(w + 0x120), m.r32(w + 0x124)]);
        e.call(m, DRAW_TILED, &a);
    } else {
        e.call(m, DRAW_RECT, &a);
    }
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DIcon` vf19 0x2b43e8, `cFE3DTiledIcon` vf20 0x2b3f38 and `cFE3DProgressBar` vf23 0x2b37d8: 1
pub fn widget_one() -> u32 {
    1
}

/// `cFE3DIcon::~cFE3DIcon` 0x2b4408(w, flags): its table, then the widget's destructor
pub fn icon_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, ICON_VTABLE);
    destroy(m, e, w, flags)
}

/// `cFE3DTiledIcon::~cFE3DTiledIcon` 0x2b3f60(w, flags): its table, then the icon's destructor
pub fn tiled_dtor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, flags: u32) -> u32 {
    m.w32(w + 0xc, TILED_VTABLE);
    icon_dtor(m, e, w, flags)
}

/// `cFE3DIcon::cFE3DIcon` 0x2b4620(w): the widget with no owner or parent, then its table
pub fn icon_ctor(m: &mut Mem, e: &mut dyn FeEnv, w: u32) -> u32 {
    widget_init(m, e, w, 0, 0);
    m.w32(w + 0xc, ICON_VTABLE);
    w
}

/// `cFE3DTiledIcon::cFE3DTiledIcon` 0x2b4190(w; `buf` 0x10 bytes): the widget with no owner or parent, the icon's
/// words (no sprite, +0x114 0) and table, sized 10 by 10 through the icon's +0x138, then its own table
pub fn tiled_ctor(m: &mut Mem, e: &mut dyn FeEnv, w: u32, buf: u32) -> u32 {
    widget_init(m, e, w, 0, 0);
    m.w32(w + 0x110, u32::MAX);
    m.w32(w + 0x114, 0);
    m.w32(w + 0xc, ICON_VTABLE);
    for (k, v) in [F(10.0), F(10.0), F(0.0), F(0.0)].iter().enumerate() {
        wf(m, buf + 4 * k as u32, *v);
    }
    vcall_class(m, e, ICON_VTABLE, w, 0x138, &[buf]);
    m.w32(w + 0xc, TILED_VTABLE);
    w
}

// ---- the progress bar

/// One arrow put beside the bar by the way it fills (`towards_top` for the arrow at +0x18c): shown while the value
/// is past its end, then moved next to that end (5 apart) and centred across
fn place_arrow(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, at: u32, towards_top: bool, centre: [F; 2], buf: u32) {
    let a = m.r32(bar + at);
    let past = if towards_top {
        le(rf(m, bar + 0x150), rf(m, bar + 0x160))
    } else {
        le(rf(m, bar + 0x160), rf(m, bar + 0x154))
    };
    if past {
        vcall(m, e, a, 0x28, &[0]);
        return;
    }
    vcall(m, e, a, 0x28, &[1]);
    let a = m.r32(bar + at);
    vcall(m, e, a, 0x130, &[buf]);
    let mut p = q(m, bar + 0x80);
    let (sx, sy) = (rf(m, buf), rf(m, buf + 4));
    let (w, h) = (rf(m, bar + 0x100), rf(m, bar + 0x104));
    let five = F(5.0);
    // (the end the arrow sits at: after the far edge, or before the near one)
    let dir = m.r32(bar + 0x168);
    let far = match (dir, towards_top) {
        (1, false) | (0, true) | (2, false) | (3, true) => Some(true),
        (0, false) | (1, true) | (3, false) | (2, true) => Some(false),
        _ => None,
    };
    if let Some(far) = far {
        if dir < 2 {
            p[0] = if far { p[0] + (w + five) } else { p[0] - (sx + five) };
            p[1] = centre[1] - sy * HALF;
        } else {
            p[0] = centre[0] - sx * HALF;
            p[1] = if far { p[1] + (h + five) } else { p[1] - (sy + five) };
        }
    }
    let a = m.r32(bar + at);
    let o = q(m, a + 0x80);
    put(m, buf + 0x20, [p[0] - o[0], p[1] - o[1], p[2] - o[2], F(1.0)]);
    vcall(m, e, a, 0x70, &[buf + 0x20]);
}

/// `FE3DProgressBar_UpdateFill` 0x2ae3c0(bar; `buf` 0x40 bytes): each arrow there is shown or hidden by the value
/// and moved beside its end of the bar
pub fn update_fill(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, buf: u32) {
    let (a, top) = (m.r32(bar + 0x188), m.r32(bar + 0x18c));
    if a == 0 && top == 0 {
        return;
    }
    let cx = rf(m, bar + 0x80) + rf(m, bar + 0x100) * HALF;
    let cy = rf(m, bar + 0x84) + rf(m, bar + 0x104) * HALF;
    if a != 0 {
        place_arrow(m, e, bar, 0x188, false, [cx, cy], buf);
    }
    if m.r32(bar + 0x18c) != 0 {
        place_arrow(m, e, bar, 0x18c, true, [cx, cy], buf);
    }
}

/// `FE3DProgressBar_StepUp` 0x2ae760(bar; `buf`): the value up a step, held to the highest (+0x158) when set, else
/// to the top; the arrows updated; the value
pub fn step_up(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, buf: u32) -> F {
    let v = rf(m, bar + 0x160) + rf(m, bar + 0x164);
    wf(m, bar + 0x160, v);
    let hi = rf(m, bar + 0x158);
    if lt(hi, v) && !eq(hi, NO_LIMIT) {
        wf(m, bar + 0x160, hi);
    } else {
        let top = rf(m, bar + 0x150);
        if lt(top, rf(m, bar + 0x160)) {
            wf(m, bar + 0x160, top);
        }
    }
    update_fill(m, e, bar, buf);
    rf(m, bar + 0x160)
}

/// `FE3DProgressBar_StepDown` 0x2ae7f0(bar; `buf`): the value down a step, held to the lowest (+0x15c) when set,
/// else to the bottom; the arrows updated; the value
pub fn step_down(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, buf: u32) -> F {
    let v = rf(m, bar + 0x160) - rf(m, bar + 0x164);
    wf(m, bar + 0x160, v);
    let lo = rf(m, bar + 0x15c);
    if lt(v, lo) && !eq(lo, NO_LIMIT) {
        wf(m, bar + 0x160, lo);
    } else {
        let bottom = rf(m, bar + 0x154);
        if lt(rf(m, bar + 0x160), bottom) {
            wf(m, bar + 0x160, bottom);
        }
    }
    update_fill(m, e, bar, buf);
    rf(m, bar + 0x160)
}

/// `cFE3DProgressBar` vf11 0x2ae880 (drawn; `buf` 0x50 bytes): while shown, with the renderer's state raised: the
/// background tiled (flipped to fill the other way for 1 and 2); the end sprite at the bar's far end; the cap (up to
/// the highest the value may go) one state higher; the fill two higher, shaded from the low colour to the colour at
/// the value; the arrows' text while chosen. For a way of filling other than 0..3 it stops after raising the state
/// (and leaves it raised)
pub fn bar_draw(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, buf: u32) {
    if m.r32(bar + 0x14) == 0 {
        return;
    }
    let mut flip = 0u32;
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(bar + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    let bottom = rf(m, bar + 0x154);
    let t = rf(m, bar + 0x160) - bottom;
    let d = [1, 2, 3].map(|k| rf(m, bar + 0x120 + 4 * k) - rf(m, bar + 0x110 + 4 * k));
    let range = rf(m, bar + 0x150) - bottom;
    let low: Vec<u32> = (0..4).map(|k| m.r32(bar + 0x110 + 4 * k)).collect();
    let mut at_value = [rf(m, bar + 0x110), F(0.0), F(0.0), F(0.0)];
    for k in 0..3 {
        at_value[k + 1] = rf(m, bar + 0x114 + 4 * k as u32) + d[k] * t / range;
    }
    let dir = m.r32(bar + 0x168) as i32;
    if !(0..4).contains(&dir) {
        return;
    }
    let (mut x, mut y) = (rf(m, bar + 0x80), rf(m, bar + 0x84));
    let (fw, fh);
    // (the four corners' colours: the low colour where the fill starts, the value's where it ends)
    let corners: [[u32; 4]; 4];
    let lv = at_value.map(b);
    let lo = [low[0], low[1], low[2], low[3]];
    if dir < 2 {
        let w = rf(m, bar + 0x100);
        fh = rf(m, bar + 0x104);
        let mut v = t * w / range;
        if dir == 1 {
            x = x + w;
            flip = 1;
            v = -v;
        }
        fw = v;
        corners = [lo, lv, lo, lv];
    } else {
        let h = rf(m, bar + 0x104);
        fw = rf(m, bar + 0x100);
        let mut v = t * h / range;
        if dir == 2 {
            y = y + h;
            flip = 2;
            v = -v;
        }
        fh = v;
        corners = [lv, lv, lo, lo];
    }
    for (k, c) in corners.iter().enumerate() {
        for (i, x) in c.iter().enumerate() {
            m.w32(buf + 0x10 * k as u32 + 4 * i as u32, *x);
        }
    }
    e.call(m, DRAWER_COLOUR, &[DRAWER, bar + 0x130]);
    let back = m.r32(bar + 0x16c);
    let r = sprite_rect(m, e, back);
    let g = |m: &Mem, o: u32| m.r32(bar + o);
    let args = vec![r, flip, g(m, 0x80), g(m, 0x84), g(m, 0x100), g(m, 0x104), g(m, 0x178), g(m, 0x17c)];
    e.call(m, DRAW_TILED, &args);
    let end = m.r32(bar + 0x170);
    if (end as i32) >= 0 {
        let (bx, by, bw, bh) = (rf(m, bar + 0x80), rf(m, bar + 0x84), rf(m, bar + 0x100), rf(m, bar + 0x104));
        let six = F(6.0);
        let (fl, a) = match dir {
            0 => (0, [bx + bw, by, six, bh]),
            1 => (8, [bx, by, six, bh]),
            2 => (4, [bx, by, bw, six]),
            _ => (0, [bx, by + bh, bw, six]),
        };
        let r = sprite_rect(m, e, end);
        e.call(m, DRAW_RECT, &[r, fl, b(a[0]), b(a[1]), b(a[2]), b(a[3])]);
    }
    let raise = m.r32(bar + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise).wrapping_add(1)]);
    let hi = rf(m, bar + 0x158);
    if lt(rf(m, bar + 0x160), hi) && !eq(hi, NO_LIMIT) {
        let k = (hi - rf(m, bar + 0x154)) / range;
        e.call(m, DRAWER_COLOUR, &[DRAWER, bar + 0x140]);
        let fill = m.r32(bar + 0x174);
        let r = sprite_rect(m, e, fill);
        let (w, h, tx, ty) = (rf(m, bar + 0x100), rf(m, bar + 0x104), rf(m, bar + 0x180), rf(m, bar + 0x184));
        let a = match m.r32(bar + 0x168) {
            0 => Some([x, y, k * w, fh, k * tx, ty]),
            1 => Some([x, y, -k * w, fh, k * tx, ty]),
            2 => Some([x, y, fw, -k * h, tx, k * ty]),
            3 => Some([x, y, fw, k * h, tx, k * ty]),
            _ => None,
        };
        if let Some(a) = a {
            let mut args = vec![r, 0];
            args.extend(a.map(b));
            e.call(m, DRAW_TILED, &args);
        }
        let raise = m.r32(bar + 0x70);
        rcall(m, e, 0x1d0, &[state.wrapping_add(raise).wrapping_add(2)]);
    }
    let fill = m.r32(bar + 0x174);
    let r = sprite_rect(m, e, fill);
    let k = t / range;
    let (tx, ty) = (rf(m, bar + 0x180), rf(m, bar + 0x184));
    let (tx, ty) = if m.r32(bar + 0x168) < 2 { (k * tx, ty) } else { (tx, k * ty) };
    e.call(m, DRAW_SHADED, &[r, 0, buf, buf + 0x10, buf + 0x20, buf + 0x30, b(x), b(y), b(fw), b(fh), b(tx), b(ty)]);
    if m.r32(bar + 0xa0) != 0 {
        for at in [0x188, 0x18c] {
            let a = m.r32(bar + at);
            if a != 0 {
                vcall(m, e, a, 0x60, &[]);
            }
        }
    }
    rcall(m, e, 0x1d0, &[state]);
}

/// `cFE3DProgressBar` vf28 0x2aef28(bar, input; `buf`): while enabled (+0xa4 clear) and given an input: the pad's
/// way along the bar steps up (the top arrow told +0x20 with 1), the other way down (the bottom arrow told), and the
/// owner told (+0xc0); accept answers alone
pub fn bar_on_input(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, input: u32, buf: u32) -> u32 {
    if m.r32(bar + 0xa4) != 0 || m.r32(input) == 0 {
        return 0;
    }
    let up = [(0, INPUT_RIGHT), (1, INPUT_LEFT), (2, INPUT_UP), (3, INPUT_DOWN)];
    let down = [(0, INPUT_LEFT), (1, INPUT_RIGHT), (2, INPUT_DOWN), (3, INPUT_UP)];
    let pressed = |m: &mut Mem, e: &mut dyn FeEnv, ways: &[(u32, u32)]| {
        ways.iter().any(|&(d, f)| m.r32(bar + 0x168) == d && e.call(m, f, &[input]) != 0)
    };
    let arrow = if pressed(m, e, &up) {
        step_up(m, e, bar, buf);
        0x18c
    } else if pressed(m, e, &down) {
        step_down(m, e, bar, buf);
        0x188
    } else {
        return (e.call(m, INPUT_ACCEPT, &[input]) != 0) as u32;
    };
    let a = m.r32(bar + arrow);
    if a != 0 {
        vcall(m, e, a, 0x20, &[1]);
    }
    let owner = m.r32(bar + 0xec);
    if owner != 0 {
        vcall(m, e, owner, 0xc0, &[bar]);
    }
    1
}

/// `cFE3DProgressBar` vf39 0x2b37e0: the arrows updated
pub fn bar_vf39(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, buf: u32) {
    update_fill(m, e, bar, buf);
}

/// `cFE3DProgressBar` vf1 0x2b39b8: both arrows destroyed (+8 with 3) and cleared
pub fn bar_vf1(m: &mut Mem, e: &mut dyn FeEnv, bar: u32) {
    for at in [0x188, 0x18c] {
        let a = m.r32(bar + at);
        if a != 0 {
            vcall(m, e, a, 8, &[3]);
        }
    }
    m.w32(bar + 0x18c, 0);
    m.w32(bar + 0x188, 0);
}

/// `cFE3DProgressBar::~cFE3DProgressBar` 0x2b3a20(bar, flags): its table, its vf1, then the widget's destructor
pub fn bar_dtor(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, flags: u32) -> u32 {
    m.w32(bar + 0xc, PROGRESS_VTABLE);
    bar_vf1(m, e, bar);
    destroy(m, e, bar, flags)
}

/// `cFE3DProgressBar::cFE3DProgressBar` 0x2b3d10(bar): the widget with no owner or parent, then its table
pub fn bar_ctor(m: &mut Mem, e: &mut dyn FeEnv, bar: u32) -> u32 {
    widget_init(m, e, bar, 0, 0);
    m.w32(bar + 0xc, PROGRESS_VTABLE);
    bar
}
