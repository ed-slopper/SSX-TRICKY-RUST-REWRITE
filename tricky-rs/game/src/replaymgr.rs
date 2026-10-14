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
    step_keyframes(m, mgr, true)
}
pub fn step_back(m: &mut Mem, mgr: u32) {
    step_keyframes(m, mgr, false)
}
fn step_keyframes(m: &mut Mem, mgr: u32, forward: bool) {
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

// Playback control and the manager's life (board row G12g). The manager also has: +0x28 the command (from the
// overlay), +0x2c the race frame recorded, +0x38 every how many frames a keyframe is kept for good, +0x3c a
// countdown, +0x40 recording held (the pad streams full), +0x44, +0x4c / +0x50 frozen, +0x54 the replay shown after
// the race (looping), +0x58 a stream overflowed. The game's state is reached through gApp (0x337c58) +0x730.

/// gApp
pub const GAPP: u32 = 0x337c58;

/// The world and camera calls playback makes (the camera's through [`CamEnv`])
pub trait ReplayEnv: CamEnv {
    /// `Game_IsMenuOrOverlayActive` 0x17f0f8
    fn menu_active(&mut self) -> bool;
    /// `Broadphase_SortAll` 0x14fe18 and `Contact_SortPairs` 0x150428 (on gApp+0x730 → +0x30)
    fn broadphase_sort(&mut self);
    fn contact_sort(&mut self);
    /// `Replay_SaveKeyframe` 0x18f448(keyframe, frame, flag): the world's snapshot into the keyframe's stream (sets
    /// +0x58 when it overflows)
    fn save_keyframe(&mut self, m: &mut Mem, mgr: u32, k: u32, frame: u32, flag: u32);
    /// `Replay_RestoreKeyframe` 0x18f5c8(keyframe): the world put back from it
    fn restore_keyframe(&mut self, m: &mut Mem, mgr: u32, k: u32);
    /// `HUD_HideItem` 0x1a7860(the HUD, item, -1)
    fn hud_hide(&mut self, item: u32);
    /// `Race_ResetRiderRenders` 0x1128b8, `Replay_RefreshRiders` 0x1147f8, `Camera_StartCutFade` 0x174c30,
    /// `CamScript_Stop` 0x175eb8(the camera, 1), `Audio_CancelRaceEvents` 0x210b50
    fn reset_rider_renders(&mut self);
    fn refresh_riders(&mut self);
    fn start_cut_fade(&mut self);
    fn cam_script_stop(&mut self);
    fn cancel_race_events(&mut self);
    /// `BxList_DeleteAll` 0x17da50(the node lists, list), `FUN_00115df0` and `World_ResetInstances` 0x115fe8 on the
    /// world
    fn delete_nodes(&mut self, list: u32);
    fn world_reset_a(&mut self);
    fn world_reset_instances(&mut self);
    /// `EAMem_Alloc` 0x2ccf70(size), `Mem_Free` 0x14a040, `Mem_Delete` 0x114f80
    fn alloc(&mut self, m: &mut Mem, size: u32) -> u32;
    fn free(&mut self, m: &mut Mem, p: u32);
    fn delete(&mut self, m: &mut Mem, p: u32);
}

fn game(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x730)
}

/// A rider's chase-camera target: its player's (race +0x8c) boarder's +0x460 (address 0x460 for no player)
fn rider_target(m: &Mem, rider: u32) -> u32 {
    let race = m.r32(game(m) + 0x1c);
    let player = m.r32(race.wrapping_add(rider.wrapping_mul(4)).wrapping_add(0x8c));
    let boarder = if player != 0 { m.r32(player) } else { 0 };
    m.r32(boarder.wrapping_add(0x460))
}

fn chase_to(m: &Mem, env: &mut dyn ReplayEnv, rider: u32) {
    let target = rider_target(m, rider);
    let c = env.chase_params();
    env.set_chase_params([c[0], c[1], c[2], c[3], target], 0);
}

fn sort(env: &mut dyn ReplayEnv) {
    env.broadphase_sort();
    env.contact_sort();
}

/// `EAMem_Set` on memory (as `replay::ea_mem_set`)
fn mem_set(m: &mut Mem, at: u32, value: u32, n: u32) {
    let mut b = vec![0u8; n as usize];
    crate::replay::ea_mem_set(&mut b, at, value);
    for (k, v) in b.iter().enumerate() {
        m.w8(at + k as u32, *v);
    }
}

/// `Replay_Step` 0x190878(jump): playing on, a frame forward: at the end of a playlist entry the next one (at the
/// last, the looping replay starts again on the focused rider; else mode 3); the camera cut advanced; every 60
/// frames the riders refreshed, every 10 the contacts sorted. With `jump` (or at the end of a looping replay) the
/// world is put back at the keyframe for the frame and the camera set for it.
pub fn step(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32, mut jump: bool) {
    loop {
        if jump {
            break;
        }
        let frame = m.r32(mgr + FRAME);
        if frame < m.r32(mgr + END) {
            let pl = m.r32(mgr + PLAYLIST);
            if pl != 0 && frame >= playlist_current_end(m, pl) {
                if !playlist_next(m, pl) {
                    if m.r32(mgr + 0x54) != 0 {
                        let r = m.r32(game(m) + 0x68) as i32;
                        m.w32(mgr + RIDER, r as u32);
                        if r < 0 {
                            m.w32(mgr + RIDER, 0);
                        }
                        m.w32(mgr + PLAYLIST, 0);
                        m.w32(mgr + FRAME, 0);
                        seek_start(m, mgr);
                        step(m, env, mgr, true);
                        chase_to(m, env, m.r32(mgr + RIDER));
                        return;
                    }
                    m.w32(mgr + MODE, 3);
                    return;
                }
                seek_start(m, mgr);
                step(m, env, mgr, true);
            }
            let t = rider_cam_track(m, mgr);
            if cam_track_advance(m, t, m.r32(mgr + FRAME)) {
                apply_camera_cut(m, env, mgr);
            }
            let f = m.r32(mgr + FRAME);
            if f % 60 == 0 {
                env.refresh_riders();
            } else if f % 10 == 0 {
                sort(env);
            }
            m.w32(mgr + FRAME, m.r32(mgr + FRAME).wrapping_add(1));
            return;
        }
        if m.r32(mgr + 0x54) == 0 {
            m.w32(mgr + FRAME, m.r32(mgr + END));
            m.w32(mgr + MODE, 3);
            return;
        }
        m.w32(mgr + PLAYLIST, 0);
        m.w32(mgr + FRAME, 0);
        seek_start(m, mgr);
        jump = true;
    }
    let pl = m.r32(mgr + PLAYLIST);
    let k = if pl == 0 { find_keyframe(m, mgr, m.r32(mgr + FRAME)) } else { playlist_current_keyframe(m, pl) };
    m.w32(mgr + PLAYING, k);
    m.w32(mgr + FRAME, m.r32(k + kf::FRAME));
    seek_inputs(m, mgr, m.r32(mgr + FRAME));
    env.restore_keyframe(m, mgr, m.r32(mgr + PLAYING));
    env.hud_hide(7);
    env.hud_hide(4);
    env.reset_rider_renders();
    if m.r32(mgr + 0x54) != 0 && m.r32(mgr + PLAYLIST) != 0 {
        let h = playlist_current(m, mgr + 0x38ec);
        chase_to(m, env, m.r32(h + hl::RIDER));
        env.select_preset(7);
    } else {
        let t = rider_cam_track(m, mgr);
        cam_track_seek_frame(m, t, m.r32(mgr + FRAME));
        apply_camera_cut(m, env, mgr);
    }
    if m.r32(mgr + PLAYLIST) != 0 {
        env.start_cut_fade();
    }
    env.refresh_riders();
}

fn jumped(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    let mode = if m.r32(mgr + FRAME) == 0 { 6 } else { 5 };
    m.w32(mgr + MODE, mode);
    env.start_cut_fade();
    m.w32(mgr + 0x28, 0);
}

