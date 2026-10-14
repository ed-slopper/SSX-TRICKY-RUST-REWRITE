//! The instant replay's on-screen controls and the race menu's replay page (board rows G12n, G12o), on the game's
//! bytes (`pf::Mem`), checked in the function runner (`tests/replayui.rs`). The drawing is the draw calls the game
//! makes (`DrawEnv`, `PanelEnv`). The constructors' fonts and strings and the renderer's screen-space switch
//! (0x1ee660 / 0x1ee6e8) are not here yet.
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

// ---- the drawing (G12o) ----

/// The font colour the HUD's text and sprites take (`Font_SetColorV` 0x14eee8 sets it from four floats)
pub const FONT_COLOR: u32 = 0x337fe8;

/// What the drawing calls out to. Floats are passed as the game computes them (PS2 rounding).
pub trait DrawEnv {
    /// The renderer's virtual at `slot` (+0x1c4 / +0x1d4 / +0x1e4 / +0x1fc of the object at the game's +0x724,
    /// through its vtable at +0x10)
    fn renderer(&mut self, slot: u32, a1: u32, a2: u32);
    /// `SpriteSet_GetRect` 0x1f9e50 (the sprite set, the sprite): the rectangle (+4 width, +8 height)
    fn sprite_rect(&mut self, set: u32, id: u32) -> u32;
    /// `HUD_DrawSprite` 0x1c0068 (x, y, w, h, the overlay, the sprite or -5 for a box, the corner)
    #[allow(clippy::too_many_arguments)]
    fn sprite(&mut self, x: f32, y: f32, w: f32, h: f32, o: u32, id: i32, corner: u32);
    /// `Font_SetColorV` 0x14eee8 (0x337fe8, the four floats)
    fn set_color(&mut self, c: [f32; 4]);
    /// `Loc_GetString` 0x2c5160 (the game's string table, the id)
    fn loc(&mut self, id: u32) -> u32;
    /// `Font_MeasureWide` 0x19dc78 (scale, the font, the string, 0): the width
    fn measure_wide(&mut self, scale: f32, font: u32, s: u32) -> i32;
    /// `Font_MeasureAscii` 0x19da30 (scale, the font, the string, 0)
    fn measure_ascii(&mut self, scale: f32, font: u32, s: u32) -> i32;
    /// `HUD_DrawLabelBox` 0x1a9928 (y, the overlay, the string, the style)
    fn label_box(&mut self, y: f32, o: u32, s: u32, style: u32);
    /// The text printers (scale, x, y, the font, the string): `Font_PrintfShadowed` 0x19d3f0,
    /// `Font_PrintfShadowedA` 0x19d568, `Font_PrintfShadowedCentered` 0x19d6e0, `Font_PrintShadowedRight` 0x19d868
    fn print(&mut self, func: u32, scale: f32, x: f32, y: f32, font: u32, s: u32);
    /// `Camera_GetChaseParams` 0x178968: its fifth output (the chase camera's index)
    fn chase_index(&mut self) -> i32;
    /// `Camera_GetPreset` 0x1786c0
    fn camera_preset(&mut self) -> u32;
    /// `Backdrop_DrawFrame` 0x1a9bb8 (y, the backdrop, 15)
    fn backdrop_frame(&mut self, y: f32, b: u32, a1: u32);
    /// `Overlay_DrawMenuList` 0x1aa630 (x, y, scale, the backdrop, 12, 0, the items' strings, 3, the selection,
    /// 116, 1, then on the stack 1 and the items' enabled flags)
    #[allow(clippy::too_many_arguments)]
    fn menu_list(&mut self, x: f32, y: f32, scale: f32, b: u32, items: [u32; 3], sel: u32, enabled: [u32; 3]);
}

fn fb(bits: u32) -> F {
    F(f32::from_bits(bits))
}

const P4: u32 = 0x3ecccccd;
const TENTH: u32 = 0x3dcccccd;

