//! The replay manager's keyframe list, seeking and playlists (board row G12d), 1:1 on the game's bytes at their
//! addresses ([`Mem`], as for Pathfinder): the manager, its keyframes, highlights and playlists point at each other,
//! so the port keeps the same pointers. Checked in the runner (`tests/replaymgr.rs`).
//!
//! The manager (gApp+0x730 → +0x40): +0 the first keyframe, +4 the free keyframes, +8 how many (up to 16), +0xc a
//! keyframe stream's size, +0x10 recording started, +0x14 the keyframe playing, +0x18 where eviction starts, +0x1c
//! the keyframe being written, +0x20 the mode, +0x24 the rider whose highlights count, +0x30 the end frame, +0x34
//! the frame, +0x48 playing on, +0x38ec its own playlist, +0x391c the playlist playing (null none), +0x3930 the four
//! riders' pad streams, +0x394c each rider's highlight count, +0x3dc8 each rider's ten highlight pointers (+0x28
//! apart; a highlight's first word is its keyframe), +0x3e68 the keyframes (0x24 each: the stream, +0x10 next,
//! +0x14 previous, +0x18 pinned count, +0x1c in use, +0x20 the frame).

pub use crate::pf::Mem;

/// A walk the game would never finish (a list made circular): the port stops
fn guard(n: &mut u32) {
    *n += 1;
    assert!(*n < 1_000_000, "replay: the game walks this keyframe list forever");
}

pub const FIRST: u32 = 0x0;
pub const FREE: u32 = 0x4;
pub const PLAYING: u32 = 0x14;
pub const EVICT_FROM: u32 = 0x18;
pub const WRITING: u32 = 0x1c;
pub const MODE: u32 = 0x20;
pub const RIDER: u32 = 0x24;
pub const END: u32 = 0x30;
pub const FRAME: u32 = 0x34;
pub const PLAYING_ON: u32 = 0x48;
pub const PLAYLIST: u32 = 0x391c;
pub const INPUTS: u32 = 0x3930;
pub const HIGHLIGHT_COUNT: u32 = 0x394c;
pub const HIGHLIGHTS: u32 = 0x3dc8;

/// A keyframe's links and words
pub mod kf {
    pub const NEXT: u32 = 0x10;
    pub const PREV: u32 = 0x14;
    pub const PINS: u32 = 0x18;
    pub const USED: u32 = 0x1c;
    pub const FRAME: u32 = 0x20;
}

/// `ReplayKeyframe_Reset` 0x193068: the stream's write and read pointers and count, the pins, use and frame
pub fn keyframe_reset(m: &mut Mem, k: u32) {
    for off in [0xc, 0x20, 0x18, 0x1c, 4, 8] {
        m.w32(k + off, 0);
    }
}

/// `Replay_AllocKeyframe` 0x18ee68: the first free keyframe (one evicted when none is free), unlinked and unused
pub fn alloc_keyframe(m: &mut Mem, mgr: u32) -> u32 {
    if m.r32(mgr + FREE) == 0 {
        evict_keyframe(m, mgr);
    }
    let k = m.r32(mgr + FREE);
    m.w32(mgr + FREE, m.r32(k + kf::NEXT));
    m.w32(k + kf::USED, 0);
    m.w32(k + kf::PINS, 0);
    m.w32(k + kf::NEXT, 0);
    m.w32(k + kf::PREV, 0);
    k
}

/// `Replay_FindEvictableKeyframe` 0x18eec0(from; the first keyframe when 0): after it, the first unpinned keyframe
/// that has another unpinned one somewhere after it; 0 for none
pub fn find_evictable_keyframe(m: &Mem, mgr: u32, from: u32) -> u32 {
    let mut steps = 0u32;
    let from = if from == 0 { m.r32(mgr + FIRST) } else { from };
    let mut a = m.r32(from + kf::NEXT);
    loop {
        guard(&mut steps);
        if a == 0 {
            return 0;
        }
        if m.r32(a + kf::PINS) == 0 {
            break;
        }
        a = m.r32(a + kf::NEXT);
    }
    let mut b = m.r32(a + kf::NEXT);
    loop {
        guard(&mut steps);
        if b == 0 {
            return 0;
        }
        if m.r32(b + kf::PINS) == 0 {
            return a;
        }
        b = m.r32(b + kf::NEXT);
            guard(&mut steps);
    }
}

/// `Replay_EvictKeyframe` 0x18ef20: the evictable keyframe from +0x18 (else from the second keyframe) is freed;
/// +0x18 moves past it. (The game then walks on from it past pinned keyframes and drops what it finds.)
pub fn evict_keyframe(m: &mut Mem, mgr: u32) {
    let mut steps = 0u32;
    let mut k = find_evictable_keyframe(m, mgr, m.r32(mgr + EVICT_FROM));
    if k == 0 {
        let second = m.r32(m.r32(mgr + FIRST) + kf::NEXT);
        k = find_evictable_keyframe(m, mgr, second);
    }
    m.w32(mgr + EVICT_FROM, m.r32(k + kf::NEXT));
    let mut v = m.r32(k + kf::NEXT);
    while m.r32(v + kf::PINS) != 0 {
        guard(&mut steps);
        v = m.r32(v + kf::NEXT);
    }
    free_keyframe(m, mgr, k);
}

/// `Replay_FreeKeyframe` 0x18efa0: unlinked (the eviction start moves back to its previous one when it was there),
/// reset and put first on the free list
pub fn free_keyframe(m: &mut Mem, mgr: u32, k: u32) {
    if m.r32(mgr + 0x10) == k {
        m.w32(mgr + 0x10, m.r32(k + kf::PREV));
    }
    keyframe_reset(m, k);
    let prev = m.r32(k + kf::PREV);
    if prev != 0 {
        m.w32(prev + kf::NEXT, m.r32(k + kf::NEXT));
    }
    let next = m.r32(k + kf::NEXT);
    if next != 0 {
        m.w32(next + kf::PREV, m.r32(k + kf::PREV));
    }
    m.w32(k + kf::PREV, 0);
    m.w32(k + kf::NEXT, m.r32(mgr + FREE));
    m.w32(mgr + FREE, k);
}

/// `Replay_TryFreeKeyframe` 0x191728: freed when it has a previous keyframe and two after it, and is neither in use
/// nor pinned
pub fn try_free_keyframe(m: &mut Mem, mgr: u32, k: u32) {
    if m.r32(k + kf::PREV) != 0 {
        let next = m.r32(k + kf::NEXT);
        if next != 0 && m.r32(next + kf::NEXT) != 0 && m.r32(k + kf::USED) == 0 && m.r32(k + kf::PINS) == 0 {
            free_keyframe(m, mgr, k);
        }
    }
}

