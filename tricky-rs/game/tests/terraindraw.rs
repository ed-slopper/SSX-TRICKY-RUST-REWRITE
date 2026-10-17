//! `tricky_game::terraindraw` (board row G7a1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random world (live cells with slots listing Bezier patches: boxes, corners near
//! and far from the camera, show-off ones), renderer (a camera matrix, the Bezier manager's two LOD distances),
//! game (the view count, the course's draw distances, sometimes none); then `cTerrainObjNode::Draw` or
//! `Patch_DrawVisibleCells`. The cells' visits, the renderer's and the manager's virtuals are logging stand-ins.
//! The memory, the calls and the patch draws (each edge's steps) compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::terraindraw::{self as t, TerrainEnv};

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
fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const APP: u32 = ARENA;
const GAME: u32 = ARENA + 0x100;
const COURSE: u32 = ARENA + 0x500;
const RENDER: u32 = ARENA + 0x600;
const RVT: u32 = ARENA + 0x3000;
const BZ: u32 = ARENA + 0xa00;
const BZVT: u32 = ARENA + 0xa40;
const CAMMAT: u32 = ARENA + 0xb00;
const GRID: u32 = ARENA + 0xc00;
const CELLS: u32 = ARENA + 0x4000;
const TABLE: u32 = ARENA + 0x4400;
const RECS: u32 = ARENA + 0xe00;
const LISTS: u32 = ARENA + 0x1300;
const PATCHES: u32 = ARENA + 0x1500;
const NPATCH: u32 = 8;
const NCELL: u32 = 5;
const RSLOTS: [u32; 6] = [0x20, 0xf8, 0x280, 0x290, 0x2c0, 0x2d0];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    V(u32, u32, Vec<u32>),
    Patch { this: u32, patch: u32, steps: [u8; 4], shorts: [u32; 2], clip: u32 },
}

fn cull_answer(func: u32, min: &[u32; 4]) -> u32 {
    mix(func ^ min[0] ^ min[1].rotate_left(7)) % 4
}
fn vanswer(func: u32, near: u32, far: u32) -> u32 {
    match func {
        f if f == vaddr(0x20) => BZ,
        f if f == vaddr(0xf8) => CAMMAT,
        f if f == vaddr(0x130) => near,
        f if f == vaddr(0x138) => far,
        _ => 0,
    }
}