/// `Replay_DoCommand` 0x191830: the overlay's command (+0x28): 1 to the start, 2 to the end, 3 a keyframe back, 4 a
/// keyframe on (each paused there, mode 5, or 6 at frame 0, with a cut fade), 5 one frame (mode 4), 6 pause /
/// play, 7 pause, 8 slow, 9 and 10 restart (mode 5); none plays a frame unless paused. The command is then cleared.
pub fn do_command(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    match m.r32(mgr + 0x28) {
        1 => {
            m.w32(mgr + PLAYLIST, 0);
            seek_start(m, mgr);
            step(m, env, mgr, true);
            jumped(m, env, mgr);
            return;
        }
        2 => {
            m.w32(mgr + PLAYLIST, 0);
            seek_end(m, mgr);
            step(m, env, mgr, true);
            jumped(m, env, mgr);
            return;
        }
        3 => {
            m.w32(mgr + PLAYLIST, 0);
            let frame = m.r32(mgr + FRAME);
            let k = find_keyframe(m, mgr, frame);
            m.w32(mgr + PLAYING, k);
            let next = m.r32(k + kf::NEXT);
            if frame == m.r32(k + kf::FRAME) || (next != 0 && m.r32(next + kf::FRAME) == frame) {
                step_back(m, mgr);
            }
            step(m, env, mgr, true);
            jumped(m, env, mgr);
            return;
        }
        4 => {
            m.w32(mgr + PLAYLIST, 0);
            if m.r32(m.r32(mgr + PLAYING) + kf::NEXT) == 0 {
                m.w32(mgr + 0x28, 0);
                return;
            }
            step_forward(m, mgr);
            step(m, env, mgr, true);
            m.w32(mgr + MODE, 5);
            env.start_cut_fade();
            m.w32(mgr + 0x28, 0);
            return;
        }
        5 => {
            m.w32(mgr + PLAYLIST, 0);
            step(m, env, mgr, false);
            m.w32(mgr + MODE, 4);
        }
        6 => {
            let mode = if m.r32(mgr + MODE) == 3 { 1 } else { 3 };
            m.w32(mgr + MODE, mode);
        }
        7 => m.w32(mgr + MODE, 3),
        8 => m.w32(mgr + MODE, 2),
        9 | 10 => {
            seek_start(m, mgr);
            step(m, env, mgr, true);
            m.w32(mgr + MODE, 5);
        }
        _ => {
            if m.r32(mgr + MODE) != 3 {
                m.w32(mgr + PLAYING_ON, 1);
                step(m, env, mgr, false);
            }
        }
    }
    m.w32(mgr + 0x28, 0);
}

/// `Replay_SetPlaylist` 0x191b18(playlist): set; a playlist starts from its first entry
pub fn set_playlist(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32, pl: u32) {
    m.w32(mgr + PLAYLIST, pl);
    if pl != 0 {
        seek_start(m, mgr);
        step(m, env, mgr, true);
    }
}

/// `Replay_RecordKeyframe` 0x18f328(frame, flag): a keyframe taken (evicting if need be) and linked after the last
/// one at or before the frame (the first keyframe is never compared), the world saved into it; freed again (0)
/// when its stream overflowed, else the newest when last
pub fn record_keyframe(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32, frame: u32, flag: u32) -> u32 {
    let k = alloc_keyframe(m, mgr);
    if m.r32(mgr + 0x10) == 0 {
        m.w32(mgr + FIRST, k);
    } else {
        let mut prev = m.r32(mgr + FIRST);
        let mut cur = m.r32(prev + kf::NEXT);
        let mut steps = 0u32;
        if cur == 0 {
            m.w32(k + kf::NEXT, 0);
        } else {
            let mut linked = false;
            while m.r32(cur + kf::FRAME) <= frame {
                guard(&mut steps);
                let n2 = m.r32(cur + kf::NEXT);
                prev = cur;
                if n2 == 0 {
                    m.w32(k + kf::NEXT, 0);
                    linked = true;
                    break;
                }
                cur = n2;
            }
            if !linked {
                m.w32(cur + kf::PREV, k);
                m.w32(k + kf::NEXT, m.r32(prev + kf::NEXT));
            }
        }
        m.w32(prev + kf::NEXT, k);
        m.w32(k + kf::PREV, prev);
    }
    env.save_keyframe(m, mgr, k, frame, flag);
    sort(env);
    if m.r32(mgr + 0x58) == 0 {
        if m.r32(k + kf::NEXT) == 0 {
            m.w32(mgr + 0x10, k);
        }
        k
    } else {
        free_keyframe(m, mgr, k);
        0
    }
}

/// `Replay_StartPlayback` 0x191b60(playlist): the pins released; the race's frame kept as a keyframe (once, unless
/// frozen); playing (mode 7) from the start
pub fn start_playback(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32, pl: u32) {
    release_all_pins(m, mgr);
    if m.r32(mgr + 0x4c) == 0 && m.r32(mgr + WRITING) == 0 {
        let k = record_keyframe(m, env, mgr, m.r32(mgr + 0x2c), 1);
        m.w32(mgr + WRITING, k);
    }
    m.w32(mgr + PLAYLIST, pl);
    m.w32(mgr + 0x28, 0);
    m.w32(mgr + 0x54, 0);
    m.w32(mgr + MODE, 7);
    seek_start(m, mgr);
    step(m, env, mgr, true);
}

/// `Replay_StopPlayback` 0x191be0: the looping replay's camera back to rider 0; the pins released; unless frozen
/// (+0x4c), the race put back from its kept keyframe, that keyframe freed (when not +0x50) and the riders
/// refreshed; mode 0 (13 with +0x50)
pub fn stop_playback(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    if m.r32(mgr + 0x54) != 0 {
        m.w32(mgr + RIDER, 0);
        env.set_replay_mode(0, 0);
    }
    m.w32(mgr + 0x54, 0);
    release_all_pins(m, mgr);
    if m.r32(mgr + 0x4c) != 0 {
        return;
    }
    m.w32(mgr + PLAYLIST, 0);
    if m.r32(mgr + WRITING) != 0 {
        env.restore_keyframe(m, mgr, m.r32(mgr + WRITING));
    }
    if m.r32(mgr + 0x50) == 0 {
        let k = m.r32(mgr + WRITING);
        if k != 0 {
            free_keyframe(m, mgr, k);
            m.w32(mgr + WRITING, 0);
        }
        env.refresh_riders();
        m.w32(mgr + MODE, 0);
    } else {
        env.refresh_riders();
        m.w32(mgr + MODE, 0xd);
    }
}

/// `Replay_Abort` 0x193c98: a looping replay stopped (and the race's sound events cancelled); mode 13
pub fn abort(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    if m.r32(mgr + 0x54) != 0 {
        stop_playback(m, env, mgr);
        env.cancel_race_events();
    }
    m.w32(mgr + MODE, 0xd);
}

/// `Replay_SaveCurrentKeyframe` 0x193d30: before saving to the card: the highlight ended and the race's frame kept as
/// a keyframe, once (unless frozen)
pub fn save_current_keyframe(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    m.w32(mgr + PLAYLIST, 0);
    if m.r32(mgr + WRITING) == 0 && m.r32(m.r32(game(m) + 0x40) + 0x4c) == 0 {
        end_highlight(m, mgr);
        let k = record_keyframe(m, env, mgr, m.r32(mgr + 0x2c), 1);
        m.w32(mgr + WRITING, k);
    }
}

/// `Replay_GetStatus` 0x191d88(out, 0x78 bytes): the keyframe count, the end frame, each keyframe's frame (unpinned or
/// the kept one; each keeps its place), the rider's highlights and its camera track
pub fn get_status(m: &mut Mem, mgr: u32, out: u32) {
    for k in 0..0x78 / 4 {
        m.w32(out + 4 * k, 0);
    }
    let n = keyframe_count(m, mgr);
    m.w32(out, n as u32);
    m.w32(out + 4, m.r32(mgr + END));
    let mut k = m.r32(mgr + FIRST);
    let mut i = 0u32;
    let mut steps = 0u32;
    while k != 0 {
        guard(&mut steps);
        if m.r32(k + kf::PINS) == 0 || k == m.r32(mgr + WRITING) {
            m.w32(out + 8 + 4 * i, m.r32(k + kf::FRAME));
        }
        k = m.r32(k + kf::NEXT);
        i += 1;
    }
    m.w32(out + 0x48, 0);
    let rider = m.r32(mgr + RIDER);
    let mut j = 0;
    while (j as i32) < m.r32(count_at(mgr, rider)) as i32 {
        let c = m.r32(out + 0x48);
        m.w32(out + 0x4c + 4 * c, m.r32(list_at(mgr, rider, j)));
        m.w32(out + 0x48, c + 1);
        j += 1;
    }
    m.w32(out + 0x74, mgr + CAM_TRACKS + rider.wrapping_mul(CAM_TRACK_SIZE));
}