/// `Replay_PruneKeyframes` 0x191778(all): from the second keyframe, each one neither in use nor pinned is freed;
/// without `all` the last keyframe is kept
pub fn prune_keyframes(m: &mut Mem, mgr: u32, all: bool) {
    let mut steps = 0u32;
    let first = m.r32(mgr + FIRST);
    if first == 0 {
        return;
    }
    let mut cur = m.r32(first + kf::NEXT);
    if cur == 0 {
        return;
    }
    let mut nxt = m.r32(cur + kf::NEXT);
    if !all && nxt == 0 {
        return;
    }
    loop {
        guard(&mut steps);
        if m.r32(cur + kf::USED) == 0 && m.r32(cur + kf::PINS) == 0 {
            free_keyframe(m, mgr, cur);
        }
        if nxt == 0 {
            return;
        }
        cur = nxt;
        if !all && m.r32(cur + kf::NEXT) == 0 {
            return;
        }
        nxt = m.r32(cur + kf::NEXT);
    }
}

/// `FUN_00191820`: the keyframe before the one at +0x10 (that one when it is first)
pub fn keyframe_before_latest(m: &Mem, mgr: u32) -> u32 {
    let latest = m.r32(mgr + 0x10);
    let prev = m.r32(latest + kf::PREV);
    if prev == 0 { latest } else { prev }
}

/// `Replay_IsKeyframeInHighlight` 0x190f58(rider, keyframe): one of the rider's highlights starts at it
pub fn is_keyframe_in_highlight(m: &Mem, mgr: u32, rider: u32, k: u32) -> bool {
    let n = m.r32(mgr + HIGHLIGHT_COUNT + rider.wrapping_mul(4)) as i32;
    (0..n.max(0) as u32).any(|i| m.r32(m.r32(mgr + HIGHLIGHTS + i * 4 + rider.wrapping_mul(0x28))) == k)
}

fn usable(m: &Mem, mgr: u32, k: u32) -> bool {
    m.r32(k + kf::PINS) == 0 || is_keyframe_in_highlight(m, mgr, m.r32(mgr + RIDER), k)
}

/// `Replay_KeyframeAt` 0x190d88(n): the n-th keyframe, 0 past the end
pub fn keyframe_at(m: &Mem, mgr: u32, mut n: u32) -> u32 {
    let mut steps = 0u32;
    let mut k = m.r32(mgr + FIRST);
    while n != 0 {
        guard(&mut steps);
        if k == 0 {
            return 0;
        }
        n -= 1;
        k = m.r32(k + kf::NEXT);
    }
    k
}

/// `Replay_FindKeyframe` 0x190db0(frame): the keyframe at the frame, or the last before it, among those unpinned or
/// in the rider's highlights; the last keyframe when none
pub fn find_keyframe(m: &Mem, mgr: u32, frame: u32) -> u32 {
    let mut steps = 0u32;
    let mut k = m.r32(mgr + FIRST);
    loop {
        guard(&mut steps);
        let next = m.r32(k + kf::NEXT);
        if next == 0 {
            return k;
        }
        let f = m.r32(k + kf::FRAME);
        if f == frame && usable(m, mgr, k) {
            return k;
        }
        if f < frame && frame < m.r32(next + kf::FRAME) && usable(m, mgr, k) {
            return k;
        }
        k = next;
    }
}

/// `Replay_NextKeyframe` 0x190e78(frame) / `Replay_PrevKeyframe` 0x190ee8: from the keyframe found, the next (or
/// previous) usable one; the one found when there is none
pub fn next_keyframe(m: &Mem, mgr: u32, frame: u32) -> u32 {
    step_keyframe(m, mgr, frame, kf::NEXT)
}
pub fn prev_keyframe(m: &Mem, mgr: u32, frame: u32) -> u32 {
    step_keyframe(m, mgr, frame, kf::PREV)
}
fn step_keyframe(m: &Mem, mgr: u32, frame: u32, link: u32) -> u32 {
    let mut steps = 0u32;
    let found = find_keyframe(m, mgr, frame);
    let mut k = m.r32(found + link);
    while k != 0 && !usable(m, mgr, k) {
        guard(&mut steps);
        k = m.r32(k + link);
    }
    if k != 0 { k } else { found }
}

/// `Replay_KeyframeCount` 0x190ff0: the keyframes, 0 before recording started (+0x10)
pub fn keyframe_count(m: &Mem, mgr: u32) -> i32 {
    let mut steps = 0u32;
    if m.r32(mgr + 0x10) == 0 {
        return 0;
    }
    let (mut n, mut k) = (0, m.r32(mgr + FIRST));
    while k != 0 {
        guard(&mut steps);
        n += 1;
        k = m.r32(k + kf::NEXT);
    }
    n
}

/// `Replay_KeyframeIndex` 0x191038(keyframe): its place, not counting the one being written; 0 when not found
pub fn keyframe_index(m: &Mem, mgr: u32, target: u32) -> i32 {
    let mut steps = 0u32;
    let (mut n, mut k) = (0, m.r32(mgr + FIRST));
    loop {
        guard(&mut steps);
        if k == 0 {
            return 0;
        }
        if k == target {
            return n;
        }
        if k != m.r32(mgr + WRITING) {
            n += 1;
        }
        k = m.r32(k + kf::NEXT);
    }
}

/// `Replay_KeyframeCountUsed` 0x191078: the keyframes but the one being written; 0 before recording started
pub fn keyframe_count_used(m: &Mem, mgr: u32) -> i32 {
    let mut steps = 0u32;
    if m.r32(mgr + 0x10) == 0 {
        return 0;
    }
    let (mut n, mut k) = (0, m.r32(mgr + FIRST));
    if k == 0 {
        return 0;
    }
    loop {
        guard(&mut steps);
        if k != m.r32(mgr + WRITING) {
            n += 1;
        }
        k = m.r32(k + kf::NEXT);
        if k == 0 {
            return n;
        }
    }
}

/// `ReplayInput_SeekFrame` 0x192758 on a pad stream in memory (as `replay::Input::seek_frame`)
pub fn input_seek_frame(m: &mut Mem, a: u32, frame: u32) -> bool {
    let count = m.r32(a + 0x8000) as i32;
    if count <= 0 {
        return false;
    }
    let mut i = 0i32;
    let mut end = m.r8(a + 0x6664) as u32;
    while end <= frame {
        i += 1;
        if count <= i {
            return false;
        }
        end = end.wrapping_add(m.r8(a + 0x6664 + i as u32) as u32);
    }
    m.w32(a + 0x8004, i as u32);
    m.w32(a + 0x8008, end.wrapping_sub(frame));
    true
}

fn seek_inputs(m: &mut Mem, mgr: u32, frame: u32) {
    for r in 0..4 {
        let a = m.r32(mgr + INPUTS + 4 * r);
        if a != 0 {
            input_seek_frame(m, a, frame);
        }
    }
}

