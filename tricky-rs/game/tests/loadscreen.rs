//! `tricky_game::loadscreen` (board row G9w1) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the loading screens' tables and
//! strings at 0x3a4b00.., as the ELF loads them) with a screen in the arena: built by the port's `init` (or random),
//! its fields made random where the functions read them (the step, the fade, the pictures' list, the files), the
//! renderer and the screen's own table pointing at stand-ins. Then one of the row's functions runs on both sides.
//! Every callee is a stand-in on both sides answering from the case's seed (a file's out words written, the fills
//! done for real); the draws log their float arguments and the colour stand-in the four words it is given. The
//! runner's stack is cleared before each case and a buffer on it logs as one token. The calls, the answers and the
//! memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::loadscreen as ls;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::TextBoxEnv;

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

const DATA: (u32, u32) = (0x328000, 0x18000);
const RODATA: (u32, u32) = (0x3a4b00, 0xa00);
/// The speech's pointer (the sound banks are on it)
const SPEECH_AT: (u32, u32) = (0x343000, 0x100);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x5000;
const THIS: u32 = ARENA;
const RENDER: u32 = ARENA + 0x1800;
const RVT: u32 = ARENA + 0x1900;
const SVT: u32 = ARENA + 0x1b00;
const APPOBJ: u32 = ARENA + 0x1c00;
const PICS: u32 = ARENA + 0x1d00;
const SHEETS: u32 = ARENA + 0x2000;
/// The sprite set (the screen's +4: its +0x15e8.. sheets)
const SET: u32 = ARENA + 0x3000;
/// The packed pictures' buffer
const HEAP: (u32, u32) = (0x0110_0000, 0x12000);
const SCRATCH: u32 = 0x01f8_0000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
/// The renderer's and the screen's own virtuals' stand-ins
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
fn sv(off: u32) -> u32 {
    0x0180_0000 + off
}
const ROFFS: [u32; 25] = [
    0x148, 0x138, 0x150, 0x140, 0x68, 0x1d0, 0x1f0, 0x1f8, 0x70, 0x1a0, 0x88, 0xd8, 0x58, 0x60, 0x98, 0x1e0, 0x1c0,
    0xb0, 0x108, 0xe0, 0x90, 0x1a8, 0x80, 0x1e8, 0x1d8,
];
const SOFFS: [u32; 4] = [0x40, 0x48, 0x50, 0x58];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn w8(&mut self, a: u32, v: u8);
    fn w32(&mut self, a: u32, v: u32);
    fn r32(&mut self, a: u32) -> u32;
}
impl Io for Mem {
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
}
impl Io for r5900::Mem {
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write(a, &v.to_le_bytes()).unwrap()
    }
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
}
fn mapped(a: u32, n: u32) -> bool {
    [DATA, RODATA, SPEECH_AT, (ARENA, SIZE), HEAP].iter().any(|&(at, s)| a >= at && a as u64 + n as u64 <= (at + s) as u64)
}
fn stack(a: u32) -> bool {
    (0x01f0_0000..0x01ff_0000).contains(&a)
}
fn token(a: u32) -> u32 {
    if stack(a) { STACK_TOKEN } else { a }
}

struct Answers {
    seed: Cell<u64>,
    n: Cell<u64>,
}
impl Answers {
    fn next(&self) -> u32 {
        self.n.set(self.n.get() + 1);
        Rng(self.seed.get() ^ self.n.get().wrapping_mul(0x1234_5678_9abc_def1)).next()
    }
}

