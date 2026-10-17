//! `tricky_game::broadphase` (board row E7d) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case builds a broadphase with the port's own functions (`init`, two to fourteen boxes made,
//! given corners near each other and owners, fitted, some taken out again and some moved), so the axes' lists,
//! the table and the pairs and their links are as the game's would be; then one of the row's functions runs on both
//! sides on random arguments (the values moved first for the sorts). The allocator (a bump), the fill (done for
//! real), the frees and the owners' key virtual (their word at `this`) are stand-ins on both sides. The calls,
//! the answers and the memory are compared.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::Runner;
use tricky_game::broadphase as bp;
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

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x60000;
const BP: u32 = ARENA;
/// A second header, for the inits
const BP2: u32 = ARENA + 0x100;
const BOXES: u32 = ARENA + 0x1000;
const CORNERS: u32 = ARENA + 0x2000;
const OWNERS: u32 = ARENA + 0x3000;
const VTABLES: u32 = ARENA + 0x3800;
const HEAP: u32 = ARENA + 0x8000;
/// The owners' key virtual
const KEY: u32 = 0x01ff_0100;
const NB: u32 = 14;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

trait Io {
    fn r32(&mut self, a: u32) -> u32;
    fn w32(&mut self, a: u32, v: u32);
}
impl Io for Mem {
    fn r32(&mut self, a: u32) -> u32 {
        Mem::r32(self, a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        Mem::w32(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r32(&mut self, a: u32) -> u32 {
        self.read_u32(a)
    }
    fn w32(&mut self, a: u32, v: u32) {
        self.write(a, &v.to_le_bytes()).unwrap()
    }
}
fn inside(a: u32, n: u32) -> bool {
    a >= ARENA && (a as u64 + n as u64) <= (ARENA + SIZE) as u64
}

fn standin(io: &mut dyn Io, func: u32, w: &[u32], next: &Cell<u32>) -> (u32, Call) {
    let mut log = Vec::new();
    let answer = match func {
        bp::ALLOC => {
            log.extend(&w[..3]);
            let a = next.get();
            next.set(a + ((w[1] + 15) & !15));
            a
        }
        bp::MEMSET => {
            log.extend(&w[..3]);
            if inside(w[0], w[2]) {
                for k in (0..w[2]).step_by(4) {
                    io.w32(w[0] + k, w[1]);
                }
            }
            0
        }
        bp::FREE | bp::DELETE => {
            log.push(w[0]);
            0
        }
        KEY => {
            log.push(w[0]);
            io.r32(w[0])
        }
        _ => panic!("no stand-in for {func:#x}"),
    };
    (answer, Call(func, log))
}

struct Env<'a> {
    log: Vec<Call>,
    next: &'a Cell<u32>,
}
impl TextBoxEnv for Env<'_> {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], _: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(8, 0);
        let (a, c) = standin(m, func, &w, self.next);
        self.log.push(c);
        assert!(self.log.len() < 100_000, "the port calls on and on");
        a
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}
fn bx(k: u32) -> u32 {
    BOXES + 0x90 * k
}
fn corners(g: &mut Rng, m: &mut Mem, k: u32) {
    for j in 0..3 {
        let c = { let v = g.f(-10.0, 10.0); g.pick(&[v, v, v.round()]) };
        let h = { let v = g.f(0.5, 6.0); g.pick(&[v, v, v.round().max(1.0)]) };
        wf(m, CORNERS + 0x20 * k + 4 * j, c - h);
        wf(m, CORNERS + 0x20 * k + 0x10 + 4 * j, c + h);
    }
}
/// The live pairs, oldest first
fn live(m: &Mem) -> Vec<u32> {
    let mut v = Vec::new();
    let mut p = m.r32(BP + 0x20);
    // (bounded: a broken list must fail, not fill the memory)
    while p != 0 && v.len() < 4000 {
        v.push(p);
        p = m.r32(p + 0x18);
    }
    v
}
/// The ends on an axis
fn ends(m: &Mem, axis: u32) -> Vec<u32> {
    let mut v = Vec::new();
    let mut e = m.r32(BP + 0x2c + 8 * axis);
    while e != 0 && v.len() < 4000 {
        v.push(e);
        e = m.r32(e + 0xc);
    }
    v
}
/// A box's pairs, first on
fn box_pairs(m: &Mem, b: u32) -> Vec<u32> {
    let mut v = Vec::new();
    let mut p = m.r32(b + 4);
    while p != 0 && v.len() < 200 {
        v.push(p);
        p = m.r32(p + 0x10 + 4 * (m.r32(p + 4) == b) as u32);
    }
    v
}

#[test]
fn broadphase_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x14f608u32, 0x14f6c8, 0x14f720, 0x14f740, 0x14f760, 0x14f770, 0x14f780, 0x14f7a8, 0x14f7e0, 0x14f820, 0x14f8c8,
        0x14f920, 0x14f940, 0x14f960, 0x14fa00, 0x14fa70, 0x14fb38, 0x14fd80, 0x14fdb8, 0x14fe18, 0x14fe68, 0x14fec8,
        0x14ffc8, 0x150248, 0x1503b0, 0x1503e0, 0x150428, 0x110838, 0x1506d8, 0x150740, 0x150670, 0x1507e8, 0x150818,
        0x150848,
    ];
    for f in funcs {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let next: Rc<Cell<u32>> = Rc::default();
    for func in [bp::ALLOC, bp::MEMSET, bp::FREE, bp::DELETE, KEY] {
        let (l, n) = (log.clone(), next.clone());
        r.stub(func, move |c, mem| {
            let w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let (a, call) = standin(mem, func, &w, &n);
            l.borrow_mut().push(call);
            assert!(l.borrow().len() < 100_000, "the original calls on and on");
            c.set_gpr(2, a as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x5ce1_4000_0000_0001);
    let (mut pairs, mut swapped, mut sorted) = (0usize, 0usize, 0usize);
    let per = 300u32;
    for case in 0..funcs.len() as u32 * per {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize])] };
        for k in 0..0x8000 / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        next.set(HEAP);
        let mut env = Env { log: Vec::new(), next: &next };
        // the broadphase, as the race makes it (its lists cleared), and its boxes
        bp::init(&mut m, &mut env, BP);
        for o in [0x20, 0x24, 0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40] {
            m.w32(BP + o, 0);
        }
        let nb = 2 + g.below(NB - 2);
        let mut fitted = Vec::new();
        for k in 0..NB {
            let b = bx(k);
            bp::proxy_init(&mut m, b);
            corners(&mut g, &mut m, k);
            bp::proxy_set_box(&mut m, b, CORNERS + 0x20 * k, CORNERS + 0x20 * k + 0x10);
            let (o, vt) = (OWNERS + 0x40 * k, VTABLES + 0x40 * k);
            m.w32(b + 0x84, o);
            m.w32(o + 8, vt);
            let adj = g.pick(&[0u16, 0x10]);
            m.w16(vt + 0x30, adj);
            m.w32(vt + 0x34, KEY);
            let any = g.next();
            m.w32(o + adj as u32, g.pick(&[1u32, 2, 3, 0x8000_0001, any]));
            if k < nb {
                bp::fit_box(&mut m, BP, b);
                fitted.push(b);
            }
        }
        for _ in 0..g.below(3) {
            if fitted.len() > 2 {
                let i = g.below(fitted.len() as u32) as usize;
                bp::remove_box(&mut m, BP, fitted.remove(i));
            }
        }
        for _ in 0..g.below(4) {
            let i = g.below(fitted.len() as u32) as usize;
            let k = (fitted[i] - BOXES) / 0x90;
            corners(&mut g, &mut m, k);
            bp::move_box(&mut m, BP, fitted[i]);
        }
        if g.below(2) == 0 {
            bp::sort_pairs(&mut m, &mut env, BP);
        }
        let spare: Vec<u32> = (0..NB).map(bx).filter(|b| !fitted.contains(b)).collect();
        pairs += live(&m).len();
        let which = (case % funcs.len() as u32) as usize;
        let func = funcs[which];
        let rb = |g: &mut Rng| fitted[g.below(fitted.len() as u32) as usize];
        // the values moved before the sorts
        if matches!(func, 0x14fe18 | 0x14fec8 | 0x14fe68 | 0x14ffc8) {
            let n = if matches!(func, 0x14fe18 | 0x14fec8) { 1 + g.below(NB) } else { 1 };
            for _ in 0..n {
                let k = g.below(NB);
                corners(&mut g, &mut m, k);
            }
        }
        // the owners' keys changed before the pair sorts
        if matches!(func, 0x150428 | 0x150848) {
            for k in 0..NB {
                let (o, vt) = (OWNERS + 0x40 * k, VTABLES + 0x40 * k);
                let any = g.next();
                m.w32(o + m.r16(vt + 0x30) as u32, g.pick(&[1u32, 2, 3, 0x8000_0001, any]));
            }
        }
        let axis = g.below(3);
        let with_next = |m: &Mem, g: &mut Rng, axis: u32, other_box: bool| -> Option<u32> {
            let v: Vec<u32> = ends(m, axis)
                .into_iter()
                .filter(|&e| {
                    let n = m.r32(e + 0xc);
                    n != 0 && (!other_box || m.r32(n) != m.r32(e))
                })
                .collect();
            if v.is_empty() { None } else { Some(v[g.below(v.len() as u32) as usize]) }
        };
        let regs: Vec<u32> = match func {
            0x14f608 => vec![BP2, 1 + g.below(40)],
            0x14f6c8 | 0x14f8c8 => vec![g.pick(&[BP + 0x10, BP + 4]), g.below(4)],
            0x14f720 | 0x14f740 => {
                let mut e = bp::pool_at(&m, BP + 0x10, g.below(0x100));
                if func == 0x14f740 {
                    if let Some(&p) = live(&m).last() {
                        e = p;
                    }
                }
                vec![BP + 0x10, e]
            }
            0x14f760 => vec![BP + 0x10, g.below(0x100)],
            // (and below the pool: the index is a signed shift)
            0x14f770 => vec![BP + 0x10, bp::pool_at(&m, BP + 0x10, g.below(0x100)).wrapping_sub(g.pick(&[0, 0x2000])) + g.pick(&[0, 0x10])],
            0x14f780 => {
                let b = g.pick(&spare);
                vec![BP + 0x2c + 8 * axis, b + 8 + 0x14 * axis + g.pick(&[0, 0x3c])]
            }
            0x14f7a8 => {
                let v = ends(&m, axis);
                vec![BP + 0x2c + 8 * axis, v[g.below(v.len() as u32) as usize]]
            }
            0x14f7e0 => match with_next(&m, &mut g, axis, false) {
                Some(e) => vec![BP + 0x2c + 8 * axis, e],
                None => vec![BP + 0x2c + 8 * axis, m.r32(BP + 0x2c + 8 * axis)],
            },
            0x14f820 => vec![BP2, 1 + g.below(40)],
            0x14f920 => vec![BP + 4],
            0x14f940 => vec![BP + 4, g.next()],
            0x14f960 => vec![BP2],
            0x14fa00 => vec![BP, g.below(4)],
            0x14fa70 => vec![BP, spare[g.below(spare.len() as u32) as usize]],
            0x14fb38 => vec![BP, rb(&mut g)],
            0x14fd80 => match live(&m) {
                v if !v.is_empty() => vec![BP, v[g.below(v.len() as u32) as usize]],
                _ => vec![BP, bp::alloc_pair(&mut m, BP)],
            },
            0x14fdb8 | 0x14fe18 | 0x150428 => vec![BP],
            0x14fe68 => vec![BP, rb(&mut g)],
            0x14fec8 => vec![BP, axis],
            0x14ffc8 => vec![BP, axis, rb(&mut g)],
            0x150248 => match with_next(&m, &mut g, axis, true) {
                Some(e) => vec![BP, axis, e],
                None => vec![BP, axis, BOXES + 8],
            },
            0x1503b0 => {
                let i = g.below(0xff);
                vec![BP, i, i + 1 + g.below(0xff - i)]
            }
            0x1503e0 => vec![BP, g.below(0x100), g.below(0x100)],
            0x110838 => {
                let v = live(&m);
                let p = if v.is_empty() { BP + 0x10 } else { v[g.below(v.len() as u32) as usize] };
                let o = rb(&mut g);
                vec![p, g.pick(&[m.r32(p), m.r32(p + 4), o])]
            }
            0x1506d8 => {
                let p = bp::alloc_pair(&mut m, BP);
                let (a, b) = (rb(&mut g), rb(&mut g));
                m.w32(p, a);
                m.w32(p + 4, b);
                vec![g.pick(&[a, b]), p]
            }
            0x150740 => {
                let with: Vec<u32> = fitted.iter().copied().filter(|&b| m.r32(b + 4) != 0).collect();
                if with.is_empty() {
                    let p = bp::alloc_pair(&mut m, BP);
                    let b = rb(&mut g);
                    m.w32(p, b);
                    m.w32(p + 4, rb(&mut g));
                    bp::link(&mut m, b, p);
                    vec![b, p]
                } else {
                    let b = with[g.below(with.len() as u32) as usize];
                    let v = box_pairs(&m, b);
                    vec![b, v[g.below(v.len() as u32) as usize]]
                }
            }
            0x150670 => vec![g.pick(&spare)],
            0x1507e8 | 0x150818 => vec![rb(&mut g), CORNERS + 0x20 * g.below(NB), CORNERS + 0x20 * g.below(NB) + 0x10],
            0x150848 => vec![rb(&mut g)],
            _ => unreachable!(),
        };
        let before: Vec<Vec<u32>> = (0..3).map(|a| ends(&m, a)).collect();
        let order = |m: &Mem| (live(m), fitted.iter().map(|&b| box_pairs(m, b)).collect::<Vec<_>>());
        let was = order(&m);
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        let heap = next.get();
        env.log.clear();
        let w = |k: usize| regs[k];
        let ours: u32 = match func {
            0x14f608 => bp::pool_init(&mut m, &mut env, w(0), w(1)),
            0x14f6c8 | 0x14f8c8 => {
                bp::array_delete(&mut m, &mut env, w(0), w(1));
                u32::MAX
            }
            0x14f720 => bp::pool_take(&mut m, w(0)),
            0x14f740 => {
                bp::pool_give(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14f760 => bp::pool_at(&m, w(0), w(1)),
            0x14f770 => bp::pool_index(&m, w(0), w(1)),
            0x14f780 => {
                bp::list_append(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14f7a8 => {
                bp::list_remove(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14f7e0 => {
                bp::swap_next(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14f820 => bp::ids_init(&mut m, &mut env, w(0), w(1)),
            0x14f920 => bp::ids_take(&mut m, w(0)),
            0x14f940 => {
                bp::ids_give(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14f960 => bp::init(&mut m, &mut env, w(0)),
            0x14fa00 => {
                bp::delete(&mut m, &mut env, w(0), w(1));
                u32::MAX
            }
            0x14fa70 => bp::fit_box(&mut m, w(0), w(1)),
            0x14fb38 => {
                bp::remove_box(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14fd80 => {
                bp::free_pair(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14fdb8 => bp::alloc_pair(&mut m, w(0)),
            0x14fe18 => {
                bp::sort_all(&mut m, w(0));
                u32::MAX
            }
            0x14fe68 => {
                bp::move_box(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14fec8 => {
                bp::sort_axis(&mut m, w(0), w(1));
                u32::MAX
            }
            0x14ffc8 => {
                bp::move_box_on_axis(&mut m, w(0), w(1), w(2));
                u32::MAX
            }
            0x150248 => {
                bp::toggle_overlap(&mut m, w(0), w(1), w(2));
                u32::MAX
            }
            0x1503b0 => bp::pair_index(&m, w(0), w(1) as i32, w(2) as i32) as u32,
            0x1503e0 => bp::pair_flags(&m, w(0), w(1) as i32, w(2) as i32),
            0x150428 => {
                bp::sort_pairs(&mut m, &mut env, w(0));
                u32::MAX
            }
            0x110838 => bp::has_id(&m, w(0), w(1)),
            0x1506d8 => {
                bp::link(&mut m, w(0), w(1));
                u32::MAX
            }
            0x150740 => {
                bp::unlink(&mut m, w(0), w(1));
                u32::MAX
            }
            0x150670 => bp::proxy_init(&mut m, w(0)),
            0x1507e8 | 0x150818 => {
                bp::proxy_set_box(&mut m, w(0), w(1), w(2));
                u32::MAX
            }
            0x150848 => {
                bp::sort_box_pairs(&mut m, &mut env, w(0));
                u32::MAX
            }
            _ => unreachable!(),
        };
        let after: Vec<Vec<u32>> = (0..3).map(|a| ends(&m, a)).collect();
        if after != before {
            swapped += 1;
        }
        if matches!(func, 0x150428 | 0x150848) && order(&m) != was {
            sorted += 1;
        }
        next.set(heap);
        log.borrow_mut().clear();
        for (k, v) in regs.iter().enumerate() {
            r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        let theirs = r.cpu.gpr(2) as u32;
        if ours != u32::MAX {
            assert_eq!(theirs, ours, "case {case} {func:#x} {regs:x?}: the answer");
        }
        assert_eq!(*log.borrow(), env.log, "case {case} {func:#x}: the calls");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x} {regs:x?}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{pairs} live pairs, {swapped} lists changed, {sorted} sorts that moved pairs");
    assert!(pairs > 4000 && swapped > 600 && sorted > 100);
}
