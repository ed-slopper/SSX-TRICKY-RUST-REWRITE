//! The front end's manager (board row G9b2d1) on the game's bytes: `cFE3D`'s per-frame update (vf5) and draw (vf6),
//! its player count and its small helpers, every call out through [`FeEnv`]; checked in the function runner
//! (`tests/fe_manager.rs`). Its set-up and tear-down (the constructors, the destructor, vf1, vf4) are object glue
//! left to the app (row G9b2d2).
//!
//! The manager (gApp+0x734): +4 the players (a signed byte), +5 a byte, +6 the state (0 the menus, 1 leaving for the
//! game, 2 left), +8 a word, +0x60 a vector, +0x74 / +0x78 the two players, +0x7c the players' picks (two bytes
//! each), +0x80 a list of things drawn and updated, +0xa4 the 3D scene (its vtable at +0), +0xa8 the screen stack
//! (the finished screens at +0xcc), +0xf0 the game type, +0xf4 the idle frames, +0xf8 the attract film (none while
//! the menus run), +0x124 the video manager (its vtable at +8).

use crate::fe3dstate::{vcall, FeEnv, ALLOC, FILL, GAPP};
use crate::fe_characcept::CAREER;
use crate::fe_charselect::SEEN;
use crate::fe_helpers::{add_player, last};
use crate::fe_riderprofile::{SET_FILM, VIDEO_VTABLE};
use crate::fe_title::BASE_CTOR;
use crate::fe_trickbook::CLEAR_SLOT;
use crate::pf::Mem;

/// The calls: the transition's update (0x1fece8) and the fade's draw (0x1ff000) on gApp+0x72c; a player's cheats
/// (0x26cf60); the drawn list's update (0x26ff88) and draws (0x26feb8, 0x26ff40); the sound bank's update (0x17d950
/// with 3 and 4 on 0x337c60) and draw-side calls (0x17d9d8); the pads' check (0x26cef8); the stack's close
/// (0x275ac8) and push (0x275b10); the music's tick (0x20ee58: the word at 0x343050, 0, f12 0.02); the game made on
/// leaving (by the career's +0x3710, its +0x2afc and the players: 0x24c748 (0x1e68 bytes, with 1), 0x24d7f8
/// (0x9f0), 0x247a60 (0xd64), 0x245840 (0xec4)) and handed to the app (0x17c6b8); the course's draw (0x1ce278 the
/// camera, 0x2008f8 and 0x200390 on the course at 0x346488); the character select made (0x276be8) and the pads'
/// rule (0x265af8)
pub const TRANSITION_UPDATE: u32 = 0x1fece8;
pub const FADE_DRAW: u32 = 0x1ff000;
pub const CHEATS: u32 = 0x26cf60;
pub const DRAWN_UPDATE: u32 = 0x26ff88;
pub const DRAWN_DRAW_COURSE: u32 = 0x26feb8;
pub const DRAWN_DRAW: u32 = 0x26ff40;
pub const SOUND_BANK: u32 = 0x337c60;
pub const SOUND_UPDATE: u32 = 0x17d950;
pub const SOUND_DRAW: u32 = 0x17d9d8;
pub const PAD_CONNECTED: u32 = 0x26cef8;
pub const STACK_CLOSE: u32 = 0x275ac8;
pub const STACK_PUSH: u32 = 0x275b10;
pub const MUSIC_TICK: u32 = 0x20ee58;
pub const MUSIC_WORD: u32 = 0x343050;
pub const GAME_HAND: u32 = 0x17c6b8;
pub const GAMES: [(u32, u32); 4] = [(0x24c748, 0x1e68), (0x24d7f8, 0x9f0), (0x247a60, 0xd64), (0x245840, 0xec4)];
pub const CAMERA: u32 = 0x1ce278;
pub const COURSE: u32 = 0x346488;
pub const COURSE_DRAW: u32 = 0x2008f8;
pub const COURSE_DRAW2: u32 = 0x200390;
pub const CHARSELECT_CTOR: u32 = 0x276be8;
pub const ATTRACT_FILM: u32 = 0x3a8d48;
pub const ORDER: u32 = 0x3470b8;

fn i8_at(m: &Mem, at: u32) -> i32 {
    m.r8(at) as i8 as i32
}

