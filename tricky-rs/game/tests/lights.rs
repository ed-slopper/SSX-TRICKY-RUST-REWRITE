//! `tricky_game::lights` (board row G7d1) against the original in the function runner (TRICKY_ELF; skipped without
//! it): every case a fresh random world grid (3 × 3 TGroups, some missing, of 2 × 2 INodes listing lights of every
//! kind, flag, falloff and power, some within 100 of the point), random quads and matrices; then one of 10
//! functions. `Vec4_Scale` runs as the original; the delete is a logging stand-in. The memory, the calls and the
//! answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::lights::{self as l, LightEnv};
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
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x6000;
const WORLD: u32 = ARENA;
const GRID: u32 = ARENA + 0x100;
const TABLE: u32 = ARENA + 0x200;
const TGROUPS: u32 = ARENA + 0x300;
const INODES: u32 = ARENA + 0x600;
const LISTS: u32 = ARENA + 0x1600;
const LIGHTS: u32 = ARENA + 0x2000;
const NLIGHTS: u32 = 24;
const OUT: u32 = ARENA + 0x3000;
const POS: u32 = ARENA + 0x3400;
const IN: u32 = ARENA + 0x3800;
const N: u32 = 2;
const TSIZE: f32 = 1000.0;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, u32);

struct Ours(Vec<Call>);
impl LightEnv for Ours {
    fn delete(&mut self, _m: &mut Mem, p: u32) {
        self.0.push(Call(0x114f80, p));
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn lights_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x25a028u32, 0x260d08, 0x102f50] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let lg = log.clone();
    r.stub(0x114f80, move |c, _| lg.borrow_mut().push(Call(0x114f80, c.gpr(4) as u32)));
    let mut g = Rng(0x1167_0000_0000_0001);
    let mut counts = [0usize; 10];
    let (mut kept, mut empty) = (0usize, 0usize);
    for case in 0..40_000u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (l::LIGHT_MAN, vec![0; 4])] };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        m.w32(l::LIGHT_MAN, g.next());
        // the grid: 3 × 3 TGroups of 2 × 2 INodes, TSIZE a side, its corner at −1500
        m.w32(WORLD, GRID);
        wf(&mut m, GRID + 4, -1500.0);
        wf(&mut m, GRID + 8, -1500.0);
        wf(&mut m, GRID + 0x28, TSIZE);
        m.w32(GRID + 0x2c, 3);
        m.w32(GRID + 0x30, 3);
        m.w32(GRID + 0x44, N);
        m.w32(GRID + 0x4c, TABLE);
        for t in 0..9 {
            let tg = TGROUPS + 0x40 * t;
            m.w32(TABLE + 4 * t, if g.below(8) == 0 { 0 } else { tg });
            m.w32(tg + 0x38, INODES + 0x4c * N * N * t);
            for i in 0..N * N {
                let node = INODES + 0x4c * (N * N * t + i);
                let list = LISTS + 0x20 * (N * N * t + i);
                m.w32(node + 0x44, list);
                m.w16(node + 0x2e, [0u16, 1, 2, 4, 8, 0xffff][g.below(6) as usize]);
                for k in 0..8 {
                    m.w32(list + 4 * k, LIGHTS + 0x58 * g.below(NLIGHTS));
                }
            }
        }
        // the point, now and then not on a 16-byte boundary (its x and y read as words, its quad rounded down)
        let pos = POS + if g.below(6) == 0 { 4 * (1 + g.below(3)) } else { 0 };
        // (a third of the time on whole numbers with w 1, and some lights exactly 100 away along x)
        let whole = g.below(3) == 0;
        let (px, py, pz) = if whole {
            (g.below(3600) as f32 - 1800.0, g.below(3600) as f32 - 1800.0, g.below(400) as f32 - 200.0)
        } else {
            (g.f(-1800.0, 1800.0), g.f(-1800.0, 1800.0), g.f(-200.0, 200.0))
        };
        wf(&mut m, pos, px);
        wf(&mut m, pos + 4, py);
        wf(&mut m, pos + 8, pz);
        wf(&mut m, pos + 12, if whole { 1.0 } else { [1.0, 0.0, g.f(-1.0, 2.0)][g.below(3) as usize] });
        for k in 0..NLIGHTS {
            let at = LIGHTS + 0x58 * k;
            m.w32(at, [0u32, 1, 2, 3, 1, 2, 4, u32::MAX][g.below(8) as usize]);
            m.w32(at + 4, g.next() & 0x3ff);
            for c in 0..3 {
                wf(&mut m, at + 0x10 + 4 * c, if g.below(5) == 0 { g.f(-1.0, 0.0) } else { g.f(0.0, 1.5) });
            }
            let d = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
            let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
            for c in 0..3 {
                wf(&mut m, at + 0x1c + 4 * c, d[c as usize] / n);
            }
            let reach = [60.0f32, 150.0, 800.0][g.below(3) as usize];
            wf(&mut m, at + 0x28, px + g.f(-reach, reach));
            wf(&mut m, at + 0x2c, py + g.f(-reach, reach));
            wf(&mut m, at + 0x30, g.f(-reach, reach));
            if whole && g.below(4) == 0 {
                wf(&mut m, at + 0x28, px + 100.0);
                wf(&mut m, at + 0x2c, py);
                wf(&mut m, at + 0x30, pz);
            }
            wf(&mut m, at + 0x4c, g.f(-1.0, 1.0));
            m.w32(at + 0x50, g.below(5));
            wf(&mut m, at + 0x54, [0.0, 1.0, 2.0, 3.5, -1.0, -2.0, g.f(-4.0, 6.0)][g.below(7) as usize]);
        }
        r.mem.write(ARENA, &m.regions[0].1).unwrap();
        r.mem.write(l::LIGHT_MAN, &m.regions[1].1).unwrap();
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let kind = g.below(10) as usize;
        let mut ans = None;
        // outputs: the colours anywhere on a word, the directions on 16 bytes (now and then not)
        let odd = |g: &mut Rng| 4 * g.below(4);
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                let this = OUT + odd(&mut g);
                ans = Some(l::light_man_construct(&mut m, this));
                (0x1ce0f0, vec![this])
            }
            1 => {
                let (this, fl) = (OUT, g.below(4));
                l::light_man_destruct(&mut m, &mut ours, this, fl);
                (0x1ce110, vec![this, fl])
            }
            2 => {
                ans = Some(l::light_man(&m));
                (0x1ce400, vec![])
            }
            3 => {
                let out = OUT + if g.below(5) == 0 { 4 } else { 0 };
                let ins: Vec<u32> = (0..7).map(|k| IN + 0x20 * k + odd(&mut g)).collect();
                for k in 0..0x100 / 4 {
                    wf(&mut m, IN + 4 * k, g.f(-2.0, 2.0));
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                l::lights_build(&mut m, out, ins[0], ins[1], ins[2], ins[3], ins[4], ins[5], ins[6]);
                (0x1ce140, std::iter::once(out).chain(ins).collect())
            }
            4 => {
                let out = OUT + odd(&mut g);
                l::lights_clear(&mut m, out);
                (0x1ce278, vec![out])
            }
            5 => {
                for k in 0..0x80 / 4 {
                    wf(&mut m, IN + 4 * k, g.f(-2.0, 2.0));
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                let (a, b) = (IN + odd(&mut g), IN + 0x40);
                l::mat4_rotate_by(&mut m, a, b);
                (0x1ce310, vec![a, b])
            }
            6 => {
                // three strengths in order (ties now and then), three lights, a new one (its colour's sum)
                let mut s = [g.f(0.0, 2.0), g.f(0.0, 2.0), g.f(0.0, 2.0)];
                s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if g.below(4) == 0 {
                    s[1] = s[[0, 2][g.below(2) as usize]];
                }
                for k in 0..3 {
                    wf(&mut m, IN + 4 * k, s[k as usize]);
                }
                for k in 0..0x80 / 4 {
                    wf(&mut m, IN + 0x10 + 4 * k, g.f(-1.0, 1.0));
                }
                let c = IN + 0xa0 + odd(&mut g);
                if g.below(4) == 0 {
                    // a sum exactly one of the strengths
                    let t = s[g.below(3) as usize];
                    wf(&mut m, c + 4, t);
                    wf(&mut m, c + 8, 0.0);
                    wf(&mut m, c + 12, 0.0);
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                let (cols, dirs, dir) = (OUT + odd(&mut g), OUT + 0x40, IN + 0x80);
                l::insert_strongest(&mut m, IN, cols, dirs, dir, c);
                (0x25a028, vec![IN, cols, dirs, dir, c])
            }
            7 => {
                let tg = TGROUPS + 0x40 * g.below(9);
                let (dx, dy) = (g.f(-700.0, 700.0), g.f(-700.0, 700.0));
                ans = Some(l::cell_at(&m, WORLD, tg, tricky_game::ps2f::F(dx), tricky_game::ps2f::F(dy)));
                r.cpu.set_f(12, dx);
                r.cpu.set_f(13, dy);
                (0x260d08, vec![WORLD, tg])
            }
            _ => {
                let amb = OUT + odd(&mut g);
                let cols = OUT + 0x20 + odd(&mut g);
                let dirs = OUT + 0x80 + if g.below(6) == 0 { 4 } else { 0 };
                l::get_lights_at(&mut m, WORLD, pos, amb, cols, dirs);
                let ks = (0..3).filter(|k| m.r32(dirs + 0x10 * k) != 1f32.to_bits() || m.r32(cols + 0x10 * k + 4) != 0).count();
                kept += ks;
                empty += (ks == 0) as usize;
                (0x25a160, vec![WORLD, pos, amb, cols, dirs])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {func:#x} answer");
        }
        assert_eq!(*log.borrow(), ours.0, "case {case}: {func:#x} calls");
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&theirs), w(b));
            }
        }
    }
    println!("{counts:?}, {kept} lights kept, {empty} gatherings with none");
    assert!(kept > 5000 && empty > 100);
}
