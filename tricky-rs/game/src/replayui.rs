//! The instant replay's on-screen controls and the race menu's replay page (board row G12n), on the game's bytes
//! (`pf::Mem`), checked in the function runner (`tests/replayui.rs`). The drawing (`cReplayControlOverlay::Draw`,
//! `HUD_DrawReplayCamInfo`) and the constructors' fonts and strings are not here.
//!
//! `cReplayControlOverlay` (the controls shown while a replay plays, a `cBackdropOverlay`):
//! - +0x350 the panel's y (sliding to its mode's), +0x354 / +0x358 the y for modes 1 / 2 (480.0 for 0),
//!   +0x35c..+0x36c its box
//! - +0x378 the replay's status (`Replay_GetStatus`, 0x78 bytes: +0x37c the end frame, +0x3c0 the rider's
//!   highlight count, +0x3c4 the highlights)
//! - +0x430 its own playlist (12 words), +0x460 the end frame shown (1800 at most) and +0x468 its scale
//! - +0x46c the frame a highlight was begun at, +0x470 the blink counter (0..60), +0x474 the slide offset (back to
//!   0 by 5 a frame), +0x478 the message's alpha (1.0 down by 0.016 a frame after +0x47c frames), +0x480 its string
//! - +0x484 the rider shown, +0x488 the panel's mode (0..+0x48c), +0x490 / +0x494 the held button repeating and how
//!   long, +0x49c hidden, +0x4a0 highlighting, +0x4a4 the panel moved, +0x4a8 the full (editing) controls
//!
//! The replay manager's calls go through the env (`replaymgr` has them all); only the overlay's own playlist is
//! worked on here.

use crate::pf::Mem;
use crate::ps2f::F;
use crate::replaymgr::{self as rm, GAPP};
use crate::sndqueue::{queue_begin, Queue, QueueEnv};

/// The pad the replay listens to (`DAT_003381e0`: +0xc the buttons last frame, +0x10 held)
pub const PAD: u32 = 0x3381e0;
/// The global the full controls follow (`ReplayCam` cuts on: 1)
pub const CUTS_ON: u32 = rm::CUTS_ON;

pub mod at {
    pub const Y: u32 = 0x350;
    pub const Y1: u32 = 0x354;
    pub const Y2: u32 = 0x358;
    pub const STATUS: u32 = 0x378;
    pub const END_FRAME: u32 = 0x37c;
    pub const HL_COUNT: u32 = 0x3c0;
    pub const HL: u32 = 0x3c4;
    pub const PLAYLIST: u32 = 0x430;
    pub const SHOWN_END: u32 = 0x460;
    pub const SCALE: u32 = 0x468;
    pub const HL_FROM: u32 = 0x46c;
    pub const BLINK: u32 = 0x470;
    pub const SLIDE: u32 = 0x474;
    pub const MSG_ALPHA: u32 = 0x478;
    pub const MSG_HOLD: u32 = 0x47c;
    pub const MSG: u32 = 0x480;
    pub const RIDER: u32 = 0x484;
    pub const MODE: u32 = 0x488;
    pub const MAX_MODE: u32 = 0x48c;
    pub const REPEAT: u32 = 0x490;
    pub const REPEAT_MASK: u32 = 0x494;
    pub const HIDDEN: u32 = 0x49c;
    pub const HIGHLIGHTING: u32 = 0x4a0;
    pub const MOVED: u32 = 0x4a4;
    pub const FULL: u32 = 0x4a8;
}

