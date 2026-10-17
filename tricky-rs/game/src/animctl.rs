//! The rider's animation controller (board row G5a2): the object at rider+0x46c0 that starts the rider's clips on
//! its three layers, advances them each tick, and ends them with each clip's end action. It works on the sequences
//! and layers of [`crate::animseq`] (row G5a1) at their addresses. The pose blend (`AnimCtl_BlendPose` 0x15f7c8),
//! the clip data (`AnimData_*`, the bank 0x3211a8) and the motion-driven modes (`AnimMode_*` but the plain advance)
//! are calls out, rows G5c and G5b. Checked in the function runner (`tests/animctl.rs`).
//!
//! **The controller's fields:**
//! - +8, +0xc, +0x10: each layer's current clip (0x300 none);
//! - +0x14 the stance;
//! - +0x18 the speed new clips take (1);
//! - +0x1c (−1), copied to a new sequence's +0x18;
//! - +0x20 the facing new clips take (0);
//! - +0x24 the rider;
//! - +0x28 + 0x70 k: layer k (a list, [`crate::animseq`]).
//!
//! **The clip table** 0x325fa8 has 7 words a clip id (0x1c bytes): +0 the category, +4 the lookup mode, +8 the
//! update mode (0..3 time, 4..11 driven by the rider: G5b), +0xc the end action, +0x10 the layer, +0x14 the blend-in
//! frames, +0x18 the end's fade frames.

use crate::animseq::{self as seq, SeqEnv};
use crate::pf::Mem;
use crate::ps2f::F;

/// The clip table (0x325fa8, 0x1c bytes a clip id)
pub const CLIP_TABLE: u32 = 0x325fa8;
/// No clip
pub const NONE: u32 = 0x300;
/// `AnimClip_ResolveAnmIndex` 0x15ada8(clip, rider): the clip's animation (0x585 none)
pub const RESOLVE: u32 = 0x15ada8;
/// `AnimData_GetClipId` 0x158d68(bank, animation): the entry and its sub-index as two shorts
pub const GET_CLIP_ID: u32 = 0x158d68;
/// `AnimData_GetMarkerFrame` 0x1587e8(entry, sub, k) / `AnimData_SetMarker` 0x158818(entry, sub, k, frame) /
/// `AnimData_GetMarkerPhase` 0x158798(entry, sub, k)
pub const GET_MARKER_FRAME: u32 = 0x1587e8;
pub const SET_MARKER: u32 = 0x158818;
pub const GET_MARKER_PHASE: u32 = 0x158798;
/// The sound calls: 0x235780(the audio system at 0x343050, rider, sound) a clip's marker sound, 0x235778(the audio
/// system, clip type) a clip started
pub const AUDIO: u32 = 0x343050;
pub const MARKER_SOUND: u32 = 0x235780;
pub const CLIP_STARTED: u32 = 0x235778;
/// `AnimIds_Init` 0x15aec0, run by the first controller made (the flag at 0x32b38c)
pub const IDS_INIT: u32 = 0x15aec0;
pub const IDS_DONE: u32 = 0x32b38c;
/// `AnimCtl_BlendPose` 0x15f7c8 (row G5c)
pub const BLEND_POSE: u32 = 0x15f7c8;
/// The update modes' functions (0x379130): 0..3 the plain advance, then the motion-driven ones (row G5b)
pub const MODES: [u32; 12] =
    [0x161340, 0x161348, 0x161350, 0x161358, 0x160680, 0x1605a0, 0x161300, 0x161118, 0x160140, 0x1601d0, 0x160b20, 0x160af0];

fn rf(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}

/// Clip `c`'s row in the clip table
pub fn row(c: u32) -> u32 {
    CLIP_TABLE.wrapping_add(c.wrapping_mul(0x1c))
}

/// Layer k
pub fn layer(ctl: u32, k: u32) -> u32 {
    ctl.wrapping_add(k.wrapping_mul(0x70)).wrapping_add(0x28)
}

/// The sequences of a layer, newest first
fn seqs(m: &Mem, l: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut n = m.r32(l + 0x30);
    while n != l + 0x38 {
        out.push(n);
        n = m.r32(n + 0x30);
    }
    out
}

