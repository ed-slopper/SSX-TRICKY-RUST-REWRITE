//! `tricky_game::fe_scene`'s dialog box and group extras (board row G9z3) against the original in the function
//! runner (TRICKY_ELF; skipped without it): `cFE3DDialog`'s constructor, frame, layouts, renders, update,
//! accessors and destructor, the group's extent and destructor, and two memory helpers. Every case builds a random
//! dialog (its flags, sprites, edge, box and colours), renderer and pool in the arena; one function then runs on
//! both sides. The group's free of its children (a copy, checked in `tests/fe_wgroup.rs`) runs for real on both
//! sides; every other callee is a stand-in answering from the case's seed: the sprite rectangles, the renderer's
//! state, the list's operations (whose words on the stack are logged by value), the destructors and frees, and the
//! objects' virtuals (a table of stand-ins; the dialog class's size from the game's table). The calls are compared
//! with their arguments, the answers and the memory after.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP, WIDGET_PLACE};
use tricky_game::fe_scene as sc;
use tricky_game::fe_widgets::{LIST_INIT, UPDATE_COLOR};
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const DATA: (u32, u32) = (0x337c00, 0x400);
/// the game's words: the dialog's table; the pool's base and size; the scene's colours
const GAME: [(u32, u32); 1] = [(0x3a9d00, 0x200)];
const POOL: (u32, u32) = (0x345e00, 0x100);
const COLOURS: (u32, u32) = (0x3d2e00, 0x200);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x3000;
const APPOBJ: u32 = ARENA + 0x800;
const VT: u32 = ARENA + 0x1000;
const R: u32 = ARENA + 0x1400;
const D: u32 = ARENA + 0x1800;
const RECTS: u32 = ARENA + 0x2000;
const VEC: u32 = ARENA + 0x2100;
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn slot(off: u32) -> u32 {
    0x0181_0000 + off
}
const SLOTS: [u32; 5] = [0x48, 0x50, 0x138, 0x1d0, 0x1d8];
/// the group's free of its children (a copy of `FE3DWGroup_FreeChildren4`)
const FREE_CHILDREN: u32 = 0x2691c8;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
}
fn stack(a: u32) -> bool {
    (0x01fe_c000..0x01ff_0000).contains(&a) || (SCRATCH..SCRATCH + 0x100).contains(&a)
}
fn token(a: u32) -> u32 {
    if (0x01f0_0000..0x01ff_0000).contains(&a) { STACK_TOKEN } else { a }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
    /// the dialog class's size, read from the game's table
    class_size: Cell<u32>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

fn arity(f: u32, ans: &Answers) -> (usize, usize) {
    match f {
        sc::SPRITE_DRAW => (2, 4),
        sc::VEC4_SET => (1, 4),
        sc::FILL => (3, 0),
        sc::POOL_FREE | sc::SPRITE_RECT | sc::FONT_SET_COLOUR | sc::LIST_APPLY | sc::LIST_DTOR | sc::WIDGET_DTOR
        | WIDGET_PLACE | sc::EXTENT_OP_DTOR | sc::UPDATE_OP_DTOR | sc::RENDER_TEXT_OP_DTOR | sc::RENDER_OP_DTOR => {
            (2, 0)
        }
        f if f == slot(0x1d8) || f == slot(0x48) => (1, 0),
        f if f & 0xffff_0000 == 0x0181_0000 || f == ans.class_size.get() => (2, 0),
        _ => (1, 0),
    }
}

fn standin(io: &mut dyn Io, f: u32, w: &[u32], fl: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(f, ans);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&fl[..nf]);
    // (a vector or an operation on the stack by value)
    for &a in w[..ni].iter() {
        if stack(a) {
            log.extend((0..4).map(|k| token(io.r32(a + 4 * k))));
        }
    }
    let r = ans.next();
    let answer = match f {
        sc::SPRITE_RECT => RECTS + 0x10 * (r % 4),
        f if f == slot(0x1d8) => r % 8,
        _ => r,
    };
    (answer, Call(f, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl FeEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, f: u32, args: &[u32]) -> u32 {
        let (ni, nf) = arity(f, self.ans);
        let mut w: Vec<u32> = args.iter().take(ni).copied().collect();
        w.resize(8, 0);
        let mut fl: Vec<u32> = args.iter().skip(ni).take(nf).copied().collect();
        fl.resize(4, 0);
        let (a, c) = standin(m, f, &w, &fl, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

const FUNCS: [u32; 26] = [
    0x267f08, 0x267f18, 0x267f58, 0x268308, 0x2686c0, 0x268808, 0x268940, 0x268a80, 0x268bd0, 0x268c38, 0x268ce0,
    0x268cf8, 0x268d00, 0x268d08, 0x268d10, 0x268d38, 0x268d60, 0x268d70, 0x268d80, 0x268d88, 0x268d90, 0x268db0,
    0x268dd0, 0x268e38, 0x2691f8, 0x2692e8,
];

/// One function on our side; its answer (v0, or f0 for the edge) where it has one
fn ours(m: &mut Mem, e: &mut dyn FeEnv, f: u32, a: [u32; 4], f12: u32) -> Option<u32> {
    let buf = SCRATCH;
    Some(match f {
        0x267f08 => return { sc::zero_108(m, e, a[0]); None },
        0x267f18 => return { sc::free_block(m, e, a[0]); None },
        0x267f58 => sc::dialog_ctor(m, e, a[0], a[1], a[2], buf),
        0x268308 => return { sc::dialog_draw_frame(m, e, a[0]); None },
        0x2686c0 => return { sc::dialog_layout_boxed(m, e, a[0], f12, buf); None },
        0x268808 => return { sc::dialog_layout_bar(m, e, a[0], f12, buf); None },
        0x268940 => return { sc::dialog_render(m, e, a[0], a[1], buf); None },
        0x268a80 => return { sc::dialog_render_text(m, e, a[0], buf); None },
        0x268bd0 => return { sc::dialog_update(m, e, a[0], buf); None },
        0x268c38 => return { sc::scene_colours_init(m); None },
        0x268ce0 => return { sc::vec4_set_from(m, e, a[0], a[1]); None },
        0x268cf8 | 0x268e38 => 1,
        0x268d00 => return { sc::dialog_set_edge(m, a[0], f12); None },
        0x268d08 => sc::dialog_edge(m, a[0]),
        0x268d10 => return { sc::dialog_set_corner_colour(m, a[0], a[1]); None },
        0x268d38 => sc::dialog_corner_colour(m, a[0], a[1]),
        0x268d60 => return { sc::dialog_set_sprite(m, a[0], a[1], a[2]); None },
        0x268d70 => sc::dialog_sprite(m, a[0], a[1]),
        0x268d80 => return { sc::dialog_set_sprite(m, a[0], 0, a[1]); None },
        0x268d88 => sc::dialog_sprite(m, a[0], 0),
        0x268d90 => sc::dialog_has_bottom(m, a[0]) as u32,
        0x268db0 => sc::dialog_has_top(m, a[0]) as u32,
        0x268dd0 => return { sc::dialog_dtor(m, e, a[0], a[1]); None },
        0x2691f8 => return { sc::group_extent(m, e, a[0], a[1], a[2], buf); None },
        0x2692e8 => return { sc::group_dtor(m, e, a[0], a[1]); None },
        _ => unreachable!("{f:#x}"),
    })
}

#[test]
fn dialog_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=c").unwrap();
    for f in FUNCS {
        r.allow(f);
    }
    r.allow(FREE_CHILDREN);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0), class_size: Cell::new(0) });
    let game: Vec<(u32, Vec<u8>)> = GAME
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            (at, b)
        })
        .collect();
    let word = |at: u32| {
        let (base, b) = game.iter().find(|(base, b)| (*base..*base + b.len() as u32).contains(&at)).unwrap();
        u32::from_le_bytes(b[(at - base) as usize..][..4].try_into().unwrap())
    };
    ans.class_size.set(word(sc::DIALOG_VTABLE + 0x138 + 4));
    let mut stubs = vec![
        sc::FILL, sc::POOL_FREE, sc::HEAP_FREE, UPDATE_COLOR, LIST_INIT, sc::SPRITE_RECT, sc::SPRITE_DRAW,
        sc::FONT_SET_COLOUR, sc::VEC4_SET, sc::LIST_APPLY, sc::EXTENT_OP_DTOR, sc::UPDATE_OP_DTOR,
        sc::RENDER_TEXT_OP_DTOR, sc::RENDER_OP_DTOR, sc::LIST_DELETE_ALL, sc::THING_UNLINK, sc::LIST_DTOR,
        sc::WIDGET_DTOR, WIDGET_PLACE, ans.class_size.get(),
    ];
    stubs.extend(SLOTS.map(slot));
    for f in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(f, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let fl: Vec<u32> = (12..16).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, f, &w, &fl, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let mut data = vec![0u8; DATA.1 as usize];
    r.mem.read(DATA.0, &mut data).unwrap();
    let mut g = Rng(0x9e5e_0000_0000_0001);
    let per = 400u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut regions = vec![(DATA.0, data.clone())];
        regions.extend(game.iter().cloned());
        regions.extend([
            (POOL.0, vec![0; POOL.1 as usize]),
            (COLOURS.0, vec![0; COLOURS.1 as usize]),
            (ARENA, vec![0; SIZE as usize]),
            (SCRATCH, vec![0; 0x100]),
        ]);
        let mut m = Mem { regions };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..COLOURS.1 / 4 {
            m.w32(COLOURS.0 + 4 * k, g.next());
        }
        m.w32(GAPP, APPOBJ);
        m.w32(APPOBJ + 0x724, R);
        m.w32(R + 0x10, VT);
        for off in SLOTS {
            m.w16(VT + off, g.pick(&[0u16, 4]));
            m.w32(VT + off + 4, slot(off));
        }
        // (the pool: a base and a size, the block inside it, at its base or outside)
        m.w32(sc::POOL, 0x0130_0000);
        m.w32(sc::POOL + 4, 0x1000);
        // (the dialog: shown and active either way, its sprites, edge, box, lift)
        m.w32(D + 0xc, VT);
        m.w32(D + 0x10, g.below(2));
        m.w32(D + 0x14, g.below(2));
        m.w32(D + 0x70, g.below(4));
        for k in 0..5 {
            m.w32(D + 0x140 + 4 * k, g.pick(&[0xffff_ffffu32, 0x11, 3, 0]));
        }
        m.w32(D + 0x154, if g.below(4) == 0 { 0 } else { g.f(1.0, 40.0).to_bits() });
        for at in [0x80u32, 0x84, 0x100, 0x104] {
            m.w32(D + at, g.f(-50.0, 500.0).to_bits());
        }
        for k in 0..16 {
            m.w32(RECTS + 4 * k, g.f(1.0, 64.0).to_bits());
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        let block = g.pick(&[0x0130_0000u32, 0x0130_0010, 0x0130_0ff0, 0x0130_1000, 0x012f_fff0, 0x0100_0000]);
        let a = match func {
            0x267f18 => [block, 0, 0, 0],
            0x268ce0 => [VEC, VEC + 0x20, 0, 0],
            0x268d10 | 0x268d38 => [if func == 0x268d38 { VEC } else { D }, if func == 0x268d38 { D } else { VEC }, 0, 0],
            0x268d60 | 0x268d70 => [D, g.below(5), g.next() & 0xff_ffff, 0],
            0x2691f8 => [D, VEC, VEC + 0x20, 0],
            // (words that are not pointers kept clear of the stack)
            _ => [D, g.next() & 0xff_ffff, g.next() & 0xff_ffff, g.next() & 0xff_ffff],
        };
        let f12 = g.f(10.0, 470.0).to_bits();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let mine = ours(&mut m, &mut env, func, a, f12);
        ans.n.set(0);
        log.borrow_mut().clear();
        for (k, v) in a.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, f32::from_bits(f12));
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if let Some(v) = mine {
            let theirs = if func == 0x268d08 { r.cpu.f(0).to_bits() } else { r.cpu.gpr(2) as u32 };
            assert_eq!(theirs, v, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
}
