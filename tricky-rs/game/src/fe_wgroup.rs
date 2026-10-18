//! The front end's widget bases and their lists (board row G9x1) on the game's bytes: the widget group's
//! operations over its children (`FE3DWGroup_*`, the list at +0x110) and the plain group's (`cFE3DGroup`, the list at
//! +0xa0), the list operations they apply (`ListFE3D*Op_Apply`: each calls one virtual of the child), the list's walk
//! (`cList` vf1), `cFE3DThing`'s small virtuals, the group's, list's, node's and thing's constructor and
//! destructors, and the widget's placing and sliding in. Every call out goes through [`FeEnv`]; the functions of
//! the row that call each other do so directly. Checked in the function runner (`tests/fe_wgroup.rs`).
//!
//! An operation is built on the stack: its table (+0) and an argument (+4, or a vector at +0x10), applied over the
//! list, then destroyed (its destructor with 2).

use crate::fe3dstate::{vcall, FeEnv, GAPP};
use crate::pf::Mem;
use crate::ps2f::F;

/// The list's walk (0x2ac220) and its other helpers: delete all (0x2ac178), init (0x2abfb0), a node's unlink
/// (0x2abf38); the widget's update of its activity (0x2ad380) and its reset (0x2ad2c0); the tween's start (0x2ac348)
/// and run (0x2ac3b0); the progress bar's set (0x2ae3c0); the free (0x114f80); a group's render's matrix (0x2abc18,
/// its destructor 0x270218, the identity at 0x346440)
pub const LIST_APPLY: u32 = 0x2ac220;
pub const LIST_DELETE_ALL: u32 = 0x2ac178;
pub const LIST_INIT: u32 = 0x2abfb0;
pub const NODE_UNLINK: u32 = 0x2abf38;
pub const WIDGET_UPDATE_ACTIVITY: u32 = 0x2ad380;
pub const WIDGET_RESET_ACTIVITY: u32 = 0x2ad2c0;
pub const TWEEN_START: u32 = 0x2ac348;
pub const TWEEN_RUN: u32 = 0x2ac3b0;
pub const BAR_SET: u32 = 0x2ae3c0;
pub const FREE: u32 = 0x114f80;
pub const MATRIX_MUL: u32 = 0x2abc18;
pub const MATRIX_DTOR: u32 = 0x270218;
pub const IDENTITY: u32 = 0x346440;
/// The operations' tables and destructors
pub const OP_UPDATE_ACTIVITY: (u32, u32) = (0x3aa6a8, 0x2693a0);
pub const OP_RESET_ACTIVITY: (u32, u32) = (0x3aa6c0, 0x2693a8);
pub const OP_SKIP_ACTIVITY: (u32, u32) = (0x3aa6d8, 0x2693b0);
pub const OP_ON_INPUT: (u32, u32) = (0x3aa678, 0x269390);
pub const OP_UPDATE: (u32, u32) = (0x3a9fe0, 0x269440);
pub const OP_RENDER_TEXT: (u32, u32) = (0x3a9ff8, 0x2694c8);
pub const OP_RENDER: (u32, u32) = (0x3aa010, 0x269558);
pub const OP_FORMAT: (u32, u32) = (0x3aa690, 0x269398);
pub const OP_OFFSET: (u32, u32) = (0x3aa6f0, 0x2693b8);
/// The classes' tables
pub const THING_VTABLE: u32 = 0x3b5a00;
pub const GROUP_VTABLE: u32 = 0x3b58d0;
pub const LIST_VTABLE: u32 = 0x3b5ae8;
pub const NODE_VTABLE: u32 = 0x3b5b00;
/// The slide-in's target and start vectors
pub const SLIDE_TO: u32 = 0x3d36f0;
pub const SLIDE_FROM: u32 = 0x3d3700;

const ONE: u32 = 0x3f80_0000;

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}