/// `Replay_Update` 0x191298, each frame (not under a menu): recording (modes 0, 12) a keyframe every 60 race frames
/// (every +0x38-th kept for good), the contacts sorted every 10, the end frame and race frame counted; frozen, one
/// last keyframe kept and mode 13. Mode 2 begins a highlight every 10 frames; 3 ends it; 4..7 pause (3); 8 runs the
/// command then pauses; 9 builds the looping replay's playlist (the focused rider's pinned kind-1 highlights, then the
/// other's, then the kind-2 ones) and starts it; 11 resets the world for a replay (mode 12). The others run the
/// command.
pub fn update(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    if env.menu_active() {
        return;
    }
    let c = m.r32(mgr + 0x3c) as i32;
    if c > 0 {
        m.w32(mgr + 0x3c, (c - 1) as u32);
    }
    m.w32(mgr + PLAYING_ON, 0);
    if m.r32(mgr + 0x28) == 7 {
        m.w32(mgr + MODE, 3);
    }
    match m.r32(mgr + MODE) {
        0 | 12 => {
            m.w32(mgr + MODE, 0);
            m.w32(mgr + WRITING, 0);
            if m.r32(mgr + 0x50) != 0 {
                if m.r32(mgr + 0x4c) != 0 {
                    return;
                }
                prune_keyframes(m, mgr, true);
                let k = record_keyframe(m, env, mgr, m.r32(mgr + 0x2c), 0);
                if k != 0 {
                    m.w32(k + kf::USED, m.r32(k + kf::USED).wrapping_add(1));
                }
                m.w32(mgr + MODE, 0xd);
                return;
            }
            let held = m.r32(mgr + 0x40);
            let rf = m.r32(mgr + 0x2c);
            if held == 0 && rf % 60 == 0 {
                prune_keyframes(m, mgr, false);
                let k = record_keyframe(m, env, mgr, m.r32(mgr + 0x2c), 0);
                let every = m.r32(mgr + 0x38);
                assert!(every != 0, "Replay_Update: the game traps (a keyframe interval of 0)");
                if m.r32(mgr + 0x2c) % every == 0 && k != 0 {
                    m.w32(k + kf::USED, m.r32(k + kf::USED).wrapping_add(1));
                }
            } else if rf % 10 == 0 {
                sort(env);
            }
            if m.r32(mgr + 0x40) == 0 {
                m.w32(mgr + END, m.r32(mgr + END).wrapping_add(1));
            }
            m.w32(mgr + 0x2c, m.r32(mgr + 0x2c).wrapping_add(1));
        }
        2 => {
            if m.r32(mgr + FRAME) % 10 == 0 {
                let ok = begin_highlight(m, mgr, &mut |m, frame| record_keyframe(m, &mut *env, mgr, frame, 0));
                m.w32(mgr + MODE, if ok { 1 } else { 3 });
            }
            do_command(m, env, mgr);
        }
        3 => {
            end_highlight(m, mgr);
            do_command(m, env, mgr);
        }
        4..=7 => m.w32(mgr + MODE, 3),
        8 => {
            do_command(m, env, mgr);
            m.w32(mgr + MODE, 3);
        }
        9 => build_looping_replay(m, env, mgr),
        10 | 13 => {}
        11 => {
            env.delete_nodes(3);
            env.delete_nodes(0xd);
            env.delete_nodes(4);
            env.world_reset_a();
            env.world_reset_instances();
            generate_camera_tracks(m, env, mgr);
            m.w32(mgr + MODE, 0xc);
        }
        _ => do_command(m, env, mgr),
    }
}

fn build_looping_replay(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    let pl = mgr + 0x38ec;
    env.cam_script_stop();
    playlist_clear(m, pl);
    let r = m.r32(game(m) + 0x68) as i32;
    m.w32(mgr + RIDER, r as u32);
    if r < 0 {
        m.w32(mgr + RIDER, 0);
    }
    let rider = m.r32(mgr + RIDER);
    let own = mgr + PINS + rider * 4;
    let other = if rider != 0 { mgr + PINS } else { mgr + PINS + 4 };
    let mut added = 0;
    for (at, kind1) in [(own, true), (other, true), (own, false), (other, false)] {
        if m.r32(at) == 0 {
            continue;
        }
        for s in 0..4 {
            let h = m.r32(m.r32(at) + pin::SLOTS + 4 * s);
            if h != 0 && (m.r32(h + hl::KIND) == 1) == kind1 {
                added += 1;
                playlist_add(m, pl, h);
            }
        }
    }
    generate_camera_tracks(m, env, mgr);
    env.set_replay_mode(1, m.r32(mgr + RIDER));
    start_playback(m, env, mgr, if added < 1 { 0 } else { pl });
    m.w32(mgr + 0x54, 1);
    m.w32(mgr + MODE, 1);
}

/// `ReplayMgr_Init` 0x18e850(stream kilobytes, two players, seconds): the camera tracks and playlist cleared, the 16
/// keyframes' streams emptied; a stream of 0x8000 + 0xc00 per unit, as many keyframes as fit in 0xc9800 bytes (at
/// most 16), a keyframe kept for good every (seconds × 3600 / (keyframes − 10)) frames, rounded down to 10; the
/// pins' slots and quotas (two players 4 / 3 / 1, else 2 / 1 / 1); then `Replay_Init`
pub fn mgr_init(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32, units: i32, two: i64, secs: i32) {
    for t in 0..4 {
        cam_track_init(m, mgr + CAM_TRACKS + t * CAM_TRACK_SIZE);
    }
    playlist_clear(m, mgr + 0x38ec);
    for i in 0..16 {
        let k = mgr + 0x3e68 + 0x24 * i;
        for w in 0..4 {
            m.w32(k + 4 * w, 0);
        }
        m.w32(k + 0x18, 0);
        m.w32(k + 0x1c, 0);
    }
    let size = units.wrapping_mul(0xc00).wrapping_add(0x8000);
    m.w32(mgr + 0xc, size as u32);
    assert!(size != 0, "ReplayMgr_Init: the game traps (a stream of 0 bytes)");
    let mut n = 0xc9800i32.wrapping_div(size);
    m.w32(mgr + 8, n as u32);
    if n > 0x10 {
        n = 0x10;
        m.w32(mgr + 8, 0x10);
    }
    let d = n - 10;
    assert!(d != 0, "ReplayMgr_Init: the game traps (ten keyframes)");
    m.w32(mgr + 0x38, ((secs.wrapping_mul(0xe10).wrapping_div(d)) as u32 / 10) * 10);
    if two == 1 {
        m.w32(mgr + SLOT_COUNT, 4);
        m.w32(mgr + QUOTA1, 3);
        m.w32(mgr + QUOTA2, 1);
    } else {
        m.w32(mgr + QUOTA2, 1);
        m.w32(mgr + SLOT_COUNT, 2);
        m.w32(mgr + QUOTA1, 1);
    }
    init(m, env, mgr);
}

/// `Replay_Init` 0x18eac8: lists, pins, pads and highlights emptied, each keyframe's stream allocated
/// (`Replay_AllocStream`: filled with 0xdeadc0ed), then `Replay_Clear`
pub fn init(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    m.w32(mgr + RECORDING_FROM, u32::MAX);
    m.w32(mgr + FIRST, 0);
    m.w32(mgr + FREE, 0);
    m.w32(mgr + 0x10, 0);
    m.w32(mgr + RIDER, 0);
    m.w32(mgr + RECORDING_HL, 0);
    m.w32(mgr + HL_FREE, 0);
    for r in 0..4 {
        m.w32(mgr + INPUTS + 4 * r, 0);
        m.w32(mgr + PINS + 4 * r, 0);
        m.w32(mgr + HIGHLIGHT_COUNT + 4 * r, 0);
        for i in 0..10 {
            m.w32(list_at(mgr, r, i), 0);
        }
    }
    let n = m.r32(mgr + 8) as i32;
    for i in 0..n.max(0) as u32 {
        let k = mgr + 0x3e68 + 0x24 * i;
        let size = m.r32(mgr + 0xc);
        let p = env.alloc(m, size);
        if p != 0 {
            mem_set(m, p, 0xdead_c0ed, size);
        }
        m.w32(k + 8, p);
        m.w32(k, p);
        m.w32(k + 4, p);
        m.w32(k + 0xc, 0);
    }
    clear(m, mgr);
}

/// `Replay_Clear` 0x18ebd0: back to recording from nothing: mode 10, every count, frame and flag zeroed (the
/// save's words at +0x40a8.. too), the pads rewound, the pins cleared, the highlights emptied, the keyframes reset
/// and all free (the last first), the camera tracks cleared, and ten highlight records free (the 9th first)
pub fn clear(m: &mut Mem, mgr: u32) {
    m.w32(mgr + MODE, 10);
    m.w32(mgr + RECORDING_FROM, u32::MAX);
    for off in [
        0x28, 0x54, 0x4c, 0x40, 0x3c, 0x2c, 0x30, 0x34, 0x48, 0x44, 0x1c, 0x391c, 0x10, 0x14, 0x18, 0x395c, 0x50, 0x40a8,
        0x40ac, 0x40b0, 0x40b4, 0x40c0, 0x40c4, 0x40c8, 0x40cc, 0x40d0, 0x40dc, 0x40e0, 0x40e4, 0x40e8, 0x40f0, 0x40f8,
        0x40d4, 0x40d8,
    ] {
        m.w32(mgr + off, 0);
    }
    for r in 0..4 {
        let input = m.r32(mgr + INPUTS + 4 * r);
        if input != 0 {
            m.w32(input + 0x8008, 0);
            m.w32(input + 0x8000, 0);
            m.w32(input + 0x8004, 0);
        }
        let p = m.r32(mgr + PINS + 4 * r);
        if p != 0 {
            pin_clear(m, mgr, p);
        }
        let mut i = 0;
        while (i as i32) < m.r32(count_at(mgr, r)) as i32 {
            let h = m.r32(list_at(mgr, r, i));
            m.w32(h, 0);
            highlight_reset(m, h);
            m.w32(list_at(mgr, r, i), 0);
            i += 1;
        }
        m.w32(count_at(mgr, r), 0);
    }
    let mut k = m.r32(mgr + FIRST);
    let mut steps = 0u32;
    while k != 0 {
        guard(&mut steps);
        keyframe_reset(m, k);
        k = m.r32(k + kf::NEXT);
    }
    let n = m.r32(mgr + 8) as i32;
    m.w32(mgr + FIRST, 0);
    m.w32(mgr + FREE, 0);
    for i in 0..n.max(0) as u32 {
        let k = mgr + 0x3e68 + 0x24 * i;
        m.w32(k + kf::PREV, 0);
        m.w32(k + kf::NEXT, m.r32(mgr + FREE));
        m.w32(mgr + FREE, k);
    }
    for t in 0..4 {
        let t = mgr + CAM_TRACKS + t * CAM_TRACK_SIZE;
        cam_track_init(m, t);
        cam_track_seek_frame(m, t, 0);
    }
    let first = mgr + 0x3968;
    m.w32(mgr + HL_FREE, first);
    m.w32(first + hl::NEXT_FREE, 0);
    for i in 1..10 {
        let h = first + 0x1c * i;
        m.w32(h + hl::NEXT_FREE, m.r32(mgr + HL_FREE));
        m.w32(mgr + HL_FREE, h);
    }
}

