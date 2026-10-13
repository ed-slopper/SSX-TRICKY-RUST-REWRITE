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

// A camera track (the three at +0x5c, 0xe24 apart): +0 the count, +4 the cut playing, +8 the cuts (each a pointer
// to a word holding its keyframe)

/// `ReplayCamTrack_SeekForward` 0x1954b8(frame): the first cut whose keyframe is after the frame
pub fn cam_track_seek_forward(m: &mut Mem, t: u32, frame: u32) -> bool {
    let n = m.r32(t) as i32;
    for i in 0..n.max(0) as u32 {
        if frame < m.r32(m.r32(m.r32(t + 8 + 4 * i)) + kf::FRAME) {
            m.w32(t + 4, i);
            return true;
        }
    }
    false
}

/// `ReplayCamTrack_SeekBack` 0x195510(frame): from the last cut back, the one before a cut at the frame, or the
/// last cut before it; the first when the frame is past the first cut's
pub fn cam_track_seek_back(m: &mut Mem, t: u32, frame: u32) -> bool {
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