/// What the controls call out to. `m` is given where the call reads or writes the game's memory.
pub trait OverlayEnv {
    /// `Overlay_PlayMoveSound` 0x1a8648 (the overlay)
    fn move_sound(&mut self, o: u32);
    /// `Audio_PlayReplayScrollSound` 0x214428 (the audio system): `replay_sound(.., 0x7d)`
    fn scroll_sound(&mut self);
    /// `Audio_PlayReplayCameraSound` 0x214650: `replay_sound(.., 0x7f)`
    fn camera_sound(&mut self);
    /// `SpeechSys_ClearEvent` 0x228d10 (the audio system, 0)
    fn speech_clear(&mut self);
    /// `Camera_GetPreset` 0x1786c0
    fn camera_preset(&mut self) -> u32;
    /// `Camera_NextPreset` 0x178680
    fn camera_next_preset(&mut self, n: u32);
    /// `Camera_AdjustChase` 0x178be8 (1.0, `which`)
    fn camera_adjust_chase(&mut self, which: u32);
    /// `WorldEmitter_UpdateAll` 0x22d370 (`DAT_0034370c`)
    fn emitters_update(&mut self);
    /// `Audio_UpdateCrowdBoxes` 0x215370 (the audio system, the race's +0x290)
    fn crowd_boxes(&mut self, v: u32);
    /// `Audio_ResumeAll` 0x2153f8
    fn resume_all(&mut self);
    /// `Audio_StopAll` 0x215488
    fn stop_all(&mut self);
    /// `Replay_SetPlaylist` 0x191b18 (`rm::set_playlist`)
    fn set_playlist(&mut self, m: &mut Mem, mgr: u32, pl: u32);
    /// `Replay_GetStatus` 0x191d88 (`rm::get_status`)
    fn get_status(&mut self, m: &mut Mem, mgr: u32, out: u32);
    /// `Replay_CanAddHighlight` 0x1921e0 (`rm::can_add_highlight`)
    fn can_add_highlight(&mut self, m: &mut Mem, mgr: u32) -> u32;
    /// `Replay_DropCurrentHighlight` 0x1925f8 (`rm::drop_current_highlight`)
    fn drop_current_highlight(&mut self, m: &mut Mem, mgr: u32) -> bool;
    /// `Replay_ApplyCameraCut` 0x190c28 (`rm::apply_camera_cut`)
    fn apply_camera_cut(&mut self, m: &mut Mem, mgr: u32);
    /// `Replay_UpdateCamTrack` 0x190c70 (`rm::update_cam_track`)
    fn update_cam_track(&mut self, m: &mut Mem, mgr: u32, frame: u32);
    /// `ReplayCam_CanInsertCut` 0x190c88 (`rm::can_insert_cut`)
    fn can_insert_cut(&mut self, m: &mut Mem, mgr: u32) -> bool;
    /// `ReplayCam_InsertCut` 0x190c40 (`rm::insert_cut`)
    fn insert_cut(&mut self, m: &mut Mem, mgr: u32, frame: u32) -> bool;
    /// `ReplayCam_NextCut` 0x190cb8 (`rm::next_cut`)
    fn next_cut(&mut self, m: &mut Mem, mgr: u32, frame: u32) -> bool;
}

fn fl(m: &Mem, a: u32) -> f32 {
    f32::from_bits(m.r32(a))
}
fn race(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP) + 0x730)
}
fn mgr(m: &Mem) -> u32 {
    m.r32(race(m) + 0x40)
}
fn mode(m: &Mem) -> u32 {
    m.r32(mgr(m) + rm::MODE)
}
fn frame(m: &Mem) -> u32 {
    m.r32(mgr(m) + rm::FRAME)
}

/// 0x1ec728: the panel slides 6 a frame towards its mode's y (+0x4a4 set while it moves)
pub fn slide(m: &mut Mem, o: u32) {
    let cur = fl(m, o + at::Y);
    m.w32(o + at::MOVED, 0);
    let target = match m.r32(o + at::MODE) {
        2 => fl(m, o + at::Y2),
        1 => fl(m, o + at::Y1),
        _ => 480.0,
    };
    if cur == target {
        return;
    }
    let (n, done) = if cur < target {
        let n = (F(cur) + F(6.0)).0;
        (n, target < n)
    } else {
        let n = (F(cur) - F(6.0)).0;
        (n, n < target)
    };
    m.w32(o + at::Y, n.to_bits());
    if done {
        m.w32(o + at::Y, target.to_bits());
    }
    m.w32(o + at::MOVED, 1);
}

/// 0x1eded0: a message shown (its string) for 60 frames, then fading
pub fn show_message(m: &mut Mem, o: u32, msg: u32) {
    m.w32(o + at::MSG, msg);
    m.w32(o + at::MSG_HOLD, 0x3c);
    m.w32(o + at::MSG_ALPHA, 1.0f32.to_bits());
}

/// 0x1ec6e8 (`Reset`'s tail): the panel put at its mode's y
pub fn place(m: &mut Mem, o: u32) {
    let y = match m.r32(o + at::MODE) {
        2 => m.r32(o + at::Y2),
        1 => m.r32(o + at::Y1),
        _ => 480.0f32.to_bits(),
    };
    m.w32(o + at::Y, y);
}

