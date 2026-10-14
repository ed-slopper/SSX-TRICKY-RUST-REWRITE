//! The roller's and the anim objects' load constructors (board row G12m) against the game in the function runner
//! (TRICKY_ELF; skipped without it): a node of each class saved by our (checked) save, then loaded into a fresh node
//! by the game and by us; the world calls are stand-ins that log (the place / matrix / transform ones by the values in
//! their stack buffers), every call and all the memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::snapshot::{self as sn, LoadEnv, LoadEnv2};

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

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

#[derive(Default)]
struct World {
    log: Vec<Call>,
    pool: u32,
    radius: u32,
    tree_at: u32,
}

fn place_of(node: u32) -> [u32; 8] {
    core::array::from_fn(|k| (node ^ (k as u32 * 0x111)).rotate_left(k as u32))
}
fn mat_of(pq: [u32; 8]) -> [u32; 16] {
    core::array::from_fn(|k| pq[k % 8] ^ k as u32)
}

struct Ours(World);
impl LoadEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.0.log.push(Call(func, args.to_vec()));
        match func {
            0x239c50 => {
                self.0.radius += 1;
                (0.25 * self.0.radius as f32).to_bits()
            }
            0x260ca0 => self.0.tree_at,
            _ => 0,
        }
    }
    fn pool_alloc(&mut self, _m: &mut Mem, _s: u32, _n: u32) -> u32 {
        unreachable!()
    }
}
impl LoadEnv2 for Ours {
    fn pool_alloc_flags(&mut self, _m: &mut Mem, size: u32, flags: u32, name: u32) -> u32 {
        self.0.log.push(Call(0x23d3b0, vec![size, flags, name]));
        let p = self.0.pool;
        self.0.pool += (size + 0xf) & !0xf;
        p
    }
    fn roller_place(&mut self, _m: &mut Mem, node: u32) -> [u32; 8] {
        self.0.log.push(Call(0x13e6a0, vec![node]));
        place_of(node)
    }
    fn mat4_from_pos_quat(&mut self, _m: &mut Mem, pq: [u32; 8]) -> [u32; 16] {
        self.0.log.push(Call(0x250428, pq.to_vec()));
        mat_of(pq)
    }
    fn sph_tree_set_transform(&mut self, _m: &mut Mem, tree: u32, mat: [u32; 16]) {
        self.0.log.push(Call(0x239d60, [vec![tree], mat.to_vec()].concat()));
    }
}

const PLAIN: [(u32, usize); 12] = [
    (0x1537d0, 1),
    (0x239ba8, 1),
    (0x150670, 1),
    (0x260ca0, 2),
    (0x2391e8, 2),
    (0x239c50, 1),
    (0x123098, 2),
    (0x14fa70, 2),
    (0x1507e8, 3),
    (0x1cb0d8, 2),
    (0x199258, 1),
    (0x142ac8, 1),
];