/// `Replay_IsAtEnd` 0x18f020: playing (modes 1, 2, 7) at or past the end frame or no longer on (+0x48), or past
/// the playlist entry's end; modes 3, 5, 6, 8, 9 always; mode 4 at or past the end frame
pub fn is_at_end(m: &Mem, mgr: u32) -> bool {
    let (end, frame) = (m.r32(mgr + END), m.r32(mgr + FRAME));
    match m.r32(mgr + MODE) {
        1 | 2 | 7 => {
            let at = end <= frame || m.r32(mgr + PLAYING_ON) == 0;
            let pl = m.r32(mgr + PLAYLIST);
            if pl != 0 && frame >= playlist_current_end(m, pl) {
                return true;
            }
            at
        }
        3 | 5 | 6 | 8 | 9 => true,
        4 => end <= frame,
        _ => false,
    }
}

/// `Replay_SeekStart` 0x18f0d8: playing off, the frame and keyframe at the start (of the playlist entry when one
/// plays), the pads sought
pub fn seek_start(m: &mut Mem, mgr: u32) {
    m.w32(mgr + PLAYING_ON, 0);
    let pl = m.r32(mgr + PLAYLIST);
    if pl == 0 {
        m.w32(mgr + FRAME, 0);
        m.w32(mgr + PLAYING, m.r32(mgr + FIRST));
    } else {
        m.w32(mgr + FRAME, playlist_current_start(m, pl));
        m.w32(mgr + PLAYING, playlist_current_keyframe(m, pl));
    }
    seek_inputs(m, mgr, m.r32(mgr + FRAME));
}

/// `Replay_SeekEnd` 0x18f168: the last keyframe (`Replay_FindKeyframe` of 100,000,000) and its frame, or the
/// playlist entry's start
pub fn seek_end(m: &mut Mem, mgr: u32) {
    m.w32(mgr + PLAYING_ON, 0);
    let pl = m.r32(mgr + PLAYLIST);
    if pl == 0 {
        let k = find_keyframe(m, mgr, 100_000_000);
        m.w32(mgr + PLAYING, k);
        m.w32(mgr + FRAME, m.r32(k + kf::FRAME));
    } else {
        m.w32(mgr + FRAME, playlist_current_start(m, pl));
        m.w32(mgr + PLAYING, playlist_current_keyframe(m, pl));
    }
    seek_inputs(m, mgr, m.r32(mgr + FRAME));
}

/// `Replay_SeekToKeyframe` 0x191100: the keyframe for the frame (the playlist entry's when one plays), the frame at
/// it, the pads sought
pub fn seek_to_keyframe(m: &mut Mem, mgr: u32) {
    let pl = m.r32(mgr + PLAYLIST);
    let k = if pl == 0 { find_keyframe(m, mgr, m.r32(mgr + FRAME)) } else { playlist_current_keyframe(m, pl) };
    m.w32(mgr + PLAYING, k);
    m.w32(mgr + FRAME, m.r32(k + kf::FRAME));
    seek_inputs(m, mgr, m.r32(mgr + FRAME));
}

/// `Replay_StepForward` 0x191190 / `Replay_StepBack` 0x1911e0: to the next (previous) keyframe; nothing while a
/// playlist plays
pub fn step_forward(m: &mut Mem, mgr: u32) {
    step(m, mgr, true)
}
pub fn step_back(m: &mut Mem, mgr: u32) {
    step(m, mgr, false)
}
fn step(m: &mut Mem, mgr: u32, forward: bool) {
    if m.r32(mgr + PLAYLIST) != 0 {
        return;
    }
    let frame = m.r32(mgr + FRAME);
    let k = if forward { next_keyframe(m, mgr, frame) } else { prev_keyframe(m, mgr, frame) };
    m.w32(mgr + PLAYING, k);
    m.w32(mgr + FRAME, m.r32(k + kf::FRAME));
    seek_to_keyframe(m, mgr);
}

/// `FUN_00191278`: mode 10 becomes 11
pub fn mode_10_to_11(m: &mut Mem, mgr: u32) {
    if m.r32(mgr + MODE) == 10 {
        m.w32(mgr + MODE, 11);
    }
}

// A playlist: +0 the count (up to 10), +4 the entry playing, +8 the entries (highlights: +0 the keyframe, +4 the
// length in frames)

/// `ReplayPlaylist_Current` 0x195428: the entry playing
pub fn playlist_current(m: &Mem, pl: u32) -> u32 {
    m.r32(pl + m.r32(pl + 4).wrapping_mul(4) + 8)
}
/// `ReplayPlaylist_CurrentKeyframe` 0x1955a0
pub fn playlist_current_keyframe(m: &Mem, pl: u32) -> u32 {
    m.r32(playlist_current(m, pl))
}
/// `ReplayPlaylist_CurrentStart` 0x195440: its keyframe's frame
pub fn playlist_current_start(m: &Mem, pl: u32) -> u32 {
    m.r32(playlist_current_keyframe(m, pl) + kf::FRAME)
}
/// `ReplayPlaylist_CurrentEnd` 0x195460: its start plus its length
pub fn playlist_current_end(m: &Mem, pl: u32) -> u32 {
    let e = playlist_current(m, pl);
    m.r32(m.r32(e) + kf::FRAME).wrapping_add(m.r32(e + 4))
}
/// `ReplayPlaylist_Next` 0x195488: on to the next entry; false at the last
pub fn playlist_next(m: &mut Mem, pl: u32) -> bool {
    let (n, i) = (m.r32(pl) as i32, m.r32(pl + 4) as i32);
    if n.wrapping_sub(1) <= i {
        return false;
    }
    m.w32(pl + 4, (i + 1) as u32);
    true
}
/// `ReplayPlaylist_Add` 0x1955b8: up to ten entries
pub fn playlist_add(m: &mut Mem, pl: u32, h: u32) {
    let n = m.r32(pl) as i32;
    if n < 10 {
        m.w32(pl, (n + 1) as u32);
        m.w32(pl + (n as u32) * 4 + 8, h);
    }
}
/// `FUN_001955e0`: the entry playing set
pub fn playlist_set_current(m: &mut Mem, pl: u32, i: u32) {
    m.w32(pl + 4, i);
}
/// `ReplayPlaylist_Clear` 0x195690
pub fn playlist_clear(m: &mut Mem, pl: u32) {
    for k in 0..12 {
        m.w32(pl + 4 * k, 0);
    }
}

// More on playlists: seeking by frame (the overlay's scrubbing)

/// `ReplayPlaylist_SeekForward` 0x1954b8(frame): the first entry whose keyframe is after the frame
pub fn playlist_seek_forward(m: &mut Mem, t: u32, frame: u32) -> bool {
    let n = m.r32(t) as i32;
    for i in 0..n.max(0) as u32 {
        if frame < m.r32(m.r32(m.r32(t + 8 + 4 * i)) + kf::FRAME) {
            m.w32(t + 4, i);
            return true;
        }
    }
    false
}