/// `Replay_FreeAll` 0x18f208: the pins cleared, the highlights' keyframe words zeroed and the lists emptied, each
/// keyframe's stream freed (`Replay_FreeStream`), no first keyframe
pub fn free_all(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) {
    for r in 0..4 {
        let p = m.r32(mgr + PINS + 4 * r);
        if p != 0 {
            pin_clear(m, mgr, p);
        }
        let mut i = 0;
        while (i as i32) < m.r32(count_at(mgr, r)) as i32 {
            m.w32(m.r32(list_at(mgr, r, i)), 0);
            i += 1;
        }
        m.w32(count_at(mgr, r), 0);
    }
    let n = m.r32(mgr + 8) as i32;
    for i in 0..n.max(0) as u32 {
        let k = mgr + 0x3e68 + 0x24 * i;
        let p = m.r32(k);
        if p != 0 {
            env.free(m, p);
        }
        m.w32(k + 0xc, 0);
        m.w32(k, 0);
        m.w32(k + 4, 0);
        m.w32(k + 8, 0);
    }
    m.w32(mgr + FIRST, 0);
}

/// `Replay_AddInputStream` 0x191ca0: in the first empty pad place, a new pad stream (0x800c bytes filled with
/// 0xdeadc0de, then reset); 0 when all four are taken
pub fn add_input_stream(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32) -> u32 {
    for r in 0..4 {
        let at = mgr + INPUTS + 4 * r;
        if m.r32(at) == 0 {
            let p = env.alloc(m, 0x800c);
            if p != 0 {
                mem_set(m, p, 0xdead_c0de, 0x800c);
            }
            m.w32(p + 0x8008, 0);
            m.w32(p + 0x8000, 0);
            m.w32(p + 0x8004, 0);
            m.w32(at, p);
            return p;
        }
    }
    0
}

/// `Replay_RemoveInputStream` 0x191d30(stream): its place emptied and the stream deleted
pub fn remove_input_stream(m: &mut Mem, env: &mut dyn ReplayEnv, mgr: u32, s: u32) {
    for r in 0..4 {
        let at = mgr + INPUTS + 4 * r;
        let q = m.r32(at);
        if q == s {
            if q != 0 {
                env.delete(m, q);
            }
            m.w32(at, 0);
            return;
        }
    }
}

// The save to the memory card and the load (board row G12h). The writer's state is the manager's +0x40a8..: +0x40a8
// the 16 KB chunk going out, +0x40ac where it is written, +0x40b0 a 0xc800-byte scratch buffer, +0x40b4 / +0x40c0
// the piece being copied out and what is left of it, +0x40b8 / +0x40bc the bytes written and their sum (signed
// bytes; saved last as the checksum), +0x40c4 the stage, +0x40c8 the chunk's fill, +0x40cc / +0x40d0 an index and
// its end, +0x40d4 everything written, +0x40d8 the padding left, +0x40dc the next keyframe, +0x40e0 / +0x40e4 its
// stream to compress, +0x40e8.. / +0x40f0.. a pad stream's words and runs to compress, +0x40f8 its header.

/// The world calls the load makes, and `EAMem_Free`
pub trait SaveEnv: ReplayEnv {
    /// `Race_GetRider` 0x1814a8(i) then `BoarderRender_UpdateLighting` 0x135eb8 on its +0x5ae0
    fn update_lighting(&mut self, rider: u32);
    /// `EAMem_Free` 0x2ccfc0
    fn ea_free(&mut self, m: &mut Mem, p: u32);
}

fn region_rest(m: &Mem, at: u32) -> Vec<u8> {
    for (base, v) in &m.regions {
        if *base <= at && at < base + v.len() as u32 {
            return v[(at - base) as usize..].to_vec();
        }
    }
    panic!("replay: {at:#x} is outside the memory given");
}

fn copy_out(m: &mut Mem, to: u32, from: u32, n: u32) {
    for k in 0..n {
        let b = m.r8(from + k);
        m.w8(to + k, b);
    }
}

/// `Compress_Block`(src, n, dst, mode 0) on memory: the bytes written
fn compress_mem(m: &mut Mem, src: u32, n: u32, dst: u32) -> u32 {
    let data: Vec<u8> = (0..n).map(|k| m.r8(src + k)).collect();
    let mut out = tricky_data::huffenc::Out::default();
    let len = tricky_data::huffenc::compress_block(&data, src, &mut out, Some(0), vec![0; tricky_data::huffenc::at::LEN]);
    for k in 0..len {
        m.w8(dst + k, out.buf[k as usize]);
    }
    len
}

/// `Decompress_Block`(src, dst) on memory: the size the block says
fn decompress_mem(m: &mut Mem, src: u32, dst: u32) -> u32 {
    let rest = region_rest(m, src);
    let mut out = vec![];
    let size = tricky_data::compress::huff_decode_block(&rest, &mut out);
    for (k, b) in out.iter().enumerate() {
        m.w8(dst + k as u32, *b);
    }
    size
}

const SAVE: u32 = 0x40a8;

/// `ReplaySave_Init` 0x193d98: the chunk (0x4000, filled 0xdeadc0de) and the scratch (0xc800, 0xdeadc0ed) allocated,
/// the state zeroed and the sum started at 0xdeadbeef
pub fn save_init(m: &mut Mem, env: &mut dyn SaveEnv, mgr: u32) {
    let p = env.alloc(m, 0x4000);
    if p != 0 {
        mem_set(m, p, 0xdead_c0de, 0x4000);
    }
    m.w32(mgr + SAVE, p);
    let p = env.alloc(m, 0xc800);
    if p != 0 {
        mem_set(m, p, 0xdead_c0ed, 0xc800);
    }
    m.w32(mgr + 0x40b0, p);
    m.w32(mgr + 0x40c4, 0);
    m.w32(mgr + 0x40bc, 0xdead_beef);
    for off in [0x40b4, 0x40c0, 0x40c8, 0x40ac, 0x40cc, 0x40d0, 0x40dc, 0x40e0, 0x40e4, 0x40e8, 0x40f0, 0x40f8, 0x40d4, 0x40d8, 0x40b8] {
        m.w32(mgr + off, 0);
    }
}

/// `FUN_00194870` / `FUN_001948c0` (the same): the chunk and scratch freed
pub fn save_free(m: &mut Mem, env: &mut dyn SaveEnv, mgr: u32) {
    let p = m.r32(mgr + SAVE);
    if p != 0 {
        env.free(m, p);
    }
    let q = m.r32(mgr + 0x40b0);
    if q != 0 {
        env.free(m, q);
    }
    m.w32(mgr + 0x40b0, 0);
    m.w32(mgr + SAVE, 0);
}

/// `ReplaySave_FlushChunk` 0x193e78: as much of the piece as fits copied into the chunk
pub fn save_flush_chunk(m: &mut Mem, mgr: u32) {
    let src = m.r32(mgr + 0x40b4);
    let left = m.r32(mgr + 0x40c0) as i32;
    if src == 0 || left <= 0 {
        return;
    }
    let n = (0x4000 - m.r32(mgr + 0x40c8) as i32).min(left);
    if n > 0 {
        copy_out(m, m.r32(mgr + 0x40ac), src, n as u32);
        m.w32(mgr + 0x40ac, m.r32(mgr + 0x40ac).wrapping_add(n as u32));
        m.w32(mgr + 0x40b4, src.wrapping_add(n as u32));
        m.w32(mgr + 0x40c8, m.r32(mgr + 0x40c8).wrapping_add(n as u32));
        m.w32(mgr + 0x40c0, (left - n) as u32);
    }
}

