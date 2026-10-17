//! `tricky_game::fe_transition` (board row G9b2e) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random transition records (active, mode, phase, times, clock, saved and current values, targets and
//! deltas, the held flag, the fade's colour and alpha), every function run on both sides with the same answers from
//! the renderer's virtuals and the drawer's calls (their float arguments logged from f12..f17), the calls logged in
//! order (vectors by value) and the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{FeEnv, GAPP};
use tricky_game::fe_transition::*;
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

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) | 1
}

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
const REC: u32 = ARENA + 0x1000;
const REND: u32 = ARENA + 0x2000;
/// the values the renderer hands over (+0x130)
const RVALS: u32 = ARENA + 0x2400;
const COLOUR: u32 = ARENA + 0x2800;
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const V_BASE: u32 = 0x7f_0000;
const SCRATCH: u32 = 0x5c5c_5c5c;

fn slot(off: u32) -> u32 {
    V_BASE + off + 4
}

/// How a call's arguments are logged: the integer registers taken, the float registers (f12 on) taken, and whether
/// its second argument is a vector (words by value)
fn shape(target: u32) -> (usize, usize, usize) {
    match target {
        t if t == slot(0x98) => (1, 4, 0),
        t if t == slot(0xb0) => (1, 6, 0),
        t if t == slot(0x1c8) => (3, 1, 0),
        t if t == slot(0x1f0) => (3, 0, 0),
        t if t == slot(0x1d0) || t == slot(0x1c0) || t == slot(0x1e0) => (2, 0, 0),
        t if t == slot(0x128) => (2, 0, 5),
        SET_COLOUR => (2, 1, 3),
        DRAW_QUAD => (1, 4, 0),
        _ => (1, 0, 0),
    }
}

struct Calls {
    seed: u64,
    n: u32,
    log: Vec<(u32, Vec<u32>)>,
}
type A = Rc<RefCell<Calls>>;

fn log_call(a: &A, read: &mut dyn FnMut(u32) -> u32, target: u32, ints: Vec<u32>, floats: Vec<u32>) -> u32 {
    let (_, _, by) = shape(target);
    let mut v = ints;
    if by != 0 {
        let at = v[1];
        v.truncate(1);
        v.extend((0..by as u32).map(|k| read(at + 4 * k)));
    }
    v.extend(floats);
    for x in v.iter_mut() {
        if (0x01f0_0000..0x01ff_0000).contains(x) || (BUF..BUF + 0x100).contains(x) {
            *x = SCRATCH;
        }
    }
    let mut b = a.borrow_mut();
    b.log.push((target, v));
    b.n += 1;
    // (a multiplying mix: xorshift alone keeps successive answers of one call in a fixed relation)
    let mut g = Rng(mix(b.seed ^ (target as u64) << 16 ^ b.n as u64));
    g.next();
    match target {
        t if t == slot(0x130) => RVALS,
        t if t == slot(0x58) || t == slot(0x60) => [640u32, 480, 0x8000_0081, g.next()][g.below(4) as usize],
        _ => g.below(4),
    }
}

struct Ours(A);
impl FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let (ni, nf, _) = shape(target);
        let ints: Vec<u32> = (0..ni).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let floats: Vec<u32> = (0..nf).map(|k| args.get(ni + k).copied().unwrap_or(0)).collect();
        let cell = RefCell::new(m);
        log_call(&self.0, &mut |at| cell.borrow().r32(at), target, ints, floats)
    }
}

const REGIONS: [(u32, u32); 2] = [(ARENA, 0x8000), (0x337c40, 0x40)];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