/// `AnimCtl_Construct` 0x15ed68: each layer's two ends made skeleton sequences and the list emptied; the clip ids
/// set up once (`AnimIds_Init`); then the reset
pub fn construct(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32) -> u32 {
    for k in 0..3 {
        let l = layer(ctl, k);
        m.w32(l + 0x34, seq::SKEL_VTABLE);
        seq::skel_init(m, l);
        m.w32(l + 0x38 + 0x34, seq::SKEL_VTABLE);
        seq::skel_init(m, l + 0x38);
        seq::list_init(m, l);
    }
    if m.r32(IDS_DONE) == 0 {
        env.call(m, IDS_INIT, &[]);
        m.w32(IDS_DONE, 1);
    }
    reset(m, env, ctl);
    ctl
}

/// `AnimCtl_Reset` 0x15ee30: every layer emptied and without a clip; stance 0, speed 1, +0x1c −1, facing 0
pub fn reset(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32) {
    for k in 0..3 {
        seq::list_destroy_all(m, env, layer(ctl, k));
        m.w32(ctl + 8 + 4 * k, NONE);
    }
    m.w32(ctl + 0x14, 0);
    m.w32(ctl + 0x1c, 0xffff_ffff);
    m.w32(ctl + 0x18, 1f32.to_bits());
    m.w32(ctl + 0x20, 0);
}

/// `AnimCtl_GetClipType` 0x15eec0(layer): the category of the layer's clip (8 with none)
pub fn clip_type(m: &Mem, ctl: u32, k: u32) -> u32 {
    let c = m.r32(ctl.wrapping_add(k.wrapping_mul(4)) + 8);
    if c == NONE {
        8
    } else {
        m.r32(row(c))
    }
}

/// `AnimCtl_TurnAllLayers` 0x15eef8(angle)
pub fn turn_all(m: &mut Mem, ctl: u32, a: F) {
    for k in 0..3 {
        seq::layer_turn(m, layer(ctl, k), a);
    }
}

/// `AnimCtl_SetAllLayersStance` 0x15ef48(stance)
pub fn set_stance_all(m: &mut Mem, ctl: u32, v: u32) {
    for k in 0..3 {
        seq::layer_set_stance(m, layer(ctl, k), v);
    }
}

/// `AnimCtl_StopClipsInCategory` 0x15ef98(category): every sequence of that category (any with 0x1c) loses its stop
/// frame
pub fn stop_category(m: &mut Mem, ctl: u32, cat: u32) {
    for k in 0..3 {
        for n in seqs(m, layer(ctl, k)) {
            if cat == 0x1c || m.r32(row(m.r32(n + 0x40))) == cat {
                m.w32(n + 0x60, 0xffff_ffff);
            }
        }
    }
}

/// `AnimCtl_ClearClips` 0x15f020: layer 1 emptied and without a clip
pub fn clear_clips(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32) {
    seq::list_destroy_all(m, env, ctl + 0x98);
    m.w32(ctl + 0xc, NONE);
}

/// `AnimCtl_BlendOutAll` 0x15f050: layer 1's sequences fade out over 10 frames
pub fn blend_out_all(m: &mut Mem, ctl: u32) {
    seq::layer_blend_out_all(m, ctl + 0x98, 10);
}