/// `ReplaySave_SetChunk` 0x193f10(src, n): the next piece; its bytes counted and summed (signed)
pub fn save_set_chunk(m: &mut Mem, mgr: u32, src: u32, n: i32) {
    m.w32(mgr + 0x40b8, m.r32(mgr + 0x40b8).wrapping_add(n as u32));
    for k in 0..n.max(0) as u32 {
        let b = m.r8(src + k) as i8 as i32;
        m.w32(mgr + 0x40bc, m.r32(mgr + 0x40bc).wrapping_add(b as u32));
    }
    m.w32(mgr + 0x40b4, src);
    m.w32(mgr + 0x40d4, m.r32(mgr + 0x40d4).wrapping_add(n as u32));
    m.w32(mgr + 0x40c0, n as u32);
}

fn next_stage(m: &mut Mem, mgr: u32) {
    m.w32(mgr + 0x40c4, m.r32(mgr + 0x40c4).wrapping_add(1));
}
fn no_piece_next(m: &mut Mem, mgr: u32) {
    m.w32(mgr + 0x40b4, 0);
    m.w32(mgr + 0x40c0, 0);
    next_stage(m, mgr);
}

fn riders(m: &Mem) -> u32 {
    m.r32(m.r32(game(m) + 0x1c) + 0x7c)
}

/// `ReplaySave_Step` 0x193f70: the next 16 KB chunk (its address and length; `None` when done): the stages fill it
/// until full or done: 0 the header (0x12300003, the keyframes, +0x38, +0x40, +0x2c, +0x30, the riders, the highlight
/// counts, the rider; 0x44 bytes), 2..3 each rider's camera track, 4..5 the keyframes (a record, then the stream
/// compressed), 7 the highlights, 8..9 the pad streams (a header, the words and the runs compressed), 11 the pins,
/// 12..13 zeros up to 0x7f6d8 bytes, 15 the count and the sum
pub fn save_step(m: &mut Mem, env: &mut dyn SaveEnv, mgr: u32) -> Option<(u32, u32)> {
    if m.r32(mgr + 0x40c4) == 0x11 {
        return None;
    }
    m.w32(mgr + 0x40c8, 0);
    m.w32(mgr + 0x40ac, m.r32(mgr + SAVE));
    loop {
        save_flush_chunk(m, mgr);
        if m.r32(mgr + 0x40c8) as i32 > 0x3fff {
            break;
        }
        match m.r32(mgr + 0x40c4) {
            0 => save_write_header(m, mgr),
            1 => {
                save_set_chunk(m, mgr, m.r32(mgr + 0x40b0), 0x44);
                next_stage(m, mgr);
            }
            2 => {
                m.w32(mgr + 0x40cc, 0);
                m.w32(mgr + 0x40b4, 0);
                next_stage(m, mgr);
                m.w32(mgr + 0x40d0, riders(m));
                m.w32(mgr + 0x40c0, 0);
            }
            3 => {
                let i = m.r32(mgr + 0x40cc) as i32;
                if m.r32(mgr + 0x40d0) as i32 <= i {
                    no_piece_next(m, mgr);
                } else {
                    m.w32(mgr + 0x40cc, (i + 1) as u32);
                    save_set_chunk(m, mgr, (i as u32).wrapping_mul(CAM_TRACK_SIZE).wrapping_add(mgr + CAM_TRACKS), 0xe24);
                }
            }
            4 => {
                let first = m.r32(mgr + FIRST);
                m.w32(mgr + 0x40dc, first);
                if first != 0 && first == m.r32(mgr + WRITING) {
                    m.w32(mgr + 0x40dc, m.r32(first + kf::NEXT));
                }
                m.w32(mgr + 0x40e0, 0);
                m.w32(mgr + 0x40b4, 0);
                next_stage(m, mgr);
                m.w32(mgr + 0x40c0, 0);
            }
            5 => save_emit_keyframe(m, mgr),
            6 | 10 | 14 => no_piece_next(m, mgr),
            7 => {
                let scratch = m.r32(mgr + 0x40b0);
                let mut cur = scratch;
                for r in 0..4 {
                    let mut i = 0;
                    while (i as i32) < m.r32(count_at(mgr, r)) as i32 {
                        highlight_save(m, mgr, m.r32(list_at(mgr, r, i)), &mut cur);
                        i += 1;
                    }
                }
                save_set_chunk(m, mgr, scratch, cur.wrapping_sub(scratch) as i32);
                next_stage(m, mgr);
            }
            8 => {
                m.w32(mgr + 0x40d0, 4);
                m.w32(mgr + 0x40cc, 0);
                next_stage(m, mgr);
                for off in [0x40f8, 0x40f0, 0x40e8, 0x40ec, 0x40f4, 0x40b4, 0x40c0] {
                    m.w32(mgr + off, 0);
                }
            }
            9 => save_emit_input(m, mgr),
            0xb => {
                let scratch = m.r32(mgr + 0x40b0);
                let mut cur = scratch;
                for r in 0..4 {
                    let p = m.r32(mgr + PINS + 4 * r);
                    if p != 0 {
                        pin_save(m, mgr, p, &mut cur);
                    }
                }
                save_set_chunk(m, mgr, scratch, cur.wrapping_sub(scratch) as i32);
                next_stage(m, mgr);
            }
            0xc => {
                let scratch = m.r32(mgr + 0x40b0);
                mem_set(m, scratch, 0, 0xc800);
                next_stage(m, mgr);
                m.w32(mgr + 0x40d8, 0x7f6d8u32.wrapping_sub(m.r32(mgr + 0x40d4)));
            }
            0xd => {
                let left = m.r32(mgr + 0x40d8) as i32;
                if left < 1 {
                    next_stage(m, mgr);
                } else {
                    let n = left.min(0x4000);
                    save_set_chunk(m, mgr, m.r32(mgr + 0x40b0), n);
                    m.w32(mgr + 0x40d8, (left - n) as u32);
                }
            }
            0xf => {
                let scratch = m.r32(mgr + 0x40b0);
                copy_out(m, scratch, mgr + 0x40b8, 8);
                save_set_chunk(m, mgr, scratch, 8);
                next_stage(m, mgr);
            }
            0x10 => next_stage(m, mgr),
            _ => {}
        }
        if m.r32(mgr + 0x40c8) as i32 >= 0x4000 || m.r32(mgr + 0x40c4) == 0x11 {
            break;
        }
    }
    let _ = env;
    Some((m.r32(mgr + SAVE), m.r32(mgr + 0x40c8)))
}

fn save_write_header(m: &mut Mem, mgr: u32) {
    let h = m.r32(mgr + 0x40b0);
    let used = keyframe_count_used(m, mgr) as u32;
    m.w32(h + 4, used);
    m.w32(h + 0xc, m.r32(mgr + 0x40));
    m.w32(h + 8, m.r32(mgr + 0x38));
    m.w32(h + 0x10, m.r32(mgr + 0x2c));
    m.w32(h + 0x14, m.r32(mgr + END));
    m.w32(h, 0x1230_0003);
    m.w32(h + 0x18, riders(m));
    m.w32(h + 0x2c, m.r32(mgr + RIDER));
    for r in 0..4 {
        m.w32(h + 0x1c + 4 * r, m.r32(count_at(mgr, r)));
    }
    m.w32(mgr + 0x40b4, 0);
    m.w32(mgr + 0x40c0, 0);
    next_stage(m, mgr);
}

fn save_emit_keyframe(m: &mut Mem, mgr: u32) {
    let scratch = m.r32(mgr + 0x40b0);
    if m.r32(mgr + 0x40e0) != 0 {
        let n = compress_mem(m, m.r32(mgr + 0x40e0), m.r32(mgr + 0x40e4), scratch + 4);
        m.w32(scratch, n);
        save_set_chunk(m, mgr, scratch, (n + 4) as i32);
        m.w32(mgr + 0x40e0, 0);
        return;
    }
    let k = m.r32(mgr + 0x40dc);
    if k == 0 {
        no_piece_next(m, mgr);
        return;
    }
    m.w32(scratch, 0x1111_1113);
    m.w32(scratch + 8, m.r32(k + kf::FRAME));
    m.w32(scratch + 4, m.r32(k + 0xc));
    m.w32(scratch + 0xc, m.r32(k + kf::PINS));
    m.w32(scratch + 0x14, 0x1111_1111);
    m.w32(scratch + 0x10, m.r32(k + kf::USED));
    save_set_chunk(m, mgr, scratch, 0x18);
    let next = m.r32(k + kf::NEXT);
    m.w32(mgr + 0x40e0, m.r32(k));
    m.w32(mgr + 0x40e4, m.r32(k + 0xc));
    m.w32(mgr + 0x40dc, next);
    if next != 0 && next == m.r32(mgr + WRITING) {
        m.w32(mgr + 0x40dc, m.r32(next + kf::NEXT));
    }
}