#[test]
fn transition_record_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let a: A = Rc::new(RefCell::new(Calls { seed: 0, n: 0, log: vec![] }));
    let mut leaves = vec![SET_COLOUR, DRAW_QUAD];
    for off in (0..0x200).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    for f in leaves {
        let a2 = a.clone();
        r.stub(f, move |c, mem| {
            let (ni, nf, _) = shape(f);
            let ints: Vec<u32> = (0..ni).map(|k| c.gpr(4 + k) as u32).collect();
            let floats: Vec<u32> = (0..nf).map(|k| c.f(12 + k).to_bits()).collect();
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &mut |at| cell.borrow_mut().read_u32(at), f, ints, floats);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let methods: [u32; 9] = [0x1fecc8, 0x1fece8, 0x1ff000, 0x1ff2b0, 0x1ff4c8, 0x1ff590, 0x1ff608, 0x1ff648, 0x1ff6a0];
    // (StartIn and StartOut stop a fade through StopFade)
    r.allow(0x1ff6a0);
    let mut g = Rng(0x1fec_0000_0000_0001);
    let mut counts = [0u32; 9];
    // (with the values the EE reads differently: an infinity is the largest float, a denormal 0; and the clock one
    // tick short of 0)
    let fl = |g: &mut Rng| -> f32 {
        let special = [f32::from_bits(0x7f80_0000), f32::MAX, f32::from_bits(1), -f32::from_bits(0x3c88_8889)];
        match g.below(8) {
            k @ 0..=5 => [0.0f32, 0.5, 1.0, 2.0, (g.below(400) as f32) / 100.0, -1.0][k as usize],
            _ => special[g.below(4) as usize],
        }
    };
    for case in 0..40_000u32 {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, g.next());
        }
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x724, REND);
        r.mem.write_u32(REND + 0x10, VT);
        for off in (0..0x200).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let t = REC;
        r.mem.write_u32(t, g.below(2));
        r.mem.write_u32(t + 4, [0u32, 1, 2, 2, 3, 4, 5][g.below(7) as usize]);
        r.mem.write_u32(t + 8, g.below(5));
        for o in [0xcu32, 0x10, 0x14, 0x18] {
            let v = fl(&mut g);
            r.mem.write_f32(t + o, v);
        }
        r.mem.write_u32(t + 0x1c, g.below(4));
        r.mem.write_u32(RVALS, g.below(4));
        for k in 0..12 {
            let v = fl(&mut g);
            r.mem.write_f32(t + 0x20 + 4 * k, v);
        }
        for k in 0..4 {
            let v = fl(&mut g);
            r.mem.write_f32(RVALS + 4 + 4 * k, v);
        }
        r.mem.write_u32(t + 0x64, g.below(2));
        let which = g.below(9) as usize;
        let addr = methods[which];
        counts[which] += 1;
        let times = [fl(&mut g), fl(&mut g), fl(&mut g)];
        let blend = g.next();
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.n = 0;
            b.log.clear();
        }
        let mut m = snap(&mut r);
        r.cpu.set_gpr(4, t as u64);
        r.cpu.set_gpr(5, COLOUR as u64);
        r.cpu.set_gpr(6, blend as i32 as i64 as u64);
        r.cpu.set_f(12, times[0]);
        r.cpu.set_f(13, times[1]);
        r.cpu.set_f(14, times[2]);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().n = 0;
        let e = &mut Ours(a.clone());
        let mine_ret = match addr {
            0x1fecc8 => Some(construct(&mut m, t)),
            0x1fece8 => {
                update(&mut m, e, t);
                None
            }
            0x1ff000 => {
                draw_fade(&mut m, e, t, BUF);
                None
            }
            0x1ff2b0 => {
                start_in(&mut m, e, t, times[0]);
                None
            }
            0x1ff4c8 => {
                start_out(&mut m, t, times[0]);
                None
            }
            0x1ff590 => {
                hand_saved(&mut m, e, t, BUF);
                None
            }
            0x1ff608 => {
                hand_current(&mut m, e, t);
                None
            }
            0x1ff648 => {
                start(&mut m, t, times, COLOUR, blend);
                None
            }
            _ => {
                stop_fade(&mut m, t);
                None
            }
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