/// A virtual of the renderer (the app's +0x724, its table at +0x10)
fn rcall(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP) + 0x724);
    let vt = m.r32(r + 0x10);
    let me = r.wrapping_add(m.r16(vt + off) as i16 as u32);
    let f = m.r32(vt + off + 4);
    let mut a = vec![me];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// An operation built at `op` (its table, its argument) applied over the list at `list`; the walk's answer
fn apply(m: &mut Mem, e: &mut dyn FeEnv, list: u32, kind: (u32, u32), arg: Option<u32>, op: u32) -> u32 {
    if let Some(a) = arg {
        m.w32(op + 4, a);
    }
    m.w32(op, kind.0);
    list_apply(m, e, list, op)
}

fn op_done(m: &mut Mem, e: &mut dyn FeEnv, kind: (u32, u32), op: u32) {
    e.call(m, kind.1, &[op, 2]);
}

/// A vector's x, y, z added to the position (+0x80; the w with 0)
fn offset_position(m: &mut Mem, w: u32, v: u32) {
    let add = [rf(m, v), rf(m, v + 4), rf(m, v + 8), F(0.0)];
    for (i, a) in add.into_iter().enumerate() {
        let at = w + 0x80 + 4 * i as u32;
        let s = rf(m, at) + a;
        m.w32(at, s.0.to_bits());
    }
}

/// `FE3DWGroup_UpdateActivity4` 0x2aac40(w): the children's, then the widget's own (0x2ad380)
pub fn wgroup_update_activity(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32) {
    apply(m, e, w + 0x110, OP_UPDATE_ACTIVITY, None, op);
    e.call(m, WIDGET_UPDATE_ACTIVITY, &[w]);
    op_done(m, e, OP_UPDATE_ACTIVITY, op);
}

/// `FE3DWGroup_ResetActivity4` 0x2aac90(w): the children's, then the widget's own (0x2ad2c0)
pub fn wgroup_reset_activity(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32) {
    apply(m, e, w + 0x110, OP_RESET_ACTIVITY, None, op);
    e.call(m, WIDGET_RESET_ACTIVITY, &[w]);
    op_done(m, e, OP_RESET_ACTIVITY, op);
}

/// `FE3DWGroup_SkipActivity4` 0x2aace0(w): the children's, then the thing's own (`cFE3DThing` vf6)
pub fn wgroup_skip_activity(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32) {
    apply(m, e, w + 0x110, OP_SKIP_ACTIVITY, None, op);
    thing_vf6(m, e, w);
    op_done(m, e, OP_SKIP_ACTIVITY, op);
}

/// `FE3DWGroup_OnInput4` 0x2aad30(w, input) -> taken: none while busy (+0xa4); else whether a child took it
pub fn wgroup_on_input(m: &mut Mem, e: &mut dyn FeEnv, w: u32, input: u32, op: u32) -> u32 {
    if m.r32(w + 0xa4) != 0 {
        return 0;
    }
    let r = apply(m, e, w + 0x110, OP_ON_INPUT, Some(input), op);
    op_done(m, e, OP_ON_INPUT, op);
    (r != 0) as u32
}

/// `FE3DWGroup_Update4` 0x2aad90(w): when active (+0x10) the children's, then its own virtual +0x48
pub fn wgroup_update(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32) {
    if m.r32(w + 0x10) == 0 {
        return;
    }
    apply(m, e, w + 0x110, OP_UPDATE, None, op);
    vcall(m, e, w, 0x48, &[]);
    op_done(m, e, OP_UPDATE, op);
}

/// The renderer's state raised by `raise` (+0x70) around `f`, then put back
fn raised(m: &mut Mem, e: &mut dyn FeEnv, w: u32, f: &mut dyn FnMut(&mut Mem, &mut dyn FeEnv)) -> u32 {
    let state = rcall(m, e, 0x1d8, &[]);
    let raise = m.r32(w + 0x70);
    rcall(m, e, 0x1d0, &[state.wrapping_add(raise)]);
    f(m, e);
    state
}