/// `cReplayControlOverlay::Draw` 0x1ec808: nothing during a cut-scene (the race's +0x34) or while the overlay is
/// off (+0x498). The panel, when it moves or is up: a dark box over the screen below it (all of the screen, darker,
/// when hidden), and its frame from sprite 0x37 scaled to 340 wide in two halves. The up / down arrows (sprites 0x61,
/// then 0x65 at the top mode or 0x64) left of "more" / "less". Then the camera's line and the other parts.
pub fn draw(m: &mut Mem, env: &mut dyn PanelEnv, o: u32) {
    if m.r32(race(m) + 0x34) != 0 || m.r32(o + 0x498) != 0 {
        return;
    }
    env.renderer(0x1e4, 3, 0);
    env.renderer(0x1fc, 1, 0);
    env.renderer(0x1c4, 0, 0);
    let one = 1.0f32;
    let p4 = f32::from_bits(P4);
    let hidden = m.r32(o + at::HIDDEN) != 0;
    let (c0, c1) = if hidden {
        ([0.25, p4, p4, p4], [0.5, p4, p4, p4])
    } else {
        ([f32::from_bits(0x3edc28f6), one, one, one], [one; 4])
    };
    if m.r32(o + at::MOVED) != 0 || m.r32(o + at::MODE) != 0 {
        let w = fl(m, env.sprite_rect(m.r32(o), 0x37) + 4);
        let h = fl(m, env.sprite_rect(m.r32(o), 0x37) + 8);
        let scale = F(340.0) / F(w);
        let y = fl(m, o + at::Y);
        let top = (F(y) - F(26.0)).0;
        let h21 = F(h) * fb(0x3e570a3d);
        let full = scale * F(w);
        let h26 = F(h) * fb(0x3e851eb8);
        let left = full * fb(0x3ee8f5c3);
        env.renderer(0x1d4, 10, 0);
        if hidden {
            m.w32(FONT_COLOR, 0x3f4ccccd);
            for k in [0xc, 4, 8] {
                m.w32(FONT_COLOR + k, TENTH);
            }
            env.sprite(0.0, 0.0, 640.0, 480.0, o, -5, 0);
            for k in [0xc, 0, 4, 8] {
                m.w32(FONT_COLOR + k, 0x3f000000);
            }
        } else {
            let tenth = f32::from_bits(TENTH);
            env.set_color([f32::from_bits(0x3f6147ae), tenth, tenth, tenth]);
            env.sprite(fl(m, o + 0x35c), y, fl(m, o + 0x364), (F(480.0) - F(y)).0, o, -5, 0);
            let rest = full - left;
            env.sprite((left + F(-20.0)).0, (F(top) + h26).0, (rest + rest).0, ((F(h) - h21) - h26).0, o, -5, 0);
            env.set_color([one; 4]);
        }
        env.renderer(0x1d4, 12, 0);
        env.sprite(-20.0, top, scale.0, 1.0, o, 0x37, 2);
        env.sprite(320.0, top, scale.0, 1.0, o, 0x37, 3);
    }
    env.renderer(0x1d4, 13, 0);
    if m.r32(o + at::MOVED) == 0 {
        let at_max = m.r32(o + at::MODE) == m.r32(o + at::MAX_MODE);
        let s = env.loc(if at_max { 0x16a } else { 0x16b });
        let w = env.measure_wide(1.0, m.r32(o + 0x348), s) as f32;
        env.renderer(0x1d4, 14, 0);
        env.set_color(c0);
        let x = F(fl(m, o + 0x360)) - F(w);
        let y = fl(m, o + at::Y);
        env.sprite((x - F(42.0)).0, (F(y) - F(43.0)).0, 1.0, 1.0, o, 0x61, 0x60);
        env.renderer(0x1d4, 15, 0);
        env.set_color(c1);
        let x = F(fl(m, o + 0x360)) - F(w);
        let y = fl(m, o + at::Y);
        if at_max {
            env.sprite((x - F(42.0)).0, (F(y) - F(33.0)).0, 1.0, 1.0, o, 0x65, 0x60);
        } else {
            env.sprite((x - F(41.0)).0, (F(y) - F(52.0)).0, 1.0, 1.0, o, 100, 0x60);
        }
    }
    env.renderer(0x1d4, 13, 0);
    if m.r32(o + at::FULL) != 0 {
        draw_strip(m, env, o);
        draw_timeline(m, env, o);
        cam_info(m, env, o);
        draw_key(m, env, o);
    } else {
        cam_info(m, env, o);
        draw_prompt(m, env, o);
    }
    env.renderer(0x1c4, 1, 0);
}

/// The camera presets' names (`Camera_GetPreset` 1..10; 10 the "Artist" format at 0x394010, the others 0x14d)
pub fn preset_name(p: u32) -> i32 {
    match p {
        1 => 0x152,
        2 => 0x14a,
        3 => 0x14b,
        5 => 0x14c,
        6 => 0x14e,
        7 => 0x150,
        8 => 0x151,
        10 => -1,
        _ => 0x14d,
    }
}

/// `HUD_DrawReplayCamInfo` 0x1ecea0. The label box: "end of replay" (0xeba) at the end, else the overlay's +0x3f0.
/// While the panel moves or is up: the preset's name and the chase camera's (0x32db74 + 0x84 each). Then
/// "highlighting" (0xdec), the message fading, and "more" / "less" (0x16a / 0x16b) at the right.
pub fn cam_info(m: &mut Mem, env: &mut dyn DrawEnv, o: u32) {
    if m.r32(race(m) + 0x34) != 0 {
        return;
    }
    env.renderer(0x1e4, 3, 0);
    let one = 1.0f32;
    let (c0, c1, c2) = if m.r32(o + at::HIDDEN) != 0 {
        let p4 = f32::from_bits(P4);
        ([0.5, p4, p4, p4], [0.5, p4, p4, 0.0], [0.5, p4, 0.0, 0.0])
    } else {
        let c1 = [one, f32::from_bits(0x3f73f7cf), f32::from_bits(0x3f3a9fbe), f32::from_bits(0x3dd91687)];
        ([one; 4], c1, [one, one, 0.0, 0.0])
    };
    if at_end(m) {
        let s = env.loc(0xeba);
        env.label_box(18.0, o, s, 6);
    } else {
        env.label_box(18.0, o, o + 0x3f0, 6);
    }
    let small = f32::from_bits(0x3f2147ae);
    if m.r32(o + at::MOVED) != 0 || m.r32(o + at::MODE) != 0 {
        let name = preset_name(env.camera_preset());
        env.set_color(c0);
        let y = (F(fl(m, o + at::Y)) + F(-47.0)).0;
        env.set_color(c1);
        env.renderer(0x1d4, 0x11, 0);
        if name == -1 {
            env.print(0x19d3f0, small, 20.0, y, m.r32(o + 0x34c), 0x394010);
        } else {
            let s = env.loc(name as u32);
            env.print(0x19d568, small, 20.0, y, m.r32(o + 0x34c), s);
        }
        let i = env.chase_index();
        let s = (i.wrapping_mul(0x84) as u32).wrapping_add(0x32db74);
        env.measure_ascii(small, m.r32(o + 0x34c), s);
        env.print(0x19d6e0, small, 320.0, y, m.r32(o + 0x34c), s);
    }
    if m.r32(o + at::HIGHLIGHTING) != 0 {
        env.set_color(c2);
        let x = ((F(fl(m, o + 0x35c)) + F(fl(m, o + 0x364))) - F(71.0)).0;
        let y = (F(fl(m, o + at::Y)) - F(28.0)).0;
        let f = m.r32(m.r32(GAPP) + 0x60);
        let s = env.loc(0xdec);
        env.print(0x19d568, f32::from_bits(0x3f666666), x, y, f, s);
    }
    let a = fl(m, o + at::MSG_ALPHA);
    if 0.0 < a {
        m.w32(FONT_COLOR, a.to_bits());
        for k in [0xc, 4, 8] {
            m.w32(FONT_COLOR + k, one.to_bits());
        }
        let s = env.loc(m.r32(o + at::MSG));
        let w = env.measure_wide(1.0, m.r32(o + 0x348), s) as f32;
        let l = fl(m, o + 0x35c);
        let half = (F(fl(m, o + 0x360)) - F(l)) * F(0.5);
        let y = (F(fl(m, o + at::Y)) + F(-2.0)).0;
        let s = env.loc(m.r32(o + at::MSG));
        let x = ((F(l) + half) - F(w) * F(0.5)).0;
        env.print(0x19d568, one, x, y, m.r32(o + 0x348), s);
    }
    if m.r32(o + at::MOVED) == 0 {
        env.set_color(c1);
        let id = if m.r32(o + at::MODE) == m.r32(o + at::MAX_MODE) { 0x16a } else { 0x16b };
        let x = (F(fl(m, o + 0x360)) - F(20.0)).0;
        let y = (F(fl(m, o + at::Y)) - F(49.0)).0;
        let s = env.loc(id);
        env.print(0x19d868, one, x, y, m.r32(o + 0x348), s);
    }
}

