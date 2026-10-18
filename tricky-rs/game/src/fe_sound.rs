//! The front end's sound object (board row G9y3) on the game's bytes: `cFE3DSound`, the front end's handle on the
//! game's sound system (its object at 0x343050), and its base `cFE3DSoundBase` (54 virtuals that do nothing or
//! answer 0 or 1). Most of the sound object's virtuals hand their arguments on to one of the sound system's own
//! functions; a few map a front end event to a sound id first, set the five volumes back after a reload, or read a
//! word of the sound system. Every call out goes through [`FeEnv`]; the functions of the row that call each other
//! are called directly. Checked in the function runner (`tests/fe_sound.rs`).
//!
//! A virtual that hands on its arguments is ported as taking them (`a1` and the float `f12`, as the registers hold
//! them) and passing them, unchanged, with the sound system in place of itself.

use crate::fe3dstate::{FeEnv, GAPP};
use crate::fe_wgroup::FREE;
use crate::pf::Mem;
use crate::ps2f::F;

/// The sound system's object, the front end's sound object's and its base's tables, the career (its +8 the sound
/// settings)
pub const SOUND_SYSTEM: u32 = 0x343050;
pub const SOUND_VTABLE: u32 = 0x3ad9d8;
pub const BASE_VTABLE: u32 = 0x3ae138;
pub const CAREER: u32 = 0x32b390;
/// The sound system's calls the virtuals hand on to
pub const SND_OPEN: u32 = 0x20f160;
pub const SND_CLOSE: u32 = 0x20f578;
pub const SND_PLAY_EVENT: u32 = 0x22e5a8;
pub const FE_SOUND_EVENT: u32 = 0x265af8;
pub const SND_UPDATE: u32 = 0x20f430;
pub const SND_PAUSE: u32 = 0x20f500;
pub const SND_STOP_ALL: u32 = 0x20f530;
pub const SND_RESUME: u32 = 0x20f510;
pub const SND_GET_VOLUME: u32 = 0x211f20;
pub const SND_SET_VOLUME: u32 = 0x211db0;
pub const SND_SET_MASTER: u32 = 0x212210;
pub const SND_SET_MODE: u32 = 0x212270;
pub const SND_STOP_MUSIC: u32 = 0x21c928;
pub const SND_RELOAD: u32 = 0x225260;
pub const SND_WAIT_LOAD: u32 = 0x20dad0;
pub const SND_LOAD_BANK: u32 = 0x20da50;
pub const FE_UPDATE_ALL: u32 = 0x26ff88;
pub const SND_MUSIC_2129F8: u32 = 0x2129f8;

fn rf(m: &Mem, at: u32) -> F {
    F(f32::from_bits(m.r32(at)))
}

fn snd(m: &Mem) -> u32 {
    m.r32(SOUND_SYSTEM)
}

/// A virtual call through a table at +0 (the sound objects') or +4 (the sound system's)
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, table: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(table));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// The sound system's `f` with the arguments handed on (`a1`, `f12`)
fn on(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a1: u32, f12: F) -> u32 {
    let s = snd(m);
    e.call(m, f, &[s, a1, f12.0.to_bits()])
}

/// The sound system's `f` with only itself (what the other registers hold after an earlier call is not its)
fn on0(m: &mut Mem, e: &mut dyn FeEnv, f: u32) -> u32 {
    let s = snd(m);
    e.call(m, f, &[s])
}

/// `cFE3DSound::cFE3DSound` 0x2702e0(s, id): its table, its id (+4), the sound system opened for it
pub fn ctor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, id: u32) -> u32 {
    m.w32(s + 4, id);
    m.w32(s, SOUND_VTABLE);
    let ss = snd(m);
    e.call(m, SND_OPEN, &[ss, id]);
    s
}

/// `cFE3DSound::~cFE3DSound` 0x270328(s, flags): its table, the sound system closed, then the base's destructor
pub fn dtor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flags: u32) {
    m.w32(s, SOUND_VTABLE);
    let ss = snd(m);
    e.call(m, SND_CLOSE, &[ss, flags]);
    base_dtor(m, e, s, flags);
}