/// `ReplayPlaylist_SeekBack` 0x195510(frame): from the last entry back, the one before an entry at the frame, or
/// the last entry before it; the first when the frame is past the first entry's
pub fn playlist_seek_back(m: &mut Mem, t: u32, frame: u32) -> bool {
    let mut i = (m.r32(t) as i32).wrapping_sub(1);
    let at = |m: &Mem, i: i32| m.r32(m.r32(m.r32(t + 8 + 4 * i as u32)) + kf::FRAME);
    while i >= 1 {
        let f = at(m, i);
        if f == frame {
            m.w32(t + 4, (i - 1) as u32);
            return true;
        }
        if f < frame {
            m.w32(t + 4, i as u32);
            return true;
        }
        i -= 1;
    }
    if frame <= at(m, 0) {
        return false;
    }
    m.w32(t + 4, 0);
    true
}

// Highlights (board row G12e): 0x1c-byte records in a pool at +0x3968 (free list from +0x3964 through +0x14):
// +0 the keyframe, +4 the length in frames, +8 the kind (1, 2: the two kinds of moment a pin keeps), +0xc / +0x10
// the two kinds' scores (floats), +0x14 next free, +0x18 the rider. Each rider keeps up to ten in frame order
// (+0x394c the count, +0x3dc8 the pointers). +0x395c the highlight being recorded and +0x3960 the frame it started.
// Pins (the four at +0x3920, one per rider, 0x1c bytes): +0 the rider, +4 the highlight being made, +8 four slots of
// kept highlights (+0x3940 how many are used: 2 or 4; +0x3944 / +0x3948 the quota for kinds 1 and 2), +0x18 the
// keyframe it pins.

pub mod hl {
    pub const KEYFRAME: u32 = 0;
    pub const LEN: u32 = 4;
    pub const KIND: u32 = 8;
    pub const SCORE1: u32 = 0xc;
    pub const SCORE2: u32 = 0x10;
    pub const NEXT_FREE: u32 = 0x14;
    pub const RIDER: u32 = 0x18;
}
pub mod pin {
    pub const RIDER: u32 = 0;
    pub const MAKING: u32 = 4;
    pub const SLOTS: u32 = 8;
    pub const KEYFRAME: u32 = 0x18;
}
pub const PINS: u32 = 0x3920;
pub const SLOT_COUNT: u32 = 0x3940;
pub const QUOTA1: u32 = 0x3944;
pub const QUOTA2: u32 = 0x3948;
pub const RECORDING_HL: u32 = 0x395c;
pub const RECORDING_FROM: u32 = 0x3960;
pub const HL_FREE: u32 = 0x3964;

fn count_at(mgr: u32, rider: u32) -> u32 {
    mgr + HIGHLIGHT_COUNT + rider.wrapping_mul(4)
}
fn list_at(mgr: u32, rider: u32, i: u32) -> u32 {
    mgr + HIGHLIGHTS + i.wrapping_mul(4) + rider.wrapping_mul(0x28)
}

/// `ReplayPin_InitSlot` 0x193a78 (and its jump 0x1956c0): a highlight cleared, its rider -1
pub fn highlight_reset(m: &mut Mem, h: u32) {
    m.w32(h + 8, 0);
    m.w32(h + 0x18, u32::MAX);
    m.w32(h + 0xc, 0);
    m.w32(h + 0x10, 0);
    m.w32(h + 4, 0);
    m.w32(h, 0);
    m.w32(h + 0x14, 0);
}

fn take_highlight(m: &mut Mem, mgr: u32, rider: u32) -> u32 {
    let h = m.r32(mgr + HL_FREE);
    m.w32(mgr + HL_FREE, m.r32(h + hl::NEXT_FREE));
    highlight_reset(m, h);
    m.w32(h + hl::RIDER, rider);
    h
}

/// `Replay_InsertHighlight` 0x191eb0(frame, rider): a free highlight put in the rider's list in frame order; 0 when
/// the rider has ten or one starts at that frame
pub fn insert_highlight(m: &mut Mem, mgr: u32, frame: u32, rider: u32) -> u32 {
    let n = m.r32(count_at(mgr, rider)) as i32;
    if n > 9 {
        return 0;
    }
    for i in 0..n.max(0) {
        let f = m.r32(m.r32(m.r32(list_at(mgr, rider, i as u32))) + kf::FRAME);
        if f == frame {
            return 0;
        }
        if frame <= f {
            let h = take_highlight(m, mgr, rider);
            let mut j = m.r32(count_at(mgr, rider)) as i32 - 1;
            while i <= j {
                m.w32(list_at(mgr, rider, (j + 1) as u32), m.r32(list_at(mgr, rider, j as u32)));
                j -= 1;
            }
            m.w32(count_at(mgr, rider), m.r32(count_at(mgr, rider)).wrapping_add(1));
            m.w32(list_at(mgr, rider, i as u32), h);
            return h;
        }
    }
    let h = take_highlight(m, mgr, rider);
    let n = m.r32(count_at(mgr, rider));
    m.w32(count_at(mgr, rider), n.wrapping_add(1));
    m.w32(list_at(mgr, rider, n), h);
    h
}

/// `Replay_RemoveHighlight` 0x192058(rider, highlight): its keyframe unpinned (and freed if it may be), then out of
/// the rider's list and back on the free list
pub fn remove_highlight(m: &mut Mem, mgr: u32, rider: u32, h: u32) {
    let k = m.r32(h + hl::KEYFRAME);
    if k != 0 {
        m.w32(k + kf::PINS, m.r32(k + kf::PINS).wrapping_sub(1));
        try_free_keyframe(m, mgr, k);
    }
    m.w32(h, 0);
    let n = m.r32(count_at(mgr, rider)) as i32;
    for i in 0..n.max(0) {
        if m.r32(list_at(mgr, rider, i as u32)) == h {
            let last = m.r32(count_at(mgr, rider)) as i32 - 1;
            m.w32(h + hl::NEXT_FREE, m.r32(mgr + HL_FREE));
            m.w32(mgr + HL_FREE, h);
            let mut j = i;
            while j < last {
                m.w32(list_at(mgr, rider, j as u32), m.r32(list_at(mgr, rider, j as u32 + 1)));
                j += 1;
            }
            m.w32(count_at(mgr, rider), m.r32(count_at(mgr, rider)).wrapping_sub(1));
            return;
        }
    }
}

/// `Replay_FindHighlight` 0x192178(rider, frame): the rider's highlight starting at the frame, or 0
pub fn find_highlight(m: &Mem, mgr: u32, rider: u32, frame: u32) -> u32 {
    let n = m.r32(count_at(mgr, rider)) as i32;
    for i in 0..n.max(0) {
        let h = m.r32(list_at(mgr, rider, i as u32));
        if m.r32(m.r32(h) + kf::FRAME) == frame {
            return h;
        }
    }
    0
}