#[test]
fn last_load_constructors_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x13dee8, 0x198b48, 0x19a0a8, 0x199d68, 0x19a700, 0x1999e8, 0x1cb210, 0x1cb150, 0x1cb960, 0x13a888, 0x17d178, 0x17d090, 0x17cf98,
        0x17d4e8, 0x192ca0, 0x192e40, 0x254a28, 0x1544a8, 0x2cc808, 0x2ce114,
    ] {
        r.allow(a);
    }
    let world = Rc::new(RefCell::new(World::default()));
    for (f, n) in PLAIN.iter().copied().chain([(0x142af0, 3)]) {
        let w = world.clone();
        r.stub(f, move |c, _| {
            let mut args: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            if f == 0x1cb0d8 {
                args.push(c.f(12).to_bits());
            }
            let mut w = w.borrow_mut();
            w.log.push(Call(f, args));
            match f {
                0x239c50 => {
                    w.radius += 1;
                    let v = 0.25 * w.radius as f32;
                    c.set_f(0, v);
                }
                0x260ca0 => {
                    let v = w.tree_at;
                    c.set_gpr(2, v as u64);
                }
                _ => c.set_gpr(2, 0),
            }
        });
    }
    let w = world.clone();
    r.stub(0x23d3b0, move |c, _| {
        let (size, flags, name) = (c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32);
        let mut w = w.borrow_mut();
        w.log.push(Call(0x23d3b0, vec![size, flags, name]));
        let p = w.pool;
        w.pool += (size + 0xf) & !0xf;
        c.set_gpr(2, p as u64);
    });
    let w = world.clone();
    r.stub(0x13e6a0, move |c, m| {
        let (out, node) = (c.gpr(4) as u32, c.gpr(5) as u32);
        w.borrow_mut().log.push(Call(0x13e6a0, vec![node]));
        for (k, v) in place_of(node).iter().enumerate() {
            m.write_u32(out + 4 * k as u32, *v);
        }
    });
    let w = world.clone();
    r.stub(0x250428, move |c, m| {
        let (out, input) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let pq: [u32; 8] = core::array::from_fn(|k| m.read_u32(input + 4 * k as u32));
        w.borrow_mut().log.push(Call(0x250428, pq.to_vec()));
        for (k, v) in mat_of(pq).iter().enumerate() {
            m.write_u32(out + 4 * k as u32, *v);
        }
    });
    let w = world.clone();
    r.stub(0x239d60, move |c, m| {
        let (tree, mat) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let v: Vec<u32> = (0..16).map(|k| m.read_u32(mat + 4 * k)).collect();
        w.borrow_mut().log.push(Call(0x239d60, [vec![tree], v].concat()));
    });
    let src = r.mem.alloc(0x800);
    let dst = r.mem.alloc(0x800);
    let inst = r.mem.alloc(0x100 * 8);
    let model = r.mem.alloc(0x100);
    let extra = r.mem.alloc(0x800);
    let pool = r.mem.alloc(0x2000);
    let trees = r.mem.alloc(0x20);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x80);
    let course = r.mem.alloc(0x80);
    let lists = r.mem.alloc(14 * 0x34 + 0x10);
    let stream = r.mem.alloc(0x10);
    let sbuf = r.mem.alloc(0x2000);
    let regions = [
        (0u32, 0x200u32),
        (0x337c58, 0x18),
        (0x346440, 0x40),
        (0x34648c, 4),
        (0x34649c, 8),
        (src, 0x800),
        (dst, 0x800),
        (inst, 0x800),
        (model, 0x100),
        (extra, 0x800),
        (pool, 0x2000),
        (trees, 0x20),
        (app, 0x800),
        (game, 0x100),
        (ctl, 0x80),
        (course, 0x80),
        (lists, 14 * 0x34 + 0x10),
        (stream, 0x10),
        (sbuf, 0x2000),
    ];
    let snapshot = |r: &mut Runner| {
        let mut m = Mem::default();
        for (a, n) in regions {
            let mut b = vec![0u8; n as usize];
            r.mem.read(a, &mut b).unwrap();
            m.regions.push((a, b));
        }
        m
    };
    let mut g = Rng(0x6c61_7374_6c6f_0d01);
    let mut seen = [0u32; 5];
    for trial in 0..6000 {
        let mut m = snapshot(&mut r);
        for (a, v) in m.regions.iter_mut() {
            if ![0u32, 0x346440].contains(a) {
                v.iter_mut().for_each(|b| *b = (g.next() % 200) as u8);
            }
        }
        m.w32(0x337c58, app);
        m.w32(app + 0x730, game);
        m.w32(game + 0x40, ctl);
        m.w32(ctl + 0x58, 0);
        m.w32(ctl + 0xc, 0x2000);
        m.w32(0x34648c, course);
        m.w32(course + 0x48, inst);
        m.w32(course + 0xc, 8);
        m.w32(0x337c60, 14);
        m.w32(0x337c64, lists + 0x10);
        for l in 0..14 {
            let at = lists + 0x10 + 0x34 * l;
            m.w32(at, (g.below(3) == 0) as u32);
            m.w32(at + 4, at + 0x10);
            m.w32(at + 0x14, at + 4);
            m.w32(at + 0x1c, at + 0x28);
            m.w32(at + 0x2c, at + 0x1c);
        }
        // the instances' models: 0..4 parts, some animated; their positions and boxes small floats
        for i in 0..8 {
            let at = inst + 0x100 * i;
            m.w32(at + 0xc0, model);
            for k in 0..4 {
                m.w32(at + 0x30 + 4 * k, ((g.below(200) as f32 - 100.0) / 3.0).to_bits());
            }
            for k in 0..6 {
                m.w32(at + 0xcc + 4 * k, ((g.below(400) as f32 - 200.0) / 7.0).to_bits());
            }
        }
        m.w32(model + 4, g.below(5));
        m.w32(model + 8, model + 0x20);
        for k in 0..4 {
            m.w32(model + 0x20 + 0x18 * k + 0x10, g.below(2));
        }
        let kind = g.below(5) as usize;
        let (save, this_at) = [(0x13fb90u32, 0u32), (0x1998f8, 0), (0x19a410, 0x1c), (0x199ee0, 0), (0x19a940, 0x18)][kind];
        let load = [0x13dee8u32, 0x198b48, 0x19a0a8, 0x199d68, 0x19a700][kind];
        // the source: its dead node's instance, an overlap or not, its key states, the combo's array
        let dead = if kind == 0 { 0 } else { this_at + 0x30 };
        m.w32(src + dead + 0x28, inst + 0x100 * g.below(8));
        m.w32(src + this_at + 0x64, if kind != 0 && g.below(2) == 0 { extra + 0x10 * g.below(4) } else { 0 });
        m.w32(src + this_at + 0x60, extra + 0x100);
        m.w32(src + this_at + 0x5c, g.below(3));
        if kind == 2 {
            m.w32(src + 0x84, extra + 0x500);
        }
        for k in 0..4u32 {
            m.w32(src + this_at + 0x1e0 + 4 * k, ((g.below(200) as f32 - 100.0) / 5.0).to_bits());
        }
        m.w32(stream, sbuf);
        m.w32(stream + 4, sbuf);
        m.w32(stream + 8, sbuf);
        m.w32(stream + 0xc, 0);
        assert!(sn::save_by_address(&mut m, save, src, stream));
        m.w32(stream + 8, sbuf);
        // the anim objects' saved key count must match the model's animated parts: the destination's instance is
        // the one the link names, whose model decides; the counts may differ (the game reads what it reads)
        for (a, v) in &m.regions {
            r.mem.write(*a, v).unwrap();
        }
        let tree_at = trees + 4 * g.below(4);
        let what = format!("trial {trial} kind {kind}");
        let l = g.below(14);
        let mut ours = Ours(World { pool, tree_at, ..Default::default() });
        match kind {
            0 => sn::roller_load(&mut m, &mut ours, dst, l, stream),
            1 => sn::anim_object_load(&mut m, &mut ours, dst, l, stream),
            2 => sn::anim_combo_load(&mut m, &mut ours, dst, l, stream),
            3 => sn::anim_delta_load(&mut m, &mut ours, dst, l, stream),
            _ => sn::anim_tex_flip_load(&mut m, &mut ours, dst, l, stream),
        }
        *world.borrow_mut() = World { pool, tree_at, ..Default::default() };
        for (k, a) in [dst, l, stream].iter().enumerate() {
            r.cpu.set_gpr(4 + k, *a as u64);
        }
        r.call(load).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        assert_eq!(world.borrow().log, ours.0.log, "{what}: calls");
        let theirs = snapshot(&mut r);
        for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
            if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                let i = i & !3;
                panic!("{what}: memory at {:#x} (+{i:#x} in {a:#x}): {:x?} vs {:x?}", a + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
            }
        }
        seen[kind] += 1;
    }
    eprintln!("kinds {seen:?}");
}