/// `RaceMenuOverlay_DrawReplayMenu` 0x1f01e8: the frame, the three items (watch 0x187, save 0x161, back 0x1db; all
/// enabled, from 0x3940b8) and the title box (0xe53)
pub fn draw_menu(m: &Mem, env: &mut dyn DrawEnv, o: u32) {
    let items = [env.loc(0x187), env.loc(0x161), env.loc(0x1db)];
    let b = m.r32(o + 0x42c);
    env.backdrop_frame(165.0, b, 15);
    env.menu_list(320.0, 240.0, f32::from_bits(0x3f6b851f), b, items, m.r32(o + 0x43c), [1, 1, 1]);
    let s = env.loc(0xe53);
    env.label_box(155.0, b, s, 17);
}

// ---- the leftovers ----

/// What the replay's leftovers call out to
pub trait MiscEnv {
    /// `Replay_FreeAll` 0x18f208
    fn free_all(&mut self, m: &mut Mem, mgr: u32);
    /// `Mem_Delete` 0x114f80
    fn delete(&mut self, p: u32);
    /// `Race_GetRider` 0x1814a8 (the race, i)
    fn rider(&mut self, i: u32) -> u32;
    /// `AnimCtl_BlendPose` 0x15f7c8 (the rider + 0x46c0)
    fn blend_pose(&mut self, ctl: u32);
    /// `Boarder_UpdatePose` 0x129628
    fn update_pose(&mut self, rider: u32);
    /// `Boarder_UpdateBroadphase` 0x126748
    fn update_broadphase(&mut self, rider: u32);
    /// `Broadphase_SortAll` 0x14fe18 (the race's +0x30)
    fn broadphase_sort(&mut self, b: u32);
    /// `Contact_SortPairs` 0x150428 (the race's +0x30)
    fn contact_sort(&mut self, b: u32);
    /// `CamControl_Reset` 0x173a88 (0x3368b0)
    fn cam_control_reset(&mut self);
    /// `Camera_SetChaseParams` 0x178a28 (0, 0, 10.0, 1.0, the rider's camera target, 0)
    fn set_chase_params(&mut self, target: u32);
    /// `Camera_SelectPreset` 0x1786d0
    fn select_preset(&mut self, p: u32);
}

/// `ReplayMgr_Free` 0x18e9e8: everything freed (0x18ebc8 before it does nothing, and so do the keyframe and
/// highlight arrays' destructors 0x195710 / 0x1956f0 called with 0); the manager deleted with flag 1
pub fn mgr_free(m: &mut Mem, env: &mut dyn MiscEnv, mgr: u32, flags: u32) {
    env.free_all(m, mgr);
    if flags & 1 != 0 {
        env.delete(mgr);
    }
}

/// `Replay_RefreshRiders` 0x1147f8: each rider's pose blended (when its +0x46b0 is set) and its broadphase box
/// updated, +0x42c from +0x16c; then the broadphase and the contacts sorted
pub fn refresh_riders(m: &mut Mem, env: &mut dyn MiscEnv) {
    let mut i = 0u32;
    while (i as i32) < m.r32(m.r32(race(m) + 0x1c) + 0x88) as i32 {
        let r = env.rider(i);
        if m.r32(r + 0x46b0) != 0 {
            env.blend_pose(r + 0x46c0);
            env.update_pose(r);
        }
        env.update_broadphase(r);
        i += 1;
        m.w32(r + 0x42c, m.r32(r + 0x16c));
    }
    let b = m.r32(race(m) + 0x30);
    env.broadphase_sort(b);
    env.contact_sort(m.r32(race(m) + 0x30));
}

/// The camera's replay flag and the word cleared with it
pub const CAM_REPLAY: u32 = 0x337774;
pub const CAM_REPLAY_B: u32 = 0x337770;

