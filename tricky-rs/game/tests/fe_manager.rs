//! `tricky_game::fe_manager` (board row G9b2d1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random front-end managers (the state, players, picks, idle count, attract film, the screen stack's
//! top, the career's game words, the pads), every function run on both sides with the same answers from the calls out
//! (the renderer, the scene, the video manager, the screens, the pads, the sound bank, the music and the game's
//! constructors; float arguments logged from f12..f17), the calls logged in order, the answers and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, ALLOC, FILL, GAPP};
use tricky_game::fe_charselect::SEEN;
use tricky_game::fe_manager::*;
use tricky_game::fe_riderprofile::SET_FILM;
use tricky_game::fe_title::BASE_CTOR;
use tricky_game::fe_trickbook::CLEAR_SLOT;
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
const FE: u32 = ARENA + 0x1000;
const REND: u32 = ARENA + 0x2000;
const SCENE: u32 = ARENA + 0x2400;
const VIDEO: u32 = ARENA + 0x2800;
const P0: u32 = ARENA + 0x2c00;
const P1: u32 = ARENA + 0x2d00;
const TOP: u32 = ARENA + 0x3000;
const FILM: u32 = ARENA + 0x3400;
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const HEAP: u32 = ARENA + 0x8000;
const V_BASE: u32 = 0x7f_0000;
const SCRATCH: u32 = 0x5c5c_5c5c;
const CAREER: u32 = 0x32b390;

fn slot(off: u32) -> u32 {
    V_BASE + off + 4
}

/// The integer registers and the float registers (from f12) a call's arguments are logged from
fn shape(t: u32) -> (usize, usize) {
    match t {
        t if t == slot(0xb0) => (1, 6),
        t if t == slot(0x1c8) => (3, 1),
        t if t == slot(0x1f8) || t == slot(0x1e8) => (3, 0),
        t if t == slot(0x1e0) || t == slot(0x1d0) || t == slot(0x218) => (2, 0),
        MUSIC_TICK => (2, 1),
        ALLOC | FILL | 0x163468 => (3, 0),
        SEEN => (4, 0),
        SOUND_UPDATE | SOUND_DRAW | STACK_PUSH | GAME_HAND | SET_FILM | CLEAR_SLOT => (2, 0),
        t if t == GAMES[0].0 => (2, 0),
        _ => (1, 0),
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) | 1
}

struct Calls {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
type A = Rc<RefCell<Calls>>;

fn log_call(a: &A, target: u32, ints: Vec<u32>, floats: Vec<u32>) -> u32 {
    let mut v = ints;
    v.extend(floats);
    for x in v.iter_mut() {
        if (0x01f0_0000..0x01ff_0000).contains(x) || (BUF..BUF + 0x100).contains(x) {
            *x = SCRATCH;
        }
    }
    let mut b = a.borrow_mut();
    b.log.push((target, v));
    let n = match b.count.iter_mut().find(|c| c.0 == target) {
        Some(c) => {
            c.1 += 1;
            c.1
        }
        None => {
            b.count.push((target, 0));
            0
        }
    };
    // (a multiplying mix: xorshift alone keeps successive answers of one call in a fixed relation)
    let mut g = Rng(mix(b.seed ^ (target as u64) << 16 ^ n as u64));
    match target {
        ALLOC => {
            if g.below(8) == 0 {
                0
            } else {
                HEAP + 0x400 * (n % 16)
            }
        }
        CHARSELECT_CTOR => HEAP + 0x4000,
        t if t == slot(0x70) => [0u32, 0, 1, 0xc][g.below(4) as usize],
        _ => g.below(2),
    }
}

struct Ours(A);
impl FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let (ni, nf) = shape(target);
        let ints: Vec<u32> = (0..ni).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let floats: Vec<u32> = (0..nf).map(|k| args.get(ni + k).copied().unwrap_or(0)).collect();
        let _ = m;
        log_call(&self.0, target, ints, floats)
    }
}