/// The renderer's virtual (gApp+0x724; its vtable at +0x10)
fn renderer_call(m: &mut Mem, e: &mut dyn FeEnv, off: u32, args: &[u32]) -> u32 {
    let r = m.r32(m.r32(GAPP).wrapping_add(0x724));
    let vt = m.r32(r.wrapping_add(0x10));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![r.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// A virtual of an object whose vtable is at `vt_at`
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, vt_at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(vt_at));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, tag: u32, size: u32, align: u32) -> u32 {
    let at = e.call(m, ALLOC, &[tag, size, align]);
    if at != 0 {
        e.call(m, FILL, &[at, 0xdead_c0de, size]);
    }
    at
}

/// 0x263670 (the manager, the players): fewer players clear the slots from the new count up (0x1637d0); more seat
/// each new one (0x1635b8 with its own number twice and 0); the count kept (+4)
pub fn set_players(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, n: u32) {
    let n = n as u8 as i8 as i32;
    let cur = i8_at(m, fe + 4);
    if n < cur {
        let mut k = n;
        loop {
            e.call(m, CLEAR_SLOT, &[CAREER, k as u32]);
            k += 1;
            if k >= i8_at(m, fe + 4) {
                break;
            }
        }
    } else if cur < n {
        let more = (n - m.r8(fe + 4) as i32) as u8 as i8 as i32;
        for k in 0..more {
            let slot = (i8_at(m, fe + 4) + k) as u32;
            e.call(m, SEEN, &[CAREER, slot, slot, 0]);
        }
    }
    m.w8(fe + 4, n as u8);
}

/// `cFE3D` vf2 0x263750: when the 3D scene's +0x128 answers, +8 cleared and 1; else 0
pub fn vf2(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) -> u32 {
    let scene = m.r32(fe + 0xa4);
    if vcall_at(m, e, scene, 0, 0x128, &[]) != 0 {
        m.w32(fe + 8, 0);
        1
    } else {
        0
    }
}

/// `cFE3D` vf5 0x265398 (the frame's update): the transition and the video manager (+0x10) ticked; then by the
/// state: in the menus, each player's cheats, the drawn list, the sound bank's 3 and 4, and with no attract film any
/// player's input resets the idle count, else (unless the top screen answers its +0x70) after 0xe11 idle frames the
/// attract film (a video screen with the film 0x3a8d48) is pushed; the finished screens closed; one music tick.
/// Leaving (1): the game made by the career and players and handed to the app, state 2, then two music ticks;
/// afterwards (2 on) two music ticks.
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    let rec = m.r32(m.r32(GAPP).wrapping_add(0x72c));
    e.call(m, TRANSITION_UPDATE, &[rec]);
    let video = m.r32(fe + 0x124);
    vcall_at(m, e, video, 8, 0x10, &[]);
    let st = i8_at(m, fe + 6);
    let tick = |m: &mut Mem, e: &mut dyn FeEnv| {
        let w = m.r32(MUSIC_WORD);
        e.call(m, MUSIC_TICK, &[w, 0, 0.02f32.to_bits()]);
    };
    if st != 0 {
        if st == 1 {
            let app = m.r32(GAPP);
            let k = if m.r32(CAREER + 0x3710) == 1 {
                0
            } else if m.r32(CAREER + 0x2afc) == 6 {
                1
            } else if i8_at(m, fe + 4) == 2 {
                2
            } else {
                3
            };
            let (ctor, size) = GAMES[k];
            let at = alloc(m, e, 0x3a9090, size, 0x10);
            let game = if k == 0 { e.call(m, ctor, &[at, 1]) } else { e.call(m, ctor, &[at]) };
            e.call(m, GAME_HAND, &[app, game]);
            m.w8(fe + 6, 2);
        }
        tick(m, e);
        tick(m, e);
        return;
    }
    for k in 0..2 {
        let p = m.r32(fe + 0x74 + 4 * k);
        e.call(m, CHEATS, &[p]);
    }
    e.call(m, DRAWN_UPDATE, &[fe + 0x80]);
    e.call(m, SOUND_UPDATE, &[SOUND_BANK, 3]);
    e.call(m, SOUND_UPDATE, &[SOUND_BANK, 4]);
    if m.r32(fe + 0xf8) == 0 {
        let mut any = false;
        for k in 0..2 {
            let p = m.r32(fe + 0x74 + 4 * k);
            if e.call(m, PAD_CONNECTED, &[p]) != 0 {
                let p = m.r32(fe + 0x74 + 4 * k);
                if m.r32(p) != 0 {
                    any = true;
                }
            }
        }
        if any {
            m.w32(fe + 0xf4, 0);
        } else {
            let top = last(m, fe + 0xa8);
            if vcall(m, e, top, 0x70, &[]) == 0 {
                let idle = m.r32(fe + 0xf4).wrapping_add(1);
                m.w32(fe + 0xf4, idle);
                if (idle as i32) >= 0xe11 {
                    let v = alloc(m, e, 0x3a8d38, 0x160, 0x10);
                    e.call(m, BASE_CTOR, &[v]);
                    m.w32(v.wrapping_add(0x15c), 0);
                    m.w32(v.wrapping_add(0x154), 0);
                    m.w32(fe + 0xf8, v);
                    m.w32(v.wrapping_add(0xc), VIDEO_VTABLE);
                    m.w32(v.wrapping_add(0x158), 1);
                    e.call(m, SET_FILM, &[v, ATTRACT_FILM]);
                    let v = m.r32(fe + 0xf8);
                    vcall(m, e, v, 0x60, &[]);
                    let v = m.r32(fe + 0xf8);
                    e.call(m, STACK_PUSH, &[fe + 0xa8, v]);
                }
            }
        }
    }
    e.call(m, STACK_CLOSE, &[fe + 0xa8]);
    tick(m, e);
}