/// `FE3DWGroup_RenderText4` 0x2aadf0(w): when shown (+0x14) the children's texts, the renderer's state raised
pub fn wgroup_render_text(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32) {
    if m.r32(w + 0x14) == 0 {
        return;
    }
    let state = raised(m, e, w, &mut |m, e| {
        apply(m, e, w + 0x110, OP_RENDER_TEXT, None, op);
    });
    rcall(m, e, 0x1d0, &[state]);
    op_done(m, e, OP_RENDER_TEXT, op);
}

/// `FE3DWGroup_Render4` 0x2aaeb8(w, matrix): when shown the children drawn with `matrix`, the state raised
pub fn wgroup_render(m: &mut Mem, e: &mut dyn FeEnv, w: u32, matrix: u32, op: u32) {
    if m.r32(w + 0x14) == 0 {
        return;
    }
    let state = raised(m, e, w, &mut |m, e| {
        apply(m, e, w + 0x110, OP_RENDER, Some(matrix), op);
    });
    rcall(m, e, 0x1d0, &[state]);
    op_done(m, e, OP_RENDER, op);
}

/// `FE3DWGroup_Format4` 0x2aaf90(w): the children's format
pub fn wgroup_format(m: &mut Mem, e: &mut dyn FeEnv, w: u32, op: u32) {
    apply(m, e, w + 0x110, OP_FORMAT, None, op);
    op_done(m, e, OP_FORMAT, op);
}

/// `FE3DWGroup_FreeChildren4` 0x2aafc8(w): the children deleted, then itself unlinked (`cListNode` vf1)
pub fn wgroup_free_children(m: &mut Mem, e: &mut dyn FeEnv, w: u32) {
    e.call(m, LIST_DELETE_ALL, &[w + 0x110]);
    node_vf1(m, e, w);
}

/// An offset operation (the vector copied in at +0x10) over `list`, then the position moved by it
fn offset_by(m: &mut Mem, e: &mut dyn FeEnv, w: u32, list: u32, v: u32, op: u32) {
    for i in 0..4 {
        m.w32(op + 0x10 + 4 * i, m.r32(v + 4 * i));
    }
    apply(m, e, list, OP_OFFSET, None, op);
    for i in 0..4 {
        m.w32(op + 0x20 + 4 * i, m.r32(v + 4 * i));
    }
    m.w32(op + 0x2c, 0);
    offset_position(m, w, op + 0x20);
    op_done(m, e, OP_OFFSET, op);
}

/// `FE3DWGroup_Offset4` 0x2ab070(w, v): the children moved by `v`, then itself
pub fn wgroup_offset(m: &mut Mem, e: &mut dyn FeEnv, w: u32, v: u32, op: u32) {
    offset_by(m, e, w, w + 0x110, v, op);
}

/// The operations' applies (op, child): `ListFE3DOnInputOp_Apply4` 0x2ab380 the child's +0xe8 with the input
/// (the child when it took it, else 0); the format (+0x140), update activity (+0x48), reset (+0x40), skip (+0x38),
/// offset (+0x70 with the vector at +0x10), update (+0x68), render text (+0x60) and render (+0x58 with the matrix)
/// ones answer 0
pub fn op_on_input(m: &mut Mem, e: &mut dyn FeEnv, op: u32, child: u32) -> u32 {
    let input = m.r32(op + 4);
    if vcall(m, e, child, 0xe8, &[input]) == 0 { 0 } else { child }
}
pub fn op_plain(m: &mut Mem, e: &mut dyn FeEnv, off: u32, child: u32) -> u32 {
    vcall(m, e, child, off, &[]);
    0
}
pub fn op_offset(m: &mut Mem, e: &mut dyn FeEnv, op: u32, child: u32) -> u32 {
    vcall(m, e, child, 0x70, &[op + 0x10]);
    0
}
pub fn op_render(m: &mut Mem, e: &mut dyn FeEnv, op: u32, child: u32) -> u32 {
    let mtx = m.r32(op + 4);
    vcall(m, e, child, 0x58, &[mtx]);
    0
}