/// `AnimCtl_BeginClip` 0x15f060(seq, layer, clip, frame, marker): the clip set up on the sequence and put on the
/// layer; the clip (0x300, the sequence deleted, when it has no animation for this rider).
/// - The sequence: current, the clip, +0x28 1, source 0 the clip's animation, one source; the controller's stance,
///   facing and +0x1c.
/// - The start frame: `frame` plus marker 1..4 of the first list (`marker` 1..4; 0 when unused), the last frame
///   (`marker` 6) or 0; not below 0. The controller's speed.
/// - The update mode: 1 stops at marker 2, 2 at the last frame, 3 holds.
/// - Weight 1, the end's fade from the table.
/// - With no blend-in the layer and layer 1 are emptied first. With one, when it is layer 1 or has sequences: those
///   lose "current" and fade out, and the new one fades in from 0.
pub fn begin_clip(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32, l: u32, clip: u32, frame: u32, marker: u32) -> u32 {
    let anim = env.call(m, RESOLVE, &[clip, m.r32(ctl + 0x24)]);
    if anim == 0x585 {
        seq::vcall(m, env, s, 1);
        return NONE;
    }
    let r = row(clip);
    m.w32(s + 0x68, 1);
    m.w32(s + 0x40, clip);
    m.w32(s + 0x28, 1);
    seq::set_source(m, env, s, 0, anim);
    m.w32(s + 0x44, 1);
    m.w32(s + 0x64, m.r32(ctl + 0x14));
    m.w32(s + 0x14, m.r32(ctl + 0x20));
    m.w32(s + 0x18, m.r32(ctl + 0x1c));
    let hdr = |m: &Mem| m.r32(m.r32(s + 0x78) + 8);
    let base = match marker.wrapping_sub(1) {
        k @ 0..=3 => {
            let mk = m.r16(hdr(m) + 0xc + 2 * k) as u32;
            if mk != 0xffff {
                mk
            } else {
                0
            }
        }
        5 => (m.r16(hdr(m) + 0xa) as u32).wrapping_sub(1),
        _ => 0,
    };
    let start = F(base.wrapping_add(frame) as i32 as f32);
    m.w32(s + 0x48, if start.ee() < 0.0 { 0 } else { start.0.to_bits() });
    m.w32(s + 0x10, m.r32(ctl + 0x18));
    match m.r32(r + 8) {
        2 => {
            let last = (m.r16(hdr(m) + 0xa) as u32).wrapping_sub(1);
            m.w32(s + 0x60, last);
        }
        1 => {
            let mk = m.r16(hdr(m) + 0xe) as u32;
            m.w32(s + 0x60, mk);
        }
        3 => m.w32(s + 0x20, 1),
        _ => {}
    }
    m.w32(s + 8, 1f32.to_bits());
    seq::set_end_blend_out(m, s, m.r32(r + 0x18) as i32);
    let blend = m.r32(r + 0x14);
    if blend == 0 {
        seq::list_destroy_all(m, env, l);
        clear_clips(m, env, ctl);
    } else if l == ctl + 0x98 || m.r32(l + 0x30) != l + 0x38 {
        seq::layer_clear_current(m, l);
        seq::layer_blend_out_all(m, l, blend as i32);
        m.w32(s + 8, 0);
        seq::set_blend_in(m, s, blend as i32);
    }
    seq::list_insert(m, l, s);
    clip
}

/// `AnimCtl_FireClipSounds` 0x15f300(seq): for each of the first list's markers crossed this tick (+0x77 bits),
/// the sound at the clip header's +0x1c + 2k for the rider
pub fn fire_sounds(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32) {
    for k in 0..4 {
        if (m.r8(s + 0x77) >> k) & 1 != 0 {
            let hdr = m.r32(m.r32(s + 0x78) + 8);
            let snd = m.r16(hdr + 0x1c + 2 * k) as u32;
            env.call(m, MARKER_SOUND, &[m.r32(AUDIO), m.r32(ctl + 0x24), snd]);
        }
    }
}

/// `AnimCtl_UpdateClip` 0x15f440(seq, dt): the clip moved by its update mode (0..3 the plain advance, the rest
/// called out with the controller, the sequence and dt; past 11 nothing), its marker sounds, then its weight
/// (vtable slot 3)
pub fn update_clip(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32, dt: F) {
    let mode = m.r32(row(m.r32(s + 0x40)) + 8);
    if mode < 4 {
        seq::advance(m, s, dt);
    } else if mode < 12 {
        env.call(m, MODES[mode as usize], &[ctl, s, dt.0.to_bits()]);
    }
    fire_sounds(m, env, ctl, s);
    seq::vcall(m, env, s, 3);
}

/// `AnimCtl_UpdateLayers` 0x15f390(dt): every sequence of the three layers updated (newest first), then the pose
/// blended (`AnimCtl_BlendPose`)
pub fn update_layers(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, dt: F) {
    for k in 0..3 {
        let l = layer(ctl, k);
        let mut n = m.r32(l + 0x30);
        while n != l + 0x38 {
            update_clip(m, env, ctl, n, dt);
            n = m.r32(n + 0x30);
        }
    }
    env.call(m, BLEND_POSE, &[ctl]);
}

/// `AnimCtl_PlayClip` 0x15fcf8(clip, frame, marker): a new sequence begun on the clip's layer; the layer's current
/// clip is what `begin_clip` answers; the audio system told the layer's clip type; that clip answered
pub fn play_clip(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, clip: u32, frame: u32, marker: u32) -> u32 {
    let k = m.r32(row(clip) + 0x10);
    let l = layer(ctl, k);
    let s = seq::seq_create(m, env);
    let c = begin_clip(m, env, ctl, s, l, clip, frame, marker);
    let at = ctl.wrapping_add(k.wrapping_mul(4)) + 8;
    m.w32(at, c);
    let t = clip_type(m, ctl, k);
    env.call(m, CLIP_STARTED, &[m.r32(AUDIO), t]);
    m.r32(at)
}