/// `cFE3DSoundBase::~cFE3DSoundBase` 0x271238(s, flags): its table, freed on flag 1
pub fn base_dtor(m: &mut Mem, e: &mut dyn FeEnv, s: u32, flags: u32) {
    m.w32(s, BASE_VTABLE);
    if flags & 1 != 0 {
        e.call(m, FREE, &[s, 1]);
    }
}

/// `cFE3DSound` vf1 0x270378(s, event): the front end's own sound for the event (0x265af8) when there is a front
/// end, else (or when it has none) the sound system's
pub fn vf1(m: &mut Mem, e: &mut dyn FeEnv, event: u32, f12: F) {
    let fe = m.r32(m.r32(GAPP) + 0x734);
    if fe != 0 && e.call(m, FE_SOUND_EVENT, &[fe, event]) != 0 {
        return;
    }
    on(m, e, SND_PLAY_EVENT, event, f12);
}

/// The front end events' sound ids for vf2 (12) and vf4 (9 of 12; none for 9 and 10)
const IDS_2: [u32; 12] = [0x15, 0x13, 0x14, 0x11, 0x12, 0x19, 0x16, 0x18, 0x1a, 0x1b, 0x1c, 0x17];
const IDS_4: [Option<u32>; 12] = [
    Some(0x30), Some(0x31), Some(0x32), Some(0x33), Some(0x34), Some(0x36), Some(0x39), Some(0x38), Some(0x35), None, None,
    Some(0x37),
];

/// `cFE3DSound` vf2 0x2703d0 / vf4 0x2704f0(s, k): the sound id for front end event `k` (a signed byte; none past
/// 11) played through its own +0x10
pub fn play_event(m: &mut Mem, e: &mut dyn FeEnv, s: u32, k: u32, second: bool) {
    let k = k as u8 as i8 as i32 as u32;
    if k >= 12 {
        return;
    }
    let id = if second { IDS_4[k as usize] } else { Some(IDS_2[k as usize]) };
    if let Some(id) = id {
        vcall_at(m, e, s, 0, 0x10, &[id]);
    }
}

/// `cFE3DSound` vf3 0x2704c8(s): its own +0xc8 (its second register left holding the function)
pub fn vf3(m: &mut Mem, e: &mut dyn FeEnv, s: u32) -> u32 {
    let vt = m.r32(s);
    let adj = m.r16(vt + 0xc8) as i16 as u32;
    let f = m.r32(vt + 0xcc);
    e.call(m, f, &[s.wrapping_add(adj), f])
}

/// The virtuals that only hand on (vf5..8, 11, 16, 17, 19..21, 23, 25..27, 29, 31..33, 35, 40, 48, 53, 54): the
/// sound system's function for each
pub const FORWARDS: [(u32, u32); 24] = [
    (0x2705d0, SND_PAUSE),
    (0x2705e0, 0x225660),
    (0x2705f0, 0x225618),
    (0x270600, 0x2191b8),
    (0x270780, 0x235b70),
    (0x270938, 0x21c8e8),
    (0x270948, 0x21c9a8),
    (0x270988, 0x225ac8),
    (0x270998, 0x225b10),
    (0x2709a8, 0x225b58),
    (0x2709e8, 0x225be8),
    (0x270a40, 0x2128d8),
    (0x270a50, 0x212960),
    (0x270a60, 0x21cf38),
    (0x270ab0, 0x21d020),
    (0x270ac0, 0x21d028),
    (0x270ae8, 0x21d078),
    (0x270af8, 0x21e4f0),
    (0x270b08, 0x21e520),
    (0x270bf8, SND_RESUME),
    (0x270fc0, 0x21bca8),
    (0x271088, SND_SET_MASTER),
    (0x271108, 0x21e688),
    (0x271130, SND_SET_MODE),
];