const REGIONS: [(u32, u32); 9] = [
    (0x3b5000, 0x100),
    (0, 0x600),
    (ARENA, 0x8000),
    (HEAP, 0x8000),
    (0x337c40, 0x40),
    (0x337fe0, 0x20),
    (0x32b390, 0x3800),
    (0x3470b0, 0x160),
    (0x3d2df0, 0x60),
];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    for at in [0x32e5f0u32, 0x343050, 0x346488] {
        let mut b = vec![0u8; 0x10];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

#[test]
fn manager_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Calls { seed: 0, count: vec![], log: vec![] }));
    let mut leaves = vec![
        ALLOC, FILL, BASE_CTOR, SET_FILM, CLEAR_SLOT, SEEN, TRANSITION_UPDATE, FADE_DRAW, CHEATS, DRAWN_UPDATE,
        DRAWN_DRAW_COURSE, DRAWN_DRAW, SOUND_UPDATE, SOUND_DRAW, PAD_CONNECTED, STACK_CLOSE, STACK_PUSH, MUSIC_TICK,
        GAME_HAND, CAMERA, COURSE_DRAW, COURSE_DRAW2, CHARSELECT_CTOR, 0x163468, 0x163980, FREE,
        // (the video screen's +0x60 behind its vtable 0x3b5010)
        0x2ab910,
    ];
    leaves.extend(GAMES.iter().map(|g| g.0));
    for off in (0..0x300).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    for f in leaves {
        let a2 = a.clone();
        r.stub(f, move |c, _mem| {
            let (ni, nf) = shape(f);
            let ints: Vec<u32> = (0..ni).map(|k| c.gpr(if k < 4 { 4 + k } else { 8 + k - 4 }) as u32).collect();
            let floats: Vec<u32> = (0..nf).map(|k| c.f(12 + k).to_bits()).collect();
            let ans = log_call(&a2, f, ints, floats);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    // the lists' last (0x2ac030) and the screen's player bit (0x271888) run as the original, and the manager's own
    // helpers its functions call
    for f in [0x2ac030u32, 0x271888, 0x263670, 0x265870, 0x265970, 0x265af8, 0x265788] {
        r.allow(f);
    }
    let methods: [u32; 28] = [
        0x263670, 0x263750, 0x264fd8, 0x265398, 0x265758, 0x265788, 0x2657a8, 0x265870, 0x2658e8, 0x265970, 0x265a58,
        0x265af8, 0x265b10, 0x265bb8, 0x265bc8, 0x265be8, 0x265bf0, 0x265bf8, 0x265c00, 0x265c10, 0x265c28, 0x265c50,
        0x265c58, 0x265c68, 0x265c70, 0x265c78, 0x265c88, 0x265c98,
    ];
    let mut g = Rng(0x2635_0000_0000_0001);
    let mut counts = [0u32; 28];
    for case in 0..40_000u32 {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(2) == 0 { 0 } else { g.below(4) });
        }
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(HEAP + 4 * k, 0x5a5a_5a5a ^ k);
        }
        r.mem.write(0, &[0u8; 0x600]).unwrap();
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x724, REND);
        r.mem.write_u32(APP + 0x72c, g.next());
        r.mem.write_u32(REND + 0x10, VT);
        r.mem.write_u32(SCENE, VT);
        r.mem.write_u32(VIDEO + 8, VT);
        for off in (0..0x300).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let fe = FE;
        r.mem.write_u8(fe + 4, [0u8, 1, 2, 3, 0xff][g.below(5) as usize]);
        r.mem.write_u8(fe + 6, [0u8, 0, 1, 2][g.below(4) as usize]);
        r.mem.write_u32(fe + 0x74, P0);
        r.mem.write_u32(fe + 0x78, P1);
        for p in [P0, P1] {
            r.mem.write_u32(p, g.below(2));
            r.mem.write_u8(p + 0x44, [0u8, 1, 0xff][g.below(3) as usize]);
        }
        r.mem.write_u32(fe + 0xa4, SCENE);
        r.mem.write_u32(fe + 0x124, VIDEO);
        r.mem.write_u32(fe + 0xf4, [0u32, 5, 0xe0f, 0xe10, 0xe11, 0x2000][g.below(6) as usize]);
        r.mem.write_u32(fe + 0xf8, [0, 0, FILM][g.below(3) as usize]);
        r.mem.write_u32(FILM + 0xc, VT);
        r.mem.write_u32(TOP + 0xc, VT);
        // the screen stack (+0xa8): empty, or one screen on it
        let (l, t) = (fe + 0xa8, fe + 0xb8);
        r.mem.write_u32(l + 4, l);
        r.mem.write_u32(t + 8, t);
        // (the update always has a screen on the stack)
        let which = g.below(28) as usize;
        let addr = methods[which];
        if g.below(4) == 0 && addr != 0x265398 {
            r.mem.write_u32(l + 8, t);
            r.mem.write_u32(t + 4, l);
        } else {
            r.mem.write_u32(l + 8, TOP);
            r.mem.write_u32(TOP + 4, l);
            r.mem.write_u32(TOP + 8, t);
            r.mem.write_u32(t + 4, TOP);
        }
        r.mem.write_u32(CAREER + 0x3710, g.below(3));
        r.mem.write_u32(CAREER + 0x2afc, [0u32, 1, 6, 7, 9][g.below(5) as usize]);
        r.mem.write_u32(0x32e5f0, g.below(8));
        r.mem.write_u32(0x343050, g.next());
        r.mem.write_u32(0x346488, g.below(2));
        for k in 0..12 {
            r.mem.write_u8(0x3470b8 + k, [g.below(14) as u8, 0xff, 0x80][g.below(3) as usize]);
        }
        // (the alternating value's bytes, past 0x7f too)
        r.mem.write_u8(0x3471f8, g.next() as u8);
        r.mem.write_u8(0x347200, g.next() as u8);
        counts[which] += 1;
        let a1 = match addr {
            0x263670 => [0u32, 1, 2, 3, 0xff][g.below(5) as usize],
            0x265788 | 0x2658e8 => [0u32, 3, 11, 12, 0xff][g.below(5) as usize],
            0x265bc8 => g.below(4),
            0x265c00 | 0x265c10 => g.below(2),
            0x265c58 => BUF,
            _ => g.next(),
        };
        let a2 = g.next();
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        r.cpu.set_gpr(4, fe as u64);
        r.cpu.set_gpr(5, a1 as i32 as i64 as u64);
        r.cpu.set_gpr(6, a2 as i32 as i64 as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone());
        let mine_ret: Option<u32> = match addr {
            0x263670 => {
                set_players(&mut m, e, fe, a1);
                None
            }
            0x263750 => Some(vf2(&mut m, e, fe)),
            0x264fd8 => Some(draw(&mut m, e, fe, BUF)),
            0x265398 => {
                update(&mut m, e, fe);
                None
            }
            0x265758 => Some(alternate(&mut m)),
            0x265788 => Some(order(&m, a1)),
            0x2657a8 => {
                set_game(&mut m, e, fe);
                None
            }
            0x265870 => Some(pads_in(&mut m, e, fe)),
            0x2658e8 => Some(place_of(&m, a1)),
            0x265970 => Some(make_character_select(&mut m, e, fe, a1)),
            0x265a58 => Some(after_players(&mut m, e, fe, a1)),
            0x265af8 => Some(pads_rule(&m)),
            0x265b10 => {
                set_lights(&mut m);
                None
            }
            0x265bb8 => {
                player_reset(&mut m, fe);
                None
            }
            0x265bc8 => {
                free_if(&mut m, e, fe, a1);
                None
            }
            0x265be8 => {
                idle_reset(&mut m, fe);
                None
            }
            0x265bf0 => Some(game_type(&m, fe)),
            0x265bf8 => {
                set_game_type(&mut m, fe, a1);
                None
            }
            0x265c00 => Some(pick(&m, fe, a1)),
            0x265c10 => {
                set_pick(&mut m, fe, a1, a2);
                None
            }
            0x265c28 => Some(two_pads(&mut m, e, fe)),
            0x265c50 => Some(vector_at(fe)),
            0x265c58 => {
                set_vector(&mut m, fe, a1);
                None
            }
            0x265c68 => Some(vf3(&m, fe)),
            0x265c70 => Some(state(&m, fe)),
            0x265c78 => {
                leave(&mut m, fe);
                None
            }
            0x265c88 => {
                set_byte5(&mut m, fe, a1);
                None
            }
            _ => Some(players(&m, fe)),
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        assert_eq!(theirs, mine, "case {case}: {addr:#x} calls");
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x100).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by function: {counts:?}");
}