/// The rider whose pad is the replay's (the race's riders' +0x870 → pad), or 0
fn pad_rider(m: &Mem) -> Option<u32> {
    let pad = m.r32(PAD);
    let n = m.r32(m.r32(race(m) + 0x1c) + 0x7c) as i32;
    (0..n.max(0) as u32).find(|&i| {
        let w = m.r32(race(m) + 0x1c);
        m.r32(m.r32(m.r32(w + i * 4 + 0x8c) + 0x870)) == pad
    })
}

/// `cReplayControlOverlay::Reset` 0x1ec548: the full controls when cuts are on; the replay follows the pad's rider
/// unless it is a loaded one (+0x4c); its camera cut applied, the status read, the command 1 (play)
pub fn reset(m: &mut Mem, env: &mut dyn OverlayEnv, o: u32) {
    m.w32(o + at::RIDER, 0);
    m.w32(o + at::FULL, (m.r32(CUTS_ON) == 1) as u32);
    if let Some(i) = pad_rider(m) {
        m.w32(o + at::RIDER, i);
    }
    let g = mgr(m);
    if m.r32(g + 0x4c) == 0 {
        m.w32(g + rm::RIDER, m.r32(o + at::RIDER));
    }
    env.apply_camera_cut(m, g);
    let g = mgr(m);
    m.w32(o + at::RIDER, m.r32(g + rm::RIDER));
    env.get_status(m, g, o + at::STATUS);
    let full = m.r32(o + at::FULL);
    m.w32(mgr(m) + 0x28, 1);
    if full == 0 {
        m.w32(o + at::MAX_MODE, 1);
        m.w32(o + at::Y2, 360.0f32.to_bits());
        m.w32(o + at::Y1, 360.0f32.to_bits());
        m.w32(o + at::MODE, 0);
    } else {
        m.w32(o + at::Y1, 360.0f32.to_bits());
        m.w32(o + at::Y2, 105.0f32.to_bits());
        m.w32(o + at::MAX_MODE, 2);
        m.w32(o + at::MODE, 1);
    }
    m.w32(o + 0x35c, 0);
    m.w32(o + at::HIDDEN, 0);
    m.w32(o + 0x498, 0);
    m.w32(o + at::HL_FROM, 0);
    m.w32(o + at::SLIDE, 0);
    m.w32(o + 0x364, 640.0f32.to_bits());
    m.w32(o + 0x36c, 320.0f32.to_bits());
    m.w32(o + 0x360, 640.0f32.to_bits());
    place(m, o);
}

/// `cReplayControlOverlay` vf16 0x1ec7d8: the camera out of replay mode (`Camera_SetReplayMode` 0x1785c0 (0, 0))
/// unless the replay is a loaded one
pub fn leave(m: &Mem, set_replay_mode: &mut dyn FnMut(u32, u32)) {
    if m.r32(mgr(m) + 0x4c) == 0 {
        set_replay_mode(0, 0);
    }
}

/// The overlay's playlist filled with the rider's highlights (all, or those not ended before the frame): its count
fn fill_playlist(m: &mut Mem, o: u32, only_ahead: bool) -> i32 {
    let pl = o + at::PLAYLIST;
    rm::playlist_clear(m, pl);
    let mut i = 0u32;
    while (i as i32) < m.r32(o + at::HL_COUNT) as i32 {
        let h = m.r32(o + at::HL + i * 4);
        let end = m.r32(m.r32(h) + 0x20).wrapping_add(m.r32(h + 4)) as i32;
        if !only_ahead || frame(m) as i32 <= end {
            rm::playlist_add(m, pl, h);
        }
        i += 1;
    }
    m.r32(pl) as i32
}