/// `Replay_CanAddHighlight` 0x1921e0: -1 at the end frame, 1 inside one of the rider's highlights, else whether the
/// rider has fewer than ten
pub fn can_add_highlight(m: &Mem, mgr: u32) -> u32 {
    let frame = m.r32(mgr + FRAME);
    if frame == m.r32(mgr + END) {
        return u32::MAX;
    }
    let rider = m.r32(mgr + RIDER);
    let n = m.r32(count_at(mgr, rider)) as i32;
    for i in 0..n.max(0) {
        let h = m.r32(list_at(mgr, rider, i as u32));
        let d = frame.wrapping_sub(m.r32(m.r32(h) + kf::FRAME)) as i32;
        if d >= 0 && d <= m.r32(h + hl::LEN) as i32 {
            return 1;
        }
    }
    (n < 10) as u32
}

/// `Replay_BeginHighlight` 0x1922a8(record: `Replay_RecordKeyframe`(frame, 0), the keyframe it saved or 0): not while
/// one is recorded or at the end frame. Inside (or up to 11 frames past) one of the rider's highlights, that one goes
/// on from its start; else a new one is inserted at the frame, on the keyframe there (one recorded when the
/// keyframe found is not at the frame; none recorded drops it) and pins it.
pub fn begin_highlight(m: &mut Mem, mgr: u32, record: &mut dyn FnMut(&mut Mem, u32) -> u32) -> bool {
    if m.r32(mgr + RECORDING_HL) != 0 {
        return false;
    }
    let frame = m.r32(mgr + FRAME);
    if frame == m.r32(mgr + END) {
        return false;
    }
    let rider = m.r32(mgr + RIDER);
    let n = m.r32(count_at(mgr, rider)) as i32;
    for i in 0..n.max(0) {
        let h = m.r32(list_at(mgr, rider, i as u32));
        let d = frame.wrapping_sub(m.r32(m.r32(h) + kf::FRAME)) as i32;
        if d >= 0 && d <= (m.r32(h + hl::LEN) as i32).wrapping_add(0xb) {
            m.w32(mgr + RECORDING_FROM, frame);
            m.w32(mgr + RECORDING_HL, h);
            m.w32(h + hl::LEN, frame.wrapping_sub(m.r32(m.r32(h) + kf::FRAME)));
            return true;
        }
    }
    let h = insert_highlight(m, mgr, frame, m.r32(mgr + RIDER));
    if h == 0 {
        return false;
    }
    m.w32(mgr + RECORDING_HL, h);
    m.w32(mgr + RECORDING_FROM, m.r32(mgr + FRAME));
    let k = find_keyframe(m, mgr, m.r32(mgr + FRAME));
    let from = m.r32(mgr + RECORDING_FROM);
    m.w32(h, k);
    if m.r32(k + kf::FRAME) != from {
        let k2 = record(m, from);
        m.w32(h, k2);
        if k2 == 0 {
            m.w32(mgr + RECORDING_HL, 0);
            remove_highlight(m, mgr, m.r32(mgr + RIDER), h);
            return false;
        }
    }
    m.w32(h + 0xc, 0);
    let k = m.r32(h);
    let pins = m.r32(k + kf::PINS);
    m.w32(h + 0x10, 0);
    m.w32(k + kf::PINS, pins.wrapping_add(1));
    m.w32(h + hl::LEN, 0);
    m.w32(h + hl::RIDER, m.r32(mgr + RIDER));
    true
}

/// `Replay_EndHighlight` 0x1924a0: the highlight being recorded runs to the frame; the rider's other highlights that
/// start inside it are merged in (what they run past it added to its length, their pins' slots cleared) and
/// removed
pub fn end_highlight(m: &mut Mem, mgr: u32) {
    let mut h = m.r32(mgr + RECORDING_HL);
    if h != 0 {
        let rider = m.r32(h + hl::RIDER);
        let now = m.r32(mgr + FRAME);
        m.w32(h + hl::LEN, m.r32(h + hl::LEN).wrapping_add(now.wrapping_sub(m.r32(mgr + RECORDING_FROM))));
        let start = m.r32(m.r32(h) + kf::FRAME);
        if m.r32(count_at(mgr, rider)) as i32 > 0 {
            let mut i = 0i32;
            loop {
                let slot = list_at(mgr, rider, i as u32);
                let o = m.r32(slot);
                if o != h {
                    let f = m.r32(m.r32(o) + kf::FRAME);
                    if start <= f && f < now {
                        let extra = m.r32(o + hl::LEN).wrapping_sub(now.wrapping_sub(f)) as i32;
                        if extra > 0 {
                            m.w32(h + hl::LEN, m.r32(h + hl::LEN).wrapping_add(extra as u32));
                        }
                        let p = m.r32(mgr + PINS + rider.wrapping_mul(4));
                        if p != 0 {
                            pin_clear_highlight_ref(m, p, m.r32(slot));
                        }
                        i -= 1;
                        let o = m.r32(slot);
                        remove_highlight(m, mgr, rider, o);
                    }
                }
                i += 1;
                if m.r32(count_at(mgr, rider)) as i32 <= i {
                    break;
                }
                h = m.r32(mgr + RECORDING_HL);
            }
        }
    }
    m.w32(mgr + RECORDING_HL, 0);
}

/// `Replay_DropCurrentHighlight` 0x1925f8: the rider's first highlight the frame is not past the end of (taken
/// from its start, without a lower bound) is removed, and its pin's slot cleared
pub fn drop_current_highlight(m: &mut Mem, mgr: u32) -> bool {
    let mut rider = m.r32(mgr + RIDER);
    let n = m.r32(count_at(mgr, rider)) as i32;
    for i in 0..n.max(0) {
        let h = m.r32(list_at(mgr, rider, i as u32));
        let d = m.r32(mgr + FRAME).wrapping_sub(m.r32(m.r32(h) + kf::FRAME)) as i32;
        if d <= m.r32(h + hl::LEN) as i32 {
            let p = m.r32(mgr + PINS + rider.wrapping_mul(4));
            if p != 0 {
                pin_clear_highlight_ref(m, p, h);
                rider = m.r32(mgr + RIDER);
            }
            let h = m.r32(list_at(mgr, rider, i as u32));
            remove_highlight(m, mgr, rider, h);
            return true;
        }
    }
    false
}

/// `Replay_ReleaseAllPins` 0x192458
pub fn release_all_pins(m: &mut Mem, mgr: u32) {
    for r in 0..4 {
        let p = m.r32(mgr + PINS + 4 * r);
        if p != 0 {
            pin_release(m, mgr, p);
        }
    }
}

/// `Replay_Freeze` 0x191230: the pins released the first time (+0x50 and +0x4c clear), then frozen (+0x50)
pub fn freeze(m: &mut Mem, mgr: u32) {
    if m.r32(mgr + 0x50) == 0 && m.r32(mgr + 0x4c) == 0 {
        release_all_pins(m, mgr);
    }
    m.w32(mgr + 0x50, 1);
}