struct Ours {
    log: Vec<Call>,
    near: u32,
    far: u32,
}
impl TerrainEnv for Ours {
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.log.push(Call::V(func, this, args.to_vec()));
        vanswer(func, self.near, self.far)
    }
    fn cull(&mut self, _m: &mut Mem, func: u32, this: u32, min: [u32; 4], max: [u32; 4]) -> u32 {
        self.log.push(Call::V(func, this, min.iter().chain(max.iter()).copied().collect()));
        cull_answer(func, &min)
    }
    fn draw_patch(&mut self, _m: &mut Mem, _func: u32, this: u32, patch: u32, steps: [u8; 4], shorts: [u32; 2], clip: u32) {
        self.log.push(Call::Patch { this, patch, steps, shorts, clip });
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn terrain_draw_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x1c7810u32, 0x2008f8, 0x25ecb8, 0x102f50, 0x260278, 0x260458, 0x260f90, 0x260f98, 0x25ec88, 0x25ecc8] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let dists = Rc::new(RefCell::new((0u32, 0u32)));
    for off in RSLOTS.into_iter().map(vaddr).chain([vaddr(0x130), vaddr(0x138)]) {
        let (l, d) = (log.clone(), dists.clone());
        r.stub(off, move |c, m| {
            let this = c.gpr(4) as u32;
            if off == vaddr(0x280) || off == vaddr(0x290) {
                let (a, b) = (c.gpr(5) as u32, c.gpr(6) as u32);
                let min: [u32; 4] = std::array::from_fn(|k| m.read_u32(a + 4 * k as u32));
                let max: [u32; 4] = std::array::from_fn(|k| m.read_u32(b + 4 * k as u32));
                c.set_gpr(2, cull_answer(off, &min) as u64);
                l.borrow_mut().push(Call::V(off, this, min.iter().chain(max.iter()).copied().collect()));
            } else if off == vaddr(0x2d0) {
                let (patch, sa, ia) = (c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32);
                let mut steps = [0u8; 4];
                m.read(sa, &mut steps).unwrap();
                let shorts = [m.read_u32(ia), m.read_u32(ia + 4)];
                l.borrow_mut().push(Call::Patch { this, patch, steps, shorts, clip: c.gpr(8) as u32 });
            } else {
                let (near, far) = *d.borrow();
                let v = vanswer(off, near, far);
                if off == vaddr(0x130) || off == vaddr(0x138) {
                    c.set_f(0, f32::from_bits(v));
                } else {
                    c.set_gpr(2, v as u64);
                }
                l.borrow_mut().push(Call::V(off, this, vec![]));
            }
        });
    }
    let mut steps = [0u8; 4];
    r.mem.read(t::LOD_STEPS, &mut steps).unwrap();
    let mut g = Rng(0x7e44_0000_0000_0001);
    let (mut patches, mut kinds) = (0usize, [0usize; 9]);
    for case in 0..20_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (t::GAPP, vec![0; 4]),
                (t::WORLD, vec![0; 0x3c + 4 * 0xa2 + 4]),
                (t::SHOW_MODE, vec![0; 4]),
                (t::LOD_STEPS, steps.to_vec()),
                (0, vec![0; 0x300]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // (the world's visible list as random as the game's memory, beyond what the walks reach)
        for k in 0..(0x3c + 4 * 0xa2 + 4) / 4 {
            m.w32(t::WORLD + 4 * k, g.next());
        }
        m.w32(t::GAPP, APP);
        m.w32(APP + 0x724, RENDER);
        let game = if g.below(10) == 0 { 0 } else { GAME };
        m.w32(APP + 0x730, game);
        m.w32(GAME + 0x290, g.below(2));
        m.w32(GAME + 0x298, 1 + g.below(2));
        m.w32(GAME + 0x2c, COURSE);
        m.w32(0x2c, if g.below(2) == 0 { 0 } else { COURSE });
        m.w32(0x298, 1 + g.below(2));
        m.w32(0x290, 0);
        wf(&mut m, COURSE + 0x60, g.f(5000.0, 60000.0));
        wf(&mut m, COURSE + 0x64, g.f(5000.0, 60000.0));
        m.w32(RENDER + 0x10, RVT);
        for off in RSLOTS {
            m.w16(RVT + off, [0u16, 0x10][g.below(2) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(BZ, BZVT);
        for off in [0x30u32, 0x38] {
            m.w16(BZVT + off, 0);
            m.w32(BZVT + off + 4, vaddr(0x100 + off));
        }
        let (near, far) = (g.f(500.0, 3000.0), g.f(4000.0, 12000.0));
        *dists.borrow_mut() = (near.to_bits(), far.to_bits());
        for k in 0..16 {
            wf(&mut m, CAMMAT + 4 * k, if k % 5 == 0 { 1.0 } else if k >= 12 { g.f(-3000.0, 3000.0) } else { g.f(-0.3, 0.3) });
        }
        m.w32(t::SHOW_MODE, [0u32, 3, 5, 1][g.below(4) as usize]);
        // the grid (3 × 3 TGroups, 1000 a side, its corner at −1500; some missing) and the visible list: TGroups
        // with boxes, masks, the slots' wholly-inside bits, their own boxes, contents and patch lists
        m.w32(t::WORLD, GRID);
        wf(&mut m, GRID + 4, -1500.0);
        wf(&mut m, GRID + 8, -1300.0);
        wf(&mut m, GRID + 0x28, 1000.0);
        m.w32(GRID + 0x2c, 3);
        m.w32(GRID + 0x30, 3);
        m.w32(GRID + 0x4c, TABLE);
        for k in 0..9 {
            m.w32(TABLE + 4 * k, if g.below(6) == 0 { 0 } else { CELLS + 0x40 * (k % NCELL) });
        }
        m.w32(GRID + 0x48, 1 + g.below(3));
        let ncell = g.below(NCELL + 1);
        for c in 0..0xa2 {
            let hole = ncell > 1 && c == ncell - 2 && case % 3 == 0;
            // (past the first empty place, whatever was there: the walks never go beyond it)
            let v = if c < ncell && !hole { CELLS + 0x40 * c } else if c <= ncell { 0 } else { g.next() | 1 };
            m.w32(t::WORLD + 0x3c + 4 * c, v);
        }
        m.w32(t::WORLD + 0x3c + 0x288, g.below(4));
        for c in 0..NCELL {
            let at = CELLS + 0x40 * c;
            for k in 0..6 {
                wf(&mut m, at + 4 * k, g.f(-2000.0, 2000.0));
            }
            m.w16(at + 0x36, g.below(8) as u16);
            m.w16(at + 0x32, g.below(8) as u16);
            m.w32(at + 0x38, RECS + 0x100 * c);
            for s in 0..3 {
                let rec = RECS + 0x100 * c + 0x4c * s;
                for k in 0..6 {
                    wf(&mut m, rec + 4 * k, g.f(-2000.0, 2000.0));
                }
                let full = g.below(4) != 0;
                m.w32(rec + 0x28, if full && g.below(2) == 0 { 1 } else { 0 });
                m.w16(rec + 0x2c, if full && g.below(2) == 0 { 1 } else { 0 });
                m.w32(rec + 0x48, if full && g.below(2) == 0 { 1 } else { 0 });
                m.w16(rec + 0x26, if g.below(4) == 0 { 1 + g.below(3) as u16 } else { 0 });
                m.w16(rec + 0x24, [0u16, 1, 2, 4, 0xffff][g.below(5) as usize]);
                let list = LISTS + 0x10 * (3 * c + s);
                m.w32(rec + 0x34, list);
                for k in 0..4 {
                    m.w32(list + 4 * k, PATCHES + 0x1c0 * g.below(NPATCH));
                }
            }
        }
        // the patches: corners near (within the near distance) and far, show-off ones
        let reach = [g.f(100.0, 2000.0), g.f(2000.0, 15000.0)];
        for p in 0..NPATCH {
            let at = PATCHES + 0x1c0 * p;
            m.w32(at + 0x1ac, if g.below(4) == 0 { 0x8000_0000 | g.next() } else { g.below(100) });
            for j in 0..4 {
                let r = reach[g.below(2) as usize];
                for k in 0..3 {
                    wf(&mut m, at + 0x168 + 0x10 * j + 4 * k, g.f(-r, r));
                }
            }
        }
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours { log: Vec::new(), near: near.to_bits(), far: far.to_bits() };
        // (the camera's place, for the visits)
        let view = m.r32(GAME + 0x290);
        for k in 0..2 {
            wf(&mut m, GAME + 0xb0 + 0x80 * view + 4 * k, g.f(-2500.0, 2500.0));
        }
        r.mem.write(ARENA, &m.regions[0].1).unwrap();
        let kind = g.below(4);
        let (func, args) = match kind {
            0 => {
                t::terrain_draw(&mut m, &mut ours);
                (0x1c7810u32, vec![ARENA])
            }
            1 => {
                t::patch_draw_visible(&mut m, &mut ours, t::WORLD);
                (0x2008f8, vec![t::WORLD])
            }
            2 => {
                let (pos, rr) = (GAME + 0xb0, [0u32, 1, 2, 4, u32::MAX][g.below(5) as usize]);
                t::visit_cells_near(&mut m, t::WORLD, pos, rr as i32);
                (0x260278, vec![t::WORLD, pos, rr])
            }
            _ => {
                t::cull_visibility(&mut m, &mut ours, t::WORLD, GAME + 0xb0, tricky_game::ps2f::F(1.0));
                (0x260458, vec![t::WORLD, GAME + 0xb0])
            }
        };
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != ours.log {
            let i = theirs.iter().zip(&ours.log).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.log.len()));
            panic!("case {case}: {func:#x} call {i}: theirs {:?} ours {:?}", theirs.get(i), ours.log.get(i));
        }
        for c in &ours.log {
            if let Call::Patch { steps, .. } = c {
                patches += 1;
                for s in steps {
                    kinds[*s as usize] += 1;
                }
            }
        }
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{patches} patches drawn, edge steps {kinds:?}");
    assert!(patches > 10_000 && kinds[4] > 1000 && kinds[6] > 1000 && kinds[8] > 1000);
}