/// `Camera_SetReplayMode` 0x1785c0 (on, the rider): the camera control reset; when on, the chase camera on the
/// rider (the first one when past the riders and the extra entries, +0x7c plus +0x84) and preset 7
pub fn set_replay_mode(m: &mut Mem, env: &mut dyn MiscEnv, on: u32, rider: u32) {
    m.w32(CAM_REPLAY, on);
    env.cam_control_reset();
    m.w32(CAM_REPLAY_B, 0);
    if on == 0 {
        return;
    }
    let w = m.r32(race(m) + 0x1c);
    let n = m.r32(w + 0x7c).wrapping_add(m.r32(w + 0x84)) as i32;
    let i = if (rider as i32) < n { rider } else { 0 };
    let target = m.r32(m.r32(m.r32(w + i.wrapping_mul(4) + 0x8c)) + 0x460);
    env.set_chase_params(target);
    env.select_preset(7);
}

// ---- the panel's larger drawings (G12p) ----

/// What the panel's larger drawings call out to besides `DrawEnv`
pub trait PanelEnv: DrawEnv {
    /// `HUD_DrawBar` 0x1a9f80 (x, y, w, h, the overlay, 1 the fill or 2 the frame)
    #[allow(clippy::too_many_arguments)]
    fn bar(&mut self, x: f32, y: f32, w: f32, h: f32, o: u32, kind: u32);
    /// `Draw_Line` 0x19c1e8 (x0, y0, x1, y1, the font colour 0x337fe8)
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32);
    /// 0x19b108 (x, y, w, h, the font colour): a filled rectangle
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32);
    /// 0x1ee660 / 0x1ee6e8 (the overlay): the renderer into and out of screen space (its virtuals +0x8c, +0xb4 with
    /// 0, 0, 512, 448, 0, 1000, and +0x10c; then +0x94)
    fn screen_space(&mut self, begin: bool, o: u32);
}

/// A word converted to a float as unsigned (the halve-and-double the compiler emits for the top bit)
fn u2f(v: u32) -> F {
    if (v as i32) >= 0 {
        F(v as i32 as f32)
    } else {
        let h = F(((v >> 1) | (v & 1)) as i32 as f32);
        h + h
    }
}
fn i2f(v: u32) -> F {
    F(v as i32 as f32)
}
fn fword(m: &Mem, a: u32) -> F {
    F(fl(m, a))
}

/// The whole replay as a strip 409.6 wide (0x1edef0, full controls, while the panel moves or is up): the strip, the
/// cuts as ticks (the one at the frame in green-white when paused), the highlights as bars (clipped at the end), the
/// highlight being made, the frame, and the playing head (sprite 42) at the frame
pub fn draw_strip(m: &mut Mem, env: &mut dyn PanelEnv, o: u32) {
    if m.r32(o + at::MOVED) == 0 && m.r32(o + at::MODE) == 0 {
        return;
    }
    env.screen_space(true, o);
    let p4 = f32::from_bits(P4);
    let hidden = m.r32(o + at::HIDDEN) != 0;
    let c0 = if hidden { [1.0, p4, p4, p4] } else { [1.0; 4] };
    let c1 = [f32::from_bits(0x3f333333), 1.0, 1.0, 0.0];
    let c2 = if hidden { [1.0, p4, 0.0, 0.0] } else { [f32::from_bits(0x3f4ccccd), 1.0, 0.0, 0.0] };
    let c3 = if hidden { [0.5, p4, 0.0, 0.0] } else { [0.5, 1.0, 0.0, 0.0] };
    let c4 = if hidden { [1.0, p4, p4, 0.0] } else { [1.0, 1.0, f32::from_bits(0x3f60e0e1), f32::from_bits(0x3e0c8c8d)] };
    let frame = i2f(m.r32(mgr(m) + rm::FRAME));
    let end = u2f(m.r32(o + at::END_FRAME));
    let x0 = (fword(m, o + 0x36c) * fb(0x3f4ccccd) - fb(0x434ccccd)).0;
    let y = (fword(m, o + at::Y) * fb(0x3f6eeeef) + fb(0x41d05f41)).0;
    let wide = fb(0x43cccccd);
    env.renderer(0x1e4, 3, 0);
    env.set_color(c0);
    env.bar(x0, y, wide.0, 9.0, o, 1);
    env.renderer(0x1f4, u32::MAX, 0);
    let head = frame / end;
    if m.r32(o + at::FULL) != 0 {
        let t = m.r32(o + 0x3ec);
        let mut i = 1u32;
        while (i as i32) < m.r32(t) as i32 {
            let c = rm::cam_track_cut_frame(m, t, i);
            let g = mgr(m);
            if m.r32(g + rm::MODE) == 3 && c == m.r32(g + rm::FRAME) {
                env.set_color(c1);
            } else {
                m.w32(FONT_COLOR + 0xc, 0);
                m.w32(FONT_COLOR, 0x3f4ccccd);
                m.w32(FONT_COLOR + 4, 1.0f32.to_bits());
                m.w32(FONT_COLOR + 8, 0);
            }
            i += 1;
            let x = F(x0) + i2f(c) / end * wide;
            let top = (F(y) - F(6.0)).0;
            env.line(x.0, top, x.0, y);
            let x = x + F(1.0);
            env.line(x.0, top, x.0, y);
        }
    }
    env.set_color(c4);
    let mut on_head = false;
    let mut i = 0u32;
    while (i as i32) < m.r32(o + at::HL_COUNT) as i32 {
        let h = m.r32(o + at::HL + 4 * i);
        let start = u2f(m.r32(m.r32(h) + 0x20));
        let mut len = u2f(m.r32(h + 4));
        let at_x = start / end * wide;
        if u2f(m.r32(o + at::END_FRAME)).0 < (start + len).0 {
            len = u2f(m.r32(o + at::END_FRAME)) - start;
        }
        env.bar((F(x0) + at_x).0, y, (len / end * wide).0, 9.0, o, 1);
        if i2f(m.r32(m.r32(m.r32(o + at::HL + 4 * i)) + 0x20)).0 == frame.0 {
            on_head = true;
        }
        i += 1;
    }
    if m.r32(o + at::FULL) != 0 && m.r32(o + at::HIGHLIGHTING) != 0 {
        env.set_color(c3);
        let from = m.r32(o + at::HL_FROM);
        let x = F(x0) + i2f(from) / end * wide;
        let w = i2f(m.r32(mgr(m) + rm::FRAME).wrapping_sub(from)) / end * wide;
        env.bar(x.0, y, w.0, 9.0, o, 1);
    }
    env.set_color(c0);
    env.bar(x0, y, wide.0, 9.0, o, 2);
    env.set_color(if on_head { c1 } else { c2 });
    let hx = F(x0) + head * wide;
    let r = env.sprite_rect(m.r32(o), 42);
    let w = F(5.0) / fword(m, r + 4);
    env.sprite((hx - F(2.0)).0, (F(y) - F(4.0)).0, w.0, f32::from_bits(0x3f19999a), o, 42, 0);
    env.screen_space(false, o);
}