fn save_emit_input(m: &mut Mem, mgr: u32) {
    let scratch = m.r32(mgr + 0x40b0);
    if m.r32(mgr + 0x40f8) != 0 {
        save_set_chunk(m, mgr, m.r32(mgr + 0x40f8), 8);
        m.w32(mgr + 0x40f8, 0);
        return;
    }
    if m.r32(mgr + 0x40e8) != 0 {
        let n = compress_mem(m, m.r32(mgr + 0x40e8), m.r32(mgr + 0x40ec), scratch + 4);
        m.w32(scratch, n);
        save_set_chunk(m, mgr, scratch, (n + 4) as i32);
        m.w32(mgr + 0x40e8, 0);
        return;
    }
    if m.r32(mgr + 0x40f0) != 0 {
        let n = compress_mem(m, m.r32(mgr + 0x40f0), m.r32(mgr + 0x40f4), scratch + 4);
        m.w32(scratch, n);
        save_set_chunk(m, mgr, scratch, (n + 4) as i32);
        m.w32(mgr + 0x40f0, 0);
        return;
    }
    let i = m.r32(mgr + 0x40cc) as i32;
    if i >= m.r32(mgr + 0x40d0) as i32 {
        no_piece_next(m, mgr);
        return;
    }
    let s = m.r32(mgr + INPUTS + 4 * i as u32);
    for off in [0x40e8, 0x40f0, 0x40f8, 0x40ec, 0x40f4] {
        m.w32(mgr + off, 0);
    }
    if s == 0 {
        m.w32(mgr + 0x40b4, 0);
        m.w32(mgr + 0x40cc, (i + 1) as u32);
        m.w32(mgr + 0x40c0, 0);
        return;
    }
    m.w32(mgr + 0x40ec, 0x6664);
    m.w32(mgr + 0x40f4, 0x1999);
    m.w32(scratch, 0x1111_1112);
    m.w32(mgr + 0x40e8, s);
    m.w32(mgr + 0x40f0, s + 0x6664);
    m.w32(scratch + 4, m.r32(s + 0x8000));
    m.w32(mgr + 0x40f8, scratch);
    m.w32(mgr + 0x40cc, m.r32(mgr + 0x40cc).wrapping_add(1));
}

/// `Highlight_Save` 0x193b00(highlight, cursor): 0x20 bytes: 0x11111115, the kind, its keyframe's frame, the length,
/// the two scores, the keyframe's place, the rider; for no keyframe, the kind set to 0 and -1 / 0 / 0 / 0 / -1 / 0.
/// (STANDIN: with no keyframe the game's second word is a stack word it never set; the port writes 0.)
pub fn highlight_save(m: &mut Mem, mgr: u32, h: u32, cur: &mut u32) {
    let at = *cur;
    *cur = at.wrapping_add(0x20);
    let k = m.r32(h);
    let rec: [u32; 8] = if k == 0 {
        m.w32(h + hl::KIND, 0);
        [0x1111_1115, 0, u32::MAX, 0, 0, 0, u32::MAX, 0]
    } else {
        [
            0x1111_1115,
            m.r32(h + hl::KIND),
            m.r32(k + kf::FRAME),
            m.r32(h + hl::LEN),
            m.r32(h + hl::SCORE1),
            m.r32(h + hl::SCORE2),
            keyframe_index(m, mgr, k) as u32,
            m.r32(h + hl::RIDER),
        ]
    };
    for (i, v) in rec.iter().enumerate() {
        m.w32(at + 4 * i as u32, *v);
    }
}

/// `Highlight_Load` 0x193bd0(highlight, cursor): back from its 0x20 bytes (the keyframe by its place)
pub fn highlight_load(m: &mut Mem, mgr: u32, h: u32, cur: &mut u32) {
    let at = *cur;
    *cur = at.wrapping_add(0x20);
    let rec: Vec<u32> = (0..8).map(|i| m.r32(at + 4 * i)).collect();
    let k = if rec[6] == u32::MAX { 0 } else { keyframe_at(m, mgr, rec[6]) };
    m.w32(h, k);
    m.w32(h + hl::KIND, rec[1]);
    m.w32(h + hl::RIDER, rec[7]);
    m.w32(h + hl::LEN, rec[3]);
    m.w32(h + hl::SCORE1, rec[4]);
    m.w32(h + hl::SCORE2, rec[5]);
}

/// `ReplayPin_Save` 0x1938c8(pin, cursor): 0x18 bytes (0x11111114, how many kept, their keyframes' frames), then each
/// kept highlight. (STANDIN: the game leaves the frames' unused words as its stack held them; the port writes 0.)
pub fn pin_save(m: &mut Mem, mgr: u32, p: u32, cur: &mut u32) {
    let at = *cur;
    *cur = at.wrapping_add(0x18);
    let mut rec = [0x1111_1114u32, 0, 0, 0, 0, 0];
    for s in 0..4 {
        let h = m.r32(p + pin::SLOTS + 4 * s);
        if h != 0 {
            let n = rec[1] as usize;
            rec[2 + n] = m.r32(m.r32(h) + kf::FRAME);
            rec[1] += 1;
        }
    }
    for (i, v) in rec.iter().enumerate() {
        m.w32(at + 4 * i as u32, *v);
    }
    for s in 0..4 {
        let h = m.r32(p + pin::SLOTS + 4 * s);
        if h != 0 {
            highlight_save(m, mgr, h, cur);
        }
    }
}

/// `ReplayPin_Load` 0x1939a0(pin, cursor): each kept highlight found by its frame among the rider's and loaded; none
/// being made
pub fn pin_load(m: &mut Mem, mgr: u32, p: u32, cur: &mut u32) -> bool {
    let at = *cur;
    *cur = at.wrapping_add(0x18);
    let n = m.r32(at + 4) as i32;
    for i in 0..n.max(0) as u32 {
        let frame = m.r32(at + 8 + 4 * i);
        let h = find_highlight(m, mgr, m.r32(p), frame);
        m.w32(p + pin::SLOTS + 4 * i, h);
        highlight_load(m, mgr, h, cur);
    }
    m.w32(p + pin::MAKING, 0);
    true
}

/// `Replay_LoadKeyframe` 0x193088(keyframe, cursor): its record (written count, frame, pins, use) and its stream
/// unpacked (`Replay_UnpackBlock`)
pub fn load_keyframe(m: &mut Mem, k: u32, cur: &mut u32) {
    let at = *cur;
    *cur = at.wrapping_add(0x18);
    m.w32(k + 0xc, m.r32(at + 4));
    m.w32(k + kf::FRAME, m.r32(at + 8));
    m.w32(k + kf::PINS, m.r32(at + 0xc));
    m.w32(k + kf::USED, m.r32(at + 0x10));
    let size = m.r32(*cur);
    *cur = cur.wrapping_add(4);
    let n = decompress_mem(m, *cur, m.r32(k));
    *cur = cur.wrapping_add(size);
    m.w32(k + 4, m.r32(k).wrapping_add(n));
    m.w32(k + 8, m.r32(k));
}

/// `ReplayInput_Load` 0x192a40 on memory (as `replay::Input::load`)
pub fn input_load(m: &mut Mem, s: u32, cur: &mut u32) {
    let at = *cur;
    m.w32(s + 0x8000, m.r32(at + 4));
    *cur = at.wrapping_add(8);
    let size = m.r32(*cur);
    *cur = cur.wrapping_add(4);
    decompress_mem(m, *cur, s);
    *cur = cur.wrapping_add(size);
    let size = m.r32(*cur);
    *cur = cur.wrapping_add(4);
    decompress_mem(m, *cur, s + 0x6664);
    *cur = cur.wrapping_add(size);
}

