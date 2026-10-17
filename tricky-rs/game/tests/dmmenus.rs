//! `tricky_game::dmmenus` (board row G9a4d) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case starts from the game's own data (0x328000..0x340000 and the menus' strings and tables at
//! 0x388c00.., as the ELF loads them) with a page in the arena (made random, or built for the destructors: the
//! resolution page's commands given stand-in tables), the app, its race, the renderer, its object, the sound
//! system and a pool of items. Then one of the row's functions runs on both sides, with the whole debug menu's code
//! (G9a4a..c) real on both. Every other callee is a stand-in on both sides answering from the case's seed. The
//! runner's stack is cleared before each case. The calls, the answers and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::dmmenus as dn;
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
const SYS_AT: (u32, u32) = (0x343000, 0x100);
const TABLES: (u32, u32) = (0x388c00, 0x1600);
const STATICS: (u32, u32) = (0x3cdde0, 0x80);
const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0xa000;
const THIS: u32 = ARENA;
const MGR: u32 = ARENA + 0x2000;
const ITEMS: u32 = ARENA + 0x2100;
const ITEM_TABLES: u32 = ARENA + 0x2900;
const APPOBJ: u32 = ARENA + 0x8000;
const RACE: u32 = ARENA + 0x2c40;
const RACE1C: u32 = ARENA + 0x2f80;
const RACE_TABLE: u32 = ARENA + 0x3100;
const RENDER: u32 = ARENA + 0x3200;
const RVT: u32 = ARENA + 0x3300;
const OBJ: u32 = ARENA + 0x3600;
const OVT: u32 = ARENA + 0x3640;
const PTR: u32 = ARENA + 0x3700;
const O40: u32 = ARENA + 0x3740;
const SYS: u32 = ARENA + 0x4000;
const STACK_TOKEN: u32 = 0x5c5c_5c5c;
fn iv(off: u32) -> u32 {
    0x0180_0000 + off
}
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
fn ov(off: u32) -> u32 {
    0x0181_0000 + off
}
const RACE_VA0: u32 = 0x0182_00a0;
const IOFFS: [u32; 5] = [8, 0x10, 0x18, 0x30, 0x38];
const ROFFS: [u32; 7] = [0x20, 0x28, 0x40, 0x48, 0x50, 0x120, 0x130];
const OOFFS: [u32; 10] = [0x20, 0x28, 0x30, 0x38, 0x48, 0x50, 0x58, 0x60, 0x68, 0x70];

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

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
    match func {
        f if f == iv(8) || f == iv(0x30) => (2, 0),
        f if f & 0xffff_0000 == 0x0180_0000 => (1, 0),
        f if f == ov(0x20) || f == ov(0x28) => (1, 1),
        f if (0x48..=0x58).contains(&(f & 0xff)) && f & 0xffff_0000 == 0x0181_0000 => (2, 0),
        f if f & 0xffff_0000 == 0x0181_0000 => (1, 0),
        RACE_VA0 => (2, 0),
        f if f == rv(0x28) || f == rv(0x48) || f == rv(0x120) => (2, 0),
        f if f & 0xffff_0000 == 0x01ff_0000 => (1, 0),
        dn::SET_MODE | dn::INPUT_RESET | dn::GET_SETTING | dn::RESET_RIDER | dn::CAREER_SAVE_A | dn::CAREER_SAVE_B => (1, 0),
        dn::QUIT_RACE | dn::OVERLAY_GET | dn::SND_RESUME | dn::SND_PAUSE | dn::SND_STOP | dn::START_MUSIC => (1, 0),
        dn::MUSIC_COUNT | dn::MUSIC_NEXT => (1, 0),
        dn::SET_BRIGHT => (1, 1),
        dn::OVERLAY_SHOW => (3, 0),
        dn::OVERLAY_SET | dn::GET_VOL | dn::SET_PRESET | dn::MUSIC_SELECT | dn::SPRINTF | 0x114f80 => (2, 0),
        dn::SET_VOL => (2, 1),
        _ => panic!("no arity for {func:#x}"),
    }
}