/// The zoomed timeline (0x1ed490, while the panel moves or is up): a window of +0x460 frames from +0x464 (plus the
/// slide +0x474) that follows the frame, jumping by the window and sliding back; the highlights inside it as
/// bars, the highlight being made, the cuts as marked ticks (full controls), the keyframes as ticks, the frame, and
/// the playing head when the frame is inside
pub fn draw_timeline(m: &mut Mem, env: &mut dyn PanelEnv, o: u32) {
    if m.r32(o + at::MOVED) == 0 && m.r32(o + at::MODE) == 0 {
        return;
    }
    env.screen_space(true, o);
    let p4 = f32::from_bits(P4);
    let p7 = f32::from_bits(0x3f333333);
    let hidden = m.r32(o + at::HIDDEN) != 0;
    let c0 = if hidden { [1.0, p4, p4, p4] } else { [1.0; 4] };
    let c1 = if hidden { [p7, p4, 0.0, 0.0] } else { [p7, 1.0, 0.0, 0.0] };
    let c2 = if hidden { [p7, p4, p4, 0.0] } else { [p7, 1.0, 1.0, 0.0] };
    let c3 = if hidden { [1.0, p4, p4, 0.0] } else { [1.0, 1.0, f32::from_bits(0x3f60e0e1), f32::from_bits(0x3e0c8c8d)] };
    let fifth = f32::from_bits(0x3e4ccccd);
    let c4 = [1.0, fifth, fifth, fifth];
    let c5 = if hidden { [0.5, p4, 0.0, 0.0] } else { [0.5, 1.0, 0.0, 0.0] };
    let frame = m.r32(mgr(m) + rm::FRAME) as i32;
    env.renderer(0x1e4, 3, 0);
    env.renderer(0x1fc, 1, 0);
    // the window
    let w = m.r32(o + at::SHOWN_END) as i32;
    let mut start = m.r32(o + 0x464) as i32;
    let scale = m.r32(o + at::SCALE) as i32;
    if start.wrapping_add(w) < frame {
        start = frame.wrapping_sub(w.wrapping_sub(scale));
        m.w32(o + at::SLIDE, 1i32.wrapping_sub(scale) as u32);
        m.w32(o + 0x464, start as u32);
    } else if frame < start {
        start = frame.wrapping_sub(scale);
        m.w32(o + at::SLIDE, scale.wrapping_sub(1) as u32);
        m.w32(o + 0x464, start as u32);
    }
    let last = (m.r32(o + at::END_FRAME) as i32).wrapping_sub(w);
    if last < start {
        m.w32(o + 0x464, last as u32);
        start = last;
        m.w32(o + at::SLIDE, 0);
    }
    if start < 0 {
        m.w32(o + 0x464, 0);
        start = 0;
        m.w32(o + at::SLIDE, 0);
    }
    let s2 = start.wrapping_add(m.r32(o + at::SLIDE) as i32);
    // the bar's place
    let p8 = fb(0x3f4ccccd);
    let r = env.sprite_rect(m.r32(o), 42);
    let off = fb(0x43333333);
    let h = fword(m, r + 8) * fb(0x3f59999a);
    let x0 = (fword(m, o + 0x36c) * p8 + F(15.0)) - off;
    let y = fword(m, o + at::Y) * fb(0x3f6eeeef) + fb(0x4289d89e);
    let half = h * F(0.5);
    let tick = half - F(1.0);
    env.set_color(c0);
    let bx = fword(m, o + 0x36c) * p8 - off;
    env.bar(bx.0, (y - half).0, f32::from_bits(0x43b33333), h.0, o, 1);
    env.renderer(0x1f4, u32::MAX, 0);
    env.set_color(c3);
    let span = fb(0x43a43333);
    let before = frame < s2;
    let mut on_head = false;
    let mut i = 0u32;
    while (i as i32) < m.r32(o + at::HL_COUNT) as i32 {
        let hl = m.r32(o + at::HL + 4 * i);
        let w = m.r32(o + at::SHOWN_END) as i32;
        let a = m.r32(m.r32(hl) + 0x20) as i32;
        let b = a.wrapping_add(m.r32(hl + 4) as i32);
        let wend = s2.wrapping_add(w);
        if s2 < b && a < wend {
            let from = if s2 < a { a } else { s2 };
            let to = if b < wend { b } else { wend };
            let wf = i2f(w as u32);
            let x = x0 + i2f(from.wrapping_sub(s2) as u32) / wf * span;
            let ww = i2f(to.wrapping_sub(from) as u32) / wf * span;
            env.bar(x.0, (y - h * F(0.5)).0, ww.0, h.0, o, 1);
        }
        if m.r32(m.r32(m.r32(o + at::HL + 4 * i)) + 0x20) as i32 == frame {
            on_head = true;
        }
        i += 1;
    }
    if m.r32(o + at::FULL) != 0 {
        if m.r32(o + at::HIGHLIGHTING) != 0 {
            env.set_color(c5);
            let hf = m.r32(o + at::HL_FROM) as i32;
            let from = if s2 < hf { hf } else { s2 };
            let w = m.r32(o + at::SHOWN_END) as i32;
            let f = m.r32(mgr(m) + rm::FRAME) as i32;
            let wend = s2.wrapping_add(w);
            let to = if f < wend { f } else { wend };
            let wf = i2f(w as u32);
            let x = x0 + i2f(from.wrapping_sub(s2) as u32) / wf * span;
            let ww = i2f(to.wrapping_sub(from) as u32) / wf * span;
            env.bar(x.0, (y - h * F(0.5)).0, ww.0, h.0, o, 1);
        }
        env.renderer(0x1f4, u32::MAX, 0);
        let mut i = 0u32;
        loop {
            let t = m.r32(o + 0x3ec);
            if (i as i32) >= m.r32(t) as i32 {
                break;
            }
            let c = rm::cam_track_cut_frame(m, t, i) as i32;
            let w = m.r32(o + at::SHOWN_END) as i32;
            if s2.wrapping_add(w) < c {
                break;
            }
            if c < s2 {
                i += 1;
                continue;
            }
            let g = mgr(m);
            if m.r32(g + rm::MODE) == 3 && m.r32(g + rm::FRAME) as i32 == c {
                env.set_color(c2);
            } else {
                m.w32(FONT_COLOR, 0x3f4ccccd);
                m.w32(FONT_COLOR + 4, 1.0f32.to_bits());
                m.w32(FONT_COLOR + 8, 0);
                m.w32(FONT_COLOR + 0xc, 0);
            }
            let bottom = y - tick;
            let up = tick + F(6.0);
            let top = y - up;
            let x = x0 + i2f(c.wrapping_sub(s2) as u32) / i2f(w as u32) * span;
            let x = F(x.0 as i32 as f32);
            env.line(x.0, top.0, x.0, bottom.0);
            let x1 = x + F(1.0);
            env.line(x1.0, top.0, x1.0, bottom.0);
            let up = up + F(4.0);
            env.fill_rect((x - F(1.0)).0, (y - up).0, 4.0, 4.0);
            i += 1;
        }
    }
    env.set_color(c4);
    let n = m.r32(o + 0x378) as i32;
    let mut i = 0;
    while i < n {
        let k = m.r32(o + 0x380 + 4 * i as u32) as i32;
        if s2.wrapping_add(m.r32(o + at::SHOWN_END) as i32) < k {
            break;
        }
        if k >= s2 {
            let x = x0 + i2f(k.wrapping_sub(s2) as u32) / i2f(m.r32(o + at::SHOWN_END)) * span;
            let (top, bottom) = ((y - tick).0, (y + tick).0);
            env.line(x.0, top, x.0, bottom);
            let x1 = x + F(1.0);
            env.line(x1.0, top, x1.0, bottom);
        }
        i += 1;
    }
    env.set_color(c0);
    env.bar(bx.0, (y - h * F(0.5)).0, f32::from_bits(0x43b33333), h.0, o, 2);
    if !before && frame <= s2.wrapping_add(m.r32(o + at::SHOWN_END) as i32) {
        let x = x0 + (i2f(frame as u32) - i2f(s2 as u32)) / i2f(m.r32(o + at::SHOWN_END)) * span;
        env.set_color(if on_head { c2 } else { c1 });
        let x = x - F(2.0);
        let yy = y - (tick + F(2.0));
        let r = env.sprite_rect(m.r32(o), 42);
        let w = F(7.0) / fword(m, r + 4);
        env.sprite((x - F(1.0)).0, (yy - F(4.0)).0, w.0, f32::from_bits(0x3f99999a), o, 42, 0);
    }
    env.screen_space(false, o);
}