/// `Replay_AddPin` 0x191a18(rider; `alloc`: `MemPool_Alloc` of 0x1c bytes): in the first empty pin place, a new pin
/// (filled with 0xdeadc0de, then set up); 0 when all four are taken
pub fn add_pin(m: &mut Mem, mgr: u32, rider: u32, alloc: &mut dyn FnMut(&mut Mem) -> u32) -> u32 {
    for r in 0..4 {
        let at = mgr + PINS + 4 * r;
        if m.r32(at) == 0 {
            let p = alloc(m);
            if p != 0 {
                for w in 0..7 {
                    m.w32(p + 4 * w, 0xdead_c0de);
                }
            }
            pin_init(m, p, rider);
            m.w32(at, p);
            return p;
        }
    }
    0
}

/// `Replay_RemovePin` 0x191ac0(pin; `delete`: `Mem_Delete`): its place emptied, the pin cleared and deleted
pub fn remove_pin(m: &mut Mem, mgr: u32, p: u32, delete: &mut dyn FnMut(&mut Mem, u32)) {
    for r in 0..4 {
        let at = mgr + PINS + 4 * r;
        let q = m.r32(at);
        if q == p {
            if q != 0 {
                pin_clear(m, mgr, q);
                delete(m, q);
            }
            m.w32(at, 0);
            return;
        }
    }
}

/// `ReplayPin_Init` 0x193118(rider) and `ReplayPin_Reset` 0x193198
pub fn pin_init(m: &mut Mem, p: u32, rider: u32) {
    m.w32(p, rider);
    pin_reset(m, p);
}
pub fn pin_reset(m: &mut Mem, p: u32) {
    m.w32(p + 4, 0);
    m.w32(p + 0x18, 0);
    for off in [0x14, 0x10, 0xc, 8] {
        m.w32(p + off, 0);
    }
}

/// `ReplayPin_Clear` 0x1931d0 (and its jump 0x1932e8): released, and every kept highlight removed
pub fn pin_clear(m: &mut Mem, mgr: u32, p: u32) {
    pin_release(m, mgr, p);
    for s in 0..4 {
        let at = p + pin::SLOTS + 4 * s;
        let h = m.r32(at);
        if h != 0 {
            remove_highlight(m, mgr, m.r32(p), h);
            m.w32(at, 0);
        }
    }
}

/// `FUN_00193250`: the quota of a kind (1: +0x3944, 2: +0x3948, else 0)
pub fn pin_quota(m: &Mem, mgr: u32, kind: u32) -> i32 {
    match kind {
        1 => m.r32(mgr + QUOTA1) as i32,
        2 => m.r32(mgr + QUOTA2) as i32,
        _ => 0,
    }
}

/// `ReplayPin_CountForRider` 0x1932a8(kind): the kept highlights of that kind
pub fn pin_count_kind(m: &Mem, p: u32, kind: u32) -> i32 {
    (0..4)
        .filter(|s| {
            let h = m.r32(p + pin::SLOTS + 4 * s);
            h != 0 && m.r32(h + hl::KIND) == kind
        })
        .count() as i32
}

/// `ReplayPin_Release` 0x1932f8 (and its jump `ReplayPin_ReleaseThunk` 0x1932f0): the pinned keyframe let go (freed
/// if it may be), the highlight being made removed
pub fn pin_release(m: &mut Mem, mgr: u32, p: u32) {
    let k = m.r32(p + pin::KEYFRAME);
    if k != 0 {
        m.w32(k + kf::PINS, m.r32(k + kf::PINS).wrapping_sub(1));
        try_free_keyframe(m, mgr, k);
        m.w32(p + pin::KEYFRAME, 0);
    }
    m.w32(p + pin::KEYFRAME, 0);
    let h = m.r32(p + pin::MAKING);
    if h != 0 {
        remove_highlight(m, mgr, m.r32(p), h);
        m.w32(p + pin::MAKING, 0);
    }
}

/// `ReplayPin_Begin` 0x193370(kind): while recording (+0x40 clear, mode 0 or past 9, not frozen): the keyframe
/// before the newest pinned and a highlight of the kind begun at its frame
pub fn pin_begin(m: &mut Mem, mgr: u32, p: u32, kind: u32) {
    let mode = m.r32(mgr + MODE) as i32;
    if m.r32(mgr + 0x40) == 0 && (mode == 0 || mode > 9) && m.r32(mgr + 0x50) == 0 {
        pin_release(m, mgr, p);
        let k = keyframe_before_latest(m, mgr);
        let frame = m.r32(k + kf::FRAME);
        m.w32(k + kf::PINS, m.r32(k + kf::PINS).wrapping_add(1));
        m.w32(p + pin::KEYFRAME, k);
        let h = insert_highlight(m, mgr, frame, m.r32(p));
        m.w32(p + pin::MAKING, h);
        if h == 0 {
            pin_release(m, mgr, p);
            return;
        }
        m.w32(h + hl::KIND, kind);
    }
}

/// `FUN_00193ad8`(highlight, frame): its length from the frame to the manager's +0x2c, plus 60
pub fn pin_set_length(m: &mut Mem, mgr: u32, h: u32, frame: u32) {
    m.w32(h + hl::LEN, m.r32(mgr + 0x2c).wrapping_sub(frame).wrapping_add(0x3c));
}

/// `ReplayPin_End` 0x193448: nothing while a replay plays (modes 1..9); recording, the highlight being made gets its
/// length and is committed (released when it has no keyframe); otherwise released
pub fn pin_end(m: &mut Mem, mgr: u32, p: u32) {
    let mode = m.r32(mgr + MODE) as i32;
    if mode != 0 && mode < 10 {
        return;
    }
    if m.r32(mgr + 0x40) == 0 {
        let h = m.r32(p + pin::MAKING);
        if h != 0 {
            let k = m.r32(p + pin::KEYFRAME);
            if k == 0 {
                pin_release(m, mgr, p);
                return;
            }
            pin_set_length(m, mgr, h, m.r32(k + kf::FRAME));
        }
        pin_commit(m, mgr, p);
        return;
    }
    pin_release(m, mgr, p);
}

fn score(m: &Mem, h: u32, kind: u32) -> f32 {
    match kind {
        1 => f32::from_bits(m.r32(h + hl::SCORE1)),
        2 => f32::from_bits(m.r32(h + hl::SCORE2)),
        _ => 0.0,
    }
}

/// `ReplayPin_FindWorstSlot` 0x1934f0(kind): of the slots in use (+0x3940), the kept highlight with the lowest score
/// of the kind (under 1e10); -1 for none
pub fn pin_find_worst_slot(m: &Mem, mgr: u32, p: u32, kind: u32) -> i32 {
    let (mut best, mut low) = (-1, 1e10f32);
    let mut i = 0i32;
    while i < m.r32(mgr + SLOT_COUNT) as i32 {
        let h = m.r32(p + pin::SLOTS + 4 * i as u32);
        if h != 0 && score(m, h, kind) < low {
            low = score(m, h, kind);
            best = i;
        }
        i += 1;
    }
    best
}