/// One of `FORWARDS` (vf30, vf36 and vf53 call rather than jump: the same)
pub fn forward(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a1: u32, f12: F) -> u32 {
    // (the master volume takes only its float)
    if f == SND_SET_MASTER {
        let s = snd(m);
        return e.call(m, f, &[s, f12.0.to_bits()]);
    }
    on(m, e, f, a1, f12)
}

/// `cFE3DSound` vf35 0x270be8: the sound system's stop of everything (a jump: its arguments handed on)
pub fn vf35(m: &mut Mem, e: &mut dyn FeEnv, a1: u32, f12: F) -> u32 {
    on(m, e, SND_STOP_ALL, a1, f12)
}

/// `cFE3DSound` vf9 0x270610(s, k): by `k` + 1 (none past 25): the music's calls (0x212428, 0x212300, 0x212348,
/// 0x212388, 0x212680, 0x2123c8, 0x2123f8 with `k`) or 0x2129f8 with a track (-1, 0..4)
pub fn vf9(m: &mut Mem, e: &mut dyn FeEnv, k: u32, f12: F) {
    let i = k.wrapping_add(1);
    let call = match i {
        1 => Some((0x212428, None)),
        2 | 6 | 10 | 16 => Some((0x212300, None)),
        3 | 7 | 11 => Some((0x212348, None)),
        4 | 8 | 12 => Some((0x212388, None)),
        5 | 9 | 21 | 24 => Some((0x212680, None)),
        13 => Some((0x2123c8, None)),
        14 => Some((0x2123f8, None)),
        18 => Some((SND_MUSIC_2129F8, Some(u32::MAX))),
        19 => Some((SND_MUSIC_2129F8, Some(0))),
        20 => Some((SND_MUSIC_2129F8, Some(2))),
        22 => Some((SND_MUSIC_2129F8, Some(3))),
        23 => Some((SND_MUSIC_2129F8, Some(4))),
        25 => Some((SND_MUSIC_2129F8, Some(1))),
        _ => None,
    };
    if let Some((f, a1)) = call {
        on(m, e, f, a1.unwrap_or(k), f12);
    }
}

/// `cFE3DSound` vf10 0x270770(s, v): the sound system's +0x7dd0
pub fn vf10(m: &mut Mem, v: u32) {
    let s = snd(m);
    m.w32(s + 0x7dd0, v);
}

/// `cFE3DSound` vf12 0x270790 / vf14 0x270860(s, on): its own +0x10 with -1; then the music stopped (vf12: when
/// off; vf14: 0x21c868 when on, stopped when off)
pub fn vf12(m: &mut Mem, e: &mut dyn FeEnv, s: u32, v: u32, fourteen: bool) {
    vcall_at(m, e, s, 0, 0x10, &[u32::MAX]);
    if fourteen && v != 0 {
        on0(m, e, 0x21c868);
    } else if v == 0 {
        on0(m, e, SND_STOP_MUSIC);
    }
}

/// `cFE3DSound` vf13 0x2707e0(s, on): its own +0x10 with -1; on: 0x21c808; off: its own +0x60, then the music
/// stopped
pub fn vf13(m: &mut Mem, e: &mut dyn FeEnv, s: u32, v: u32) {
    vcall_at(m, e, s, 0, 0x10, &[u32::MAX]);
    if v != 0 {
        on0(m, e, 0x21c808);
        return;
    }
    vcall_at(m, e, s, 0, 0x60, &[]);
    on0(m, e, SND_STOP_MUSIC);
}

/// `cFE3DSound` vf15 0x2708b8(s, on): its own +0x10 with -1; off: the music stopped; on: 0x21c8a8, then its own
/// +0x60
pub fn vf15(m: &mut Mem, e: &mut dyn FeEnv, s: u32, v: u32) {
    vcall_at(m, e, s, 0, 0x10, &[u32::MAX]);
    if v == 0 {
        on0(m, e, SND_STOP_MUSIC);
        return;
    }
    on0(m, e, 0x21c8a8);
    vcall_at(m, e, s, 0, 0x60, &[]);
}