/// `cFE3D` vf6 0x264fd8 (the frame's draw; answers 0 when the renderer isn't ready (+0x70), else 1; `buf` 0x80
/// bytes): in the menus the video manager's draw (+0x18); the attract film drawn alone (+0xe0); a film screen on top
/// (its +0x70 answering 0xc) or no screen drawn alone; otherwise the fade, and in the menus the scene: the renderer's
/// set-up (+0x1a0, +0x88, +0xd8, +0x1f8(0, 0), +0x1e0(0), the saved +0x210, +0x1e8(0, 0), +0x1c8(1, 2, 0.08)), with a
/// course loaded its camera and the drawn list over it, the 640 × 480 projection, +0x108, +0x1e0(3), +0x1c8(2, 2,
/// 0.08), the word at 0x337ff8 2.0, +0x1e8(1, 0), the drawn list, +0x1e8 back, and the frame's end (+0x80, +0x1a8,
/// +0x90, +0xe0); the others end with +0x80 alone
pub fn draw(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, buf: u32) -> u32 {
    if renderer_call(m, e, 0x70, &[]) == 0 {
        return 0;
    }
    if i8_at(m, fe + 6) == 0 {
        let video = m.r32(fe + 0x124);
        vcall_at(m, e, video, 8, 0x18, &[]);
    }
    let film = m.r32(fe + 0xf8);
    let alone = if film != 0 {
        true
    } else {
        let top = last(m, fe + 0xa8);
        top == 0 || {
            let top = last(m, fe + 0xa8);
            vcall(m, e, top, 0x70, &[]) == 0xc
        }
    };
    if alone {
        let film = m.r32(fe + 0xf8);
        if film != 0 {
            vcall(m, e, film, 0xe0, &[]);
        }
        renderer_call(m, e, 0x80, &[]);
        return 1;
    }
    let rec = m.r32(m.r32(GAPP).wrapping_add(0x72c));
    e.call(m, FADE_DRAW, &[rec]);
    if i8_at(m, fe + 6) != 0 {
        return 1;
    }
    renderer_call(m, e, 0x1a0, &[]);
    renderer_call(m, e, 0x88, &[]);
    renderer_call(m, e, 0xd8, &[]);
    renderer_call(m, e, 0x1f8, &[0, 0]);
    renderer_call(m, e, 0x1e0, &[0]);
    let saved = renderer_call(m, e, 0x210, &[]);
    renderer_call(m, e, 0x1e8, &[0, 0]);
    let fog = 0.08f32.to_bits();
    renderer_call(m, e, 0x1c8, &[1, 2, fog]);
    if m.r32(COURSE) != 0 {
        e.call(m, CAMERA, &[buf]);
        renderer_call(m, e, 0x218, &[buf]);
        renderer_call(m, e, 0x1d0, &[2]);
        e.call(m, DRAWN_DRAW_COURSE, &[fe + 0x80]);
        e.call(m, COURSE_DRAW, &[COURSE]);
        renderer_call(m, e, 0x2c8, &[]);
        e.call(m, COURSE_DRAW2, &[COURSE]);
        e.call(m, SOUND_DRAW, &[SOUND_BANK, 3]);
        e.call(m, SOUND_DRAW, &[SOUND_BANK, 4]);
    }
    renderer_call(m, e, 0xb0, &[0, 0, 640.0f32.to_bits(), 480.0f32.to_bits(), 0, 1000.0f32.to_bits()]);
    renderer_call(m, e, 0x108, &[]);
    renderer_call(m, e, 0x1e0, &[3]);
    renderer_call(m, e, 0x1c8, &[2, 2, fog]);
    m.w32(0x337ff8, 2.0f32.to_bits());
    renderer_call(m, e, 0x1e8, &[1, 0]);
    e.call(m, DRAWN_DRAW, &[fe + 0x80]);
    renderer_call(m, e, 0x1e8, &[saved, 0]);
    renderer_call(m, e, 0x80, &[]);
    renderer_call(m, e, 0x1a8, &[]);
    renderer_call(m, e, 0x90, &[]);
    renderer_call(m, e, 0xe0, &[]);
    1
}

