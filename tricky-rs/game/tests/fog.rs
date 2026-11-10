//! `tricky_game::fog` (board row G7c2) against the original in the function runner (TRICKY_ELF; skipped without it):
//! every case a fresh random world (fog definitions with point groups, cells with masks and fog slots of placed
//! volumes, their matrices and boxes), volume objects built from it, a renderer with random matrices, clip box,
//! sprite scale and packet buffer, and a fake vtable; then one of 12 functions. The base object, the allocator, the
//! free, delete, the renderer's matrix update, its virtuals and the packet's submit are logging stand-ins; `qsort`,
//! its comparison and `EAMem_Set` run as the original. The memory (the world, the objects, the sort and sprite
//! scratch, the packet), the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fog::{self as f, FogEnv};
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;

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
const SIZE: u32 = 0x20000;
const APP: u32 = ARENA;
const OBJ30: u32 = ARENA + 0x100;
const CAMMAT: u32 = ARENA + 0x200;
const RENDER: u32 = ARENA + 0x1000;
const VT: u32 = ARENA + 0x3000;
const WD: u32 = ARENA + 0x3400;
const DEFLIST: u32 = ARENA + 0x3500;
const DEFS: u32 = ARENA + 0x3600;
const GROUPS: u32 = ARENA + 0x3800;
const PTS: u32 = ARENA + 0x3a00;
const CELLS: u32 = ARENA + 0x4000;
const RECS: u32 = ARENA + 0x4400;
const PLACED: u32 = ARENA + 0x5000;
const OBJS: u32 = ARENA + 0x6000;
const MAN: u32 = ARENA + 0x6400;
const HEAP: u32 = ARENA + 0x8000;
const PKT: u32 = ARENA + 0x18000;
const NDEF: u32 = 4;
const NCELL: u32 = 5;
const MAXPTS: u32 = 10;
const VIRTUALS: [u32; 11] = [0x1a0, 0x1a8, 0x1c0, 0x1c8, 0x1d0, 0x1e0, 0x1e8, 0x1f0, 0x1f8, 0xf8, 0x280];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Base(u32, u32),
    BaseOut(u32, u32),
    Alloc(u32, u32),
    Free(u32),
    Delete(u32),
    Update(u32),
    R(u32, u32, Vec<u32>),
    Cull(u32, [u32; 4], [u32; 4]),
    Submit(u32, u32, u32),
}

fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

fn cull_answer(min: &[u32; 4], max: &[u32; 4]) -> u32 {
    match mix(min[0] ^ min[1].rotate_left(7) ^ max[2].rotate_left(13)) % 6 {
        0 => 1,
        1 | 2 => 0,
        _ => 2,
    }
}