/// `Replay_LoadStream` 0x194910(cursor): a saved replay back (false when its rider count is not the race's): frozen,
/// the header's numbers, the camera tracks, the keyframes in order, the highlights, the pad streams and pins of the
/// riders, then the command 1 and mode 8, and each rider's lighting updated
pub fn load_stream(m: &mut Mem, env: &mut dyn SaveEnv, mgr: u32, cur: &mut u32) -> bool {
    let h = *cur;
    if m.r32(h + 0x18) != riders(m) {
        return false;
    }
    m.w32(mgr + 0x4c, 1);
    m.w32(mgr + 0x38, m.r32(h + 8));
    m.w32(mgr + 0x2c, m.r32(h + 0x10));
    m.w32(mgr + 0x40, (m.r32(h + 0xc) != 0) as u32);
    m.w32(mgr + END, m.r32(h + 0x14));
    for r in (0..4).rev() {
        m.w32(mgr + HIGHLIGHT_COUNT + 4 * r, 0);
    }
    m.w32(mgr + FRAME, 0);
    *cur = h.wrapping_add(0x44);
    m.w32(mgr + 0x10, 0);
    m.w32(mgr + RIDER, m.r32(h + 0x2c));
    m.w32(mgr + WRITING, 0);
    let mut i = 0;
    while (i as i32) < riders(m) as i32 {
        copy_out(m, mgr + CAM_TRACKS + i * CAM_TRACK_SIZE, *cur, 0xe24);
        *cur = cur.wrapping_add(0xe24);
        i += 1;
    }
    for _ in 0..(m.r32(h + 4) as i32).max(0) {
        let k = alloc_keyframe(m, mgr);
        let last = m.r32(mgr + 0x10);
        if last == 0 {
            m.w32(mgr + FIRST, k);
        } else {
            m.w32(last + kf::NEXT, k);
            m.w32(k + kf::PREV, last);
        }
        m.w32(mgr + 0x10, k);
        load_keyframe(m, k, cur);
    }
    for r in 0..4 {
        m.w32(count_at(mgr, r), 0);
        let want = m.r32(h + 0x1c + 4 * r) as i32;
        if want > 0 {
            loop {
                let c = m.r32(count_at(mgr, r));
                let rec = m.r32(mgr + HL_FREE);
                m.w32(list_at(mgr, r, c), rec);
                m.w32(mgr + HL_FREE, m.r32(rec + hl::NEXT_FREE));
                highlight_reset(m, m.r32(list_at(mgr, r, c)));
                let c = m.r32(count_at(mgr, r));
                highlight_load(m, mgr, m.r32(list_at(mgr, r, c)), cur);
                let c = m.r32(count_at(mgr, r));
                m.w32(count_at(mgr, r), c + 1);
                if c as i32 + 1 >= want {
                    break;
                }
            }
        }
    }
    let mut n = m.r32(h + 0x18) as i32;
    let mut i = 0;
    while i < n {
        let s = m.r32(mgr + INPUTS + 4 * i as u32);
        if s != 0 {
            input_load(m, s, cur);
            n = m.r32(h + 0x18) as i32;
        }
        i += 1;
    }
    let mut i = 0;
    while i < n {
        let p = m.r32(mgr + PINS + 4 * i as u32);
        if p != 0 {
            pin_load(m, mgr, p, cur);
            n = m.r32(h + 0x18) as i32;
        }
        i += 1;
    }
    m.w32(mgr + 0x28, 1);
    m.w32(mgr + MODE, 8);
    m.w32(mgr + PLAYLIST, 0);
    let race = m.r32(game(m) + 0x1c);
    let mut i = 0;
    while (i as i32) < m.r32(race + 0x88) as i32 {
        env.update_lighting(i);
        i += 1;
    }
    true
}

// The career's replay buffer (the career block's +0x36fc, its size +0x3700, +0x3710 a replay's race set up; a
// replay from the card starts with its race's set-up, 0x920 bytes for the career's +0x27dc)

/// `Replay_AllocBuffer` 0x166800: 0x80000 bytes
pub fn alloc_buffer(m: &mut Mem, env: &mut dyn SaveEnv, career: u32) {
    m.w32(career + 0x3700, 0x80000);
    let p = env.alloc(m, 0x80000);
    m.w32(career + 0x36fc, p);
}

/// `Replay_FreeBuffer` 0x166840
pub fn free_buffer(m: &mut Mem, env: &mut dyn SaveEnv, career: u32) {
    let p = m.r32(career + 0x36fc);
    env.ea_free(m, p);
    m.w32(career + 0x36fc, 0);
}

/// `Replay_LoadRiderSetup` 0x166870 (first named `Replay_SaveRiderSetup`): the race's set-up taken back from the
/// buffer's start (a replay from the card brings its race with it); +0x3710 set
pub fn load_rider_setup(m: &mut Mem, career: u32) {
    m.w32(career + 0x3710, 1);
    let from = m.r32(career + 0x36fc);
    copy_out(m, career + 0x27dc, from, 0x920);
}

/// `Replay_LoadFromBuffer` 0x1668a0: the replay after the set-up loaded into the game's manager, the buffer freed
pub fn load_from_buffer(m: &mut Mem, env: &mut dyn SaveEnv, career: u32) {
    let p = m.r32(career + 0x36fc);
    if p != 0 {
        let mut cur = p + 0x920;
        let mgr = m.r32(game(m) + 0x40);
        load_stream(m, env, mgr, &mut cur);
        free_buffer(m, env, career);
    }
}

/// `EAMem_Set` on memory, for the other modules
pub fn mem_set_pub(m: &mut Mem, at: u32, value: u32, n: u32) {
    mem_set(m, at, value, n)
}

/// `Replay_Write` 0x192c10 on a stream in memory (as `replay::Stream::write`; the control through gApp): nothing once
/// a stream overflowed; too much for the control's stream size marks the overflow
pub fn stream_write_mem(m: &mut Mem, s: u32, src: u32, n: u32) {
    let ctl = m.r32(game(m) + 0x40);
    if m.r32(ctl + 0x58) != 0 {
        return;
    }
    let used = m.r32(s + 4).wrapping_sub(m.r32(s));
    if (m.r32(ctl + 0xc).wrapping_sub(used) as i32) < n as i32 {
        m.w32(ctl + 0x58, 1);
        return;
    }
    copy_out(m, m.r32(s + 4), src, n);
    m.w32(s + 4, m.r32(s + 4).wrapping_add(n));
    m.w32(s + 0xc, m.r32(s + 0xc).wrapping_add(n));
}

/// `Replay_Read` 0x192ca0 on a stream in memory: no check against what was written
pub fn stream_read_mem(m: &mut Mem, s: u32, dst: u32, n: u32) {
    copy_out(m, dst, m.r32(s + 8), n);
    m.w32(s + 8, m.r32(s + 8).wrapping_add(n));
}

// The world snapshot's orchestration (board row G12i): which nodes go into a keyframe and how they come back. Each
// class's own snapshot (vf at +0x44), load (+0x4c) and object (+0x5c) and its load constructor are the env's
// (their own rows); this is the order and the bookkeeping around them.

/// The lists whose nodes save and load themselves (0x38aac0): the rider, camera, AI and effect lists
pub const SNAPSHOT_LISTS: [u32; 4] = [2, 5, 7, 8];
/// The game's random generator's state (24 bytes, `Rand_SaveGameSnapshot` 0x23d5b0)
pub const RAND_GAME: u32 = 0x345e70;
/// `MemPool_Alloc`'s pool for nodes
pub const NODE_POOL: u32 = 0x345e48;

/// The snapshot's calls beyond the node lists'
pub trait SnapshotEnv: crate::bxlist::NodeEnv {
    /// `MemPool_Alloc` 0x23d3b0(the node pool, size, 0x40000000, name, 0, 0)
    fn pool_alloc(&mut self, m: &mut Mem, size: u32, name: u32) -> u32;
    /// A constructor at `func` called with `args` (a node class's load constructor (node, list, stream);
    /// `cDeadNode::cDeadNode` 0x13af38 (node, instance, 0))
    fn construct(&mut self, m: &mut Mem, func: u32, args: &[u32]);
}

/// The node types a snapshot can hold: (type, size, its pool name, the load constructor)
pub const NODE_TYPES: [(i32, u32, u32, u32); 31] = [
    (0x0, 0x350, 0x38a138, 0x13dee8),   // cRollerNode
    (0x2, 0x30, 0x38a128, 0x13fc58),    // cDebounceNode
    (0x6, 0x3c, 0x38a170, 0x13b128),    // cCounterNode
    (0x7, 0x100, 0x38a188, 0x140590),   // cBoostNode
    (0x8, 0x3c, 0x38a198, 0x13fe50),    // cTimerNode
    (0x9, 0x40, 0x38a1a0, 0x1401a0),    // cRailNode
    (0xa, 0x6c, 0x38a1a8, 0x1420b8),    // cUVScrollNode
    (0xb, 0x2d4, 0x38a1b8, 0x143040),   // cTexFlipNode
    (0xc, 0x1b8, 0x38a1d0, 0x143870),   // cFenceNode
    (0xd, 0x19c, 0x38a1d8, 0x144810),   // cFlagNode
    (0xe, 0x2b8, 0x38a1e0, 0x145528),   // cCrackedNode
    (0xf, 0x100, 0x38a1e8, 0x140a98),   // cLapBoostNode
    (0x10, 400, 0x38a1f8, 0x1412e8),    // cRandomBoostNode
    (0x11, 0x104, 0x38a208, 0x145ba8),  // cCrowdBoxNode
    (0x12, 0x100, 0x38a218, 0x141be0),  // cZBoostNode
    (0x13, 0x330, 0x38a1c0, 0x146258),  // cUVScrollTexFlipNode
    (0x14, 0x6e0, 0x38a230, 0x146e10),  // cMeshAnimNode
    (0x15, 0xd8, 0x38a240, 0x1479d0),   // cTrickTriggerNode
    (0x16, 0x1a0, 0x38a250, 0x148028),  // cParticleNode
    (0x17, 0x30, 0x38a270, 0x148ad0),   // cMovieNode
    (0x18, 0x1c0, 0x38a278, 0x141800),  // cTubeEndBoostNode
    (0x100, 0x68, 0x38a140, 0x198b48),  // cAnimObjectNode
    (0x101, 0x6c, 0x38a150, 0x199d68),  // cAnimDeltaNode
    (0x102, 0x88, 0x38a160, 0x19a0a8),  // cAnimComboNode
    (0x103, 0x2f8, 0x38a220, 0x19a700), // cAnimTexFlipNode
    (0x3e9, 0xf0, 0x38a0a0, 0x13d2f8),  // cScriptNode
    (0x3ea, 0xa0, 0x38a178, 0x1fa480),  // cSplinePathNode
    (0x3eb, 400, 0x38a190, 0x147d78),   // cEmitterNode
    (0x3ec, 0x1e0, 0x38a260, 0x148680), // cCollideEmitterNode
    (0x3ed, 0x228, 0x38a288, 0x148e38), // cRailManNode
    (0x3ee, 0x2c, 0x38a290, 0x13b078),  // cRestoreNode
];