/// `cReplayControlOverlay::Update` 0x1eb6d0, each frame while a replay plays: the panel slides, the message fades,
/// the slide offset returns; unless hidden, paused (7) or highlighting (2), the pad: up / down the panel's mode;
/// right-stick-like held bits (0x200000.. with 0x200) turn the chase camera when the preset is 8; L1/R1 (0x1000 /
/// 0x2000: with 0x200 to the start / end, else repeating steps back / forward); 0x400 fast forward (repeating);
/// and with the full controls 0x20 a cut removed, 0x40 a highlight begun, 0x10 dropped, 0x100000 / 0x80000 to the
/// highlight before / after, 0x20000 the highlights ahead played, 0x40000 back to the replay, 0x8000 a cut inserted;
/// 0x800 pause (6); 0x100 the next camera preset, 0xc the camera cut to the next / previous rider. The command
/// chosen goes to the manager (+0x28), the sound is stopped on pause and resumed otherwise, and the status read.
pub fn update(m: &mut Mem, env: &mut dyn OverlayEnv, o: u32) {
    let pad = m.r32(PAD);
    if pad == 0 {
        return;
    }
    slide(m, o);
    let a = fl(m, o + at::MSG_ALPHA);
    if 0.0 < a {
        let hold = m.r32(o + at::MSG_HOLD) as i32;
        if hold < 1 {
            let n = (F(a) - F(f32::from_bits(0x3c83126f))).0;
            m.w32(o + at::MSG_ALPHA, n.to_bits());
            if n < 0.0 {
                m.w32(o + at::MSG_ALPHA, 0);
            }
        } else {
            m.w32(o + at::MSG_HOLD, (hold - 1) as u32);
        }
    }
    let s = m.r32(o + at::SLIDE) as i32;
    if s != 0 {
        if s < 0 {
            let n = s.wrapping_add(5);
            m.w32(o + at::SLIDE, n as u32);
            if n > 0 {
                m.w32(o + at::SLIDE, 0);
            }
        } else {
            let n = s.wrapping_sub(5);
            m.w32(o + at::SLIDE, n as u32);
            if n < 0 {
                m.w32(o + at::SLIDE, 0);
            }
        }
    }
    if m.r32(o + at::HIDDEN) != 0 {
        return;
    }
    let b = (m.r32(o + at::BLINK) as i32).wrapping_add(1);
    m.w32(o + at::BLINK, b as u32);
    if b > 60 {
        m.w32(o + at::BLINK, 0);
    }
    let md = mode(m);
    if md == 7 {
        return;
    }
    if md == 3 {
        m.w32(o + at::HIGHLIGHTING, 0);
    }
    if mode(m) == 2 {
        return;
    }
    let mut cmd = 0u32;
    let mut rep = m.r32(o + at::REPEAT) as i32;
    let mut held = m.r32(pad + 0x10);
    let mut new = held & !m.r32(pad + 0xc);
    let mask = m.r32(o + at::REPEAT_MASK);
    m.w32(o + at::REPEAT, 0);
    if mask != 0 {
        held &= mask | 0x1e00200;
        new &= mask | 0x1e00200;
    }
    m.w32(o + at::REPEAT_MASK, 0);
    if held & mask == 0 {
        rep = 0;
    }
    if new & 1 != 0 {
        if (m.r32(o + at::MODE) as i32) < m.r32(o + at::MAX_MODE) as i32 {
            env.scroll_sound();
            m.w32(o + at::MOVED, 1);
            m.w32(o + at::MODE, m.r32(o + at::MODE).wrapping_add(1));
        }
    } else if new & 2 != 0 && 0 < m.r32(o + at::MODE) as i32 {
        env.scroll_sound();
        m.w32(o + at::MOVED, 1);
        m.w32(o + at::MODE, m.r32(o + at::MODE).wrapping_sub(1));
    }
    dispatch(m, env, o, held, new, rep, &mut cmd);

    if held & 0x1e00000 != 0 && m.r32(o + at::FULL) != 0 && env.camera_preset() == 8 {
        let g = mgr(m);
        env.update_cam_track(m, g, m.r32(g + rm::FRAME));
    }
    if cmd == 0 {
        if mode(m) == 3 {
            env.resume_all();
        }
    } else if cmd == 6 {
        env.stop_all();
    } else {
        env.resume_all();
    }
    if cmd != 0 {
        m.w32(mgr(m) + 0x28, cmd);
    }
    env.get_status(m, mgr(m), o + at::STATUS);
    let e = m.r32(o + at::END_FRAME);
    if e.wrapping_sub(1) < 0x707 {
        m.w32(o + at::SHOWN_END, e);
        m.w32(o + at::SCALE, 1);
    } else {
        m.w32(o + at::SHOWN_END, 0x708);
        m.w32(o + at::SCALE, 0xb4);
    }
}

/// `mode != 0 && mode < 10 && frame == end` (the replay at its end)
fn at_end(m: &Mem) -> bool {
    let g = mgr(m);
    let md = m.r32(g + rm::MODE) as i32;
    md != 0 && md < 10 && m.r32(g + rm::FRAME) == m.r32(g + rm::END)
}

