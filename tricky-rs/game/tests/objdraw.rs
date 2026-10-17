//! `tricky_game::objdraw` (board row G7b1) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random world (a light grid with lights of every kind, live cells with slots
//! listing instances), instances (matrices, light blocks, boxes, flags), models with bone tables, a roller, an
//! animated mesh and an animated object; then one of 5 draws. `World_GetLightsAt`, `Lights_Build` and
//! `Mat4_RotateBy` run as the original; the renderer's virtuals, the light manager's, the objects' matrices and the
//! animation's bones are logging stand-ins. The model draws are logged with the matrix, light block and bones they
//! are given (by content: the game builds them on its stack). The memory (bar the port's scratch), the calls and the
//! draws compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::lights;
use tricky_game::objdraw::{self as o, ObjEnv};
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
fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x10000;
const APP: u32 = ARENA;
const RENDER: u32 = ARENA + 0x100;
const RVT: u32 = ARENA + 0x200;
const LMAN: u32 = ARENA + 0x600;
const LVT: u32 = ARENA + 0x640;
const SUBVT: u32 = ARENA + 0x700;
const GRID: u32 = ARENA + 0x800;
const TABLE: u32 = ARENA + 0x900;
const TGROUPS: u32 = ARENA + 0xa00;
const INODES: u32 = ARENA + 0xd00;
const LLISTS: u32 = ARENA + 0x1d00;
const LIGHTS: u32 = ARENA + 0x2100;
const NLIGHTS: u32 = 16;
const CELLS: u32 = ARENA + 0x2800;
const RECS: u32 = ARENA + 0x2a00;
const ILISTS: u32 = ARENA + 0x3000;
const INSTS: u32 = ARENA + 0x3400;
const NINST: u32 = 6;
const MODELS: u32 = ARENA + 0x4000;
const BONETAB: u32 = ARENA + 0x4100;
const BONES: u32 = ARENA + 0x4c00;
const NODE: u32 = ARENA + 0x5000;
const MAT: u32 = ARENA + 0x5800;
const SCRATCH: u32 = 0x0180_0000;
const NCELL: u32 = 5;
const N: u32 = 2;
const RSLOTS: [u32; 4] = [0x1c8, 0x1e0, 0x280, 0x2a8];
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    C(u32, Vec<u32>),
    V(u32, u32, Vec<u32>),
    Draw { this: u32, model: u32, part: u32, matrix: Vec<u32>, lights: Vec<u32>, bones: Vec<u32> },
}

fn cull_answer(min: &[u32; 4], max: &[u32; 4]) -> u32 {
    mix(min[0] ^ max[1].rotate_left(9)) % 3
}
fn vanswer(func: u32, this: u32) -> u32 {
    if func == vaddr(0x80) {
        (mix(this) % 2) as u32
    } else {
        0
    }
}
/// The stand-ins' matrices: `RollerNode_PlaceFromBody` from the node, `Mat4_FromPosQuat` a place over the grid
fn frame_answer(node: u32) -> [u32; 16] {
    std::array::from_fn(|k| (((mix(node ^ k as u32) >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0).to_bits())
}
fn posquat_answer(input: &[u32; 16]) -> [u32; 16] {
    let place = |k: u32| ((mix(input[k as usize] ^ k) % 3000) as f32 - 1500.0).to_bits();
    let one = 1f32.to_bits();
    [one, 0, 0, 0, 0, one, 0, 0, 0, 0, one, 0, place(0), place(1), place(2), one]
}
fn bone_answer(k: u32) -> u32 {
    BONES + 0x40 * (k % 8)
}
fn read_words(m: &mut dyn FnMut(u32) -> u32, at: u32, n: u32) -> Vec<u32> {
    (0..n).map(|k| m(at + 4 * k)).collect()
}
fn bone_words(m: &mut dyn FnMut(u32) -> u32, model: u32, bones: u32) -> Vec<u32> {
    if bones == 0 {
        return vec![];
    }
    let n = (m(model + 4) as i32).clamp(0, 24) as u32;
    read_words(m, bones, 16 * n)
}

struct Ours(Vec<Call>);
impl ObjEnv for Ours {
    fn cull(&mut self, _m: &mut Mem, func: u32, this: u32, min: [u32; 4], max: [u32; 4]) -> u32 {
        self.0.push(Call::V(func, this, min.iter().chain(max.iter()).copied().collect()));
        cull_answer(&min, &max)
    }
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.0.push(Call::V(func, this, args.to_vec()));
        vanswer(func, this)
    }
    fn draw_model(&mut self, m: &mut Mem, func: u32, this: u32, model: u32, part: u32, matrix: u32, lts: u32, bones: u32) {
        let _ = func;
        let mut rd = |a: u32| m.r32(a);
        let matrix = read_words(&mut rd, matrix, 16);
        let lights = read_words(&mut rd, lts, 32);
        let bones = bone_words(&mut rd, model, bones);
        self.0.push(Call::Draw { this, model, part, matrix, lights, bones });
    }
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        if func == o::POS_QUAT {
            self.0.push(Call::C(func, (0..16).map(|k| m.r32(args[1] + 4 * k)).collect()));
        } else {
            self.0.push(Call::C(func, vec![args[1]]));
        }
        match func {
            o::ROLLER_FRAME => {
                for (k, w) in frame_answer(args[1]).into_iter().enumerate() {
                    m.w32(args[0] + 4 * k as u32, w);
                }
                0
            }
            o::POS_QUAT => {
                let input = std::array::from_fn(|k| m.r32(args[1] + 4 * k as u32));
                for (k, w) in posquat_answer(&input).into_iter().enumerate() {
                    m.w32(args[0] + 4 * k as u32, w);
                }
                0
            }
            _ => bone_answer(args[1]),
        }
    }
}

