//! Our prewind (the crouch and wind-up before a jump) against the original's (board row G1a4): the game's own
//! `PrewindState_Update` 0x1050e8 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped
//! without it), with its helpers run as the original (`Boarder_UpdateBoost`, `Boarder_SetCrouchTarget`,
//! `PrewindState_SetWindTarget`, `Boarder_SetSteerTarget`, `PrewindState_UpdateWindClip`, `Boarder_IsTricky`,
//! `Math_Atan`; each checked on its own in tests/boarder.rs) and the clip, state, reset and sound calls recorded,
//! and `tricky_game::boarder::prewind_update` on the same rider: every field and every call bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::boarder::{prewind_update, Call, Fields, Prewind, PrewindCall};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

const GAPP: u32 = 0x337c58;

fn put4(r: &mut Runner, at: u32, v: &[f32]) {
    for (k, x) in v.iter().enumerate() {
        r.mem.write_f32(at + 4 * k as u32, *x);
    }
}

fn write(r: &mut Runner, rd: u32, stats: u32, st: u32, p: &Prewind) {
    let m = &mut r.mem;
    let f = &p.f;
    m.write_u32(st + 8, p.started);
    for (o, v) in [(0x1c, f.meter), (0x24, f.uber_timer), (0x130, f.boost_level), (0x1fc, f.x1fc), (0x208, f.crouch), (0x20c, f.crouch_rate), (0x210, f.crouch_target), (0x250, f.x250), (0x254, f.x254), (0x258, f.x258), (0x25c, f.x25c), (0x260, f.x260), (0x264, f.x264), (0x1c8, p.x1c8), (0x214, p.steer), (0x218, p.steer_rate), (0x21c, p.steer_target)] {
        m.write_f32(rd + o, v);
    }
    m.write_u32(rd + 0x28, f.full_count as u32);
    for (o, v) in [(0x1dc, p.kind), (0x418, p.x418 as u32), (0x290, p.surface), (0x1b4, p.switch), (0x420, p.class), (0x304, p.x304), (0x58a8, p.ubers as u32)] {
        m.write_u32(rd + o, v);
    }
    m.write_u8(stats + 0x18, p.windup_stat);
    put4(r, rd + 0x220, &p.spin);
    put4(r, rd + 0x22c, &p.flip);
    put4(r, rd + 0x150, &p.vel);
    put4(r, rd + 0x180, &p.board);
}

fn read(r: &mut Runner, rd: u32, st: u32, like: &Prewind) -> Prewind {
    let m = &mut r.mem;
    let g = |m: &mut r5900::Mem, o: u32| m.read_f32(rd + o);
    let f = Fields {
        meter: g(m, 0x1c),
        uber_timer: g(m, 0x24),
        full_count: m.read_u32(rd + 0x28) as i32,
        boost_level: g(m, 0x130),
        x1fc: g(m, 0x1fc),
        crouch: g(m, 0x208),
        crouch_rate: g(m, 0x20c),
        crouch_target: g(m, 0x210),
        x250: g(m, 0x250),
        x254: g(m, 0x254),
        x258: g(m, 0x258),
        x25c: g(m, 0x25c),
        x260: g(m, 0x260),
        x264: g(m, 0x264),
    };
    let v3 = |m: &mut r5900::Mem, o: u32| [m.read_f32(rd + o), m.read_f32(rd + o + 4), m.read_f32(rd + o + 8)];
    Prewind {
        f,
        started: m.read_u32(st + 8),
        spin: v3(m, 0x220),
        flip: v3(m, 0x22c),
        x1c8: g(m, 0x1c8),
        steer: g(m, 0x214),
        steer_rate: g(m, 0x218),
        steer_target: g(m, 0x21c),
        x304: m.read_u32(rd + 0x304),
        ..*like
    }
}

fn bits(p: &Prewind) -> Vec<u32> {
    let f = &p.f;
    [f.meter, f.uber_timer, f.boost_level, f.x1fc, f.crouch, f.crouch_rate, f.crouch_target, f.x250, f.x254, f.x258, f.x25c, f.x260, f.x264, p.x1c8, p.steer, p.steer_rate, p.steer_target]
        .iter()
        .chain(p.spin.iter())
        .chain(p.flip.iter())
        .map(|x| x.to_bits())
        .chain([f.full_count as u32, p.started, p.x304])
        .collect()
}

