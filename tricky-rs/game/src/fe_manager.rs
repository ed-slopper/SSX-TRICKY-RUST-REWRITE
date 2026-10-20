//! The front end's manager (board row G9b2d1) on the game's bytes: `cFE3D`'s per-frame update (vf5) and draw (vf6),
//! its player count and its small helpers, every call out through [`FeEnv`]; checked in the function runner
//! (`tests/fe_manager.rs`). Its constructors, destructor and vf4 (row G9b2d2a) are here too, checked in
//! `tests/fe_manager_setup.rs`; vf1, the 19 objects' set-up, is row G9b2d2b.
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
use crate::ps2f::F;

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

/// The set-up and tear-down's calls (board row G9b2d2a): a drawn list's constructor (0x2abfb0), the lesson's set up
/// (0x158d18 on 0x3211a8 with 0x7d3e8) and the career's two-player test (0x164288); the manager's lights' destructor
/// (0x261e98 on +0x70), the fades' reset (0x1ff6a0 on gApp+0x72c), a model's unload (0x2abf38), the stack's clear
/// (0x275c60), the drawn list's clear (0x26feb0), the course's unload (0x25f498 on 0x346488, 1), a player's
/// destructor (0x265bc8), the fonts' release (0x19c8d0), the lesson's stop (0x159070) and reset (0x158cc8), the world
/// lists' clear (0x17d440 on 0x337c60), the pool's (0x23d368 on 0x345e48), the stack's, the drawn list's and the
/// lights' own destructors (0x2759f0, 0x265dc0, 0x265da0); a model's load (0x266320), loaded test (0x2663e8) and
/// finish (0x2661e8), the frame wait (0x2ced00); a video screen's film (0x2abb58) and its first screen (0x2aba70)
pub mod setup {
    pub const VTABLE: u32 = 0x3a90c0;
    pub const LIST_VTABLE: u32 = 0x3a9100;
    pub const LIST_CTOR: u32 = 0x2abfb0;
    pub const LESSON_INIT: u32 = 0x158d18;
    pub const LESSON: u32 = 0x3211a8;
    pub const LESSON_FUNC: u32 = 0x7d3e8;
    pub const TWO_PLAYERS: u32 = 0x164288;
    pub const LIGHTS_DTOR: u32 = 0x261e98;
    pub const FADES_RESET: u32 = 0x1ff6a0;
    pub const MODEL_UNLOAD: u32 = 0x2abf38;
    pub const STACK_CLEAR: u32 = 0x275c60;
    pub const DRAWN_CLEAR: u32 = 0x26feb0;
    pub const COURSE_UNLOAD: u32 = 0x25f498;
    pub const PLAYER_DTOR: u32 = 0x265bc8;
    pub const FONT_RELEASE: u32 = 0x19c8d0;
    pub const LESSON_STOP: u32 = 0x159070;
    pub const LESSON_RESET: u32 = 0x158cc8;
    pub const LISTS_CLEAR: u32 = 0x17d440;
    pub const POOL_CLEAR: u32 = 0x23d368;
    pub const STACK_DTOR: u32 = 0x2759f0;
    pub const DRAWN_DTOR: u32 = 0x265dc0;
    pub const LIGHTS_DTOR2: u32 = 0x265da0;
    pub const MODEL_LOAD: u32 = 0x266320;
    pub const MODEL_LOADED: u32 = 0x2663e8;
    pub const MODEL_FINISH: u32 = 0x2661e8;
    pub const FRAME_WAIT: u32 = 0x2ced00;
    pub const FIRST_SCREEN: u32 = 0x2aba70;
    pub const FILM_NAME: u32 = 0x3a8d38;
    /// The boot films: the logo's and the intro's
    pub const FILMS: [u32; 2] = [0x3a8d48, 0x3a8d60];
    /// The first screen the menus push when there is no film (the word at 0x3471fc)
    pub const FIRST: u32 = 0x3471fc;
}

/// The part both constructors share: the table, +0x70 3 and +0x71 -1, the picks (+0x7c..+0x7f) cleared, the drawn
/// list (+0x80, its table at +0xa0) and the stack's two lists (+0xa8, +0xcc) made
fn fe_base(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    m.w32(fe, setup::VTABLE);
    m.w8(fe + 0x70, 3);
    m.w8(fe + 0x71, 0xff);
    for k in 0..4 {
        m.w8(fe + 0x7c + k, 0);
    }
    e.call(m, setup::LIST_CTOR, &[fe + 0x80]);
    m.w32(fe + 0x80 + 0x20, setup::LIST_VTABLE);
    e.call(m, setup::LIST_CTOR, &[fe + 0xa8]);
    e.call(m, setup::LIST_CTOR, &[fe + 0xcc]);
}