/// The end actions (0x161360..0x161640), by the table's +0xc: each takes the sequence off layer 0 (`ClipList_Remove`,
/// whatever layer it is on) and most play the next clip; the answer becomes the clip's layer's current clip
fn end_action(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32, action: u32, clip: u32) -> u32 {
    let l0 = ctl + 0x28;
    match action {
        // `AnimCtl_EndActionStop` 0x161640: off the clip's own layer; none (or the clip, if the sequence's differs)
        0 => {
            let k = m.r32(row(clip) + 0x10);
            let ans = if clip != m.r32(s + 0x40) { clip } else { NONE };
            seq::list_remove(m, env, layer(ctl, k), s);
            ans
        }
        // `AnimCtl_EndActionToBase` 0x1613a0: the rider's six words at +0x1fc..+0x210 cleared, then 0x1ff
        1 => {
            seq::list_remove(m, env, l0, s);
            let r = m.r32(ctl + 0x24);
            for o in [0x204, 0x1fc, 0x200, 0x210, 0x208, 0x20c] {
                m.w32(r + o, 0);
            }
            play_clip(m, env, ctl, 0x1ff, 0, 5)
        }
        2 => {
            seq::list_remove(m, env, l0, s);
            play_clip(m, env, ctl, 0x223, 0, 5)
        }
        3 => {
            seq::list_remove(m, env, l0, s);
            play_clip(m, env, ctl, 0x202, 0, 5)
        }
        // `AnimCtl_EndActionChain` 0x161400: the chained clips' next (others: the clip itself)
        4 => {
            let c = m.r32(s + 0x40);
            seq::list_remove(m, env, l0, s);
            match chain_next(c) {
                Some(n) => play_clip(m, env, ctl, n, 0, 5),
                None => c,
            }
        }
        5 => {
            seq::list_remove(m, env, l0, s);
            play_clip(m, env, ctl, 0x2df, 0, 5)
        }
        6 => {
            seq::list_remove(m, env, l0, s);
            play_clip(m, env, ctl, 0x2de, 0, 5)
        }
        // `AnimCtl_EndActionToRide` 0x1615e8: 0x202 in ground motion (rider+0x420 2), else 0x1ff
        _ => {
            seq::list_remove(m, env, l0, s);
            let next = if m.r32(m.r32(ctl + 0x24) + 0x420) == 2 { 0x202 } else { 0x1ff };
            play_clip(m, env, ctl, next, 0, 5)
        }
    }
}

/// `AnimCtl_EndActionChain`'s table (0x379620, clip ids 0x226..0x277)
pub fn chain_next(c: u32) -> Option<u32> {
    Some(match c {
        0x226 => 0x22e,
        0x227 => 0x22f,
        0x228 => 0x230,
        0x229 => 0x231,
        0x22a => 0x232,
        0x22b => 0x233,
        0x22c => 0x234,
        0x22d => 0x235,
        0x270 => 0x235,
        0x271 => 0x234,
        0x272 => 0x22f,
        0x273 => 0x22e,
        0x274 => 0x235,
        0x275 => 0x234,
        0x276 | 0x277 => 0x22f,
        _ => return None,
    })
}

/// `AnimCtl_HandleClipEnd` 0x15f5b8(seq): a sequence that isn't current is just taken off its clip's layer; a current
/// one runs its clip's end action (0..7; past 7 nothing), whose answer becomes that layer's current clip
pub fn handle_end(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, s: u32) {
    let clip = m.r32(s + 0x40);
    let r = row(clip);
    let k = m.r32(r + 0x10);
    if m.r32(s + 0x68) == 0 {
        seq::list_remove(m, env, layer(ctl, k), s);
        return;
    }
    let action = m.r32(r + 0xc);
    if action < 8 {
        let v = end_action(m, env, ctl, s, action, clip);
        m.w32(ctl.wrapping_add(k.wrapping_mul(4)) + 8, v);
    }
}

/// `AnimCtl_FlushEndedClips` 0x15f700: the ended sequences of the three layers (newest first, layer by layer) listed,
/// then each one's end handled
pub fn flush_ended(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32) {
    let mut ended = Vec::new();
    for k in 0..3 {
        for n in seqs(m, layer(ctl, k)) {
            if m.r32(n + 0x24) != 0 {
                ended.push(n);
            }
        }
    }
    for n in ended {
        handle_end(m, env, ctl, n);
    }
}

/// The clip's animation entry: (`AnimData_GetClipId`'s entry as the bank's record, its sub-index)
fn clip_entry(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, clip: u32) -> (u32, u32) {
    let anim = env.call(m, RESOLVE, &[clip, m.r32(ctl + 0x24)]);
    let id = env.call(m, GET_CLIP_ID, &[seq::ANIM_BANK, anim]);
    let entry = (id & 0xffff).wrapping_mul(0x18).wrapping_add(0x724).wrapping_add(seq::ANIM_BANK);
    (entry, id >> 16)
}