/// The start / select legend (`HUD_DrawStartSelectPrompt` 0x1ee720, without the full controls): the buttons'
/// sprites in two rows 35 and 83 below the panel, their names (0x1a9, 0x1b4, 0x1b3, 0x1b7, 0x1b2, 0x1b1) right of
/// x 125, 340 and 568, and the "SELECT" / "START" words (0x394048 / 0x394050). With the game's +0x58 at 4 the
/// columns are closer.
pub fn draw_prompt(m: &mut Mem, env: &mut dyn DrawEnv, o: u32) {
    let hidden = m.r32(o + at::HIDDEN) != 0;
    let (c0, c1) = if hidden {
        let c = [f32::from_bits(0x3e4ccccd), f32::from_bits(0x3f4ccccd), f32::from_bits(0x3f4ccccd), f32::from_bits(0x3f4ccccd)];
        (c, c)
    } else {
        ([f32::from_bits(0x3eb33333), 1.0, 1.0, 1.0], [1.0; 4])
    };
    let one = F(1.0);
    let (mut a, mut b, mut c) = (F(340.0), F(568.0), F(154.0));
    env.renderer(0x1d4, 14, 0);
    let (mut d, mut e) = (F(381.0), F(593.0));
    env.renderer(0x1e4, 3, 0);
    env.set_color(c1);
    let font = m.r32(o + 0x348);
    let mut half = i2f(m.r32(font + 0xc)) * F(0.5);
    if m.r32(m.r32(GAPP) + 0x58) == 4 {
        half = half - one;
        a = F(190.0);
        b = F(450.0);
        c = F(150.0);
        d = F(375.0);
        e = F(570.0);
    }
    let y = fword(m, o + at::Y);
    let r1 = y + F(35.0);
    let r2 = y + F(83.0);
    env.sprite(c.0, r1.0, 1.0, 1.0, o, 116, 96);
    env.sprite(c.0, r2.0, 1.0, 1.0, o, 107, 96);
    env.sprite(d.0, (r1 - one).0, 1.0, 1.0, o, 106, 96);
    env.sprite((d - F(10.0)).0, r2.0, 1.0, 1.0, o, 98, 96);
    env.sprite((d + F(10.0)).0, r2.0, 1.0, 1.0, o, 99, 96);
    env.sprite(e.0, (r1 + one).0, 1.0, 1.0, o, 113, 96);
    let t1 = r1 - half;
    env.sprite(e.0, (r2 + one).0, 1.0, 1.0, o, 111, 96);
    env.set_color(c0);
    env.sprite(d.0, r2.0, 1.0, 1.0, o, 97, 96);
    let t2 = r2 - half;
    env.set_color(c1);
    env.renderer(0x1d4, 13, 0);
    let s85 = f32::from_bits(0x3f59999a);
    for (id, x, ty) in [(0x1a9, F(125.0), t1), (0x1b4, F(125.0), t2), (0x1b3, a, t1), (0x1b7, a, t2), (0x1b2, b, t1), (0x1b1, b, t2)] {
        let s = env.loc(id);
        let w = env.measure_wide(s85, font, s);
        let x = x - F(w as f32);
        let s = env.loc(id);
        env.print(0x19d568, s85, x.0, ty.0, font, s);
    }
    let s7 = f32::from_bits(0x3f333333);
    let w = env.measure_ascii(s7, font, 0x394048);
    env.print(0x19d3f0, s7, (c - F(w as f32) * F(0.5)).0, (t2 + F(20.0)).0, font, 0x394048);
    let w = env.measure_ascii(f32::from_bits(0x3f4ccccd), font, 0x394050);
    env.print(0x19d3f0, s7, (d - F(w as f32) * F(0.5)).0, (t1 + F(20.0)).0, font, 0x394050);
}