/// `cFE3DGroup` vf1 0x2abc70(g): its children deleted (+0xa0), then itself unlinked
pub fn group_vf1(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    e.call(m, LIST_DELETE_ALL, &[g + 0xa0]);
    node_vf1(m, e, g);
}

/// `cFE3DGroup` vf10 0x2abca0(g, matrix): when shown the children drawn with the identity (0x346440, 0x40 bytes,
/// copied to `frame`) times `matrix`, the state raised
pub fn group_render(m: &mut Mem, e: &mut dyn FeEnv, g: u32, matrix: u32, frame: u32) {
    if m.r32(g + 0x14) == 0 {
        return;
    }
    let state = raised(m, e, g, &mut |m, e| {
        for i in 0..0x10 {
            m.w32(frame + 4 * i, m.r32(IDENTITY + 4 * i));
        }
        e.call(m, MATRIX_MUL, &[frame, matrix]);
        apply(m, e, g + 0xa0, OP_RENDER, Some(frame), frame + 0x40);
    });
    rcall(m, e, 0x1d0, &[state]);
    op_done(m, e, OP_RENDER, frame + 0x40);
    e.call(m, MATRIX_DTOR, &[frame, 2]);
}

/// `cFE3DGroup` vf11 0x2abda8(g): when shown the children's texts, the state raised
pub fn group_render_text(m: &mut Mem, e: &mut dyn FeEnv, g: u32, op: u32) {
    if m.r32(g + 0x14) == 0 {
        return;
    }
    let state = raised(m, e, g, &mut |m, e| {
        apply(m, e, g + 0xa0, OP_RENDER_TEXT, None, op);
    });
    rcall(m, e, 0x1d0, &[state]);
    op_done(m, e, OP_RENDER_TEXT, op);
}

/// `cFE3DGroup` vf7 0x2ac580(g): the children's reset, then the thing's (vf7)
pub fn group_reset(m: &mut Mem, e: &mut dyn FeEnv, g: u32, op: u32) {
    apply(m, e, g + 0xa0, OP_RESET_ACTIVITY, None, op);
    thing_vf7(m, e, g);
    op_done(m, e, OP_RESET_ACTIVITY, op);
}

/// `cFE3DGroup` vf6 0x2ac5d0(g): the children's skip, then the thing's (vf6)
pub fn group_skip(m: &mut Mem, e: &mut dyn FeEnv, g: u32, op: u32) {
    apply(m, e, g + 0xa0, OP_SKIP_ACTIVITY, None, op);
    thing_vf6(m, e, g);
    op_done(m, e, OP_SKIP_ACTIVITY, op);
}

/// `cFE3DGroup` vf13 0x2ac620(g, v): the children moved by `v`, then itself
pub fn group_offset(m: &mut Mem, e: &mut dyn FeEnv, g: u32, v: u32, op: u32) {
    offset_by(m, e, g, g + 0xa0, v, op);
}

/// `cFE3DGroup::~cFE3DGroup` 0x2ac6b8(g, flags): its table, its children deleted, its list's destructor, the
/// thing's
pub fn group_dtor(m: &mut Mem, e: &mut dyn FeEnv, g: u32, flags: u32) {
    m.w32(g + 0xc, GROUP_VTABLE);
    group_vf1(m, e, g);
    list_dtor(m, e, g + 0xa0, 2);
    thing_dtor(m, e, g, flags);
}