/// `cFE3D::cFE3D` 0x263280(fe, from_game) -> fe: the base; +5, +6 and +0xc..+0x18, +0x50, +0xa4 cleared, the game
/// type 2 (+0xf0), the idle frames and the film cleared; the app's step cleared, the career's +0x3710 cleared and,
/// coming from the game, its +0x3260 1; the lesson set up, +8 cleared, the players by the career's two-player test
pub fn fe_ctor(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, from_game: u32) -> u32 {
    m.w32(fe, setup::VTABLE);
    m.w8(fe + 0x70, 3);
    m.w8(fe + 0x71, 0xff);
    m.w8(fe + 5, 0);
    m.w8(fe + 6, 0);
    for at in [0xcu32, 0x10, 0x14, 0x18, 0x50] {
        m.w32(fe + at, 0);
    }
    for k in 0..4 {
        m.w8(fe + 0x7c + k, 0);
    }
    e.call(m, setup::LIST_CTOR, &[fe + 0x80]);
    m.w32(fe + 0xa4, 0);
    m.w32(fe + 0x80 + 0x20, setup::LIST_VTABLE);
    e.call(m, setup::LIST_CTOR, &[fe + 0xa8]);
    e.call(m, setup::LIST_CTOR, &[fe + 0xcc]);
    m.w32(fe + 0xf0, 2);
    m.w32(fe + 0xf4, 0);
    let app = m.r32(GAPP);
    m.w32(app + 8, 0);
    m.w32(fe + 0xf8, 0);
    m.w32(CAREER + 0x3710, 0);
    if from_game != 0 {
        m.w32(CAREER + 0x3260, 1);
    }
    e.call(m, setup::LESSON_INIT, &[setup::LESSON, setup::LESSON_FUNC]);
    m.w32(fe + 8, 0);
    let n = e.call(m, setup::TWO_PLAYERS, &[CAREER]);
    m.w8(fe + 4, n as u8);
    set_players(m, e, fe, n as u8 as i8 as i32 as u32);
    fe
}

/// `cFE3D::cFE3D` 0x265ca0(fe) -> fe: the base only
pub fn fe_ctor_base(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) -> u32 {
    fe_base(m, e, fe);
    fe
}

/// An object destroyed through its table at `vt_at` (+8 with 3), when there is one
fn destroy_at(m: &mut Mem, e: &mut dyn FeEnv, o: u32, vt_at: u32) {
    if o != 0 {
        vcall_at(m, e, o, vt_at, 8, &[3]);
    }
}

/// `cFE3D::~cFE3D` 0x2633d0(fe, flags): its table; the lights, the fades reset, the twelve models (+0x20..)
/// unloaded and destroyed, the stack and drawn list cleared, the course unloaded; +0x18 destroyed and cleared; the
/// two scenes (+0x10, +0x14) closed (+0x18) and destroyed; both players; the 3D scene (+0xa4) closed (+0x120) and
/// destroyed; the video manager (+0x124); the effects loader freed, the fonts released, the renderer's +0x148, the
/// lesson stopped and reset, the world lists and the pool cleared, the app's front end (+0x734) cleared; the stack,
/// the drawn list and the lights destroyed (with 2), then the module's destructor
pub fn fe_dtor(m: &mut Mem, e: &mut dyn FeEnv, fe: u32, flags: u32) {
    m.w32(fe, setup::VTABLE);
    e.call(m, setup::LIGHTS_DTOR, &[fe + 0x70]);
    let fades = m.r32(m.r32(GAPP) + 0x72c);
    e.call(m, setup::FADES_RESET, &[fades]);
    for k in 0..12u32 {
        let at = fe + 0x20 + 4 * k;
        let o = m.r32(at);
        e.call(m, setup::MODEL_UNLOAD, &[o]);
        let o = m.r32(at);
        destroy_at(m, e, o, 0xc);
    }
    e.call(m, setup::STACK_CLEAR, &[fe + 0xa8]);
    e.call(m, setup::DRAWN_CLEAR, &[fe + 0x80]);
    e.call(m, setup::COURSE_UNLOAD, &[COURSE, 1]);
    let o = m.r32(fe + 0x18);
    destroy_at(m, e, o, 4);
    m.w32(fe + 0x18, 0);
    for at in [0x10u32, 0x14] {
        let o = m.r32(fe + at);
        vcall_at(m, e, o, 0x10, 0x18, &[]);
        let o = m.r32(fe + at);
        destroy_at(m, e, o, 0x10);
    }
    for at in [0x74u32, 0x78] {
        let p = m.r32(fe + at);
        if p != 0 {
            e.call(m, setup::PLAYER_DTOR, &[p, 3]);
        }
    }
    let sc = m.r32(fe + 0xa4);
    vcall_at(m, e, sc, 0, 0x120, &[]);
    let sc = m.r32(fe + 0xa4);
    destroy_at(m, e, sc, 0);
    let vm = m.r32(fe + 0x124);
    destroy_at(m, e, vm, 8);
    let app = m.r32(GAPP);
    crate::app::free_effects_loader(m, e, app);
    for at in [0x5cu32, 0x60] {
        let f = m.r32(m.r32(GAPP) + at);
        e.call(m, setup::FONT_RELEASE, &[f]);
    }
    renderer_call(m, e, 0x148, &[]);
    e.call(m, setup::LESSON_STOP, &[setup::LESSON]);
    e.call(m, setup::LESSON_RESET, &[setup::LESSON]);
    e.call(m, setup::LISTS_CLEAR, &[SOUND_BANK]);
    e.call(m, setup::POOL_CLEAR, &[0x345e48]);
    let app = m.r32(GAPP);
    m.w32(app + 0x734, 0);
    e.call(m, setup::STACK_DTOR, &[fe + 0xa8, 2]);
    e.call(m, setup::DRAWN_DTOR, &[fe + 0x80, 2]);
    e.call(m, setup::LIGHTS_DTOR2, &[fe + 0x70, 2]);
    crate::game_main::module_dtor(m, e, fe, flags);
}