/// The button key's pad sprites (0x33f0b0: x and y from the screen's middle and the key's top, the sprite)
pub const KEY_SPRITES: [(f32, f32, u32); 13] = [
    (18.0, 155.0, 0x6b),
    (-18.0, 155.0, 0x6a),
    (28.0, 185.0, 0x6c),
    (-28.0, 185.0, 0x6c),
    (-68.0, 155.0, 0x61),
    (56.0, 155.0, 0x75),
    (94.0, 155.0, 0x72),
    (75.0, 136.0, 0x73),
    (75.0, 175.0, 0x74),
    (-64.0, 42.0, 0x6f),
    (-64.0, 72.0, 0x6e),
    (75.0, 42.0, 0x71),
    (75.0, 72.0, 0x70),
];

/// One of the button key's labels (0x33f150, 0x20 bytes each)
#[derive(Clone, Copy, Debug)]
pub struct KeyLine {
    /// +0, +4: from the screen's middle and the key's top
    pub x: f32,
    pub y: f32,
    /// +8: the string
    pub text: u32,
    /// +0xc: 0 centred with a line down, 1 left of x with a line left, 2 right of x with a line right
    pub kind: u32,
    /// +0x10: the line's length (none when not positive); +0x14: a bend up (negative) or down at its end
    pub len: f32,
    pub bend: f32,
    /// +0x18: a sprite beside the text (-1 none) and its corner (+0x1c)
    pub sprite: i32,
    pub corner: u32,
}

const fn kl(x: f32, y: f32, text: u32, kind: u32, len: f32, bend: f32, sprite: i32, corner: u32) -> KeyLine {
    KeyLine { x, y, text, kind, len, bend, sprite, corner }
}

/// The button key's labels (0x33f150)
pub const KEY_LINES: [KeyLine; 18] = [
    kl(-108.0, 64.0, 0x1a5, 2, 20.0, 0.0, -1, 0),
    kl(118.0, 64.0, 0x1b0, 1, 21.0, 0.0, -1, 0),
    kl(-108.0, 36.0, 0x1b1, 2, 20.0, 0.0, -1, 0),
    kl(118.0, 36.0, 0x1b2, 1, 21.0, 0.0, -1, 0),
    kl(-108.0, 92.0, 0x1b3, 2, 85.0, 48.0, -1, 0),
    kl(16.0, 101.0, 0x1b4, 0, 26.0, 0.0, -1, 0),
    kl(-108.0, 120.0, 0x1b5, 2, 35.0, 7.0, -1, 0),
    kl(-108.0, 176.0, 0x1b6, 2, 38.0, -7.0, -1, 0),
    kl(-108.0, 148.0, 0x1b7, 2, 13.0, 0.0, -1, 0),
    kl(-108.0, 232.0, 0x1a6, 2, 77.0, -34.0, 0x2f, 0x10),
    kl(118.0, 187.0, 0x1a7, 1, 62.0, -29.0, -1, 0),
    kl(118.0, 114.0, 0x1aa, 1, 42.0, 4.0, -1, 0),
    kl(118.0, 139.0, 0x1a8, 1, 11.0, 0.0, -1, 0),
    kl(118.0, 163.0, 0x1a9, 1, 29.0, 0.0, -1, 0),
    kl(55.0, 211.0, 0x1ab, 1, 28.0, -16.0, -1, 0),
    kl(-108.0, 204.0, 0x1ad, 2, 20.0, 28.0, 0x2f, 0),
    kl(55.0, 235.0, 0x1ae, 1, 28.0, -22.0, 0x2f, 0x10),
    kl(55.0, 259.0, 0x1af, 1, 28.0, -21.0, 0x2f, 0),
];