/// The pad's buttons to a command (`Update`'s middle; returns to its tail)
fn dispatch(m: &mut Mem, env: &mut dyn OverlayEnv, o: u32, held: u32, new: u32, rep: i32, cmd: &mut u32) {
    // the chase camera turned while the preset is 8 (with 0x200 the other way)
    for (bit, with, without) in [(0x200000u32, 6u32, 2u32), (0x400000, 7, 3), (0x1000000, 8, 0), (0x800000, 9, 1)] {
        if held & bit != 0 {
            if env.camera_preset() != 8 {
                return;
            }
            env.camera_adjust_chase(if held & 0x200 != 0 { with } else { without });
            return;
        }
    }
    let n1000 = new & 0x1000;
    if held & 0x200 != 0 {
        if n1000 != 0 {
            env.speech_clear();
            env.set_playlist(m, mgr(m), 0);
            if mode(m) == 3 {
                if frame(m) != 0 {
                    *cmd = 1;
                    env.move_sound(o);
                }
                return;
            }
            return paused(env, o, cmd);
        }
        if new & 0x2000 != 0 {
            env.speech_clear();
            env.set_playlist(m, mgr(m), 0);
            if mode(m) != 3 {
                return paused(env, o, cmd);
            }
            if at_end(m) || m.r32(mgr(m) + 0x2c) == frame(m) {
                return;
            }
            *cmd = 2;
            env.move_sound(o);
            return;
        }
        if held & 0x2000 == 0 {
            return;
        }
    }
    // L1 / R1: steps back / forward, repeating while held (the first press and then every 15 frames)
    for (bit, c) in [(0x1000u32, 3u32), (0x2000, 4)] {
        if new & bit != 0 || held & bit != 0 {
            m.w32(o + at::REPEAT_MASK, bit);
            let r = if rep != 0 && rep < 15 {
                m.w32(o + at::REPEAT, (rep + 1) as u32);
                rep + 1
            } else {
                0
            };
            if r != 0 {
                return;
            }
            if mode(m) != 3 {
                env.move_sound(o);
                *cmd = 7;
                m.w32(o + at::REPEAT_MASK, 0);
                return;
            }
            let g = mgr(m);
            let blocked = if bit == 0x1000 { m.r32(g + rm::FRAME) == 0 } else { m.r32(g + 0x2c) == m.r32(g + rm::FRAME) };
            if blocked {
                return;
            }
            env.speech_clear();
            if new & bit != 0 {
                env.move_sound(o);
            }
            *cmd = c;
            m.w32(o + at::REPEAT, 1);
            return;
        }
    }
    // fast forward, repeating
    if new & 0x400 != 0 || held & 0x400 != 0 {
        if m.r32(o + at::FULL) == 0 || at_end(m) {
            return;
        }
        if rep == 0 || rep >= 15 {
            if new & 0x400 != 0 {
                env.move_sound(o);
            }
            *cmd = 5;
        }
        m.w32(o + at::REPEAT_MASK, 0x400);
        m.w32(o + at::REPEAT, (rep + 1) as u32);
        return;
    }
    let full = m.r32(o + at::FULL) != 0;
    if new & 0x20 != 0 {
        if !full {
            return;
        }
        env.set_playlist(m, mgr(m), 0);
        if mode(m) != 3 {
            return paused(env, o, cmd);
        }
        let g = mgr(m);
        let f = m.r32(g + rm::FRAME);
        if f == 0 {
            env.move_sound(o);
            show_message(m, o, 0x15b);
        } else if env.next_cut(m, g, f) {
            env.move_sound(o);
            show_message(m, o, 0x14f);
        }
        return;
    }
    if new & 0x40 != 0 {
        if !full {
            return;
        }
        env.set_playlist(m, mgr(m), 0);
        if mode(m) != 3 {
            return paused(env, o, cmd);
        }
        if at_end(m) {
            return;
        }
        let v = env.can_add_highlight(m, mgr(m));
        if v == 1 {
            env.move_sound(o);
            *cmd = 8;
            m.w32(o + at::HIGHLIGHTING, v);
            m.w32(o + at::HL_FROM, frame(m));
            return;
        }
        if env.can_add_highlight(m, mgr(m)) == 0 {
            env.move_sound(o);
            show_message(m, o, 0x16d);
        }
        return;
    }
    if new & 0x10 != 0 {
        if !full {
            return;
        }
        env.set_playlist(m, mgr(m), 0);
        if mode(m) != 3 {
            return paused(env, o, cmd);
        }
        if env.drop_current_highlight(m, mgr(m)) {
            env.move_sound(o);
            show_message(m, o, 0x16c);
        }
        return;
    }
    if new & 0x800 != 0 {
        if mode(m) != 3 {
            return pause_playlist(m, env, o, cmd, 7);
        }
        if at_end(m) {
            return;
        }
        return pause_playlist(m, env, o, cmd, 6);
    }
    if new & 0x80 != 0 {
        return;
    }
    if new & 0x100 != 0 {
        env.move_sound(o);
        env.camera_sound();
        env.camera_next_preset(1);
        if m.r32(o + at::FULL) == 0 && env.camera_preset() == 8 {
            env.camera_next_preset(1);
        }
        let g = mgr(m);
        env.update_cam_track(m, g, m.r32(g + rm::FRAME));
        return;
    }
    if new & 0xc != 0 {
        if (m.r32(m.r32(race(m) + 0x1c) + 0x88) as i32) < 2 {
            return;
        }
        env.move_sound(o);
        env.apply_camera_cut(m, mgr(m));
        env.camera_adjust_chase(if new & 4 != 0 { 4 } else { 5 });
        env.emitters_update();
        env.crowd_boxes(m.r32(race(m) + 0x290));
        let g = mgr(m);
        env.update_cam_track(m, g, m.r32(g + rm::FRAME));
        return;
    }
    let pl = o + at::PLAYLIST;
    if new & 0x100000 != 0 || new & 0x80000 != 0 {
        if !full {
            return;
        }
        if mode(m) != 3 {
            return pause_playlist(m, env, o, cmd, 7);
        }
        let back = new & 0x100000 != 0;
        if fill_playlist(m, o, false) < 1 {
            env.set_playlist(m, mgr(m), 0);
            return;
        }
        let f = frame(m);
        let found = if back { rm::playlist_seek_back(m, pl, f) } else { rm::playlist_seek_forward(m, pl, f) };
        if !found {
            return;
        }
        env.move_sound(o);
        if back {
            *cmd = 10;
            let f = frame(m);
            rm::playlist_seek_back(m, pl, f);
        } else {
            *cmd = 9;
        }
        env.set_playlist(m, mgr(m), pl);
        return;
    }
    if new & 0x20000 != 0 {
        if !full {
            return;
        }
        if mode(m) != 3 {
            return pause_playlist(m, env, o, cmd, 7);
        }
        if fill_playlist(m, o, true) < 1 {
            return;
        }
        *cmd = 6;
        env.move_sound(o);
        rm::playlist_set_current(m, pl, 0);
        env.set_playlist(m, mgr(m), pl);
        return;
    }
    if new & 0x40000 != 0 {
        if !full || (m.r32(pl) as i32) < 1 {
            return;
        }
        env.set_playlist(m, mgr(m), 0);
        if mode(m) == 3 {
            return;
        }
        return paused(env, o, cmd);
    }
    if new & 0x8000 != 0 && full {
        if mode(m) != 3 {
            *cmd = 7;
            return;
        }
        let g = mgr(m);
        if !env.can_insert_cut(m, g) {
            env.move_sound(o);
            show_message(m, o, 0x153);
        } else {
            let g = mgr(m);
            if env.insert_cut(m, g, m.r32(g + rm::FRAME)) {
                env.move_sound(o);
                show_message(m, o, 0x149);
            }
        }
    }
}