/// `ReplayPin_PickSlot` 0x1935d8(highlight): an empty slot; else, under the kind's quota (less one), the worst of
/// the other kind when that kind is at its quota; at it, the worst of this kind unless the new one scores lower;
/// -1 for none (and for a kind-1 highlight scoring 0 or less)
pub fn pin_pick_slot(m: &Mem, mgr: u32, p: u32, h: u32) -> i32 {
    let kind = m.r32(h + hl::KIND);
    if kind == 1 && score(m, h, 1) <= 0.0 {
        return -1;
    }
    let slots = m.r32(mgr + SLOT_COUNT) as i32;
    let mut found = -1i32;
    if slots > 0 {
        if m.r32(p + pin::SLOTS) == 0 {
            found = 0;
        } else {
            let mut i = 0;
            loop {
                i += 1;
                found = -1;
                if slots <= i {
                    break;
                }
                found = i;
                if m.r32(p + pin::SLOTS + 4 * i as u32) == 0 {
                    break;
                }
            }
        }
    }
    if found < 0 {
        let n = pin_count_kind(m, p, kind);
        let q = pin_quota(m, mgr, kind);
        if n < q - 1 {
            let other = match kind {
                1 => Some(2),
                2 => Some(1),
                _ => None,
            };
            if let Some(o) = other {
                if pin_quota(m, mgr, o) <= pin_count_kind(m, p, o) {
                    found = pin_find_worst_slot(m, mgr, p, o);
                }
            }
        } else {
            found = pin_find_worst_slot(m, mgr, p, kind);
            if found >= 0 {
                let worst = m.r32(p + pin::SLOTS + 4 * found as u32);
                if score(m, h, kind) < score(m, worst, kind) {
                    found = -1;
                }
            }
        }
    }
    found
}

/// `ReplayPin_Commit` 0x1937d0: the highlight being made (of a kind) kept in the slot picked (the highlight there
/// removed), on the pinned keyframe; released when it has no kind or no slot
pub fn pin_commit(m: &mut Mem, mgr: u32, p: u32) {
    let h = m.r32(p + pin::MAKING);
    if h != 0 && m.r32(h + hl::KIND) != 0 {
        let rider = m.r32(h + hl::RIDER);
        let s = pin_pick_slot(m, mgr, p, h);
        if s >= 0 {
            let at = p + pin::SLOTS + 4 * s as u32;
            let old = m.r32(at);
            if old != 0 {
                remove_highlight(m, mgr, rider, old);
            }
            let k = m.r32(p + pin::KEYFRAME);
            m.w32(at, m.r32(p + pin::MAKING));
            let h = m.r32(p + pin::MAKING);
            m.w32(p + pin::MAKING, 0);
            m.w32(h, k);
            m.w32(p + pin::KEYFRAME, 0);
            return;
        }
    }
    pin_release(m, mgr, p);
}

/// `ReplayPin_ClearHighlightRef` 0x1938a0(highlight): the slots holding it emptied
pub fn pin_clear_highlight_ref(m: &mut Mem, p: u32, h: u32) {
    for s in 0..4 {
        if m.r32(p + pin::SLOTS + 4 * s) == h {
            m.w32(p + pin::SLOTS + 4 * s, 0);
        }
    }
}

// Camera tracks (board row G12f): one per rider at +0x5c, 0xe24 apart: +0 the cut count (up to 128), +4 the cut
// playing (-1 none), +8 the default cut, +0x24 the cuts (0x1c each: the five chase-camera words, the preset, the
// frame). The flag at 0x32ca4c (the edited tracks in use) picks the cuts over the default.

/// The flag at 0x32ca4c
pub const CUTS_ON: u32 = 0x32ca4c;
pub const CAM_TRACKS: u32 = 0x5c;
pub const CAM_TRACK_SIZE: u32 = 0xe24;
pub const MAX_CUTS: i32 = 0x80;

/// The camera calls the tracks make
pub trait CamEnv {
    /// `Camera_GetChaseParams` 0x178968: the five chase words
    fn chase_params(&mut self) -> [u32; 5];
    /// `Camera_GetPreset` 0x1786c0
    fn preset(&mut self) -> u32;
    /// `Camera_SelectPreset` 0x1786d0
    fn select_preset(&mut self, p: u32);
    /// `Camera_SetChaseParams` 0x178a28(the five words, 1)
    fn set_chase_params(&mut self, c: [u32; 5], snap: u32);
    /// `Camera_SetReplayMode` 0x1785c0(on, rider)
    fn set_replay_mode(&mut self, on: u32, rider: u32);
}

fn cut(t: u32, i: i32) -> u32 {
    t.wrapping_add(0x24).wrapping_add((i as u32).wrapping_mul(0x1c))
}
fn cut_frame(m: &Mem, t: u32, i: i32) -> u32 {
    m.r32(cut(t, i) + 0x18)
}
fn write_cut(m: &mut Mem, at: u32, c: [u32; 5], preset: u32) {
    for (k, v) in c.iter().enumerate() {
        m.w32(at + 4 * k as u32, *v);
    }
    m.w32(at + 0x14, preset);
}
fn cuts_on(m: &Mem) -> bool {
    m.r32(CUTS_ON) != 0
}

/// `ReplayCamTrack_Init` 0x194cc0 (and its jump 0x194d10): no cuts, the default zeroed, none playing
pub fn cam_track_init(m: &mut Mem, t: u32) {
    for k in 0..0xe00 / 4 {
        m.w32(t + 0x24 + 4 * k, 0);
    }
    for k in 0..7 {
        m.w32(t + 8 + 4 * k, 0);
    }
    m.w32(t, 0);
    m.w32(t + 4, u32::MAX);
}

/// `ReplayCamTrack_Generate` 0x194d18(rider): set up afresh with the replay camera's view of the rider at frame 0
pub fn cam_track_generate(m: &mut Mem, env: &mut dyn CamEnv, t: u32, rider: u32) {
    cam_track_init(m, t);
    env.set_replay_mode(1, rider);
    cam_track_insert(m, env, t, 0);
    env.set_replay_mode(0, rider);
    cam_track_seek_frame(m, t, 0);
}

/// `ReplayCamTrack_Insert` 0x194d78(frame): the camera now as a cut at the frame (at frame 0 also the default); a
/// cut already at the frame is replaced (false), else one goes in after the last cut before the frame, or at the
/// end when none is before it; false when full
pub fn cam_track_insert(m: &mut Mem, env: &mut dyn CamEnv, t: u32, frame: u32) -> bool {
    let c = env.chase_params();
    if frame == 0 {
        let p = env.preset();
        write_cut(m, t + 8, c, p);
    }
    let n = m.r32(t) as i32;
    for i in 0..n.max(0) {
        if cut_frame(m, t, i) == frame {
            let p = env.preset();
            write_cut(m, cut(t, i), c, p);
            cam_track_seek_frame(m, t, frame);
            return false;
        }
    }
    if n >= MAX_CUTS {
        return false;
    }
    let mut i = n - 1;
    while i >= 0 {
        if cut_frame(m, t, i) < frame {
            let mut j = n;
            loop {
                j -= 1;
                if j <= i {
                    break;
                }
                for k in 0..7 {
                    m.w32(cut(t, j + 1) + 4 * k, m.r32(cut(t, j) + 4 * k));
                }
            }
            let p = env.preset();
            write_cut(m, cut(t, i + 1), c, p);
            m.w32(cut(t, i + 1) + 0x18, frame);
            m.w32(t, m.r32(t).wrapping_add(1));
            cam_track_seek_frame(m, t, frame);
            return true;
        }
        i -= 1;
    }
    let p = env.preset();
    let n = m.r32(t) as i32;
    write_cut(m, cut(t, n), c, p);
    m.w32(cut(t, n) + 0x18, frame);
    m.w32(t, (n + 1) as u32);
    cam_track_seek_frame(m, t, frame);
    true
}