/// The button key (0x1eee00, full controls, while the panel is above its +0x354): the pad's sprites, the labels
/// (scale 0.9; 0.7605 with the game's +0x58 at 2 or 3) and their lines 3 wide from the text to the button, with a bend
/// at the end and a sprite beside some
pub fn draw_key(m: &mut Mem, env: &mut dyn DrawEnv, o: u32) {
    if fl(m, o + 0x354) <= fl(m, o + at::Y) {
        return;
    }
    let (c0, c1, c2) = if m.r32(o + at::HIDDEN) != 0 {
        let p8 = f32::from_bits(0x3f4ccccd);
        let c = [f32::from_bits(0x3e4ccccd), p8, p8, p8];
        (c, c, c)
    } else {
        ([1.0; 4], [1.0; 4], [1.0; 4])
    };
    let mid = F(320.0);
    let mut scale = F(f32::from_bits(0x3f666666));
    env.renderer(0x1d4, 13, 0);
    env.renderer(0x1e4, 3, 0);
    env.set_color(c2);
    let top = fword(m, o + at::Y) + F(70.0);
    let mode = m.r32(m.r32(GAPP) + 0x58);
    if mode == 2 || mode == 3 {
        scale = fb(0x3f42b021);
    }
    for (x, y, id) in KEY_SPRITES {
        env.renderer(0x1d4, 14, 0);
        env.sprite((mid + F(x)).0, (top + F(y)).0, 1.0, 1.0, o, id as i32, 0x60);
    }
    env.renderer(0x1d4, 13, 0);
    env.renderer(0x1e4, 3, 0);
    let font = m.r32(o + 0x348);
    for l in KEY_LINES {
        let s = env.loc(l.text);
        let mut w = F(env.measure_wide(scale.0, font, s) as f32);
        if l.kind == 0 {
            w = w * F(0.5);
        } else if l.kind == 1 {
            w = F(0.0);
        }
        env.set_color(c1);
        let x = (mid + F(l.x)) - w;
        let y = top + F(l.y);
        let s = env.loc(l.text);
        env.print(0x19d568, scale.0, x.0, y.0, font, s);
    }
    env.renderer(0x1e4, 3, 0);
    env.set_color(c0);
    let three = F(3.0);
    let label = F(f32::from_bits(0x3f266666));
    for l in KEY_LINES {
        let len = F(l.len);
        if !(0.0 < len.0) {
            continue;
        }
        let fh = i2f(m.r32(m.r32(o + 0x348) + 0xc));
        let (end_x, end_y);
        if l.kind == 0 {
            let x = mid + F(l.x);
            end_x = x + len;
            end_y = ((top + F(l.y)) + fh * scale) + F(5.0);
            env.sprite(x.0, end_y.0, 3.0, len.0, o, -5, 0);
        } else if l.kind == 1 {
            let x = (mid + F(l.x)) - F(2.0);
            end_x = x - len;
            end_y = (top + F(l.y)) + fh * scale * fb(P4);
            env.sprite(x.0, end_y.0, len.0, three.0, o, -5, 8);
            if l.sprite != -1 {
                let s = env.loc(l.text);
                let w = F(env.measure_wide(scale.0, m.r32(o + 0x348), s) as f32);
                let r = env.sprite_rect(m.r32(o), l.sprite as u32);
                let (sx, sy) = if l.corner == 0 {
                    ((mid + F(l.x)) + (w + F(7.0)), (top + F(l.y)) + F(2.0))
                } else {
                    let fh = i2f(m.r32(m.r32(o + 0x348) + 0xc));
                    ((mid + F(l.x)) + (w + F(7.0)), ((top + F(l.y)) + fh * F(0.5)) - fword(m, r + 8) * F(0.5))
                };
                env.sprite(sx.0, sy.0, label.0, label.0, o, l.sprite, l.corner);
            }
        } else {
            let x = (mid + F(l.x)) + F(4.0);
            end_x = x + len;
            end_y = (top + F(l.y)) + fh * scale * fb(P4);
            env.sprite(x.0, end_y.0, len.0, three.0, o, -5, 0);
            if l.sprite != -1 {
                let s = env.loc(l.text);
                let w = F(env.measure_wide(scale.0, m.r32(o + 0x348), s) as f32);
                let r = env.sprite_rect(m.r32(o), l.sprite as u32);
                let (sx, sy) = if l.corner == 0 {
                    let off = (w + fword(m, r + 4) * fb(P4)) + F(10.0);
                    ((mid + F(l.x)) - off, (top + F(l.y)) + F(4.0))
                } else {
                    let off = (w + fword(m, r + 8) * fb(P4)) + F(10.0);
                    let fh = i2f(m.r32(m.r32(o + 0x348) + 0xc));
                    ((mid + F(l.x)) - off, ((top + F(l.y)) + fh * F(0.5)) - fword(m, r + 8) * F(0.5))
                };
                env.sprite(sx.0, sy.0, label.0, label.0, o, l.sprite, l.corner);
            }
        }
        let bend = F(l.bend);
        if bend.0 == 0.0 {
            continue;
        }
        let abs = F(bend.0.abs());
        if bend.0 < 0.0 {
            let y = end_y + bend;
            if l.kind == 2 {
                env.sprite((end_x - three).0, y.0, abs.0, three.0, o, -5, 17);
            } else {
                env.sprite(end_x.0, y.0, abs.0, three.0, o, -5, 16);
            }
        } else {
            env.sprite(end_x.0, end_y.0, abs.0, three.0, o, -5, 16);
        }
    }
}