fn pool_new(m: &mut Mem, env: &mut dyn SnapshotEnv, size: u32, name: u32) -> u32 {
    let p = env.pool_alloc(m, size, name);
    if p != 0 {
        mem_set(m, p, 0xdead_c0de, size);
    }
    p
}

/// `Replay_SaveKeyframe` 0x18f448(keyframe, frame, flag): the stream emptied, the game's random state, the frame;
/// the nodes of lists 2, 5, 7, 8 save themselves (vf +0x44); with the flag lists 3 and 4 emptied first; lists 3 and
/// 4 saved with their counts (`Replay_SaveNodeList`), then the dead objects' references
pub fn save_keyframe(m: &mut Mem, env: &mut dyn SnapshotEnv, mgr: u32, k: u32, frame: u32, flag: u32) {
    use crate::bxlist as bx;
    m.w32(mgr + 0x58, 0);
    m.w32(k + 0xc, 0);
    m.w32(k + 4, m.r32(k));
    stream_write_mem(m, k, RAND_GAME, 24);
    m.w32(k + kf::FRAME, frame);
    for l in SNAPSHOT_LISTS {
        let mut n = bx::first(m, bx::WORLD, l);
        while n != 0 {
            bx::call_virtual(m, env, n, 0x44, &[k]);
            n = bx::next_or_null(m, bx::WORLD, n, l);
        }
    }
    if flag != 0 {
        bx::delete_all(m, env, bx::WORLD, 3);
        bx::delete_all(m, env, bx::WORLD, 4);
    }
    save_node_list(m, env, k, 3);
    save_node_list(m, env, k, 4);
    save_obj_refs(m, env, k);
}

/// `Replay_SaveNodeList` 0x18f5d0(keyframe, list): the count (and a zero word), then each node's snapshot
pub fn save_node_list(m: &mut Mem, env: &mut dyn SnapshotEnv, k: u32, l: u32) {
    use crate::bxlist as bx;
    let mut count = 0u32;
    let mut n = bx::first(m, bx::WORLD, l);
    while n != 0 {
        count += 1;
        n = bx::next_or_null(m, bx::WORLD, n, l);
    }
    write_words(m, k, &[count, 0]);
    if count as i32 > 0 {
        let mut n = bx::first(m, bx::WORLD, l);
        while n != 0 {
            bx::call_virtual(m, env, n, 0x44, &[k]);
            n = bx::next_or_null(m, bx::WORLD, n, l);
        }
    }
}

/// A few words written from a scratch place (the game's stack): through the low memory the port keeps for it
fn write_words(m: &mut Mem, k: u32, words: &[u32]) {
    let ctl = m.r32(game(m) + 0x40);
    if m.r32(ctl + 0x58) != 0 {
        return;
    }
    let n = 4 * words.len() as u32;
    let used = m.r32(k + 4).wrapping_sub(m.r32(k));
    if (m.r32(ctl + 0xc).wrapping_sub(used) as i32) < n as i32 {
        m.w32(ctl + 0x58, 1);
        return;
    }
    let at = m.r32(k + 4);
    for (i, w) in words.iter().enumerate() {
        m.w32(at + 4 * i as u32, *w);
    }
    m.w32(k + 4, at.wrapping_add(n));
    m.w32(k + 0xc, m.r32(k + 0xc).wrapping_add(n));
}

fn read_words<const N: usize>(m: &mut Mem, k: u32) -> [u32; N] {
    let at = m.r32(k + 8);
    let mut w = [0u32; N];
    for (i, v) in w.iter_mut().enumerate() {
        *v = m.r32(at + 4 * i as u32);
    }
    m.w32(k + 8, at.wrapping_add(4 * N as u32));
    w
}

/// `Replay_SaveObjRefs` 0x190660(keyframe): list 13's count (and a zero word), then each node's object (vf +0x5c) as
/// an instance number
pub fn save_obj_refs(m: &mut Mem, env: &mut dyn SnapshotEnv, k: u32) {
    use crate::bxlist as bx;
    let mut count = 0u32;
    let mut n = bx::first(m, bx::WORLD, 0xd);
    while n != 0 {
        count += 1;
        n = bx::next_or_null(m, bx::WORLD, n, 0xd);
    }
    write_words(m, k, &[count, 0]);
    if count as i32 > 0 {
        let mut n = bx::first(m, bx::WORLD, 0xd);
        while n != 0 {
            let obj = bx::call_virtual(m, env, n, 0x5c, &[]);
            let course = m.r32(0x34648c);
            let idx = (obj.wrapping_sub(m.r32(course + 0x48)) as i32) >> 8;
            write_words(m, k, &[idx as u32]);
            n = bx::next_or_null(m, bx::WORLD, n, 0xd);
        }
    }
}

/// `Replay_RestoreKeyframe` 0x18f5c8 (0x18f6c0)(keyframe): lists 3, 13 and 4 emptied, the stream rewound, the game's
/// random state back; the nodes of lists 2, 5, 7, 8 load themselves (vf +0x4c); lists 3 and 4 made again
/// (`Replay_RestoreNodes`), then the dead objects (`Replay_LoadDeadNodes`)
pub fn restore_keyframe(m: &mut Mem, env: &mut dyn SnapshotEnv, mgr: u32, k: u32) {
    use crate::bxlist as bx;
    bx::delete_all(m, env, bx::WORLD, 3);
    bx::delete_all(m, env, bx::WORLD, 0xd);
    bx::delete_all(m, env, bx::WORLD, 4);
    m.w32(k + 8, m.r32(k));
    stream_read_mem(m, k, RAND_GAME, 24);
    for l in SNAPSHOT_LISTS {
        let mut n = bx::first(m, bx::WORLD, l);
        while n != 0 {
            bx::call_virtual(m, env, n, 0x4c, &[k]);
            n = bx::next_or_null(m, bx::WORLD, n, l);
        }
    }
    restore_nodes(m, env, mgr, k, 3);
    restore_nodes(m, env, mgr, k, 4);
    load_dead_nodes(m, env, k);
}

/// `Replay_RestoreNodes` 0x18f800(keyframe, list): as many nodes as saved made again, then the list's pending ones
/// merged
pub fn restore_nodes(m: &mut Mem, env: &mut dyn SnapshotEnv, mgr: u32, k: u32, l: u32) {
    let [count, _] = read_words::<2>(m, k);
    for _ in 0..(count as i32).max(0) {
        restore_node(m, env, mgr, k, l);
    }
    crate::bxlist::merge_pending(m, env, crate::bxlist::WORLD, l);
}

/// `Replay_RestoreNode` 0x18f8a8(keyframe, list): the saved node's type (the header's third word, peeked) picks its
/// size, pool name and load constructor; an unknown type makes nothing (and reads nothing)
pub fn restore_node(m: &mut Mem, env: &mut dyn SnapshotEnv, _mgr: u32, k: u32, l: u32) {
    let at = m.r32(k + 8);
    let ty = m.r32(at + 8) as i32;
    if let Some(&(_, size, name, ctor)) = NODE_TYPES.iter().find(|t| t.0 == ty) {
        let p = pool_new(m, env, size, name);
        env.construct(m, ctor, &[p, l, k]);
    }
}

/// `Replay_LoadDeadNodes` 0x190780(keyframe): for each saved reference a dead node (0x2c bytes) on its instance
pub fn load_dead_nodes(m: &mut Mem, env: &mut dyn SnapshotEnv, k: u32) {
    let [count, _] = read_words::<2>(m, k);
    for _ in 0..(count as i32).max(0) {
        let [idx] = read_words::<1>(m, k);
        let p = pool_new(m, env, 0x2c, 0x38a298);
        let course = m.r32(0x346488 + 4);
        let inst = if (idx as i32) < m.r32(course + 0xc) as i32 { m.r32(course + 0x48).wrapping_add(idx << 8) } else { 0 };
        env.construct(m, 0x13af38, &[p, inst, 0]);
    }
}