/// `ReplayCamTrack_UpdateCurrent` 0x1950b8(frame): the camera now into the last cut at or before the frame (with the
/// cuts on), else into the default
pub fn cam_track_update_current(m: &mut Mem, env: &mut dyn CamEnv, t: u32, frame: u32) {
    let c = env.chase_params();
    if !cuts_on(m) {
        let p = env.preset();
        write_cut(m, t + 8, c, p);
        return;
    }
    let mut i = m.r32(t) as i32;
    loop {
        i -= 1;
        if i < 0 {
            return;
        }
        if frame >= cut_frame(m, t, i) {
            break;
        }
    }
    let p = env.preset();
    write_cut(m, cut(t, i), c, p);
}

/// `FUN_001951d0`: with the cuts on, whether another cut fits
pub fn cam_track_can_insert(m: &Mem, t: u32) -> bool {
    cuts_on(m) && (m.r32(t) as i32) < MAX_CUTS
}

/// `ReplayCamTrack_RemoveAt` 0x1951f8(frame): with the cuts on and more than one, the first cut at or past the frame
/// is removed (never the first cut)
pub fn cam_track_remove_at(m: &mut Mem, t: u32, frame: u32) -> bool {
    if !cuts_on(m) {
        return false;
    }
    let n = m.r32(t) as i32;
    if n <= 1 {
        return false;
    }
    let mut i = 1;
    while i < n {
        if frame <= cut_frame(m, t, i) {
            let mut n = m.r32(t) as i32;
            while i < n - 1 {
                for k in 0..7 {
                    m.w32(cut(t, i) + 4 * k, m.r32(cut(t, i + 1) + 4 * k));
                }
                i += 1;
                n = m.r32(t) as i32;
            }
            m.w32(t, (n - 1) as u32);
            return true;
        }
        i += 1;
    }
    false
}

/// `ReplayCamTrack_SeekFrame` 0x1952d8(frame): the last cut at or before the frame (the count less one when none;
/// 0 with the cuts off)
pub fn cam_track_seek_frame(m: &mut Mem, t: u32, frame: u32) {
    if !cuts_on(m) {
        m.w32(t + 4, 0);
        return;
    }
    let n = m.r32(t) as i32;
    let mut i = n;
    loop {
        i -= 1;
        if i < 0 {
            m.w32(t + 4, (n - 1) as u32);
            return;
        }
        if frame >= cut_frame(m, t, i) {
            m.w32(t + 4, i as u32);
            return;
        }
    }
}

/// `ReplayCamTrack_Advance` 0x195340(frame): on to the next cut when the frame has reached it
pub fn cam_track_advance(m: &mut Mem, t: u32, frame: u32) -> bool {
    if !cuts_on(m) {
        return false;
    }
    let n = m.r32(t) as i32;
    let next = (m.r32(t + 4) as i32).wrapping_add(1);
    if n < 1 || n <= next {
        return false;
    }
    if cut_frame(m, t, next) <= frame {
        m.w32(t + 4, next as u32);
        return true;
    }
    false
}

/// `FUN_001953a0`(i): a cut's frame (0 with the cuts off)
pub fn cam_track_cut_frame(m: &Mem, t: u32, i: u32) -> u32 {
    if cuts_on(m) {
        m.r32(i.wrapping_mul(0x1c).wrapping_add(t).wrapping_add(0x3c))
    } else {
        0
    }
}

/// `ReplayCamTrack_Apply` 0x1953c8: the cut playing (the default with the cuts off) to the camera
pub fn cam_track_apply(m: &Mem, env: &mut dyn CamEnv, t: u32) {
    let at = if cuts_on(m) { cut(t, m.r32(t + 4) as i32) } else { t + 8 };
    env.select_preset(m.r32(at + 0x14));
    let c = [m.r32(at), m.r32(at + 4), m.r32(at + 8), m.r32(at + 0xc), m.r32(at + 0x10)];
    env.set_chase_params(c, 1);
}

/// The rider's (+0x24) camera track
pub fn rider_cam_track(m: &Mem, mgr: u32) -> u32 {
    mgr + CAM_TRACKS + m.r32(mgr + RIDER).wrapping_mul(CAM_TRACK_SIZE)
}

/// `Replay_ApplyCameraCut` 0x190c28, `ReplayCam_InsertCut` 0x190c40(frame), `Replay_UpdateCamTrack` 0x190c70(frame),
/// `FUN_00190c88`: the same on the rider's track
pub fn apply_camera_cut(m: &Mem, env: &mut dyn CamEnv, mgr: u32) {
    cam_track_apply(m, env, rider_cam_track(m, mgr));
}
pub fn insert_cut(m: &mut Mem, env: &mut dyn CamEnv, mgr: u32, frame: u32) -> bool {
    let t = rider_cam_track(m, mgr);
    cam_track_insert(m, env, t, frame)
}
pub fn update_cam_track(m: &mut Mem, env: &mut dyn CamEnv, mgr: u32, frame: u32) {
    let t = rider_cam_track(m, mgr);
    cam_track_update_current(m, env, t, frame)
}
pub fn can_insert_cut(m: &Mem, mgr: u32) -> bool {
    cam_track_can_insert(m, rider_cam_track(m, mgr))
}

/// `ReplayCam_NextCut` 0x190cb8(frame): a cut removed at the frame, then on to the next and applied when reached;
/// false when none was removed
pub fn next_cut(m: &mut Mem, env: &mut dyn CamEnv, mgr: u32, frame: u32) -> bool {
    if !cam_track_remove_at(m, rider_cam_track(m, mgr), frame) {
        return false;
    }
    if cam_track_advance(m, rider_cam_track(m, mgr), frame) {
        apply_camera_cut(m, env, mgr);
    }
    true
}

/// `Replay_GenerateCameraTracks` 0x190d40: each of the four riders' tracks generated
pub fn generate_camera_tracks(m: &mut Mem, env: &mut dyn CamEnv, mgr: u32) {
    for r in 0..4 {
        cam_track_generate(m, env, mgr + CAM_TRACKS + r * CAM_TRACK_SIZE, r);
    }
}