fn standin(func: u32, w: &[u32], f: &[u32], ans: &Answers) -> (u32, Call) {
    let (ni, nf) = arity(func);
    let mut log: Vec<u32> = w[..ni].iter().map(|&a| token(a)).collect();
    log.extend(&f[..nf]);
    let r = ans.next();
    let answer = match func {
        f if f == iv(0x18) => [0u32, 1, r][(r % 3) as usize],
        f if f == iv(0x10) => [0u32, 1, 2, 3, 4, r][(r % 6) as usize],
        f if f == rv(0x20) => OBJ,
        f if f == rv(0x130) => PTR,
        f if f == rv(0x40) => r % 6,
        f if f == rv(0x50) => [0u32, 1, 3, 5, 0x80][(r % 5) as usize],
        dn::MUSIC_COUNT => [0u32, 1, 3, 0xffff_ffff][(r % 4) as usize],
        dn::GET_VOL => ((r % 13) as f32 / 11.0 - 0.05).to_bits(),
        _ => r,
    };
    (answer, Call(func, log))
}

struct Env<'a> {
    log: Vec<Call>,
    ans: &'a Answers,
}
impl TextBoxEnv for Env<'_> {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32], fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let mut f: Vec<u32> = fargs.iter().map(|x| x.0.to_bits()).collect();
        f.resize(8, 0);
        let (a, c) = standin(func, &w, &f, self.ans);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

const FUNCS: [u32; 28] = [
    0x186bb0, 0x186cf8, 0x186dd0, 0x186e30, 0x1870a0, 0x187170, 0x187468, 0x1875d0, 0x187750, 0x187828, 0x187850,
    0x187ad0, 0x187ad8, 0x187bd0, 0x187c20, 0x187cb8, 0x187f58, 0x188060, 0x188080, 0x188310, 0x1883b8, 0x188410,
    0x188530, 0x188638, 0x1886f8, 0x1887f8, 0x188908, 0x188990,
];

