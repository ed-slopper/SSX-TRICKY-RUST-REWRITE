//! Live world cells (board row E7b) against the game in the function runner (TRICKY_ELF; skipped without it): a
//! made-up grid of cells, INodes and objects (with and without controllers of each kind, event scripts or none),
//! the AI world's slots driven by the game's `AIWorld_MarkCellsAround`, `AIWorld_UpdateActiveCells`, its ageing,
//! `AIWorld_ReleaseActiveCells` and the whole tick `cAIWorld::vf1` with riders and a camera; every script started
//! and controller call in order, the objects' flags, the slots and the riders' cells compared with
//! `tricky_game::cells`.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::{Mem, Runner};
use tricky_game::cells::{AiCells, CellEnv, RiderCell, Slot};
use tricky_game::world::{Cell, Node, World};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Spawn(i32, usize),
    Wants(usize),
    C7c(usize),
    C74(usize),
    Kill(usize),
}

const W: u32 = 0x346488;
const MODE: u32 = 0x32de8c;
const NOBJ: usize = 40;

struct Env {
    flags: Vec<u32>,
    ctrl: Vec<Option<u32>>,
    scripts: Vec<[i32; 7]>,
    wants: Vec<bool>,
    w7c: Vec<bool>,
    log: Vec<Call>,
}

impl CellEnv for Env {
    fn flags(&mut self, o: usize) -> &mut u32 {
        &mut self.flags[o]
    }
    fn controller(&self, o: usize) -> Option<u32> {
        self.ctrl[o]
    }
    fn event_script(&self, o: usize, e: u32) -> i32 {
        self.scripts[o][e as usize]
    }
    fn spawn(&mut self, s: i32, o: usize) {
        self.log.push(Call::Spawn(s, o));
    }
    fn ctrl_wants(&mut self, o: usize) -> bool {
        self.log.push(Call::Wants(o));
        self.wants[o]
    }
    fn ctrl_7c(&mut self, o: usize) -> bool {
        self.log.push(Call::C7c(o));
        self.w7c[o]
    }
    fn ctrl_74(&mut self, o: usize) {
        self.log.push(Call::C74(o));
    }
    fn ctrl_kill(&mut self, o: usize) {
        self.log.push(Call::Kill(o));
    }
}

fn wf(m: &mut Mem, a: u32, v: f32) {
    m.write_f32(a, v);
}

fn h16(m: &mut Mem, a: u32, v: i16) {
    m.write(a, &v.to_le_bytes()).unwrap();
}

fn r16(m: &mut Mem, a: u32) -> i16 {
    let mut b = [0u8; 2];
    m.read(a, &mut b).unwrap();
    i16::from_le_bytes(b)
}

/// per-case memory from a fixed region (the test heap would fill), zeroed
fn bump(m: &mut Mem, bp: &mut u32, n: u32) -> u32 {
    let at = *bp;
    *bp = (at + n + 15) & !15;
    m.write(at, &vec![0u8; n as usize]).unwrap();
    at
}