/// `cFE3DGroup::cFE3DGroup` 0x2ac708(g): the thing's words (links, flags, the colour white, the position (0, 0, 0,
/// 1), the size 0), its list set up, its table
pub fn group_ctor(m: &mut Mem, e: &mut dyn FeEnv, g: u32, tmp: u32) -> u32 {
    m.w32(g + 0xc, THING_VTABLE);
    m.w32(g + 0x40, 0);
    m.w32(g, 0xffff_ffff);
    m.w32(g + 0x28, 1);
    m.w32(g + 0x14, 1);
    m.w32(g + 4, g);
    m.w32(g + 8, g);
    for o in [0x10u32, 0x18, 0x1c, 0x20, 0x24, 0x2c, 0x30, 0x4c, 0x44, 0x48, 0x50, 0x5c, 0x54, 0x58] {
        m.w32(g + o, 0);
    }
    for o in [0x60u32, 0x6c, 0x64, 0x68] {
        m.w32(g + o, ONE);
    }
    m.w32(tmp + 0xc, ONE);
    for o in [0u32, 4, 8] {
        m.w32(tmp + o, 0);
    }
    m.w32(g + 0x70, 0);
    let v: Vec<u32> = (0..4).map(|i| m.r32(tmp + 4 * i)).collect();
    m.w32(g + 0x74, 0);
    for (i, x) in v.into_iter().enumerate() {
        m.w32(g + 0x80 + 4 * i as u32, x);
    }
    for o in [0u32, 4, 8] {
        m.w32(tmp + o, 0);
    }
    let y = m.r32(tmp + 4);
    m.w32(g + 0x90, y);
    m.w32(g + 0x94, y);
    let z = m.r32(tmp + 8);
    m.w32(g + 0x98, z);
    m.w32(g + 0xc, GROUP_VTABLE);
    e.call(m, LIST_INIT, &[g + 0xa0]);
    g
}

/// `cList` vf1 0x2ac220(list, op) -> the first answer that isn't 0: each node from the list's +8 on (while its
/// next isn't itself) given the operation through its virtual +0x18
pub fn list_apply(m: &mut Mem, e: &mut dyn FeEnv, list: u32, op: u32) -> u32 {
    let mut node = m.r32(list + 8);
    let mut next = m.r32(node + 8);
    if next == node {
        return 0;
    }
    loop {
        let r = vcall(m, e, node, 0x18, &[op]);
        if r != 0 {
            return r;
        }
        node = next;
        next = m.r32(node + 8);
        if next == node {
            return 0;
        }
    }
}

/// `cFE3DThing` vf6 0x2ac428(t): when active (+0x10) and not looping (+0x2c), its frame to the end (+0x24 from
/// +0x28) and its virtual +0x48
pub fn thing_vf6(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    if m.r32(t + 0x10) == 0 || m.r32(t + 0x2c) != 0 {
        return;
    }
    m.w32(t + 0x24, m.r32(t + 0x28));
    vcall(m, e, t, 0x48, &[]);
}

/// `cFE3DThing` vf7 0x2ac478(t): its tween restarted (+0x18 1, +0x24 0) and its virtual +0x48
pub fn thing_vf7(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    m.w32(t + 0x18, 1);
    m.w32(t + 0x24, 0);
    vcall(m, e, t, 0x48, &[]);
}

/// `cFE3DThing`'s small virtuals: vf15..vf27 0 (0x2aca18..0x2aca78), vf14 0x2aca80 1, vf8 / vf10 / vf11 nothing,
/// vf12 0x2acbb8 its virtual +0x48, vf13 0x2acaf8 its position moved by a vector, vf9 0x2acc80 its colour (+0x60)
/// set, vf3 0x2accf8 / vf4 0x2accf0 its +0x10 / +0x14 set
pub fn thing_zero() -> u32 {
    0
}
pub fn thing_one() -> u32 {
    1
}
pub fn thing_nothing() {}
pub fn thing_vf12(m: &mut Mem, e: &mut dyn FeEnv, t: u32) {
    vcall(m, e, t, 0x48, &[]);
}
pub fn thing_vf13(m: &mut Mem, t: u32, v: u32, tmp: u32) {
    for i in 0..4 {
        m.w32(tmp + 4 * i, m.r32(v + 4 * i));
    }
    m.w32(tmp + 0xc, 0);
    offset_position(m, t, tmp);
}
pub fn thing_vf9(m: &mut Mem, t: u32, c: u32) {
    for i in 0..4 {
        m.w32(t + 0x60 + 4 * i, m.r32(c + 4 * i));
    }
}
pub fn thing_set(m: &mut Mem, t: u32, at: u32, v: u32) {
    m.w32(t + at, v);
}