const ALL: [u32; 211] = [
    0x186bb0, 0x186cf8, 0x186dd0, 0x186e30, 0x1870a0, 0x187170, 0x187468, 0x1875d0, 0x187750, 0x187828,
    0x187850, 0x187ad0, 0x187ad8, 0x187bd0, 0x187c20, 0x187cb8, 0x187f58, 0x188060, 0x188080, 0x188310,
    0x1883b8, 0x188410, 0x188530, 0x188638, 0x1886f8, 0x1887f8, 0x188908, 0x188990, 0x1889b8, 0x1889d8,
    0x188a00, 0x188a80, 0x188ac8, 0x188ad0, 0x188ad8, 0x188ae8, 0x188b10, 0x188b20, 0x188b48, 0x188b68,
    0x188b88, 0x188ba8, 0x188bd0, 0x188bf8, 0x188c50, 0x188d20, 0x188df0, 0x188e78, 0x188e80, 0x188e88,
    0x189240, 0x1892e8, 0x189390, 0x1893a0, 0x189450, 0x189478, 0x1894a8, 0x189518, 0x189560, 0x189590,
    0x1895b0, 0x1895c8, 0x1895e0, 0x1895e8, 0x189718, 0x189720, 0x189760, 0x1897a0, 0x1897a8, 0x1897b0,
    0x1897f0, 0x189898, 0x189948, 0x1899c0, 0x189a90, 0x189b98, 0x189c38, 0x189c80, 0x189cc0, 0x189d38,
    0x189d78, 0x18a000, 0x18a118, 0x18a1d0, 0x18a228, 0x18a298, 0x18a2d0, 0x18a320, 0x18a368, 0x18a3d8,
    0x18a410, 0x18a460, 0x18a498, 0x18a4e0, 0x18a538, 0x18a548, 0x18a5d0, 0x18a628, 0x18a6b8, 0x18a700,
    0x18a708, 0x18a740, 0x18a790, 0x18a7d8, 0x18a7e0, 0x18a818, 0x18a820, 0x18a878, 0x18a900, 0x18a940,
    0x18aab0, 0x18aae8, 0x18ab68, 0x18abf8, 0x18ada8, 0x18ade0, 0x18ae80, 0x18af10, 0x18b0c0, 0x18b0f8,
    0x18b198, 0x18b1d0, 0x18b240, 0x18b258, 0x18b660, 0x18b6b8, 0x18b810, 0x18b840, 0x18b870, 0x18b8f8,
    0x18b9a0, 0x18b9a8, 0x18b9b0, 0x18b9f8, 0x18ba68, 0x18bb58, 0x18bb88, 0x18bbb8, 0x18bd78, 0x18beb0,
    0x18bf28, 0x18bf50, 0x18bfa8, 0x18bfb0, 0x18c0a8, 0x18c118, 0x18c1c8, 0x18c288, 0x18c318, 0x18c380,
    0x18c728, 0x18c9d0, 0x18cc78, 0x18ceb8, 0x18d0c8, 0x18d280, 0x18d360, 0x18d6a0, 0x18d8d8, 0x18dce0,
    0x18dd58, 0x18de50, 0x18de60, 0x18df38, 0x18df60, 0x18df70, 0x18dfd0, 0x18e028, 0x18e080, 0x18e0e8,
    0x18e140, 0x18e198, 0x18e228, 0x18e280, 0x18e2d8, 0x18e2e0, 0x18e2f0, 0x18e2f8, 0x18e308, 0x18e360,
    0x18e368, 0x18e378, 0x18e380, 0x18e390, 0x18e3e8, 0x18e3f0, 0x18e3f8, 0x18e450, 0x18e4a8, 0x18e4b0,
    0x18e4b8, 0x18e510, 0x18e518, 0x18e520, 0x18e578, 0x18e580, 0x18e5d8, 0x18e5e0, 0x18e638, 0x18e640,
    0x18e648, 0x18e650, 0x18e6b0, 0x18e6b8, 0x18e6c0, 0x18e718, 0x18e720, 0x18e730, 0x18e740, 0x18e788,
    0x18e790,
];