/// 0x1eba60: the command 7 (pause) with the move sound
fn paused(env: &mut dyn OverlayEnv, o: u32, cmd: &mut u32) {
    *cmd = 7;
    env.move_sound(o);
}

/// 0x1ec2fc / 0x1ec300: the command, the move sound, and the replay back to no playlist
fn pause_playlist(m: &mut Mem, env: &mut dyn OverlayEnv, o: u32, cmd: &mut u32, c: u32) {
    *cmd = c;
    env.move_sound(o);
    env.set_playlist(m, mgr(m), 0);
}

/// The replay sounds 0x214428 (scroll, 0x7d) and 0x214650 (camera, 0x7f): a queue entry with the sound manager's
/// slot, then the pending sounds played (what that returns the caller keeps in the audio system's +0x6e48)
pub fn replay_sound(q: &mut Queue, id: u32, env: &mut dyn QueueEnv) -> u64 {
    let i = queue_begin(q, 0, id, None);
    q.a40[i] = env.get_slot(4);
    env.play_pending()
}

// ---- the race menu's replay page (`cRaceMenuOverlay`) and its memory-card save (`cReplayOverlay`) ----

/// What the race menu's replay page calls out to
pub trait MenuEnv {
    /// `Replay_ApplyCameraCut` 0x190c28
    fn apply_camera_cut(&mut self, m: &mut Mem, mgr: u32);
    /// `Replay_SaveCurrentKeyframe` 0x193d30
    fn save_current_keyframe(&mut self, m: &mut Mem, mgr: u32);
    /// `MemCardOp_Create` 0x1cee68 (the overlay)
    fn memcard_create(&mut self, o: u32);
    /// `MemCardOp_Begin` 0x1ce8b0 (the overlay, the size)
    fn memcard_begin(&mut self, o: u32, size: u32);
    /// `MemCardOp_StartSave` 0x1ce778 (the overlay, 0)
    fn memcard_start_save(&mut self, o: u32, a: u32);
    /// `MemCardOp_Destroy` 0x1ceeb0 (the overlay)
    fn memcard_destroy(&mut self, o: u32);
    /// `Camera_GetFocusRider` 0x174008 (the race's +0xa0)
    fn focus_rider(&mut self, cam: u32) -> u32;
    /// `Camera_SetFocusRider` 0x173c50
    fn set_focus_rider(&mut self, cam: u32, rider: u32);
    /// `Game_RestoreViews` 0x181d50
    fn restore_views(&mut self);
    /// The page's buttons (its own virtual +0x1c: `cReplayOverlay` vf2 0x1f0498, `Overlay_GetNewButtons` of the
    /// backdrop at +0x42c with -1)
    fn new_buttons(&mut self, o: u32) -> u32;
    /// `Overlay_PlayMoveSound` 0x1a8648 / `_PlayBackSound` 0x1a8658 / `_PlaySelectSound` 0x1a8668 (the backdrop)
    fn move_sound(&mut self, b: u32);
    fn back_sound(&mut self, b: u32);
    fn select_sound(&mut self, b: u32);
    /// `RaceMenuOverlay_SetState` 0x1ef990
    fn set_state(&mut self, o: u32, s: u32);
}