/// 0x265758: the next of two alternating values: the byte at 0x3471f8 × 2 + the byte at 0x347200 (signed), the
/// latter flipped
pub fn alternate(m: &mut Mem) -> u32 {
    let a = m.r8(0x3471f8) as u32;
    let b = m.r8(0x347200) as u32;
    m.w8(0x347200, (b ^ 1) as u8);
    (b.wrapping_add(a << 1) as u8 as i8 as i32) as u32
}

/// 0x265788 (the manager, a place): the character at that place of the front end's order (bytes at 0x3470b8)
pub fn order(m: &Mem, k: u32) -> u32 {
    let k = k as u8 as i8 as i32;
    (m.r8(ORDER.wrapping_add(k as u32)) as i8 as i32) as u32
}

/// 0x2657a8 (the manager): the career set for the game (+0x2b08 cleared; by the mode at +0x27dc+0x320: 6 the
/// tutorial (+0x2af8 10, one player), 7 the trick book (its trick's +0x10 from 0x163468, the manager's players), 1
/// the manager's players, any other the career's +0x16b8), the career's 0x163980, leaving (+6 1)
pub fn set_game(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    let c = CAREER;
    m.w32(c + 0x2b08, 0);
    let mode = m.r32(c + 0x27dc + 0x320);
    match mode {
        6 => {
            m.w32(c + 0x2af8, 10);
            m.w32(c + 0x27dc, 1);
        }
        7 => {
            let arg = m.r32(c + 0x27dc + 0x31c);
            let r = e.call(m, 0x163468, &[c, 2, arg]);
            m.w32(c + 0x2b08, m.r32(r.wrapping_add(0x10)));
            m.w32(c + 0x27dc, i8_at(m, fe + 4) as u32);
        }
        1 => {
            m.w32(c + 0x27dc, i8_at(m, fe + 4) as u32);
        }
        _ => {
            let v = m.r32(c + 0x16b8);
            m.w32(c + 0x27dc, v);
        }
    }
    e.call(m, 0x163980, &[c]);
    m.w8(fe + 6, 1);
}

/// 0x265870 (the manager): the players whose pads are in
pub fn pads_in(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) -> u32 {
    let mut n = 0u32;
    for k in 0..2 {
        let p = m.r32(fe + 0x74 + 4 * k);
        if e.call(m, PAD_CONNECTED, &[p]) != 0 {
            n += 1;
        }
    }
    n
}

/// 0x2658e8 (the manager, a character): its place in the order, or 0
pub fn place_of(m: &Mem, c: u32) -> u32 {
    let c = c as u8 as i8 as i32;
    for k in 0..12u32 {
        if order(m, k) as i32 == c {
            return k;
        }
    }
    0
}

/// 0x265970 (the manager; `_a1` unused): the character select made (0x154 bytes, 0x276be8); with the pads' rule
/// (0x265af8) each player not yet picked (its +0x44 0) added to it, or every player when none was; without it every
/// player
pub fn make_character_select(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, _a1: u32) -> u32 {
    let at = alloc(m, e, 0x3a90a0, 0x154, 0);
    let sel = e.call(m, CHARSELECT_CTOR, &[at]);
    if pads_rule(m) != 0 {
        m.w8(sel.wrapping_add(0x2d), 0);
        for k in 0..2u32 {
            let p = m.r32(fe + 0x74 + 4 * k);
            if m.r8(p.wrapping_add(0x44)) == 0 {
                add_player(m, sel, k);
            }
        }
        if m.r8(sel.wrapping_add(0x2d)) == 0 {
            add_player(m, sel, u32::MAX);
        }
    } else {
        add_player(m, sel, u32::MAX);
    }
    sel
}

/// 0x265af8: the pads' rule (the word at 0x32e5f0's low two bits clear)
pub fn pads_rule(m: &Mem) -> u32 {
    (m.r32(0x32e5f0) & 3 == 0) as u32
}

/// 0x265bb8 (a player): not picked (+0x44 -1)
pub fn player_reset(m: &mut Mem, p: u32) {
    m.w8(p + 0x44, 0xff);
}