fn wf(m: &mut Mem, at: u32, v: f32) {
    m.w32(at, v.to_bits());
}

#[test]
fn object_draws_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x200680u32, 0x200390, 0x13f9f8, 0x147518, 0x199320, 0x25ecb8, 0x25a160, 0x25a028, 0x260d08, 0x1ce140, 0x1ce310, 0x102f50] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for off in RSLOTS.into_iter().chain([0x78, 0x80]) {
        let l = log.clone();
        r.stub(vaddr(off), move |c, m| {
            let (f, this) = (vaddr(off), c.gpr(4) as u32);
            match off {
                0x280 => {
                    let (a, b) = (c.gpr(5) as u32, c.gpr(6) as u32);
                    let min = std::array::from_fn(|k| m.read_u32(a + 4 * k as u32));
                    let max = std::array::from_fn(|k| m.read_u32(b + 4 * k as u32));
                    c.set_gpr(2, cull_answer(&min, &max) as u64);
                    l.borrow_mut().push(Call::V(f, this, min.iter().chain(max.iter()).copied().collect()));
                }
                0x2a8 => {
                    let (model, part, mat, lts, bones) = (c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32);
                    assert_eq!(c.gpr(10), 0, "the model draw's last word");
                    let mut rd = |a: u32| m.read_u32(a);
                    let matrix = read_words(&mut rd, mat, 16);
                    let lights = read_words(&mut rd, lts, 32);
                    let bones = bone_words(&mut rd, model, bones);
                    l.borrow_mut().push(Call::Draw { this, model, part, matrix, lights, bones });
                }
                _ => {
                    let args = match off {
                        0x1c8 => vec![c.gpr(5) as u32, c.gpr(6) as u32, c.f(12).to_bits()],
                        0x80 => vec![],
                        _ => vec![c.gpr(5) as u32],
                    };
                    c.set_gpr(2, vanswer(f, this) as u64);
                    l.borrow_mut().push(Call::V(f, this, args));
                }
            }
        });
    }
    let l = log.clone();
    r.stub(o::ROLLER_FRAME, move |c, m| {
        let (out, node) = (c.gpr(4) as u32, c.gpr(5) as u32);
        for (k, w) in frame_answer(node).into_iter().enumerate() {
            m.write_u32(out + 4 * k as u32, w);
        }
        l.borrow_mut().push(Call::C(o::ROLLER_FRAME, vec![node]));
    });
    let l = log.clone();
    r.stub(o::POS_QUAT, move |c, m| {
        let (out, inp) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let input = std::array::from_fn(|k| m.read_u32(inp + 4 * k as u32));
        for (k, w) in posquat_answer(&input).into_iter().enumerate() {
            m.write_u32(out + 4 * k as u32, w);
        }
        l.borrow_mut().push(Call::C(o::POS_QUAT, input.to_vec()));
    });
    let l = log.clone();
    r.stub(o::ANIM_BONE, move |c, _| {
        let k = c.gpr(5) as u32;
        c.set_gpr(2, bone_answer(k) as u64);
        l.borrow_mut().push(Call::C(o::ANIM_BONE, vec![k]));
    });
    let mut g = Rng(0x0bd2_0000_0000_0001);
    let mut counts = [0usize; 5];
    let mut draws = 0usize;
    for case in 0..12_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (o::GAPP, vec![0; 4]),
                (o::WORLD, vec![0; 0x3c + 4 * 0xa2 + 4]),
                (o::SHOW_MODE, vec![0; 4]),
                (o::DRAWN, vec![0; 0x600]),
                (lights::LIGHT_MAN, vec![0; 4]),
                (o::WORLD_MATRIX, vec![0; 0x40]),
                (SCRATCH, vec![0; 0x800]),
            ],
        };
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        for k in 0..0x600 / 4 {
            m.w32(o::DRAWN + 4 * k, g.next());
        }
        // (the port's scratch as random as the game's stack)
        for k in 0..0x800 / 4 {
            m.w32(SCRATCH + 4 * k, g.next());
        }
        for k in 0..16 {
            wf(&mut m, o::WORLD_MATRIX + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
        }
        m.w32(o::GAPP, APP);
        m.w32(APP + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in RSLOTS {
            m.w16(RVT + off, [0u16, 0x10][g.below(2) as usize]);
            m.w32(RVT + off + 4, vaddr(off));
        }
        m.w32(lights::LIGHT_MAN, LMAN);
        m.w32(LMAN, LVT);
        m.w16(LVT + 0x78, 0);
        m.w32(LVT + 0x7c, vaddr(0x78));
        m.w32(o::SHOW_MODE, [0u32, 3, 5, 1][g.below(4) as usize]);
        // the light grid: 3 × 3 TGroups of 2 × 2 INodes, 1000 a side, its corner at −1500; lights near
        m.w32(o::WORLD, GRID);
        wf(&mut m, GRID + 4, -1500.0);
        wf(&mut m, GRID + 8, -1500.0);
        wf(&mut m, GRID + 0x28, 1000.0);
        m.w32(GRID + 0x2c, 3);
        m.w32(GRID + 0x30, 3);
        m.w32(GRID + 0x44, N);
        m.w32(GRID + 0x4c, TABLE);
        m.w32(GRID + 0x48, 1 + g.below(3));
        for t in 0..9 {
            let tg = TGROUPS + 0x40 * t;
            m.w32(TABLE + 4 * t, if g.below(8) == 0 { 0 } else { tg });
            m.w32(tg + 0x38, INODES + 0x4c * N * N * t);
            for i in 0..N * N {
                let node = INODES + 0x4c * (N * N * t + i);
                let list = LLISTS + 0x20 * (N * N * t + i) % 0x400;
                m.w32(node + 0x44, list);
                m.w16(node + 0x2e, g.below(5) as u16);
                for k in 0..8 {
                    m.w32(list + 4 * k, LIGHTS + 0x58 * g.below(NLIGHTS));
                }
            }
        }
        for k in 0..NLIGHTS {
            let at = LIGHTS + 0x58 * k;
            m.w32(at, g.below(4));
            m.w32(at + 4, g.next() & 0x300);
            for c in 0..3 {
                wf(&mut m, at + 0x10 + 4 * c, g.f(0.0, 1.0));
                wf(&mut m, at + 0x1c + 4 * c, g.f(-0.6, 0.6));
                wf(&mut m, at + 0x28 + 4 * c, g.f(-1600.0, 1600.0));
            }
            wf(&mut m, at + 0x4c, g.f(-1.0, 1.0));
            m.w32(at + 0x50, g.below(4));
            wf(&mut m, at + 0x54, [0.0, 1.0, 2.0][g.below(3) as usize]);
        }
        // models with bone tables (parents before their children), instances
        for md in 0..4 {
            let model = MODELS + 0x20 * md;
            m.w32(model + 4, [0u32, 1, 3, 6, 24, u32::MAX][g.below(6) as usize]);
            m.w32(model + 8, BONETAB + 0x18 * 24 * md);
            m.w32(model + 0x10, g.below(4));
        }
        for b in 0..4 * 24 {
            let rec = BONETAB + 0x18 * b;
            let local = b % 24;
            m.w32(rec, if local == 0 || g.below(3) == 0 { u32::MAX } else { g.below(local) });
            m.w32(rec + 0x10, g.below(2));
            m.w32(rec + 0x14, BONES + 0x40 * g.below(8));
        }
        for k in 0..8 * 16 {
            wf(&mut m, BONES + 4 * k, g.f(-1.5, 1.5));
        }
        for i in 0..NINST {
            let inst = INSTS + 0x100 * i;
            for k in 0..16 {
                wf(&mut m, inst + 4 * k, if k == 12 || k == 13 { g.f(-1600.0, 1600.0) } else { g.f(-1.5, 1.5) });
            }
            m.w32(inst + 0xc0, MODELS + 0x20 * g.below(4));
            m.w32(inst + 0xe8, g.below(8));
        }
        // the cells: masks, the slots' wholly-inside bits, instance counts (and the show-off ones) and lists
        let ncell = g.below(NCELL + 1);
        for c in 0..0xa2 {
            // (now and then a hole, with cells after it that the walk never reaches)
            let hole = ncell > 1 && c == ncell - 2 && case % 3 == 0;
            m.w32(o::WORLD + 0x3c + 4 * c, if c < ncell && !hole { CELLS + 0x40 * c } else { 0 });
        }
        for c in 0..NCELL {
            let at = CELLS + 0x40 * c;
            m.w16(at + 0x36, g.below(8) as u16);
            m.w16(at + 0x32, g.below(8) as u16);
            m.w32(at + 0x38, RECS + 0x100 * c);
            for s in 0..3 {
                let rec = RECS + 0x100 * c + 0x4c * s;
                m.w16(rec + 0x26, g.below(4) as u16);
                m.w16(rec + 0x28, g.below(4) as u16);
                let list = ILISTS + 0x40 * (3 * c + s) % 0x400;
                m.w32(rec + 0x38, list);
                for k in 0..4 {
                    m.w32(list + 4 * k, INSTS + 0x100 * g.below(NINST));
                }
            }
        }
        // the node (a roller's, an animated mesh's or an animated object's fields)
        let inst = INSTS + 0x100 * g.below(NINST);
        m.w32(NODE + 0x28, inst);
        m.w32(NODE + 0x58, inst);
        m.w32(NODE + 0x6cc, (g.below(5) != 0) as u32);
        m.w32(NODE + 0x6d0, g.below(2));
        m.w32(NODE + 0x6c0, if g.below(2) == 0 { 0 } else { MAT });
        for k in 0..16 {
            wf(&mut m, MAT + 4 * k, if k == 12 || k == 13 { g.f(-1600.0, 1600.0) } else { g.f(-1.5, 1.5) });
        }
        for k in 0..24 * 4 {
            wf(&mut m, NODE + 0x38 + 4 * k, g.f(-1.0, 1.0));
            wf(&mut m, NODE + 0x3c0 + 4 * k, g.f(-1600.0, 1600.0));
        }
        // (the animated object's +0x30 object: its vtable at +0x38, over the mesh's first quaternion's w)
        m.w32(NODE + 0x38, SUBVT);
        // (and its instance at +0x58, over the mesh's third quaternion's w)
        m.w32(NODE + 0x58, inst);
        m.w16(SUBVT + 0x80, [0u16, 0x10][g.below(2) as usize]);
        m.w32(SUBVT + 0x84, vaddr(0x80));
        for (at, b) in &m.regions {
            if *at != SCRATCH {
                r.mem.write(*at, b).unwrap();
            }
        }
        log.borrow_mut().clear();
        let mut ours = Ours(Vec::new());
        let kind = g.below(5) as usize;
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                let (mat, lt) = (MAT, INSTS + 0x40);
                o::instance_draw(&mut m, &mut ours, inst, mat, lt);
                (0x200680, vec![o::WORLD, inst, mat, lt])
            }
            1 => {
                o::draw_visible_cells(&mut m, &mut ours, o::WORLD);
                (0x200390, vec![o::WORLD])
            }
            2 => {
                o::roller_draw(&mut m, &mut ours, NODE, SCRATCH);
                (0x13f9f8, vec![NODE])
            }
            3 => {
                o::mesh_anim_draw(&mut m, &mut ours, NODE, SCRATCH);
                (0x147518, vec![NODE])
            }
            _ => {
                let test = g.below(2);
                o::anim_object_draw(&mut m, &mut ours, NODE, MAT, test, SCRATCH);
                (0x199320, vec![NODE, MAT, test])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != ours.0 {
            let i = theirs.iter().zip(&ours.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(ours.0.len()));
            panic!("case {case}: {func:#x} call {i}: theirs {:?} ours {:?}", theirs.get(i), ours.0.get(i));
        }
        draws += ours.0.iter().filter(|c| matches!(c, Call::Draw { .. })).count();
        for (at, b) in &m.regions {
            if *at == SCRATCH {
                continue;
            }
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
    }
    println!("{counts:?}, {draws} models drawn");
    assert!(draws > 5000);
}