struct Ours {
    log: Vec<Call>,
    heap: u32,
}
impl FogEnv for Ours {
    fn base_construct(&mut self, _m: &mut Mem, this: u32, a1: u32) {
        self.log.push(Call::Base(this, a1));
    }
    fn base_destruct(&mut self, _m: &mut Mem, this: u32, a1: u32) {
        self.log.push(Call::BaseOut(this, a1));
    }
    fn alloc(&mut self, _m: &mut Mem, name: u32, size: u32) -> u32 {
        self.log.push(Call::Alloc(name, size));
        let p = self.heap;
        self.heap += (size + 15) & !15;
        p
    }
    fn mem_free(&mut self, _m: &mut Mem, p: u32) {
        self.log.push(Call::Free(p));
    }
    fn delete(&mut self, _m: &mut Mem, p: u32) {
        self.log.push(Call::Delete(p));
    }
    fn update_matrices(&mut self, _m: &mut Mem, r: u32) {
        self.log.push(Call::Update(r));
    }
    fn renderer(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.log.push(Call::R(func, this, args.to_vec()));
        if func == vaddr(0xf8) {
            CAMMAT
        } else {
            0
        }
    }
    fn cull_box(&mut self, _m: &mut Mem, func: u32, _this: u32, min: [u32; 4], max: [u32; 4]) -> u32 {
        self.log.push(Call::Cull(func, min, max));
        cull_answer(&min, &max)
    }
    fn submit(&mut self, _m: &mut Mem, r: u32, packet: u32, end: u32) {
        self.log.push(Call::Submit(r, packet, end));
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

/// A volume object built from definition `d` by our own build (the world's state the draws start from)
fn build_obj(m: &mut Mem, at: u32, d: u32, heap: &mut Ours) {
    f::volume_construct(m, at);
    f::volume_build(m, heap, at, d);
}

#[test]
fn fog_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x252318u32, 0x25ecb8, 0x1ca2c8, 0x1ca2e0, 0x1ca320, 0x1ca358, 0x1ca3a8, 0x1ca5a0, 0x1c9b20, 0x1c9b60, 0x1c9ba8,
        0x1c9cd0, 0x1c9d58, 0x102f50, 0x2cc808, 0x2fb5c8, 0x1cad28,
    ] {
        r.allow(a);
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let heap = Rc::new(RefCell::new(HEAP));
    let l = log.clone();
    r.stub(0x17cf50, move |c, _| l.borrow_mut().push(Call::Base(c.gpr(4) as u32, c.gpr(5) as u32)));
    let l = log.clone();
    r.stub(0x17cfe0, move |c, _| l.borrow_mut().push(Call::BaseOut(c.gpr(4) as u32, c.gpr(5) as u32)));
    let (l, h) = (log.clone(), heap.clone());
    r.stub(0x2ccf70, move |c, _| {
        let size = c.gpr(5) as u32;
        l.borrow_mut().push(Call::Alloc(c.gpr(4) as u32, size));
        let p = *h.borrow();
        *h.borrow_mut() += (size + 15) & !15;
        c.set_gpr(2, p as u64);
    });
    let l = log.clone();
    r.stub(0x14a040, move |c, _| l.borrow_mut().push(Call::Free(c.gpr(4) as u32)));
    let l = log.clone();
    r.stub(0x114f80, move |c, _| l.borrow_mut().push(Call::Delete(c.gpr(4) as u32)));
    let l = log.clone();
    r.stub(0x1e85b0, move |c, _| l.borrow_mut().push(Call::Update(c.gpr(4) as u32)));
    let l = log.clone();
    r.stub(0x1e8838, move |c, _| l.borrow_mut().push(Call::Submit(c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32)));
    for off in VIRTUALS {
        let l = log.clone();
        r.stub(vaddr(off), move |c, m| {
            let this = c.gpr(4) as u32;
            match off {
                0x280 => {
                    let (a, b) = (c.gpr(5) as u32, c.gpr(6) as u32);
                    let min = [0, 1, 2, 3].map(|k| m.read_u32(a + 4 * k));
                    let max = [0, 1, 2, 3].map(|k| m.read_u32(b + 4 * k));
                    l.borrow_mut().push(Call::Cull(vaddr(off), min, max));
                    c.set_gpr(2, cull_answer(&min, &max) as u64);
                }
                0xf8 | 0x1a0 | 0x1a8 => {
                    l.borrow_mut().push(Call::R(vaddr(off), this, vec![]));
                    c.set_gpr(2, if off == 0xf8 { CAMMAT as u64 } else { 0 });
                }
                0x1c8 => l.borrow_mut().push(Call::R(vaddr(off), this, vec![c.gpr(5) as u32, c.gpr(6) as u32, c.f(12).to_bits()])),
                0x1e8 | 0x1f0 | 0x1f8 => l.borrow_mut().push(Call::R(vaddr(off), this, vec![c.gpr(5) as u32, c.gpr(6) as u32])),
                _ => l.borrow_mut().push(Call::R(vaddr(off), this, vec![c.gpr(5) as u32])),
            }
        });
    }
    // the classes' vtables (0x3916a8..0x391768) as the exe has them
    let mut vts = vec![0u8; 0xc0];
    r.mem.read(0x3916a8, &mut vts).unwrap();
    let mut g = Rng(0x1ca5_0000_0000_0001);
    let mut counts = [0usize; 12];
    let (mut submits, mut culls) = (0usize, 0usize);
    for case in 0..12_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (f::GAPP, vec![0; 4]),
                (f::WORLD, vec![0; 0x3c + 4 * 0xa2 + 4]),
                (f::SORT, vec![0; 8 * 16]),
                (f::SPRITES, vec![0; 0x30 * 16]),
                (0x3916a8, vts.clone()),
            ],
        };
        for k in 0..0x8000 / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..32 {
            m.w32(f::SORT + 4 * k, g.next());
        }
        m.w32(f::GAPP, APP);
        m.w32(APP + 0x724, RENDER);
        m.w32(APP + 0x30, OBJ30);
        m.w32(RENDER + 0x10, VT);
        for off in VIRTUALS {
            m.w32(VT + off, 0x40);
            m.w32(VT + off + 4, vaddr(off));
        }
        m.w32(RENDER + 0x680, PKT + 0x10 * g.below(4));
        // the renderer's matrices: +0x5b0 (depth), +0x630 (its scale and cap), +0x1770 (screen), the camera's
        for (base, scale) in [(0x5b0u32, 1.0f32), (0x1770, 1.0)] {
            for row in 0..4 {
                for col in 0..4 {
                    let v = if row == col { g.f(0.5, 1.5) } else { g.f(-0.3, 0.3) } * scale;
                    wf(&mut m, RENDER + base + 0x10 * row + 4 * col, v);
                }
            }
        }
        wf(&mut m, RENDER + 0x1770 + 0x30 + 0xc, g.f(400.0, 3000.0));
        wf(&mut m, RENDER + 0x630, g.f(100.0, 60000.0));
        wf(&mut m, RENDER + 0x630 + 0xc, g.f(0.0, 3.0));
        wf(&mut m, RENDER + 0x1730, g.f(0.5, 3.0));
        wf(&mut m, RENDER + 0x1744, g.f(0.5, 3.0));
        // (a clip box about the projected sprites' size, so that some are partly or wholly out on a side)
        for (o, lo, hi) in [(0x17b0u32, -4.0f32, 0.5f32), (0x17b4, -4.0, 0.5), (0x17b8, -2000.0, 3000.0), (0x17c0, -0.5, 4.0), (0x17c4, -0.5, 4.0), (0x17c8, -3000.0, 2000.0)] {
            wf(&mut m, RENDER + o, g.f(lo, hi));
        }
        for k in 0..16 {
            let v = if k % 5 == 0 { g.f(0.5, 1.5) } else if k >= 12 { g.f(-3000.0, 3000.0) } else { g.f(-0.3, 0.3) };
            wf(&mut m, CAMMAT + 4 * k, v);
        }
        // the world: fog definitions with point groups
        m.w32(f::WORLD + 4, WD);
        m.w32(f::WORLD, WD);
        m.w32(WD + 0x30, g.below(NDEF + 1));
        m.w32(WD + 0x70, DEFLIST);
        m.w32(WD + 0x48, 1 + g.below(3));
        for d in 0..NDEF {
            let def = DEFS + 0x40 * d;
            m.w32(DEFLIST + 4 * d, def);
            m.w32(def + 4, 1 + g.below(3));
            let grp = GROUPS + 0x10 * d;
            m.w32(def + 8, grp);
            let data = GROUPS + 0x100 + 0x40 * d;
            m.w32(grp + 4, if g.below(8) == 0 { 0 } else { data });
            let n = g.below(MAXPTS / 3 + 1);
            m.w32(data + 0x20, n);
            let pts = PTS + 0x100 * d;
            m.w32(data + 0x24, pts);
            for p in 0..n {
                let at = pts + 0x1c * p;
                for k in 0..3 {
                    wf(&mut m, at + 4 * k, g.f(-800.0, 800.0));
                }
                for k in 0..3 {
                    wf(&mut m, at + 0xc + 4 * k, g.f(0.0, 1.0));
                }
                wf(&mut m, at + 0x18, g.f(1.0, 40.0));
            }
        }
        // volume objects built for the definitions, placed volumes, cells
        let mut pre = Ours { log: Vec::new(), heap: HEAP };
        for d in 0..NDEF {
            build_obj(&mut m, OBJS + 0x20 * d, DEFS + 0x40 * d, &mut pre);
            m.w32(DEFS + 0x40 * d + 0x14, OBJS + 0x20 * d);
        }
        *heap.borrow_mut() = pre.heap;
        for v in 0..12 {
            let at = PLACED + 0x60 * v;
            for row in 0..4 {
                for col in 0..4 {
                    let x = if row == 3 && col < 3 { g.f(-5000.0, 5000.0) } else if row == col { g.f(0.5, 1.5) } else { g.f(-0.3, 0.3) };
                    wf(&mut m, at + 0x10 * row + 4 * col, x);
                }
            }
            m.w32(at + 0x40, DEFS + 0x40 * g.below(NDEF));
            for k in 0..3 {
                let c = g.f(-15000.0, 15000.0);
                let e = g.f(10.0, 3000.0);
                wf(&mut m, at + 0x44 + 4 * k, c - e);
                wf(&mut m, at + 0x50 + 4 * k, c + e);
            }
        }
        let ncell = g.below(NCELL + 1);
        for c in 0..0xa2 {
            // (now and then a hole, with a cell after it that the walk never reaches)
            let hole = ncell > 1 && c == ncell - 2 && (case % 5 == 0);
            m.w32(f::WORLD + 0x3c + 4 * c, if c < ncell && !hole { CELLS + 0x40 * c } else { 0 });
        }
        for c in 0..NCELL {
            let at = CELLS + 0x40 * c;
            m.w16(at + 0x36, g.below(8) as u16);
            // (every slot tested here, so the answer the draws get is always a fresh one)
            m.w16(at + 0x32, 0);
            m.w32(at + 0x38, RECS + 0x200 * c);
            for s in 0..3 {
                let rec = RECS + 0x200 * c + 0x4c * s;
                let nv = g.below(4);
                m.w16(rec + 0x30, nv as u16);
                m.w32(rec + 0x48, RECS + 0x180 + 0x200 * c + 0x10 * s);
                for k in 0..nv {
                    m.w32(RECS + 0x180 + 0x200 * c + 0x10 * s + 4 * k, PLACED + 0x60 * g.below(12));
                }
            }
        }
        // the camera close to the volumes' middle now and then
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours { log: Vec::new(), heap: *heap.borrow() };
        let kind = g.below(12) as usize;
        let mut ans = None;
        let mut fl = Vec::new();
        let obj = OBJS + 0x20 * g.below(NDEF);
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                let i = [0u32, 1, 3, 4000, g.below(5)][g.below(5) as usize];
                ans = Some(f::fog_def(&m, f::WORLD, i));
                (0x252318, vec![f::WORLD, i])
            }
            1 => {
                ans = Some(f::base_volume_construct(&mut m, obj));
                (0x1ca2c8, vec![obj])
            }
            2 => {
                let fl_ = g.below(4);
                f::base_volume_destruct(&mut m, &mut ours, obj, fl_);
                (0x1ca2e0, vec![obj, fl_])
            }
            3 => {
                ans = Some(f::volume_construct(&mut m, obj));
                (0x1ca320, vec![obj])
            }
            4 => {
                let fl_ = g.below(4);
                if g.below(3) == 0 {
                    m.w32(obj + 4, 0);
                    r.mem.write(ARENA, &m.regions[0].1).unwrap();
                }
                f::volume_destruct(&mut m, &mut ours, obj, fl_);
                (0x1ca358, vec![obj, fl_])
            }
            5 => {
                let def = DEFS + 0x40 * g.below(NDEF);
                f::volume_build(&mut m, &mut ours, obj, def);
                (0x1ca3a8, vec![obj, def])
            }
            6 => {
                let mat = PLACED + 0x60 * g.below(12);
                let clip = g.below(3);
                let a = [1.0, g.f(0.0, 1.0)][g.below(2) as usize];
                fl.push(a);
                f::volume_draw(&mut m, &mut ours, obj, mat, clip, F(a));
                (0x1ca5a0, vec![obj, mat, clip])
            }
            7 => {
                let a1 = g.next();
                ans = Some(f::manager_construct(&mut m, &mut ours, MAN, a1));
                (0x1c9b20, vec![MAN, a1])
            }
            8 => {
                m.w32(MAN + 0xc, g.below(NDEF + 1));
                for k in 0..NDEF {
                    m.w32(MAN + 0x10 + 4 * k, if g.below(4) == 0 { 0 } else { OBJS + 0x20 * k });
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                let fl_ = g.below(4);
                f::manager_destruct(&mut m, &mut ours, MAN, fl_);
                (0x1c9b60, vec![MAN, fl_])
            }
            9 => {
                f::manager_build(&mut m, &mut ours, MAN);
                (0x1c9ba8, vec![MAN])
            }
            10 => {
                m.w32(MAN + 0xc, g.below(NDEF + 1));
                for k in 0..NDEF {
                    m.w32(MAN + 0x10 + 4 * k, if g.below(4) == 0 { 0 } else { OBJS + 0x20 * k });
                }
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                f::manager_delete_all(&mut m, &mut ours, MAN);
                (0x1c9cd0, vec![MAN])
            }
            _ => {
                let mut last = 0;
                f::manager_draw(&mut m, &mut ours, MAN, &mut last);
                (0x1c9d58, vec![MAN])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        for (i, v) in fl.iter().enumerate() {
            r.cpu.set_f(12 + i, *v);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {func:#x} answer");
        }
        assert_eq!(*log.borrow(), ours.log, "case {case}: {func:#x} calls");
        submits += ours.log.iter().filter(|c| matches!(c, Call::Submit(..))).count();
        culls += ours.log.iter().filter(|c| matches!(c, Call::Cull(..))).count();
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], b[i]);
            }
        }
    }
    println!("{counts:?}, {submits} packets, {culls} box tests");
    assert!(submits > 100 && culls > 100, "the draws reach the packets and the tests");
}