/// 0x265be8 / 0x265bf0 / 0x265bf8: the idle frames cleared; the game type read and set
pub fn idle_reset(m: &mut Mem, fe: u32) {
    m.w32(fe + 0xf4, 0);
}
pub fn game_type(m: &Mem, fe: u32) -> u32 {
    m.r32(fe + 0xf0)
}
pub fn set_game_type(m: &mut Mem, fe: u32, v: u32) {
    m.w32(fe + 0xf0, v);
}

/// 0x265c00 / 0x265c10 (the manager, a player, (a pick)): the player's pick byte (+0x7c + 2 × player) read and set
pub fn pick(m: &Mem, fe: u32, p: u32) -> u32 {
    (m.r8(fe.wrapping_add(p.wrapping_mul(2)).wrapping_add(0x7c)) as i8 as i32) as u32
}
pub fn set_pick(m: &mut Mem, fe: u32, p: u32, v: u32) {
    m.w8(fe.wrapping_add(p.wrapping_mul(2)).wrapping_add(0x7c), v as u8);
}

/// 0x265c28: two or more pads in
pub fn two_pads(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) -> u32 {
    (pads_in(m, e, fe) as i32 >= 2) as u32
}

/// 0x265c50 / 0x265c58: the vector at +0x60 (its address; set from a quadword)
pub fn vector_at(fe: u32) -> u32 {
    fe + 0x60
}
pub fn set_vector(m: &mut Mem, fe: u32, v: u32) {
    for k in 0..4 {
        let x = m.r32((v & !0xf) + 4 * k);
        m.w32(((fe + 0x60) & !0xf) + 4 * k, x);
    }
}

/// `cFE3D` vf3 0x265c68: +8. 0x265c70 / 0x265c78: the state (+6) read; leaving (+6 1). 0x265c88 / 0x265c90: +5 set /
/// read. 0x265c98: the players (+4)
pub fn vf3(m: &Mem, fe: u32) -> u32 {
    m.r32(fe + 8)
}
pub fn state(m: &Mem, fe: u32) -> u32 {
    i8_at(m, fe + 6) as u32
}
pub fn leave(m: &mut Mem, fe: u32) {
    m.w8(fe + 6, 1);
}
pub fn set_byte5(m: &mut Mem, fe: u32, v: u32) {
    m.w8(fe + 5, v as u8);
}
pub fn byte5(m: &Mem, fe: u32) -> u32 {
    i8_at(m, fe + 5) as u32
}
pub fn players(m: &Mem, fe: u32) -> u32 {
    i8_at(m, fe + 4) as u32
}

/// 0x265a58 (the manager, a pass-on word): with two pads in, a 0x54-byte screen (the base constructor, vtable
/// 0x3b30b0); else one player and the character select
pub fn after_players(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, a1: u32) -> u32 {
    if pads_in(m, e, fe) as i32 >= 2 {
        let s = alloc(m, e, 0x3a90b0, 0x54, 0);
        e.call(m, BASE_CTOR, &[s]);
        m.w32(s.wrapping_add(0xc), 0x3b30b0);
        s
    } else {
        set_players(m, e, fe, 1);
        make_character_select(m, e, fe, a1)
    }
}

/// 0x265b10: the front end's light presets (the words at 0x3d2df8..0x3d2e40)
pub fn set_lights(m: &mut Mem) {
    for (at, v) in [
        (0x3d2e20u32, 0x3b03_126fu32),
        (0x3d2df8, 0x3f80_0000),
        (0x3d2e00, 0),
        (0x3d2e40, 0x3f80_0000),
        (0x3d2e18, 0x3f80_0000),
        (0x3d2e1c, 0x3d9b_3d08),
        (0x3d2e08, 0x3f80_0000),
        (0x3d2e0c, 0x3ef2_fec5),
        (0x3d2e10, 0),
        (0x3d2dfc, 0x3f76_e979),
        (0x3d2e28, u32::MAX),
        (0x3d2e2c, 0x3f80_0000),
        (0x3d2e30, 0x3f80_0000),
        (0x3d2e34, 0),
        (0x3d2e38, 0),
        (0x3d2e3c, 0x3f80_0000),
    ] {
        m.w32(at, v);
    }
}

/// The heap's free (0x114f80)
pub const FREE: u32 = 0x114f80;

/// 0x265bc8 (an object, the destructor's flags): freed when the flags ask (bit 0)
pub fn free_if(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, flags: u32) {
    if flags & 1 != 0 {
        e.call(m, FREE, &[obj]);
    }
}