/// `RaceMenuOverlay_BeginSaveReplay` 0x1ef7f0: the replay to the pad's rider (unless a loaded one), its cut, the
/// keyframe kept, the card's save begun (0x80000 bytes), the focused rider kept, the game's +0x6f0 2
pub fn begin_save(m: &mut Mem, env: &mut dyn MenuEnv, o: u32) {
    m.w32(o + 0x448, 0);
    if let Some(i) = pad_rider(m) {
        m.w32(o + 0x448, i);
    }
    let g = mgr(m);
    if m.r32(g + 0x4c) == 0 {
        m.w32(g + rm::RIDER, m.r32(o + 0x448));
    }
    env.apply_camera_cut(m, mgr(m));
    let g = mgr(m);
    m.w32(o + 0x448, m.r32(g + rm::RIDER));
    env.save_current_keyframe(m, g);
    env.memcard_create(o);
    m.w32(o + 0x440, u32::MAX);
    env.memcard_begin(o, 0x80000);
    env.memcard_start_save(o, 0);
    let r = env.focus_rider(m.r32(race(m) + 0xa0));
    m.w32(o + 0x444, r);
    m.w32(m.r32(m.r32(GAPP) + 0x724) + 0x6f0, 2);
}

/// `RaceMenuOverlay_EndSaveReplay` 0x1ef918: the card's work dropped, the views back (unless the replay plays,
/// +0x50), the focused rider back, the game's +0x6f0 1
pub fn end_save(m: &mut Mem, env: &mut dyn MenuEnv, o: u32) {
    env.memcard_destroy(o);
    if m.r32(mgr(m) + 0x50) == 0 {
        env.restore_views();
    }
    env.set_focus_rider(m.r32(race(m) + 0xa0), m.r32(o + 0x444));
    m.w32(m.r32(m.r32(GAPP) + 0x724) + 0x6f0, 1);
}

