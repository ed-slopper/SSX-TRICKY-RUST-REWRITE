//! The animation sequences and layers (board row G5a1): what the rider's animation controller (rider+0x46c0, row
//! G5a2) plays. A layer is a list of sequences; a sequence (`cBoarderSequence`, 0x90 bytes, over the skeleton
//! sequence `cSkelSequence`) is one clip being played: its weight, speed, sources and markers. The game works on
//! them through pointers and virtual calls, so the port keeps the same bytes at the same addresses ([`Mem`]) and
//! makes the calls out through [`SeqEnv`]. Checked in the function runner (`tests/animseq.rs`).
//!
//! **The list (`ClipList_*`).** A list object L is two end nodes in one: L itself (its +0x2c 0) and L+0x38 (its +0x30
//! 0). Each node links through +0x2c (towards L) and +0x30 (towards L+0x38). A new node goes right after L, so a walk
//! from L+0x30 by +0x30 meets the newest first and stops at L+0x38.
//!
//! **A sequence's fields:**
//! - the skeleton sequence's: +8 the weight (0..1), +0xc its rate a tick, +0x10 the speed, +0x14 the facing, +0x18,
//!   +0x1c a byte, +0x20 held (doesn't end by itself), +0x24 ended, +0x28, +0x2c / +0x30 the links, +0x34 the vtable;
//! - the boarder sequence's: +0x40 the clip id (0x300 none), +0x44 the sources (up to three), +0x48 each source's
//!   time in frames, +0x54 three floats (1), +0x60 the frame to stop at (−1 none), +0x64 the stance, +0x68 current,
//!   +0x6c the frame where the end's fade starts and +0x70 its rate, +0x74..+0x77 the marker bits, +0x78 each source's
//!   animation tree, +0x84 each source's key (two shorts).
//!
//! A source's tree points (+8) at its clip header: +9 the child count (the tree's children at +0xc), +0xa the length
//! in frames, +0xc and +0x14 two lists of four marker frames (0xffff unused).
//!
//! The vtables are pairs of words (this adjustment as a short, the function): slot 1 at +8 delete, slot 2 at +0x10
//! reset, slot 3 at +0x18 the weight update, slot 4 save, slot 5 load. `cSkelSequence` 0x39e470 (delete does nothing,
//! reset 0x203cf8, update 0x203a40), `cBoarderSequence` 0x379020 (0x159ff8, 0x159ee8, 0x15a3a8). The port runs its
//! own functions for those addresses and hands any other through the environment.

use crate::pf::Mem;
use crate::ps2f::F;

/// The vtables
pub const SKEL_VTABLE: u32 = 0x39e470;
pub const BDR_VTABLE: u32 = 0x379020;
/// The skeleton animation key meaning none (0x39e468)
pub const NO_KEY: u32 = 0x39e468;
/// The clip key table (0x3bc530: 0x584 words, by clip id)
pub const KEY_TABLE: u32 = 0x3bc530;
/// The animation bank a source's tree comes from (0x3211a8, `SkelAnim_Create` 0x158dc0)
pub const ANIM_BANK: u32 = 0x3211a8;
pub const SKEL_ANIM_CREATE: u32 = 0x158dc0;
/// `operator delete` 0x114f80 (the node pool's free, or the heap's)
pub const DELETE: u32 = 0x114f80;
/// `MemPool_Alloc` 0x23d3b0(pool 0x345e48, size, flags, name "BdrSeq" 0x379018, 0, 0)
pub const POOL_ALLOC: u32 = 0x23d3b0;
pub const POOL: u32 = 0x345e48;
pub const SEQ_NAME: u32 = 0x379018;