/// `AnimCtl_FindFirstMarkerFrame` 0x15fb08(clip): the frame of the clip's first marker of 0..3 that has one, or −1
pub fn first_marker_frame(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, clip: u32) -> i32 {
    let (e, sub) = clip_entry(m, env, ctl, clip);
    for k in 0..4 {
        let f = env.call(m, GET_MARKER_FRAME, &[e, sub, k]) as i32;
        if 0 <= f {
            return f;
        }
    }
    -1
}

/// `RaceScene_SetClipMarker` 0x15fbb8(clip, frame): the clip's first marker that has a frame set to `frame`; with
/// `frame` −1 all four markers then cleared
pub fn set_clip_marker(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, clip: u32, frame: u32) {
    let (e, sub) = clip_entry(m, env, ctl, clip);
    for k in 0..4 {
        if 0 <= env.call(m, GET_MARKER_FRAME, &[e, sub, k]) as i32 {
            env.call(m, SET_MARKER, &[e, sub, k, frame]);
            break;
        }
    }
    if frame == u32::MAX {
        for k in 0..4 {
            env.call(m, SET_MARKER, &[e, sub, k, u32::MAX]);
        }
    }
}

/// `AnimCtl_GetClip` 0x15fdd0(layer)
pub fn clip(m: &Mem, ctl: u32, k: u32) -> u32 {
    m.r32(ctl.wrapping_add(k.wrapping_mul(4)) + 8)
}

/// `AnimCtl_GetMarkerPhase` 0x15fde0(n, layer): marker n − 1's phase in the layer's clip (`AnimData_GetMarkerPhase`)
pub fn marker_phase(m: &mut Mem, env: &mut dyn SeqEnv, ctl: u32, n: u32, k: u32) -> F {
    let c = clip(m, ctl, k);
    let (e, sub) = clip_entry(m, env, ctl, c);
    F(f32::from_bits(env.call(m, GET_MARKER_PHASE, &[e, sub, n.wrapping_sub(1)])))
}

/// The layer's sequence of its current clip (`AnimLayer_FindClip`)
fn current(m: &Mem, ctl: u32, k: u32) -> u32 {
    seq::layer_find(m, layer(ctl, k), clip(m, ctl, k))
}

/// `AnimCtl_GetLayerPhase` 0x15fe70(layer): the current clip's phase (1 when not playing)
pub fn layer_phase(m: &Mem, ctl: u32, k: u32) -> F {
    match current(m, ctl, k) {
        0 => F(1.0),
        s => seq::phase(m, s),
    }
}

/// `AnimCtl_GetClipDuration` 0x15fec8(layer): the current clip's length over speed × 30 (0 when not playing)
pub fn clip_duration(m: &Mem, ctl: u32, k: u32) -> F {
    match current(m, ctl, k) {
        0 => F(0.0),
        s => {
            let len = m.r16(m.r32(m.r32(s + 0x78) + 8) + 0xa) as i32;
            seq::div(F(len as f32), rf(m, s + 0x10) * F(30.0))
        }
    }
}

/// `AnimCtl_IsClipAtEnd` 0x15ff40(layer): the current clip's source 0 at or past its last frame (0 when not playing)
pub fn at_end(m: &Mem, ctl: u32, k: u32) -> bool {
    match current(m, ctl, k) {
        0 => false,
        s => {
            let last = m.r16(m.r32(m.r32(s + 0x78) + 8) + 0xa) as i32 - 1;
            F(last as f32).ee() <= rf(m, s + 0x48).ee()
        }
    }
}

/// `AnimCtl_TestClipFlag` 0x15ffb8(flag, layer): bit flag − 1 of the current clip's +0x74 (past the marker). The
/// game reads it without checking that the clip plays (a sequence at 0).
pub fn test_flag(m: &Mem, ctl: u32, flag: u32, k: u32) -> bool {
    let s = current(m, ctl, k);
    (m.r8(s + 0x74) as u32 >> (flag.wrapping_sub(1) & 31)) & 1 != 0
}

/// `AnimCtl_TestClipBit` 0x160008(flag, layer): the same on +0x75 (crossed this tick)
pub fn test_bit(m: &Mem, ctl: u32, flag: u32, k: u32) -> bool {
    let s = current(m, ctl, k);
    (m.r8(s + 0x75) as u32 >> (flag.wrapping_sub(1) & 31)) & 1 != 0
}