/// How many int and float arguments a callee has (`this` counted for a virtual)
fn arity(func: u32) -> (usize, usize) {
    if func & 0xffff_0000 == 0x01ff_0000 {
        return match func & 0xffff {
            0x150 => (4, 0),
            0x138 | 0x1d0 | 0x1e0 | 0x1c0 | 0x68 => (2, 0),
            0x1f0 | 0x1f8 => (3, 0),
            0x98 => (1, 4),
            0xb0 => (1, 6),
            _ => (1, 0),
        };
    }
    if func & 0xffff_0000 == 0x0180_0000 {
        return (1, 0);
    }
    match func {
        ls::FONT_INIT | ls::FONT_DELETE | ls::ANIM_DELETE | ls::ARCHIVE_DELETE | ls::LOAD_MODULE_DELETE => (2, 0),
        ls::FONT_FREE | ls::ARCHIVE_INIT | ls::ARCHIVE_CLOSE | ls::ANIM_INIT | ls::ANIM_UPDATE => (1, 0),
        ls::FONT_PARSE => (4, 0),
        ls::ARCHIVE_OPEN => (3, 0),
        ls::ANIM_ADD_BAR => (7, 0),
        ls::ANIM_EVAL => (2, 0),
        ls::FILE_LOAD => (2, 0),
        ls::FILE_IS_DONE | ls::FILE_CANCEL | ls::UNPACKED_SIZE | ls::EA_FREE | ls::MEM_FREE => (1, 0),
        ls::FILE_FINISH => (3, 0),
        ls::SHAPE_FIND | ls::SHAPE_ENTRY | ls::DECODE => (2, 0),
        ls::EA_ALLOC | ls::MEMSET => (3, 0),
        ls::SET_COLOUR => (2, 0),
        ls::SPRITE_RECT => (2, 4),
        ls::FILLED_RECT => (1, 4),
        ls::DRAW_RIGHT | ls::PRINTF_SHADOWED_A => (2, 3),
        ls::SPRITE_TILED => (2, 6),
        ls::SPRITE_ANCHORED => (2, 4),
        ls::SET_FADE => (1, 1),
        ls::LOC => (2, 0),
        ls::SPRINTF | ls::LOAD_ICONS | ls::WSTR_FORMAT | ls::WSTR_FORMAT_ASCII | ls::EVENT_MEDAL => (3, 0),
        ls::BANK_LOAD | ls::BANK_POLL | ls::BANK_UNLOAD | ls::SPRITES_INIT | ls::SPRITES_POLL | ls::WSTR_COUNT => (1, 0),
        ls::COMPLETE_PAGES | ls::TS_INIT | ls::TS_CONSTRUCT => (1, 0),
        ls::LOOP_PLAY | ls::LOOP_STOP | ls::WSTR_COPY | ls::WSTR_SET_ASCII | ls::PLAYER_ID | ls::PLAYER_PROFILE => (2, 0),
        ls::LEVEL_NAME | ls::TRICK_BOOK | ls::COURSE_NAME | ls::COURSE_DIFFICULTY | ls::COURSE_LOCATION => (2, 0),
        ls::COURSE_DROP | ls::COURSE_LENGTH | ls::MAP_X | ls::MAP_Y | ls::ANIM_VALUE | ls::TS_SET_FONT => (2, 0),
        ls::TS_DESTRUCT_THUNK | ls::TS_DESTRUCT => (2, 0),
        ls::DRAW_ALIGNED | ls::DRAW_CURSOR => (6, 3),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    if func == ls::SET_COLOUR {
        log.extend((0..4).map(|k| io.r32(w[1] + 4 * k)));
    }
    if func == rv(0x68) {
        log.extend((0..3).map(|k| io.r32(w[1] + 4 * k)));
    }
    // the draws: the drawer's colour as it stands when they draw
    if matches!(
        func,
        ls::SPRITE_RECT | ls::FILLED_RECT | ls::SPRITE_TILED | ls::SPRITE_ANCHORED | ls::DRAW_RIGHT | ls::PRINTF_SHADOWED_A | ls::DRAW_ALIGNED | ls::DRAW_CURSOR
    ) {
        log.extend((0..4).map(|k| io.r32(ls::DRAW_COLOUR + 4 * k)));
    }
    if func == ls::DRAW_ALIGNED || func == ls::DRAW_CURSOR {
        // (the text style's +4, set by hand)
        log.push(io.r32(w[0] + 4));
    }
    let r = ans.next();
    let answer = match func {
        ls::FILE_IS_DONE => [1u32, 1, 0, 2][(r % 4) as usize],
        // (the renderer's "can draw": 0, 1 or else)
        f if f == rv(0x70) => r % 3,
        ls::FILE_LOAD => if r % 5 == 0 { 0 } else { r | 1 },
        ls::FILE_FINISH => {
            // the buffer read: an address the port and the game pass on
            if stack(w[1]) || mapped(w[1], 4) {
                io.w32(w[1], r | 0x10);
            }
            if w[2] != 0 && (stack(w[2]) || mapped(w[2], 4)) {
                io.w32(w[2], r >> 4);
            }
            0
        }
        ls::UNPACKED_SIZE => r % 2,
        ls::BANK_POLL | ls::SPRITES_POLL => [0u32, 1, 1, 7][(r % 4) as usize],
        ls::PLAYER_PROFILE => r % 12,
        // (six pages in the book; 7 too, past it)
        ls::COMPLETE_PAGES => r % 8,
        ls::EVENT_MEDAL => r % 4,
        ls::MAP_X | ls::MAP_Y => ((r % 1000) as f32 / 1000.0).to_bits(),
        ls::EA_ALLOC => if r % 4 == 0 { 0 } else { HEAP.0 },
        ls::MEMSET => {
            if mapped(w[0], w[2]) {
                for k in 0..w[2] {
                    io.w8(w[0] + k, w[1] as u8);
                }
            }
            0
        }
        // a float (the bars' lines' y), its bits
        ls::ANIM_EVAL => (r % 480) as f32 as u32,
        _ => r,
    };
    let answer = if func == ls::ANIM_EVAL { ((r % 480) as f32).to_bits() } else { answer };
    (answer, Call(func, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl TextBoxEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(8, 0);
        let (a, c) = standin(m, func, &w, &f, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn loading_screens_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x243c78u32, 0x243de8, 0x243ea0, 0x243f00, 0x244290, 0x244330, 0x2443b0, 0x244490, 0x2444c0, 0x244540, 0x2447a8,
        0x2449f8, 0x244b20, 0x244ee8, 0x244fc8, 0x245598, 0x2455c0, 0x245668, 0x245680, 0x2456b8, 0x245760, 0x245838,
        0x245840, 0x245958,
        0x2459b0, 0x245a48, 0x245b68, 0x245cb8, 0x245ec0, 0x246700, 0x246f18, 0x246f50, 0x246f88, 0x246fe0, 0x2473d8,
    ];
    for f in funcs.into_iter().chain([0x250e98]) {
        r.allow(f);
    }
    let mut stubs = vec![
        ls::FONT_INIT, ls::FONT_FREE, ls::FONT_DELETE, ls::FONT_PARSE, ls::ARCHIVE_INIT, ls::ARCHIVE_DELETE,
        ls::ARCHIVE_OPEN, ls::ARCHIVE_CLOSE, ls::ANIM_INIT, ls::ANIM_DELETE, ls::ANIM_ADD_BAR, ls::ANIM_UPDATE,
        ls::ANIM_EVAL, ls::LOAD_MODULE_DELETE, ls::FILE_LOAD, ls::FILE_IS_DONE, ls::FILE_FINISH, ls::FILE_CANCEL,
        ls::SHAPE_FIND, ls::SHAPE_ENTRY, ls::UNPACKED_SIZE, ls::DECODE, ls::EA_FREE, ls::EA_ALLOC, ls::MEMSET,
        ls::MEM_FREE, ls::SET_COLOUR, ls::SPRITE_RECT, ls::FILLED_RECT, ls::DRAW_RIGHT, ls::PRINTF_SHADOWED_A,
        ls::SPRITE_TILED, ls::SPRITE_ANCHORED, ls::SET_FADE, ls::LOC, ls::SPRINTF, ls::BANK_LOAD, ls::BANK_POLL, ls::LOOP_PLAY,
        ls::LOOP_STOP, ls::BANK_UNLOAD, ls::SPRITES_INIT, ls::LOAD_ICONS, ls::SPRITES_POLL, ls::WSTR_COPY, ls::WSTR_COUNT,
        ls::WSTR_SET_ASCII, ls::WSTR_FORMAT, ls::WSTR_FORMAT_ASCII, ls::PLAYER_ID, ls::PLAYER_PROFILE, ls::LEVEL_NAME,
        ls::TRICK_BOOK, ls::COMPLETE_PAGES, ls::EVENT_MEDAL, ls::COURSE_NAME, ls::COURSE_DIFFICULTY, ls::COURSE_LOCATION,
        ls::COURSE_DROP, ls::COURSE_LENGTH, ls::MAP_X, ls::MAP_Y, ls::ANIM_VALUE, ls::TS_INIT, ls::TS_CONSTRUCT,
        ls::TS_SET_FONT, ls::TS_DESTRUCT_THUNK, ls::TS_DESTRUCT, ls::DRAW_ALIGNED, ls::DRAW_CURSOR,
    ];
    stubs.extend(ROFFS.map(rv));
    stubs.extend(SOFFS.map(sv));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let f: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(mem, func, &w, &f, &a);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, v as i32 as i64 as u64);
            c.set_f(0, f32::from_bits(v));
        });
    }
    let read = |r: &mut Runner, (at, n): (u32, u32)| {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        b
    };
    let data = read(&mut r, DATA);
    let rodata = read(&mut r, RODATA);
    let mut g = Rng(0x5ce1_6000_0000_0001);
    let (mut steps, mut pics, mut fades, mut filled) = ([0usize; 5], 0usize, 0usize, 0usize);
    let per = 300u32;
    for case in 0..funcs.len() as u32 * per {
        let func = funcs[(case % funcs.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (RODATA.0, rodata.clone()),
                (SPEECH_AT.0, vec![0; SPEECH_AT.1 as usize]),
                (ARENA, vec![0; SIZE as usize]),
                (HEAP.0, vec![0; HEAP.1 as usize]),
                (SCRATCH, vec![0; 0x200]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (its renderer +0x724, its +0x728, strings +0x50), the renderer's table, the screen's own
        m.w32(ls::APP, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        for off in SOFFS {
            m.w16(SVT + off, g.pick(&[0u16, 0x10]));
            m.w32(SVT + off + 4, sv(off));
        }
        // the screen: the port's own init where the function reads what it sets, else random
        if g.below(2) == 0 {
            let seed = g.next() as u64;
            let a2 = Answers { seed: Cell::new(seed), n: Cell::new(0) };
            let mut e2 = Env { log: Vec::new(), ans: &a2 };
            ls::init(&mut m, &mut e2, THIS);
        }
        m.w32(THIS, SVT);
        // (step 3, the pictures, the most: it has the most paths)
        let step = g.pick(&[0u32, 1, 2, 3, 3, 3, 4, 5]);
        m.w32(THIS + 0xb4, step);
        m.w32(THIS + 0x160, g.pick(&[0u32, 0, 1]));
        let any = g.next();
        m.w32(THIS + 0x164, g.pick(&[0u32, any, any]));
        m.w32(THIS + 0x16c, g.pick(&[0u32, HEAP.0]));
        let n = g.below(5);
        m.w32(THIS + 0x170, PICS);
        m.w8(THIS + 0x174, n as u8);
        m.w32(THIS + 0x168, PICS + 0x34 * g.below(4));
        for i in 0..5 {
            m.w32(PICS + 0x34 * i, g.pick(&[0u32, 1, 1, 2]));
        }
        m.w32(THIS + 0x90, g.below(2));
        let (f1, f2) = (g.f(0.0, 1.0), g.f(0.95, 1.05));
        let fade = g.pick(&[0.0f32, 1.0, f1, f2, -0.01]);
        wf(&mut m, THIS + 0x8c, fade);
        wf(&mut m, THIS + 0x88, g.f(0.0, 100.0));
        m.w32(ls::FLAKE_MODE, g.pick(&[3u32, 0, 1]));
        wf(&mut m, SHEETS + 0x5c, g.f(0.0, 50.0));
        wf(&mut m, SHEETS + 0x2a4, g.f(0.0, 60.0));
        wf(&mut m, SHEETS + 0x2a8, g.f(0.0, 40.0));
        m.w32(ls::CAREER + 0x3708, g.pick(&[1u32, 0, 2]));
        for o in [0x2af8, 0x3814] {
            m.w32(ls::CAREER + o, g.below(12));
        }
        let pic = g.below(50);
        m.w32(THIS + 0x40c, g.pick(&[0xffff_ffffu32, pic]));
        wf(&mut m, ls::DRAW_COLOUR, g.f(0.0, 1.0));
        // the one-player track load: its sprite set, the map's size, the sprites' flag, the summary's step, modes
        m.w32(THIS + 4, SET);
        m.w32(ls::SPEECH, g.next());
        wf(&mut m, THIS + 0xb34, g.f(1.0, 300.0));
        wf(&mut m, THIS + 0xb38, g.f(1.0, 300.0));
        m.w32(THIS + 0xe24, g.below(2));
        m.w32(THIS + 0x40c, g.below(11));
        for i in 0..3 {
            m.w32(THIS + 0xe28 + 0x34 * i, g.pick(&[0u32, 1, 1, 2]));
        }
        for o in [0x2afc, 0x3818] {
            m.w32(ls::CAREER + o, g.below(11));
        }
        m.w32(APPOBJ + 8, g.below(12));
        let done = g.pick(&[0u32, 3, 9, 10, 11, 20]);
        let any = g.next();
        let (text, tex) = (g.pick(&[0u32, any]), g.below(20));
        // (y also past 2^24, where sums in another order come out differently)
        let (fx, fa) = (g.f(0.0, 640.0), g.f(0.0, 1.0));
        let fy = { let a = g.f(0.0, 480.0); let b = g.f(2e7, 2e8); g.pick(&[a, a, b]) };
        let flags = g.below(4);
        let pick = g.below(14);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let none = u32::MAX;
        let e = &mut env;
        let ours = match func {
            0x243c78 => ls::init(&mut m, e, THIS),
            0x243de8 => {
                ls::delete(&mut m, e, THIS, flags);
                none
            }
            0x243ea0 => {
                ls::start(&mut m, e, THIS);
                none
            }
            0x243f00 => ls::poll(&mut m, e, THIS, SCRATCH),
            0x244290 => {
                ls::load_pics(&mut m, e, THIS, PICS, n);
                none
            }
            0x244330 => {
                ls::close_archive(&mut m, e, THIS, SCRATCH);
                none
            }
            0x2443b0 => {
                ls::start_bars(&mut m, e, THIS, SCRATCH);
                none
            }
            0x244490 => ls::fade_in(&mut m, THIS),
            0x2444c0 => {
                ls::tick(&mut m, e, THIS);
                none
            }
            0x244540 => {
                ls::draw_flakes(&mut m, e, SHEETS, done as i32);
                none
            }
            0x2447a8 => {
                ls::draw_tip_box(&mut m, e, THIS, SHEETS, text, tex, F(fx), F(fy), F(fa), SCRATCH);
                none
            }
            0x2449f8 => {
                ls::draw_ticker(&mut m, e, SHEETS, SCRATCH);
                none
            }
            0x244b20 => {
                ls::draw(&mut m, e, THIS, SCRATCH);
                none
            }
            0x244ee8 => {
                ls::draw_spinner(&mut m, e, THIS, F(fx), F(fy));
                none
            }
            0x244fc8 => {
                ls::draw_bars(&mut m, e, THIS, SHEETS, SCRATCH);
                none
            }
            0x245598 => ls::selected_course(&m),
            0x2455c0 => ls::first_new(&mut m, e, THIS),
            0x245668 => {
                ls::first_delete(&mut m, e, THIS, flags);
                none
            }
            0x245680 => {
                ls::first_start(&mut m, e, THIS);
                none
            }
            0x2456b8 => ls::first_poll(&mut m, e, THIS, SCRATCH),
            0x245760 => {
                ls::first_draw_background(&mut m, e, THIS, SCRATCH);
                none
            }
            0x245838 => none,
            0x245840 => ls::track1_new(&mut m, e, THIS),
            0x245958 => {
                ls::track1_delete(&mut m, e, THIS, flags);
                none
            }
            0x2459b0 => {
                ls::mode_pic_name(&mut m, e, pick, SCRATCH);
                none
            }
            0x245a48 => {
                ls::track_pic_name(&mut m, e, pick, SCRATCH);
                none
            }
            0x245b68 => {
                ls::rider_pic_name(&mut m, e, pick, SCRATCH);
                none
            }
            0x245cb8 => {
                ls::track1_start(&mut m, e, THIS);
                none
            }
            0x245ec0 => ls::track1_poll(&mut m, e, THIS),
            0x246700 => {
                ls::fill_event_summary(&mut m, e, THIS);
                none
            }
            0x246f18 => {
                ls::track1_vf4(&mut m, e, THIS, SCRATCH);
                none
            }
            0x246f50 => ls::track1_vf3(&mut m, e, THIS, SCRATCH),
            0x246f88 => {
                ls::track1_vf5(&mut m, e, THIS);
                none
            }
            0x246fe0 => {
                ls::track1_draw_background(&mut m, e, THIS, SCRATCH);
                none
            }
            0x2473d8 => {
                ls::track1_draw_overlay(&mut m, e, THIS, SCRATCH);
                none
            }
            _ => unreachable!(),
        };
        if func == 0x243f00 && (step as usize) < 5 {
            steps[step as usize] += 1;
            pics += env.log.iter().filter(|c| c.0 == rv(0x150)).count();
        }
        if func == 0x245ec0 && ours == 1 {
            filled += 1;
        }
        if func == 0x244b20 {
            fades += env.log.iter().filter(|c| c.0 == ls::FILLED_RECT).count();
        }
        ans.n.set(0);
        log.borrow_mut().clear();
        let regs: Vec<u32> = match func {
            0x243de8 | 0x245668 => vec![THIS, flags],
            0x244290 => vec![THIS, PICS, n],
            0x244540 => vec![THIS, SHEETS, done],
            0x2447a8 => vec![THIS, 0, SHEETS, text, tex],
            0x2449f8 | 0x244fc8 => vec![THIS, SHEETS],
            0x245958 => vec![THIS, flags],
            0x2459b0 | 0x245a48 | 0x245b68 => vec![pick, SCRATCH],
            _ => vec![THIS],
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.cpu.set_f(12, fx);
        r.cpu.set_f(13, fy);
        r.cpu.set_f(14, fa);
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
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
    println!("{steps:?} poll steps, {pics} shapes added, {fades} draws' rects, {filled} sprite tables filled");
    assert!(steps.iter().all(|&s| s > 30) && pics > 30 && fades > 50 && filled > 50);
}