#[test]
fn prewind_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x11d038, 0x126c00, 0x105a60, 0x126948, 0x1055f0, 0x11f430, 0x2510f8] {
        r.allow(f);
    }
    let log = Rc::new(RefCell::new(Vec::<PrewindCall>::new()));
    for (addr, c) in [(0x2192a8u32, Call::BoostSound), (0x2197d8, Call::BoostEmpty), (0x219760, Call::BoostStop)] {
        let l = log.clone();
        r.stub(addr, move |_, _| l.borrow_mut().push(PrewindCall::Boost(c)));
    }
    let l = log.clone();
    r.stub(0x15fcf8, move |cpu, _| l.borrow_mut().push(PrewindCall::Clip(cpu.gpr(5) as u32)));
    let l = log.clone();
    r.stub(0x11f608, move |cpu, _| l.borrow_mut().push(PrewindCall::State(cpu.gpr(5) as u32)));
    let l = log.clone();
    r.stub(0x118f10, move |_, _| l.borrow_mut().push(PrewindCall::Reset));
    let stats = r.mem.alloc(0x50);
    let rd = r.mem.alloc(0x6000);
    let st = r.mem.alloc(0x20);
    let pad = r.mem.alloc(4);
    r.mem.write_u32(rd + 0x464, stats);
    r.mem.write_u32(st + 0xc, rd);
    // no race object: boost sounds allowed
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    r.mem.write_u32(GAPP, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x40, 0);
    let mut g = Rng(0x0e1e_d000_0000_0001);
    let mut seen = std::collections::BTreeSet::new();
    for case in 0..20000 {
        let small = |g: &mut Rng| match g.next() % 4 { 0 => 0.0, 1 => g.f(-0.25, 0.25), _ => g.f(-1.0, 1.0) };
        let p0 = Prewind {
            f: Fields {
                meter: if g.next() % 3 == 0 { 0.0 } else { g.f(0.0, 1.0) },
                uber_timer: 0.0,
                full_count: 0,
                boost_level: [0.0, 1.0, 0.25][(g.next() % 3) as usize],
                x1fc: if g.next() % 4 == 0 { 1.0 } else { 0.0 },
                crouch: g.f(0.0, 1.0),
                crouch_rate: g.f(0.0, 5.0),
                crouch_target: g.f(0.0, 1.0),
                x250: g.f(0.0, 1.0),
                x254: 1.0,
                x258: 0.5,
                x25c: [1.0, 0.0, 0.5][(g.next() % 3) as usize],
                x260: 0.0,
                x264: 0.0,
            },
            started: g.next() % 2,
            kind: [0, 0, 1, 2, 3, 4][(g.next() % 6) as usize],
            x418: if g.next() % 2 == 0 { -1 } else { 3 },
            spin: [small(&mut g), g.f(0.0, 3.0), small(&mut g)],
            flip: [small(&mut g), g.f(0.0, 3.0), small(&mut g)],
            x1c8: g.f(0.0, 1.0),
            vel: [g.f(-1500.0, 1500.0), g.f(-1500.0, 1500.0), g.f(-300.0, 300.0), 0.0],
            board: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), 0.0],
            steer: g.f(-1.0, 1.0),
            steer_rate: 0.0,
            steer_target: 0.0,
            surface: [0, 1, 1, 2, 3, 4][(g.next() % 6) as usize],
            switch: g.next() % 2,
            class: g.next() % 3,
            x304: 7,
            ubers: (g.next() % 8) as i32,
            windup_stat: g.next() as u8,
        };
        let word = g.next() << 2 ^ g.next();
        write(&mut r, rd, stats, st, &p0);
        r.mem.write_u32(pad, word);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, st as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x1050e8).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = read(&mut r, rd, st, &p0);
        let mut ours = p0;
        let calls = prewind_update(word, true, &mut ours);
        assert_eq!(bits(&theirs), bits(&ours), "case {case} pad {word:#x}\n game {theirs:?}\n ours {ours:?}");
        assert_eq!(*log.borrow(), calls, "case {case} pad {word:#x}: calls");
        seen.insert(format!("{calls:?}").chars().take(40).collect::<String>());
    }
    assert!(seen.len() >= 6, "paths seen: {seen:?}");
}