/// `cvt.s.w`: toward zero
fn int_float(v: u32) -> F {
    let v = v as i32;
    let mut f = v as f32;
    if (f as f64).abs() > (v as f64).abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    F(f)
}

/// An unsigned word as a float (`cvt.s.w` of half with the low bit kept, doubled, past 2^31)
fn unsigned_float(v: u32) -> F {
    if (v as i32) >= 0 {
        int_float(v)
    } else {
        let h = int_float((v >> 1) | (v & 1));
        h + h
    }
}

/// `cFE3D` vf4 0x2637a0(fe), the start: the renderer's +0x120 with the career's +0x1684, the viewport over the
/// whole display, its +0x140; the twelve models (+0x20..) loaded, waited on frame by frame (each frame their +0x68)
/// and finished; the scene at +0xc closed (+0x18), destroyed and cleared; both players' +0x4c 10. After a career's
/// first start (+0x3260 1) the two boot films (0x160 bytes each, video screens) made, the intro's first screen the
/// logo's, the logo pushed and +0x3260 2; otherwise the first screen (0x3471fc) pushed. +8 1
pub fn fe_start(m: &mut Mem, e: &mut dyn FeEnv, fe: u32) {
    let v = m.r32(CAREER + 0x1684);
    renderer_call(m, e, 0x120, &[v]);
    let w = unsigned_float(renderer_call(m, e, 0x58, &[]));
    let h = unsigned_float(renderer_call(m, e, 0x60, &[]));
    renderer_call(m, e, 0x98, &[0, 0, w.0.to_bits(), h.0.to_bits()]);
    renderer_call(m, e, 0x140, &[]);
    for k in 0..12u32 {
        let at = fe + 0x20 + 4 * k;
        let o = m.r32(at);
        e.call(m, setup::MODEL_LOAD, &[o]);
        loop {
            let o = m.r32(at);
            // (the first test's a1 is whatever was left there; the rest 0)
            if e.call(m, setup::MODEL_LOADED, &[o]) != 0 {
                break;
            }
            e.call(m, setup::FRAME_WAIT, &[0]);
            let o = m.r32(at);
            vcall_at(m, e, o, 0xc, 0x68, &[]);
        }
        let o = m.r32(at);
        e.call(m, setup::MODEL_FINISH, &[o, 0]);
    }
    let sc = m.r32(fe + 0xc);
    vcall_at(m, e, sc, 0x10, 0x18, &[]);
    let sc = m.r32(fe + 0xc);
    destroy_at(m, e, sc, 0x10);
    m.w32(fe + 0xc, 0);
    for k in 0..2u32 {
        let p = m.r32(fe + 0x74 + 4 * k);
        m.w32(p + 0x4c, 10);
    }
    let first = m.r32(setup::FIRST);
    if m.r32(CAREER + 0x3260) == 1 {
        let film = |m: &mut Mem, e: &mut dyn FeEnv, name: u32| {
            let v = alloc(m, e, setup::FILM_NAME, 0x160, 0x10);
            e.call(m, BASE_CTOR, &[v]);
            m.w32(v + 0x154, 0);
            m.w32(v + 0x158, 1);
            m.w32(v + 0xc, VIDEO_VTABLE);
            m.w32(v + 0x15c, 0);
            e.call(m, SET_FILM, &[v, name]);
            v
        };
        let logo = film(m, e, setup::FILMS[0]);
        let first = m.r32(setup::FIRST);
        m.w32(logo + 0x15c, first);
        vcall_at(m, e, logo, 0xc, 0x60, &[]);
        let intro = film(m, e, setup::FILMS[1]);
        m.w32(fe + 0xf8, intro);
        let v = m.r32(fe + 0xf8);
        e.call(m, setup::FIRST_SCREEN, &[v, 0]);
        let v = m.r32(fe + 0xf8);
        m.w32(v + 0x15c, logo);
        let v = m.r32(fe + 0xf8);
        vcall_at(m, e, v, 0xc, 0x60, &[]);
        let v = m.r32(fe + 0xf8);
        e.call(m, STACK_PUSH, &[fe + 0xa8, v]);
        m.w32(CAREER + 0x3260, 2);
    } else {
        e.call(m, STACK_PUSH, &[fe + 0xa8, first]);
    }
    m.w32(fe + 8, 1);
}