/// `RaceMenuOverlay_UpdateReplayMenu` 0x1efef0: the page's three items (+0x43c): 0 watch (state 0, the race's
/// +0x38 → +0x24 → +0x498 set), 1 save (state 4 for a loaded replay, else 5), 2 back (3); triangle / circle back (3)
pub fn update_menu(m: &mut Mem, env: &mut dyn MenuEnv, o: u32) {
    let b = env.new_buttons(o);
    let bd = m.r32(o + 0x42c);
    if b & 0x80 != 0 || b & 0x20 != 0 {
        env.back_sound(bd);
        env.set_state(o, 3);
        return;
    }
    let sel = m.r32(o + 0x43c);
    if b & 0x800 != 0 {
        match sel {
            0 => {
                env.select_sound(bd);
                env.set_state(o, 0);
                let x = m.r32(m.r32(race(m) + 0x38) + 0x24);
                m.w32(x + 0x498, 1);
            }
            1 => {
                env.select_sound(bd);
                let s = if m.r32(mgr(m) + 0x4c) != 0 { 4 } else { 5 };
                env.set_state(o, s);
            }
            _ => {
                env.back_sound(bd);
                env.set_state(o, 3);
            }
        }
        return;
    }
    let to = if b & 0x20001 != 0 {
        match sel {
            2 => 1,
            1 => 0,
            _ => return,
        }
    } else if b & 0x40002 != 0 {
        match sel {
            0 => 1,
            1 => 2,
            _ => return,
        }
    } else {
        return;
    };
    env.move_sound(bd);
    m.w32(o + 0x43c, to);
}

/// What the card save's page calls out to
pub trait SaveOverlayEnv {
    /// `ReplaySave_Init` 0x193d98
    fn save_init(&mut self, m: &mut Mem, mgr: u32);
    /// `Crc_Compute16` 0x2ce04c (the data, its length)
    fn crc16(&mut self, m: &mut Mem, at: u32, n: u32) -> u32;
    /// `Save_GetGameCode` 0x1d2f60 (16 bytes)
    fn game_code(&mut self) -> String;
    /// `MemCard_WriteTagREP` 0x1d2fa0 / `MemCard_WriteTagSET` 0x1d2fc8 (8 bytes)
    fn tag(&mut self, set: bool) -> String;
    /// `MemCardOp_StartWrite` 0x1d1d80 (the overlay, the name, the data, its length)
    fn start_write(&mut self, o: u32, name: &str, at: u32, n: u32);
    /// `ReplaySave_Step` 0x193f70: the next chunk (its address and length)
    fn save_step(&mut self, m: &mut Mem, mgr: u32, n: u32) -> Option<(u32, u32)>;
    /// 0x1d1f18 (the overlay, the chunk, its length)
    fn write_chunk(&mut self, o: u32, at: u32, n: u32);
    /// 0x194870 / 0x1948c0 (the manager)
    fn after_save(&mut self, m: &mut Mem, mgr: u32, which: u32);
}

/// The settings block the card save writes with the replay (0x32db6c, 0x920 bytes; its CRC at 0x32dea0 inside it)
pub const SAVE_DATA: u32 = 0x32db6c;
pub const SAVE_LEN: u32 = 0x920;

/// `cReplayOverlay` vf3 0x1f0308: the save begun: the settings' CRC (taken with its own word 0), the file named
/// "<game code>-<tag><n>" (`%s-%s%04d`: REP and the slot +0x38 plus one, or SET and 1 when +0 is set) and written
pub fn save_begin(m: &mut Mem, env: &mut dyn SaveOverlayEnv, o: u32) {
    env.save_init(m, mgr(m));
    let crc_at = SAVE_DATA + 0x334;
    m.w32(crc_at, 0);
    let c = env.crc16(m, SAVE_DATA, SAVE_LEN);
    m.w32(crc_at, c);
    let code = env.game_code();
    let set = m.r32(o) != 0;
    let tag = env.tag(set);
    let n = if set { 1 } else { m.r32(o + 0x38).wrapping_add(1) as i32 };
    let name = format!("{code}-{tag}{n:04}");
    env.start_write(o, &name, SAVE_DATA, SAVE_LEN);
}

/// `cReplayOverlay` vf4 0x1f0400: the next chunk written; false when the save is done
pub fn save_next(m: &mut Mem, env: &mut dyn SaveOverlayEnv, o: u32, n: u32) -> bool {
    match env.save_step(m, mgr(m), n) {
        Some((at, len)) => {
            env.write_chunk(o, at, len);
            true
        }
        None => false,
    }
}

/// `cReplayOverlay` vf6 0x1f0468 / vf5 0x1f0480: `FUN_00194870` / `FUN_001948c0` on the manager (the save's end
/// and its abort)
pub fn save_finish(m: &mut Mem, env: &mut dyn SaveOverlayEnv, abort: bool) {
    let g = mgr(m);
    env.after_save(m, g, if abort { 0x1948c0 } else { 0x194870 });
}