#[test]
fn cells_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [0x1160f8, 0x116420, 0x1162f8, 0x1163a0, 0x1162c0, 0x25fed0, 0x260108, 0x25fe78, 0x115e28, 0x251d70, 0x173c18, 0x11d438] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::new(RefCell::new(vec![]));
    let objs = r.mem.alloc(0x100 * NOBJ as u32);
    let phys = r.mem.alloc(0x20 * NOBJ as u32);
    let ctrls = r.mem.alloc(0x40 * NOBJ as u32);
    let vt = r.mem.alloc(0x100);
    let fns = r.mem.alloc(0x40);
    let obj_of = move |a: u32| ((a - objs) / 0x100) as usize;
    let ctrl_obj = move |a: u32| ((a - ctrls) / 0x40) as usize;
    let answers: Rc<RefCell<(Vec<bool>, Vec<bool>)>> = Rc::new(RefCell::new((vec![], vec![])));
    for (k, off) in [(0u32, 0x60u32), (1, 0x78), (2, 0x70), (3, 0xb8)] {
        r.mem.write_u32(vt + off, 0);
        r.mem.write_u32(vt + off + 4, fns + 8 * k);
        let (l, a) = (log.clone(), answers.clone());
        r.stub(fns + 8 * k, move |c, _| {
            let o = ctrl_obj(c.gpr(4) as u32);
            match k {
                0 => {
                    l.borrow_mut().push(Call::Wants(o));
                    c.set_gpr(2, a.borrow().0[o] as u64);
                }
                1 => {
                    l.borrow_mut().push(Call::C7c(o));
                    c.set_gpr(2, a.borrow().1[o] as u64);
                }
                2 => l.borrow_mut().push(Call::C74(o)),
                _ => l.borrow_mut().push(Call::Kill(o)),
            }
        });
    }
    let l = log.clone();
    r.stub(0x13d538, move |c, _| l.borrow_mut().push(Call::Spawn(c.gpr(4) as i32, obj_of(c.gpr(5) as u32))));
    // the made-up .ssf: its event slots
    let ssf = r.mem.alloc(0x20);
    let slots = r.mem.alloc(0x1c * NOBJ as u32);
    r.mem.write_u32(ssf + 0x10, slots);
    let grid = r.mem.alloc(0x60);
    let ai = r.mem.alloc(0x600);
    let mut g = Rng(0xce11_5000_0000_0001);
    let mut counts = [0usize; 3];
    for case in 0..400 {
        let mut bp = 0x80_0000u32;
        // the grid
        let (width, height, per) = (g.pick(&[3i32, 6, 9]), g.pick(&[2i32, 5, 7]), g.pick(&[1i32, 4, 16]));
        let size = g.pick(&[10000.0f32, 2500.0]);
        let origin = [g.pick(&[-20000.0f32, 0.0, -3000.0]), g.pick(&[-15000.0f32, 0.0])];
        let table = bump(&mut r.mem, &mut bp, 4 * (width * height) as u32);
        let mut cells = vec![];
        for k in 0..(width * height) as u32 {
            if g.next() % 5 == 0 {
                r.mem.write_u32(table + 4 * k, 0);
                cells.push(None);
                continue;
            }
            let tg = bump(&mut r.mem, &mut bp, 0x40);
            let nodes_at = bump(&mut r.mem, &mut bp, 0x4c * per as u32);
            r.mem.write_u32(tg + 0x38, nodes_at);
            r.mem.write_u32(table + 4 * k, tg);
            let mut nodes = vec![];
            for n in 0..per as u32 {
                let p = nodes_at + 0x4c * n;
                let all = g.pick(&[0usize, 0, 1, 3, 6]);
                let some = all.min(g.pick(&[0usize, 1, 2, 6]));
                let list: Vec<usize> = (0..all).map(|_| g.next() as usize % NOBJ).collect();
                let la = bump(&mut r.mem, &mut bp, 4 * all.max(1) as u32);
                for (i, o) in list.iter().enumerate() {
                    r.mem.write_u32(la + 4 * i as u32, objs + 0x100 * *o as u32);
                }
                h16(&mut r.mem, p + 0x26, some as i16);
                h16(&mut r.mem, p + 0x28, all as i16);
                r.mem.write_u32(p + 0x38, la);
                nodes.push(Node { min: [0.0; 3], max: [0.0; 3], patches: vec![], objects: list, n_objects: some as i16, n_objects_showoff: all as i16, blank: false });
            }
            cells.push(Some(Cell { min: [0.0; 3], max: [0.0; 3], nodes }));
        }
        let world = World { origin, size, width, height, nodes_per_cell: per, cells, ..Default::default() };
        let m = &mut r.mem;
        wf(m, grid + 4, origin[0]);
        wf(m, grid + 8, origin[1]);
        wf(m, grid + 0x28, size);
        m.write_u32(grid + 0x2c, width as u32);
        m.write_u32(grid + 0x30, height as u32);
        m.write_u32(grid + 0x48, per as u32);
        m.write_u32(grid + 0x4c, table);
        m.write_u32(W, grid);
        m.write_u32(W + 0x14, ssf);
        // the objects
        let mut env = Env { flags: vec![], ctrl: vec![], scripts: vec![], wants: vec![], w7c: vec![], log: vec![] };
        for o in 0..NOBJ as u32 {
            let a = objs + 0x100 * o;
            let fl = g.next() & !0x100 | g.pick(&[0, 0x100]);
            let ctrl = g.pick(&[None, None, Some(5u32), Some(2), Some(9)]);
            let sl = g.pick(&[-1i16, o as i16]);
            let sc = [0, 1, 2, 3, 4, 5, 6].map(|_| g.pick(&[-1i32, 3, 17, 40]));
            m.write_u32(a + 0xe8, fl);
            m.write_u32(a + 0xec, phys + 0x20 * o);
            h16(m, phys + 0x20 * o + 0x14, sl);
            for (e, s) in sc.iter().enumerate() {
                m.write_u32(slots + 0x1c * o + 4 * e as u32, *s as u32);
            }
            let ca = ctrls + 0x40 * o;
            m.write_u32(a + 0xe4, if ctrl.is_some() { ca } else { 0 });
            m.write_u32(ca + 8, vt);
            m.write_u32(ca + 0x14, ctrl.unwrap_or(0));
            env.flags.push(fl);
            env.ctrl.push(ctrl);
            env.scripts.push(if sl < 0 { [-1; 7] } else { sc });
            env.wants.push(g.next() % 2 == 0);
            env.w7c.push(g.next() % 2 == 0);
        }
        *answers.borrow_mut() = (env.wants.clone(), env.w7c.clone());
        let show = g.next() % 3 == 0;
        r.mem.write_u32(MODE, if show { g.pick(&[3u32, 5]) } else { g.pick(&[0u32, 2]) });
        // the AI world, empty
        for o in (0..0x600).step_by(4) {
            r.mem.write_u32(ai + o, 0);
        }
        let mut cells_ai = AiCells::default();
        // riders and the tick's world
        let app = bump(&mut r.mem, &mut bp, 0x800);
        let race = bump(&mut r.mem, &mut bp, 0x100);
        let rlist = bump(&mut r.mem, &mut bp, 0x100);
        let cam = bump(&mut r.mem, &mut bp, 0x440);
        r.mem.write_u32(0x337c58, app);
        r.mem.write_u32(app + 0x730, race);
        r.mem.write_u32(race + 0x1c, rlist);
        r.mem.write_u32(race + 0xa0, cam);
        r.mem.write_u32(0x337774, 0);
        let nr = g.pick(&[1usize, 2, 4]);
        let mut riders: Vec<RiderCell> = vec![];
        let rbase = bump(&mut r.mem, &mut bp, 0x500 * nr as u32);
        r.mem.write_u32(rlist + 0x88, nr as u32);
        for i in 0..nr as u32 {
            r.mem.write_u32(rlist + 0xc4 + 4 * i, rbase + 0x500 * i);
            riders.push(RiderCell::default());
        }
        let place = |g: &mut Rng| [origin[0] + (g.next() % 1000) as f32 / 1000.0 * size * width as f32 * 1.1 - size * 0.05, origin[1] + (g.next() % 1000) as f32 / 1000.0 * size * height as f32 * 1.1 - size * 0.05, 0.0, 1.0];
        for step in 0..8 {
            let what = g.next() % 5;
            log.borrow_mut().clear();
            env.log.clear();
            match what {
                0 => {
                    let p = place(&mut g);
                    let pa = bump(&mut r.mem, &mut bp, 16);
                    for (k, v) in p.iter().enumerate() {
                        wf(&mut r.mem, pa + 4 * k as u32, *v);
                    }
                    r.cpu.set_gpr(4, ai as u64);
                    r.cpu.set_gpr(5, pa as u64);
                    r.call(0x1160f8).unwrap();
                    cells_ai.mark_around(&world, p);
                }
                1 => {
                    r.cpu.set_gpr(4, ai as u64);
                    r.call(0x1162f8).unwrap();
                    cells_ai.update(&world, show, &mut env);
                }
                2 => {
                    r.cpu.set_gpr(4, ai as u64);
                    r.call(0x1162c0).unwrap();
                    cells_ai.age();
                }
                3 if step > 5 => {
                    r.cpu.set_gpr(4, ai as u64);
                    r.call(0x1163a0).unwrap();
                    cells_ai.release(&world, show, &mut env);
                }
                _ => {
                    // the whole tick
                    let race_1c = g.pick(&[1i32, 2, 4]);
                    r.mem.write_u32(rlist + 0x1c, race_1c as u32);
                    let cp = place(&mut g);
                    for (k, v) in cp.iter().take(3).enumerate() {
                        wf(&mut r.mem, cam + 0x42c + 4 * k as u32, *v);
                    }
                    for (i, rd) in riders.iter_mut().enumerate() {
                        let ra = rbase + 0x500 * i as u32;
                        if g.next() % 2 == 0 {
                            rd.pos = place(&mut g);
                        }
                        rd.prev = if g.next() % 2 == 0 { rd.cell } else { g.pick(&[0i32, 3, -1]) };
                        for (k, v) in rd.pos.iter().enumerate() {
                            wf(&mut r.mem, ra + 0x140 + 4 * k as u32, *v);
                        }
                        r.mem.write_u32(ra + 0x42c, rd.prev as u32);
                        r.mem.write_u32(ra + 0x16c, rd.cell as u32);
                    }
                    r.cpu.set_gpr(4, ai as u64);
                    r.call(0x115e28).unwrap();
                    cells_ai.tick(&world, &mut riders, race_1c, cp, show, &mut env);
                    for (i, rd) in riders.iter().enumerate() {
                        assert_eq!(r.mem.read_u32(rbase + 0x500 * i as u32 + 0x16c) as i32, rd.cell, "case {case} step {step}: rider {i} cell");
                    }
                    counts[2] += 1;
                }
            }
            let what_s = format!("case {case} step {step} ({what})");
            assert_eq!(*log.borrow(), env.log, "{what_s}: calls");
            counts[0] += env.log.len();
            let m = &mut r.mem;
            for o in 0..NOBJ {
                assert_eq!(m.read_u32(objs + 0x100 * o as u32 + 0xe8), env.flags[o], "{what_s}: object {o} flags");
            }
            for (k, s) in cells_ai.slots.iter().enumerate() {
                let at = ai + 0xc + 8 * k as u32;
                let cell = m.read_u32(at);
                let want_cell = s.cell.map_or(0, |c| m.read_u32(table + 4 * c as u32));
                let theirs = Slot { cell: s.cell, seen: r16(m, at + 4), new: r16(m, at + 6), index: m.read_u32(ai + 0x36c + 4 * k as u32) as i32 };
                assert_eq!(cell, want_cell, "{what_s}: slot {k} cell");
                if s.cell.is_some() {
                    assert_eq!(theirs, *s, "{what_s}: slot {k}");
                    counts[1] += 1;
                } else {
                    assert_eq!(theirs.index, if theirs.index == 0 { 0 } else { -1 }, "{what_s}: slot {k} index");
                }
            }
        }
    }
    eprintln!("{} calls, {} held slots checked, {} ticks", counts[0], counts[1], counts[2]);
}