/// `cFE3DSound` vf18 0x270958 / vf22 0x2709b8(s, a): the sound system's 0x225260 / 0x21c960 (handed `a`), then
/// 0x225a90 / 0x225ba0
pub fn vf18(m: &mut Mem, e: &mut dyn FeEnv, a1: u32, f12: F, twenty_two: bool) {
    let (first, then) = if twenty_two { (0x21c960, 0x225ba0) } else { (SND_RELOAD, 0x225a90) };
    on(m, e, first, a1, f12);
    on0(m, e, then);
}

/// `cFE3DSound` vf24 0x2709f8(s): its own +0x10 with -1, then 0x225c30 and 0x21c930
pub fn vf24(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    vcall_at(m, e, s, 0, 0x10, &[u32::MAX]);
    on0(m, e, 0x225c30);
    on0(m, e, 0x21c930);
}

/// `cFE3DSound` vf28 0x270a70(s, a): the sound system's 0x21d020 and 0x21cfa0, each handed `a`
pub fn vf28(m: &mut Mem, e: &mut dyn FeEnv, a1: u32, f12: F) {
    on(m, e, 0x21d020, a1, f12);
    on(m, e, 0x21cfa0, a1, f12);
}

/// The five volumes (channels 0..4) the sound system keeps
fn set_volumes(m: &mut Mem, e: &mut dyn FeEnv, v: [F; 5]) {
    for (k, x) in v.iter().enumerate() {
        let s = snd(m);
        e.call(m, SND_SET_VOLUME, &[s, k as u32, x.0.to_bits()]);
    }
}

/// `cFE3DSound` vf34 0x270b18: with sound settings in the career (+8's +0x169c): the sound system's +0x10 (its
/// table at +4) with the settings' +0x1668, the five volumes (channels 0..2, the master, 3, 4) and the mode
/// (+0x166c) from them; then the sound system's update
pub fn vf34(m: &mut Mem, e: &mut dyn FeEnv) {
    let base = CAREER + 8;
    if m.r32(base + 0x169c) != 0 {
        let s = snd(m);
        let a = m.r32(base + 0x1668);
        vcall_at(m, e, s, 4, 0x10, &[a]);
        for (k, at) in [(0u32, 0x1650u32), (1, 0x1654), (2, 0x1658)] {
            let s = snd(m);
            let v = m.r32(base + at);
            e.call(m, SND_SET_VOLUME, &[s, k, v]);
        }
        let s = snd(m);
        let v = m.r32(base + 0x165c);
        e.call(m, SND_SET_MASTER, &[s, v]);
        for (k, at) in [(3u32, 0x1660u32), (4, 0x1664)] {
            let s = snd(m);
            let v = m.r32(base + at);
            e.call(m, SND_SET_VOLUME, &[s, k, v]);
        }
        let s = snd(m);
        let mode = m.r32(base + 0x166c);
        e.call(m, SND_SET_MODE, &[s, mode]);
    }
    on0(m, e, SND_UPDATE);
}

/// `cFE3DSound` vf37 0x270c20 / vf39 0x270fb0: the sound system's +0x3838 / +0x3834
pub fn read_word(m: &Mem, at: u32) -> u32 {
    m.r32(snd(m) + at)
}

/// `cFE3DSound` vf47 0x271078: the sound system's +0x383c (a float)
pub fn vf47(m: &Mem) -> F {
    rf(m, snd(m) + 0x383c)
}

/// `cFE3DSound` vf41 / 43 / 45 / 49 / 51 (0x270fd0..0x2710d0): channel 0 / 1 / 2 / 3 / 4's volume (a float)
pub fn get_volume(m: &mut Mem, e: &mut dyn FeEnv, ch: u32) -> F {
    let s = snd(m);
    F(f32::from_bits(e.call(m, SND_GET_VOLUME, &[s, ch])))
}

/// `cFE3DSound` vf42 / 44 / 46 / 50 / 52 (0x270ff8..0x2710f8; f12): channel 0 / 1 / 2 / 3 / 4's volume set
pub fn set_volume(m: &mut Mem, e: &mut dyn FeEnv, ch: u32, v: F) {
    let s = snd(m);
    e.call(m, SND_SET_VOLUME, &[s, ch, v.0.to_bits()]);
}