/// The calls out
pub trait SeqEnv {
    /// A function of the game's with its arguments; its answer
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
}

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn wf(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
/// `div.s` as the EE does it: toward zero (the largest float for a zero divisor, as [`F`])
fn div(a: F, b: F) -> F {
    let (x, y) = (a.ee(), b.ee());
    if y == 0.0 {
        return a / b;
    }
    let q = x as f64 / y as f64;
    let mut r = q as f32;
    if (r as f64).abs() > q.abs() {
        r = f32::from_bits(r.to_bits() - 1);
    }
    F(r) * F(1.0)
}
/// `cvt.s.w`
fn fl(v: i32) -> F {
    F(v as f32)
}
/// `cvt.w.s`: toward zero, saturating
fn int(v: F) -> i32 {
    v.ee() as i32
}
/// `abs.s`
fn abs(v: F) -> F {
    F(f32::from_bits(v.0.to_bits() & 0x7fff_ffff))
}
fn memset(m: &mut Mem, a: u32, v: u8, n: u32) {
    for k in 0..n {
        m.w8(a + k, v);
    }
}

/// A source's clip length in frames (its tree's header +0xa)
fn src_len(m: &Mem, seq: u32, i: u32) -> u32 {
    let tree = m.r32(seq + 0x78 + 4 * i);
    m.r16(m.r32(tree + 8) + 0xa) as u32
}

/// A virtual call: slot `k`'s adjustment and function in the object's vtable (+0x34)
pub fn vcall(m: &mut Mem, env: &mut dyn SeqEnv, obj: u32, k: u32) -> u32 {
    let vt = m.r32(obj + 0x34);
    let adj = m.r16(vt + 8 * k) as i16 as i32 as u32;
    let f = m.r32(vt + 8 * k + 4);
    run(m, env, f, obj.wrapping_add(adj))
}

/// One of the sequences' functions by address (the ported ones run here, any other through the environment)
pub fn run(m: &mut Mem, env: &mut dyn SeqEnv, f: u32, this: u32) -> u32 {
    match f {
        0x203cf8 => skel_init(m, this),
        0x203a40 => skel_update(m, this),
        0x203d38 => {}
        0x159ff8 => seq_delete(m, env, this),
        0x159ee8 => seq_reset(m, env, this),
        0x15a3a8 => seq_update(m, this),
        _ => return env.call(m, f, &[this]),
    }
    0
}

/// `cSkelSequence` reset 0x203cf8: weight and speed 1, the rest 0, +0x18 −1, unlinked
pub fn skel_init(m: &mut Mem, s: u32) {
    let one = 1f32.to_bits();
    m.w32(s + 0x20, 0);
    m.w32(s + 0x18, 0xffff_ffff);
    m.w32(s + 0x10, one);
    m.w32(s + 0x24, 0);
    m.w32(s + 0x28, 0);
    m.w32(s + 0xc, 0);
    m.w32(s + 0x14, 0);
    m.w8(s + 0x1c, 0);
    m.w32(s + 8, one);
    m.w32(s + 0x2c, 0);
    m.w32(s + 0x30, 0);
}

/// `cSkelSequence` weight update 0x203a40: the weight moves by its rate; at 1 or more it stays 1 and stops, at 0 or
/// less it stays 0, stops and the sequence has ended
pub fn skel_update(m: &mut Mem, s: u32) {
    let w = rf(m, s + 8) + rf(m, s + 0xc);
    wf(m, s + 8, w);
    if 1.0 <= w.ee() {
        m.w32(s + 8, 1f32.to_bits());
        m.w32(s + 0xc, 0);
    } else if w.ee() <= 0.0 {
        m.w32(s + 8, 0);
        m.w32(s + 0x24, 1);
        m.w32(s + 0xc, 0);
    }
}

/// `ClipList_Init` 0x203b48
pub fn list_init(m: &mut Mem, l: u32) {
    m.w32(l + 0x2c, 0);
    m.w32(l + 0x30, l + 0x38);
    m.w32(l + 0x38 + 0x2c, l);
    m.w32(l + 0x38 + 0x30, 0);
}

/// `ClipList_Insert` 0x203ae0 (and its copy 0x203cd8): the node right after the list's first end
pub fn list_insert(m: &mut Mem, l: u32, n: u32) {
    m.w32(n + 0x2c, l);
    m.w32(n + 0x30, m.r32(l + 0x30));
    let after = m.r32(l + 0x30);
    m.w32(after + 0x2c, n);
    m.w32(l + 0x30, n);
}

/// 0x203cb0: the node taken out of its list, its links 0
pub fn list_unlink(m: &mut Mem, n: u32) {
    let (prev, next) = (m.r32(n + 0x2c), m.r32(n + 0x30));
    m.w32(prev + 0x30, next);
    let (next, prev) = (m.r32(n + 0x30), m.r32(n + 0x2c));
    m.w32(next + 0x2c, prev);
    m.w32(n + 0x2c, 0);
    m.w32(n + 0x30, 0);
}

/// `ClipList_Remove` 0x203bd8(list, node): unlinked, then deleted (vtable slot 1)
pub fn list_remove(m: &mut Mem, env: &mut dyn SeqEnv, _l: u32, n: u32) {
    let vt = m.r32(n + 0x34);
    let f = m.r32(vt + 0xc);
    list_unlink(m, n);
    let adj = m.r16(vt + 8) as i16 as i32 as u32;
    run(m, env, f, n.wrapping_add(adj));
}

/// `ClipList_RemoveAndDestroy` 0x203b00(list, node): unlinked, then reset (vtable slot 2)
pub fn list_remove_reset(m: &mut Mem, env: &mut dyn SeqEnv, _l: u32, n: u32) {
    let vt = m.r32(n + 0x34);
    let f = m.r32(vt + 0x14);
    list_unlink(m, n);
    let adj = m.r16(vt + 0x10) as i16 as i32 as u32;
    run(m, env, f, n.wrapping_add(adj));
}

/// 0x203b60(list, k): the k-th node from the newest, or 0
pub fn list_nth(m: &Mem, l: u32, k: u32) -> u32 {
    let end = l + 0x38;
    let mut n = m.r32(l + 0x30);
    let mut i = 0u32;
    while n != end {
        if i == k {
            return n;
        }
        n = m.r32(n + 0x30);
        i = i.wrapping_add(1);
    }
    0
}

/// `ClipList_Count` 0x203b98
pub fn list_count(m: &Mem, l: u32) -> u32 {
    let end = l + 0x38;
    let mut n = m.r32(l + 0x30);
    let mut c = 0;
    while n != end {
        n = m.r32(n + 0x30);
        c += 1;
    }
    c
}

/// `ClipList_DestroyAll` 0x203c20: every node deleted (vtable slot 1, its next read first), then the list emptied
pub fn list_destroy_all(m: &mut Mem, env: &mut dyn SeqEnv, l: u32) {
    let end = l + 0x38;
    let mut n = m.r32(l + 0x30);
    while n != end {
        let next = m.r32(n + 0x30);
        vcall(m, env, n, 1);
        n = next;
    }
    list_init(m, l);
}

/// `SkelAnim_DestroyTree` 0x159a80(tree): its children (+0xc, as many as its header's +9 says) first, then the tree
/// deleted
pub fn destroy_tree(m: &mut Mem, env: &mut dyn SeqEnv, t: u32) {
    if m.r32(t + 8) != 0 && m.r8(m.r32(t + 8) + 9) != 0 {
        let mut i = 0u32;
        while i < m.r8(m.r32(t + 8) + 9) as u32 {
            let c = m.r32(t + 0xc + 4 * i);
            if c != 0 {
                destroy_tree(m, env, c);
            }
            i += 1;
        }
    }
    env.call(m, DELETE, &[t]);
}

/// `BdrSeq_Create` 0x15a700: a sequence from the node pool, zeroed when given, then constructed
pub fn seq_create(m: &mut Mem, env: &mut dyn SeqEnv) -> u32 {
    let p = env.call(m, POOL_ALLOC, &[POOL, 0x90, 0xc000_0000, SEQ_NAME, 0, 0]);
    if p != 0 {
        memset(m, p, 0, 0x90);
    }
    seq_construct(m, env, p);
    p
}

/// `cBoarderSequence`'s constructor 0x159b08: the skeleton sequence's (its vtable, reset), then its own vtable and
/// 0x159b50: no trees, no keys (0xff bytes), and the reset (vtable slot 2)
pub fn seq_construct(m: &mut Mem, env: &mut dyn SeqEnv, p: u32) {
    m.w32(p + 0x34, SKEL_VTABLE);
    skel_init(m, p);
    m.w32(p + 0x34, BDR_VTABLE);
    memset(m, p + 0x78, 0, 0xc);
    memset(m, p + 0x84, 0xff, 0xc);
    vcall(m, env, p, 2);
}

/// 0x159ba8(seq, i, tree): source i's tree
pub fn seq_set_tree(m: &mut Mem, seq: u32, i: u32, tree: u32) {
    m.w32(seq.wrapping_add(i.wrapping_mul(4)) + 0x78, tree);
}

/// `cBoarderSequence` reset 0x159ee8: weight and speed 1, the three floats 1, no clip (0x300), no stop frame, the rest
/// 0 (the links too); each source's tree destroyed; the keys none
pub fn seq_reset(m: &mut Mem, env: &mut dyn SeqEnv, s: u32) {
    let one = 1f32.to_bits();
    m.w32(s + 0x18, 0xffff_ffff);
    m.w32(s + 0x60, 0xffff_ffff);
    m.w32(s + 0x40, 0x300);
    for o in [0x20, 0x24, 0x28, 0xc, 0x14] {
        m.w32(s + o, 0);
    }
    m.w8(s + 0x1c, 0);
    m.w32(s + 8, one);
    m.w32(s + 0x10, one);
    for o in [0x2c, 0x30, 0x44, 0x6c, 0x70] {
        m.w32(s + o, 0);
    }
    for i in 0..3 {
        m.w32(s + 0x48 + 4 * i, 0);
        m.w32(s + 0x54 + 4 * i, one);
    }
    m.w8(s + 0x77, 0);
    m.w32(s + 0x64, 0);
    m.w32(s + 0x68, 0);
    m.w8(s + 0x74, 0);
    m.w8(s + 0x75, 0);
    m.w8(s + 0x76, 0);
    for i in 0..3 {
        let t = m.r32(s + 0x78 + 4 * i);
        if t != 0 {
            destroy_tree(m, env, t);
            m.w32(s + 0x78 + 4 * i, 0);
        }
    }
    memset(m, s + 0x84, 0xff, 0xc);
}

/// `cBoarderSequence` delete 0x159ff8: reset (vtable slot 2), then deleted
pub fn seq_delete(m: &mut Mem, env: &mut dyn SeqEnv, s: u32) {
    vcall(m, env, s, 2);
    env.call(m, DELETE, &[s]);
}

/// `AnimClip_SetSource` 0x159bb8(seq, i, clip): the clip's key (the key table's word, or when that is none the
/// table's first that isn't); when it differs from source i's, source i takes it: its old tree destroyed, a new one
/// made (`SkelAnim_Create` 0x158dc0 on the bank), and no stop frame
pub fn set_source(m: &mut Mem, env: &mut dyn SeqEnv, seq: u32, i: u32, clip: u32) {
    let none = m.r32(NO_KEY);
    let mut key = m.r32(KEY_TABLE.wrapping_add(clip.wrapping_mul(4)));
    if key == none {
        for k in 0..0x584u32 {
            let w = m.r32(KEY_TABLE + 4 * k);
            if w != none {
                key = w;
                break;
            }
        }
    }
    let at = seq.wrapping_add(i.wrapping_mul(4));
    if m.r32(at + 0x84) == key {
        return;
    }
    m.w32(at + 0x84, key);
    let old = m.r32(at + 0x78);
    if old != 0 {
        destroy_tree(m, env, old);
    }
    let t = env.call(m, SKEL_ANIM_CREATE, &[ANIM_BANK, key]);
    m.w32(at + 0x78, t);
    m.w32(seq + 0x60, 0xffff_ffff);
}

/// `AnimClip_SetBlendOut` 0x159d88(seq, frames): fades out over that many frames (fewer than 0: what is left of
/// source 0, at least 1) at the speed; a sequence not held stops at its end in the direction it plays (frame 0
/// backwards, the last forwards)
pub fn set_blend_out(m: &mut Mem, seq: u32, frames: i32) {
    let speed = rf(m, seq + 0x10);
    let mut n = frames;
    if m.r32(seq + 0x20) == 0 {
        if speed.ee() < 0.0 {
            m.w32(seq + 0x60, 0);
        } else {
            m.w32(seq + 0x60, src_len(m, seq, 0).wrapping_sub(1));
        }
    }
    if n < 0 {
        let t = int(rf(m, seq + 0x48));
        n = (src_len(m, seq, 0) as i32).wrapping_sub(t.wrapping_add(1));
        if n <= 0 {
            n = 1;
        }
    }
    let rate = div(abs(speed) * F(-30.0), fl(n.wrapping_mul(60)));
    wf(m, seq + 0xc, rate);
}

/// `AnimClip_SetBlendIn` 0x159e38(seq, frames): the weight's rate to reach 1 over that many frames at the speed
pub fn set_blend_in(m: &mut Mem, seq: u32, frames: i32) {
    let per = div(abs(rf(m, seq + 0x10)) * F(30.0), fl(frames.wrapping_mul(60)));
    let rate = (F(1.0) - rf(m, seq + 8)) * per;
    wf(m, seq + 0xc, rate);
}

/// `AnimClip_GetPhase` 0x159e80: source 0's time over its last frame (1 for a one-frame clip), from the end when
/// playing backwards
pub fn phase(m: &Mem, seq: u32) -> F {
    let mut last = fl(src_len(m, seq, 0) as i32 - 1);
    if last.ee() == 0.0 {
        last = F(1.0);
    }
    let t = rf(m, seq + 0x48);
    if rf(m, seq + 0x10).ee() < 0.0 {
        div(last - t, last)
    } else {
        div(t, last)
    }
}

/// `AnimClip_Advance` 0x15a030(seq, dt): a held sequence at its end goes back to frame 0 on every source. Otherwise
/// the marker bits cleared, and each source's time moves by dt × speed × 30 / 60; marker k of the first list sets
/// bit k of +0x74 while the time is past it (and of +0x75 when crossed this tick), the second list +0x76 / +0x77. The
/// time stops at the stop frame and at the clip's ends; a sequence not held that was already at the end has ended.
pub fn advance(m: &mut Mem, seq: u32, dt: F) {
    let held = m.r32(seq + 0x20);
    if held != 0 {
        let last = fl(src_len(m, seq, 0) as i32 - 1);
        if last.ee() <= rf(m, seq + 0x48).ee() {
            let n = m.r32(seq + 0x44) as i32;
            for k in 0..n.max(0) as u32 {
                m.w32(seq + 0x48 + 4 * k, 0);
            }
            return;
        }
    }
    let n = m.r32(seq + 0x44) as i32;
    for o in 0x74..0x78 {
        m.w8(seq + o, 0);
    }
    if n <= 0 {
        return;
    }
    let stop = m.r32(seq + 0x60) as i32;
    for i in 0..n as u32 {
        let at = seq + 0x48 + 4 * i;
        let step = div(dt * rf(m, seq + 0x10) * F(30.0), F(60.0));
        let old = rf(m, at);
        let new = old + step;
        wf(m, at, new);
        let lo = if old.ee() < new.ee() { old } else { new };
        let hi = if new.ee() < old.ee() { old } else { new };
        let hdr = m.r32(m.r32(seq + 0x78 + 4 * i) + 8);
        for (list, past, crossed) in [(0xc, 0x74, 0x75), (0x14, 0x76, 0x77)] {
            for k in 0..4 {
                let mk = m.r16(hdr + list + 2 * k);
                if mk == 0xffff {
                    continue;
                }
                let f = fl(mk as i32);
                if f.ee() < hi.ee() {
                    m.w8(seq + past, m.r8(seq + past) | 1 << k);
                    if lo.ee() <= f.ee() {
                        m.w8(seq + crossed, m.r8(seq + crossed) | 1 << k);
                    }
                }
            }
        }
        let at_end;
        if 0.0 < step.ee() {
            if stop != -1 && fl(stop).ee() < rf(m, at).ee() {
                wf(m, at, fl(stop));
            }
            let last = fl(src_len(m, seq, i) as i32 - 1);
            if !(last.ee() < rf(m, at).ee()) {
                continue;
            }
            at_end = last.ee() <= old.ee();
            wf(m, at, last);
        } else {
            if stop != -1 && rf(m, at).ee() < fl(stop).ee() {
                wf(m, at, fl(stop));
            }
            if !(rf(m, at).ee() < 0.0) {
                continue;
            }
            at_end = old.ee() <= 0.0;
            m.w32(at, 0);
        }
        if at_end && held == 0 {
            m.w32(seq + 0x24, 1);
        }
    }
}

/// `AnimClip_SetEndBlendOut` 0x15a330(seq, frames): where the end's fade starts (source 0's length − (frames + 1))
/// and its rate (−|speed| × 30 / (60 × frames)); 0 frames: never (0x0fffffff, rate −1)
pub fn set_end_blend_out(m: &mut Mem, seq: u32, frames: i32) {
    if frames == 0 {
        m.w32(seq + 0x70, (-1f32).to_bits());
        m.w32(seq + 0x6c, 0x0fff_ffff);
        return;
    }
    let rate = div(abs(rf(m, seq + 0x10)) * F(-30.0), fl(frames) * F(60.0));
    m.w32(seq + 0x6c, src_len(m, seq, 0).wrapping_sub(frames.wrapping_add(1) as u32));
    wf(m, seq + 0x70, rate);
}

/// `cBoarderSequence` weight update 0x15a3a8: past the end's fade frame the fade's rate is taken, then the skeleton
/// sequence's update
pub fn seq_update(m: &mut Mem, seq: u32) {
    if !(int(rf(m, seq + 0x48)) < m.r32(seq + 0x6c) as i32) {
        m.w32(seq + 0xc, m.r32(seq + 0x70));
    }
    skel_update(m, seq);
}

/// The sequences of a layer, newest first
fn each(m: &Mem, layer: u32) -> Vec<u32> {
    let end = layer + 0x38;
    let mut out = Vec::new();
    let mut n = m.r32(layer + 0x30);
    while n != end {
        out.push(n);
        n = m.r32(n + 0x30);
    }
    out
}

/// `AnimLayer_BlendOutAll` 0x15a600(layer, frames): every sequence fades out
pub fn layer_blend_out_all(m: &mut Mem, layer: u32, frames: i32) {
    let end = layer + 0x38;
    let mut n = m.r32(layer + 0x30);
    while n != end {
        set_blend_out(m, n, frames);
        n = m.r32(n + 0x30);
    }
}

/// `AnimLayer_Turn` 0x15a658(layer, angle): every sequence's facing turned
pub fn layer_turn(m: &mut Mem, layer: u32, a: F) {
    for n in each(m, layer) {
        let v = rf(m, n + 0x14) + a;
        wf(m, n + 0x14, v);
    }
}

/// `AnimLayer_SetStance` 0x15a690(layer, stance)
pub fn layer_set_stance(m: &mut Mem, layer: u32, v: u32) {
    for n in each(m, layer) {
        m.w32(n + 0x64, v);
    }
}

/// `AnimLayer_FindClip` 0x15a6c8(layer, clip): the newest sequence of that clip, or 0
pub fn layer_find(m: &Mem, layer: u32, clip: u32) -> u32 {
    each(m, layer).into_iter().find(|&n| m.r32(n + 0x40) == clip).unwrap_or(0)
}

/// `AnimLayer_ClearCurrentFlags` 0x15a760
pub fn layer_clear_current(m: &mut Mem, layer: u32) {
    for n in each(m, layer) {
        m.w32(n + 0x68, 0);
    }
}