#[test]
fn debug_menus_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in ALL {
        r.allow(f);
    }
    let mut stubs = vec![
        dn::SET_MODE, dn::INPUT_RESET, dn::SET_BRIGHT, dn::GET_SETTING, dn::RESET_RIDER, dn::CAREER_SAVE_A,
        dn::CAREER_SAVE_B, dn::QUIT_RACE, dn::OVERLAY_SHOW, dn::OVERLAY_GET, dn::OVERLAY_SET, dn::SND_RESUME,
        dn::SND_PAUSE, dn::SND_STOP, dn::SET_VOL, dn::GET_VOL, dn::SET_PRESET, dn::START_MUSIC, dn::MUSIC_COUNT,
        dn::MUSIC_SELECT, dn::MUSIC_NEXT, dn::SPRINTF, 0x114f80, RACE_VA0,
    ];
    stubs.extend(IOFFS.map(iv));
    stubs.extend(ROFFS.map(rv));
    stubs.extend(OOFFS.map(ov));
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let ans = Rc::new(Answers { seed: Cell::new(0), n: Cell::new(0) });
    for func in stubs {
        let (l, a) = (log.clone(), ans.clone());
        r.stub(func, move |c, _mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let f: Vec<u32> = (12..20).map(|k| c.f(k).to_bits()).collect();
            let (v, call) = standin(func, &w, &f, &a);
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
    let tables = read(&mut r, TABLES);
    let mut g = Rng(0xd3e0_0000_0000_0001);
    let (mut built, mut changed, mut freed) = (0usize, 0usize, 0usize);
    let per = 300u32;
    for case in 0..FUNCS.len() as u32 * per {
        let func = FUNCS[(case % FUNCS.len() as u32) as usize];
        let mut m = Mem {
            regions: vec![
                (DATA.0, data.clone()),
                (SYS_AT.0, vec![0; SYS_AT.1 as usize]),
                (TABLES.0, tables.clone()),
                (STATICS.0, vec![0x5a; STATICS.1 as usize]),
                (ARENA, vec![0; SIZE as usize]),
                (0x01f8_0000, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the app (input +0x24, race +0x730, renderer +0x724), the race (its +0x1c: riders +0x88, +0xc4.., table
        // +8), its words; the renderer's table and object; the sound system; the career's mode
        m.w32(dn::APP, APPOBJ);
        m.w32(APPOBJ + 0x730, RACE);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RACE + 0x1c, RACE1C);
        m.w32(RACE1C + 0x88, g.pick(&[0u32, 1, 4, 6]));
        m.w32(RACE1C + 8, RACE_TABLE);
        m.w16(RACE_TABLE + 0xa0, g.pick(&[0u16, 8]));
        m.w32(RACE_TABLE + 0xa4, RACE_VA0);
        m.w32(RACE + 0x298, g.pick(&[1u32, 2, 2, 3]));
        m.w32(RACE + 0x40, g.pick(&[0u32, O40, O40]));
        m.w32(O40 + 0x4c, g.pick(&[0u32, 1]));
        m.w32(RACE + 0x54, g.below(2));
        m.w32(dn::CAREER + 0x2afc, g.pick(&[6u32, 1, 2]));
        m.w32(RENDER + 0x10, RVT);
        for off in ROFFS {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        m.w32(OBJ, OVT);
        for off in OOFFS {
            m.w16(OVT + off, g.pick(&[0u16, 4]));
            m.w32(OVT + off + 4, ov(off));
        }
        m.w32(dn::MUSIC, SYS);
        m.w32(SYS + 0x36e8, g.below(8));
        // the items' tables and the pool; the manager
        for t in 0..4 {
            for off in IOFFS {
                m.w16(ITEM_TABLES + 0x40 * t + off, g.pick(&[0u16, 0, 4]));
                m.w32(ITEM_TABLES + 0x40 * t + off + 4, iv(off));
            }
        }
        for i in 0..64 {
            let it = ITEMS + 0x20 * i;
            m.w32(it, g.below(12));
            let any = g.next();
            m.w32(it + 8, g.pick(&[1u32, 1, 9, any]));
            m.w32(it + 4, THIS);
            m.w32(it + 0xc, ITEM_TABLES + 0x40 * g.below(4));
        }
        m.w32(MGR, g.pick(&[0u32, 1, 2]));
        m.w32(MGR + 4, THIS);
        m.w32(MGR + 0x30, g.pick(&[0xdu32, 8, 0x4800, 0x5000, 0]));
        // the page: a menu of pool items (for the handlers), the values
        let n = g.pick(&[1u32, 3, 8]);
        m.w32(THIS + 4, MGR);
        m.w32(THIS + 8, n);
        m.w32(THIS + 0xc, g.below(n));
        for i in 0..64 {
            m.w32(THIS + 0x14 + 4 * i, ITEMS + 0x20 * g.below(64));
        }
        for o in (0x118..0x144).step_by(4) {
            m.w32(THIS + o, g.pick(&[0u32, 1, 5, 11, 12, 0xffff_ffff]));
        }
        wf(&mut m, THIS + 0x120, g.f(0.0, 2.0));
        // the resolution pages' commands (for the destructors): stand-in tables
        for base in [THIS, THIS + 0x3e0, THIS + 0x3d8 + 0x3e0] {
            for i in 0..0x80 {
                m.w32(base + 0x144 + 0x20 * i + 0xc, ITEM_TABLES + 0x40 * g.below(4));
            }
        }
        let item = ITEMS + 0x20 * g.below(64);
        m.w32(item, g.below(13));
        let regs: Vec<u32> = match func {
            0x186cf8 | 0x187bd0 => vec![THIS, MGR],
            0x186dd0 | 0x187750 | 0x187ad8 => vec![THIS, item, g.next()],
            0x1870a0 | 0x187468 | 0x187828 | 0x188080 => vec![THIS, item],
            0x188410 | 0x188530 | 0x188638 | 0x1886f8 | 0x1887f8 | 0x188908 => vec![THIS, g.below(4)],
            0x188310 => vec![],
            _ => vec![THIS],
        };
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        r.mem.write(0x01ff_0000 - 0x4000, &[0u8; 0x4000]).unwrap();
        ans.seed.set(((g.next() as u64) << 32) | g.next() as u64);
        ans.n.set(0);
        let mut env = Env { log: Vec::new(), ans: &ans };
        let none = u32::MAX;
        let e = &mut env;
        let mm = &mut m;
        let w = |i: usize| regs.get(i).copied().unwrap_or(0);
        let ours = match func {
            0x186bb0 => dn::resolution_new(mm, e, THIS),
            0x186cf8 => { dn::resolution_enter(mm, e, THIS, w(1)); none }
            0x186dd0 => dn::resolution_command(mm, e, THIS, w(1), w(2)),
            0x186e30 => dn::render_new(mm, e, THIS),
            0x1870a0 => { dn::render_changed(mm, e, THIS, w(1)); none }
            0x187170 => dn::bezier_new(mm, e, THIS),
            0x187468 => { dn::bezier_changed(mm, e, THIS, w(1)); none }
            0x1875d0 => dn::game_new(mm, e, THIS),
            0x187750 => dn::game_command(mm, e, THIS, w(1), w(2)),
            0x187828 => { dn::game_changed(mm, THIS, w(1)); none }
            0x187850 => dn::debug_new(mm, e, THIS),
            0x187ad0 => { dn::install_stub(THIS); none }
            0x187ad8 => dn::debug_command(mm, e, THIS, w(1), w(2)),
            0x187bd0 => { dn::debug_enter(mm, e, THIS, w(1)); none }
            0x187c20 => { dn::debug_leave(mm, e, THIS); none }
            0x187cb8 => dn::sound_new(mm, e, THIS),
            0x187f58 => { dn::build_music_list(mm, e, THIS); none }
            0x188060 => dn::sound_handle_input(mm, e, THIS),
            0x188080 => { dn::sound_changed(mm, e, THIS, w(1)); none }
            0x188310 => { dn::static_init(mm); none }
            0x1883b8 => dn::debug_sound_page(THIS),
            0x188410 => { dn::debug_delete(mm, e, THIS, w(1)); none }
            0x188530 => { dn::sound_delete(mm, e, THIS, w(1)); none }
            0x188638 => { dn::game_delete(mm, e, THIS, w(1)); none }
            0x1886f8 => { dn::render_delete(mm, e, THIS, w(1)); none }
            0x1887f8 => { dn::resolution_delete(mm, e, THIS, w(1)); none }
            0x188908 => { dn::bezier_delete(mm, e, THIS, w(1)); none }
            0x188990 => { dn::debug_printf(); none }
            _ => unreachable!(),
        };
        if matches!(func, 0x186bb0 | 0x186e30 | 0x187170 | 0x1875d0 | 0x187850 | 0x187cb8) {
            built += 1;
        }
        if matches!(func, 0x1870a0 | 0x187468 | 0x188080) && !env.log.is_empty() {
            changed += 1;
        }
        freed += env.log.iter().filter(|c| c.0 == iv(8) || c.0 == 0x114f80).count();
        ans.n.set(0);
        log.borrow_mut().clear();
        for (k, v) in regs.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        if ours != none {
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case} {func:#x}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for (at, b) in &m.regions {
            if *at == 0x01f8_0000 {
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
    println!("{built} pages built, {changed} changes handled, {freed} frees");
    assert!(built > 1000 && changed > 300 && freed > 1000);
}