/// The front end's wait for a frame to go by: its widgets updated (0x26ff88 on the front end's +0x80), the renderer
/// asked until it answers (+0x70), then the front end's own +0x38 (its table at +0; its second register left
/// holding the front end)
fn wait_frame(m: &mut Mem, e: &mut dyn FeEnv) {
    let app = m.r32(GAPP);
    let fe = m.r32(app + 0x734);
    let r = m.r32(app + 0x724);
    e.call(m, FE_UPDATE_ALL, &[fe.wrapping_add(0x80)]);
    loop {
        let vt = m.r32(r.wrapping_add(0x10));
        let adj = m.r16(vt.wrapping_add(0x70)) as i16 as u32;
        let f = m.r32(vt.wrapping_add(0x74));
        if e.call(m, f, &[r.wrapping_add(adj)]) != 0 {
            break;
        }
    }
    let fe = m.r32(m.r32(GAPP) + 0x734);
    vcall_at(m, e, fe, 0, 0x38, &[fe]);
}

/// `cFE3DSound` vf38 0x270c30(s, bank): when the sound system answers 1 to its +0x10 (its table at +4; handed
/// `bank`): the five volumes kept; the sound stopped; then, a frame apart each: the sound system closed, the load
/// waited for, the bank loaded, the volumes set back and the sound object's id (+4) opened again, the sound
/// updated; then reloaded
pub fn vf38(m: &mut Mem, e: &mut dyn FeEnv, s: u32, bank: u32) {
    let ss = snd(m);
    if vcall_at(m, e, ss, 4, 0x10, &[bank]) != 1 {
        return;
    }
    let mut v = [F(0.0); 5];
    for (k, x) in v.iter_mut().enumerate() {
        let ss = snd(m);
        *x = F(f32::from_bits(e.call(m, SND_GET_VOLUME, &[ss, k as u32])));
    }
    on0(m, e, SND_STOP_ALL);
    wait_frame(m, e);
    on0(m, e, SND_CLOSE);
    wait_frame(m, e);
    e.call(m, SND_WAIT_LOAD, &[]);
    wait_frame(m, e);
    e.call(m, SND_LOAD_BANK, &[bank]);
    wait_frame(m, e);
    set_volumes(m, e, v);
    let ss = snd(m);
    let id = m.r32(s + 4);
    e.call(m, SND_OPEN, &[ss, id]);
    wait_frame(m, e);
    on0(m, e, SND_UPDATE);
    wait_frame(m, e);
    on0(m, e, SND_RELOAD);
}

/// `cFE3DSoundBase`'s virtuals: those that do nothing, answer 0, answer 0.0, or answer 1
pub const BASE_NOTHING: [u32; 43] = [
    0x2712e0, 0x2712f0, 0x271308, 0x271320, 0x271338, 0x271350, 0x271368, 0x271380, 0x271390, 0x2713a8, 0x2713b0,
    0x2713b8, 0x2713c0, 0x2713c8, 0x2713d8, 0x2713e0, 0x2713e8, 0x2713f0, 0x2713f8, 0x271400, 0x271408, 0x271410,
    0x271418, 0x271420, 0x271428, 0x271430, 0x271438, 0x271440, 0x271448, 0x271450, 0x271458, 0x271460, 0x271468,
    0x271470, 0x271478, 0x271480, 0x271488, 0x271490, 0x271498, 0x2714a0, 0x2714a8, 0x2714b0, 0x2714b8,
];
pub const BASE_ZERO: [u32; 4] = [0x2712e8, 0x271388, 0x271398, 0x2713d0];
pub const BASE_FZERO: [u32; 6] = [0x2712f8, 0x271310, 0x271328, 0x271340, 0x271358, 0x271370];
pub const BASE_ONE: [u32; 1] = [0x2713a0];

pub fn base_nothing() {}

pub fn base_zero() -> u32 {
    0
}

pub fn base_fzero() -> F {
    F(0.0)
}

pub fn base_one() -> u32 {
    1
}