/// `cFE3DThing::~cFE3DThing` 0x2acd10(t, flags): its table, then the node's
pub fn thing_dtor(m: &mut Mem, e: &mut dyn FeEnv, t: u32, flags: u32) {
    m.w32(t + 0xc, THING_VTABLE);
    node_dtor(m, e, t, flags);
}

/// `cList::~cList` 0x2ace78(list, flags): its table (+0x20), its two nodes' destructors (+0x10, then itself), freed
/// on flag 1
pub fn list_dtor(m: &mut Mem, e: &mut dyn FeEnv, list: u32, flags: u32) {
    m.w32(list + 0x20, LIST_VTABLE);
    node_dtor(m, e, list + 0x10, 2);
    node_dtor(m, e, list, 2);
    if flags & 1 != 0 {
        e.call(m, FREE, &[list]);
    }
}

/// `cListNode` vf1 0x2acf28(n): unlinked
pub fn node_vf1(m: &mut Mem, e: &mut dyn FeEnv, n: u32) {
    e.call(m, NODE_UNLINK, &[n]);
}

/// `cListNode::~cListNode` 0x2acf60(n, flags): its table, unlinked, freed on flag 1
pub fn node_dtor(m: &mut Mem, e: &mut dyn FeEnv, n: u32, flags: u32) {
    m.w32(n + 0xc, NODE_VTABLE);
    node_vf1(m, e, n);
    if flags & 1 != 0 {
        e.call(m, FREE, &[n]);
    }
}

/// `FE3DWidget_Place` 0x2addb0(w, at; x, y): shown (its +0xf8 with 1), placed (+0x138 with (x, y, 0, 0) at
/// `rect`), its anchor (+0x110) set, not sliding (+0x114)
pub fn widget_place(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, x: F, y: F, rect: u32) {
    vcall(m, e, w, 0xf8, &[1]);
    for (o, v) in [(0u32, x.0.to_bits()), (4, y.0.to_bits()), (8, 0), (0xc, 0)] {
        m.w32(rect + o, v);
    }
    vcall(m, e, w, 0x138, &[rect]);
    m.w32(w + 0x110, at);
    m.w32(w + 0x114, 0);
}

/// `FE3DWidget_SlideIn` 0x2ade40(w, at, frames; x, y): placed, its anchor set, its colour (+0x50 with 0x3d36f0) and
/// a colour tween (5) from 0x3d3700 to 0x3d36f0 over `frames`, hidden (+0x20 with 0), not sliding
pub fn widget_slide_in(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, frames: u32, x: F, y: F, rect: u32) {
    for (o, v) in [(0u32, x.0.to_bits()), (4, y.0.to_bits()), (8, 0), (0xc, 0)] {
        m.w32(rect + o, v);
    }
    vcall(m, e, w, 0x138, &[rect]);
    m.w32(w + 0x110, at);
    vcall(m, e, w, 0x50, &[SLIDE_TO]);
    e.call(m, TWEEN_START, &[w, 5, SLIDE_FROM]);
    e.call(m, TWEEN_RUN, &[w, 5, SLIDE_TO, frames, 0]);
    vcall(m, e, w, 0x20, &[0]);
    m.w32(w + 0x114, 0);
}

/// `FE3DWidget_SlideInDir3` 0x2adf18 / `Dir4` 0x2adf30(w): slid in at 3 / 4 over 15 frames from (18, 18)
pub fn widget_slide_in_dir(m: &mut Mem, e: &mut dyn FeEnv, w: u32, at: u32, rect: u32) {
    widget_slide_in(m, e, w, at, 0xf, F(18.0), F(18.0), rect);
}

/// `FE3DWidget_SetValue` 0x2b3950(bar; v): its value (+0x160), then the bar's set (0x2ae3c0)
pub fn widget_set_value(m: &mut Mem, e: &mut dyn FeEnv, bar: u32, v: F) {
    m.w32(bar + 0x160, v.0.to_bits());
    e.call(m, BAR_SET, &[bar]);
}
