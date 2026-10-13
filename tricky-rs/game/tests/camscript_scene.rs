//! `tricky_game::camscript::start_scene` against `CamScript_StartScene` 0x176850 (with `FindRiders` 0x176720,
//! `CamScript_ResetActorAnims` and `Math_AngleDiff` as they are) in the function runner (TRICKY_ELF; skipped without
//! it): the scene, the actor and rider calls stubbed and logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::campath::*;
use tricky_game::camscript::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => 1.0,
            2 => (self.next() % 1000) as f32 / 1000.0,
            _ => (self.next() % 20_000) as f32 / 100.0 - 100.0,
        }
    }
}
fn fl(v: u32) -> f32 {
    (v % 4000) as f32 / 100.0 - 20.0
}

const STRIDE: u32 = 0x8000;

#[derive(Default)]
struct W {
    recs: Vec<SceneRec>,
    slots: Vec<u32>,
    chars: Vec<u32>,
    table: [u32; 8],
    oracle: Vec<u32>,
    pos: usize,
    log: Vec<Vec<u32>>,
    recs_addr: u32,
}
impl W {
    fn next(&mut self) -> u32 {
        let v = self.oracle[self.pos % self.oracle.len()];
        self.pos += 1;
        v
    }
}

struct U(Rc<RefCell<W>>);
impl KeyEnv for U {
    fn key(&mut self, _: i32) -> Key {
        unreachable!()
    }
}
impl ScriptEnv for U {
    fn owner(&mut self, _: u32) -> Owner {
        unreachable!()
    }
    fn smooth(&mut self, _: &mut [u32]) {
        unreachable!()
    }
    fn cut_fade(&mut self, _: &mut [u32]) {
        unreachable!()
    }
    fn audio_cut(&mut self) {
        unreachable!()
    }
    fn find_owner(&mut self) -> u32 {
        unreachable!()
    }
    fn first_position(&mut self, _: u32) -> [f32; 3] {
        unreachable!()
    }
    fn audio_stop(&mut self) {
        unreachable!()
    }
    fn entry_count(&mut self) -> i32 {
        self.0.borrow().slots.len() as i32
    }
    fn reset_rider_anim(&mut self, j: i32) {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![1, j as u32]);
        w.log.push(vec![10, j as u32]);
    }
}
impl SceneEnv for U {
    fn scene_open(&mut self, step: u32) -> [u32; 2] {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![50, step]);
        let _leftover = w.next();
        let w1 = w.next();
        w.log.push(vec![51]);
        [w1, w.recs_addr]
    }
    fn scene_rec(&mut self, _: u32, i: usize) -> SceneRec {
        self.0.borrow().recs[i].clone()
    }
    fn rider_count(&mut self) -> i32 {
        self.0.borrow().slots.len() as i32
    }
    fn rider_slot(&mut self, j: i32) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![1, j as u32]);
        w.slots[j as usize]
    }
    fn rider_char(&mut self, j: i32) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![1, j as u32]);
        w.chars[j as usize]
    }
    fn actor_name(&mut self, ch: u32) -> u32 {
        self.0.borrow().table[ch as usize]
    }
    fn actor_init(&mut self, name: u32, _: u32, i: usize) -> ActorLocal {
        let mut w = self.0.borrow_mut();
        w.log.push(if name != 0 { vec![52, i as u32 * 0x30, name] } else { vec![53, i as u32 * 0x30] });
        let mut a = ActorLocal::default();
        for k in 0..5 {
            a.w[k] = w.next();
        }
        for k in 0..4 {
            a.q[k] = w.next();
        }
        a.f = w.next();
        a
    }
    fn actor_close(&mut self) {
        self.0.borrow_mut().log.push(vec![54]);
    }
    fn scene_point(&mut self, i: i32) -> ScenePoint {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![55, i as u32]);
        let ok = w.next() % 2 == 1;
        let mut s = ScenePoint { ok, vec: [0; 4], out: 0.0 };
        if ok {
            for k in 0..4 {
                s.vec[k] = fl(w.next()).to_bits();
            }
        }
        s.out = fl(w.next());
        s
    }
    fn place_rider(&mut self, j: i32, pos: [u32; 4], zero: [u32; 4], heading: f32) {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![1, j as u32]);
        let mut v = vec![56, j as u32];
        v.extend(pos);
        v.extend(zero);
        v.extend([1, heading.to_bits(), 0]);
        w.log.push(v);
    }
    fn start_anim(&mut self, slot: usize, j: i32, placed: u32, pos: [u32; 4], heading: f32) {
        let mut v = vec![57, slot as u32, j as u32, placed];
        v.extend(pos);
        v.push(heading.to_bits());
        self.0.borrow_mut().log.push(v);
    }
}

#[test]
fn camera_scene_start_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x176850u32, 0x176720, 0x176f38, 0x1741d0] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0xf00);
    let app = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x100);
    let list = r.mem.alloc(0x100);
    let riders = r.mem.alloc(8 * STRIDE);
    let chars = r.mem.alloc(8 * 0x40);
    let recs = r.mem.alloc(8 * 0x30);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, race);
    r.mem.write_u32(race + 0x1c, list);
    let w = Rc::new(RefCell::new(W { recs_addr: recs, ..Default::default() }));
    macro_rules! stub {
        ($addr:expr, $body:expr) => {{
            let l = w.clone();
            r.stub($addr, move |c, m| {
                let f: &dyn Fn(&mut r5900::Cpu, &mut r5900::Mem, &mut W) = &$body;
                f(c, m, &mut l.borrow_mut())
            });
        }};
    }
    stub!(0x1814a8, |c, _, w| {
        assert_eq!(c.gpr(4) as u32, race);
        w.log.push(vec![1, c.gpr(5) as u32]);
        c.set_gpr(2, (riders + STRIDE * c.gpr(5) as u32) as u64);
    });
    stub!(0x15ee30, |c, _, w| {
        let a = c.gpr(4) as u32 - 0x46c0 - riders;
        assert_eq!(a % STRIDE, 0);
        w.log.push(vec![10, a / STRIDE]);
    });
    stub!(0x1720c0, |c, m, w| {
        w.log.push(vec![50, c.gpr(5) as u32]);
        let a = c.gpr(4) as u32;
        let w0 = w.next();
        let w1 = w.next();
        m.write_u32(a, w0);
        m.write_u32(a + 4, w1);
        m.write_u32(a + 8, recs);
    });
    stub!(0x172238, |c, _, w| {
        assert_eq!(c.gpr(5), 2);
        w.log.push(vec![51]);
    });
    stub!(0x16d9d0, |c, m, w| {
        let (a0, a1, a2) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        w.log.push(vec![52, a1 - recs, a2]);
        for k in (1..6).chain(8..12).chain(12..13) {
            let v = w.next();
            m.write_u32(a0 + 4 * k, v);
        }
    });
    stub!(0x16d990, |c, m, w| {
        let (a0, a1) = (c.gpr(4) as u32, c.gpr(5) as u32);
        w.log.push(vec![53, a1 - recs]);
        for k in (1..6).chain(8..12).chain(12..13) {
            let v = w.next();
            m.write_u32(a0 + 4 * k, v);
        }
    });
    stub!(0x16dcb8, |c, _, w| {
        assert_eq!(c.gpr(5), 2);
        w.log.push(vec![54]);
    });
    stub!(0x172100, |c, m, w| {
        assert_eq!(c.gpr(4) as u32, obj + 0x214);
        w.log.push(vec![55, c.gpr(5) as u32]);
        let ok = w.next() % 2 == 1;
        if ok {
            for k in 0..4 {
                let v = fl(w.next()).to_bits();
                m.write_u32(c.gpr(6) as u32 + 4 * k, v);
            }
        }
        let o = fl(w.next());
        m.write_f32(c.gpr(7) as u32, o);
        c.set_gpr(2, ok as u64);
    });
    stub!(0x11cae8, |c, m, w| {
        let (a0, a1, a2) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        let mut v = vec![56, (a0 - riders) / STRIDE];
        v.extend((0..4).map(|k| m.read_u32(a1 + 4 * k)));
        v.extend((0..4).map(|k| m.read_u32(a2 + 4 * k)));
        v.extend([c.gpr(7) as u32, c.f(12).to_bits(), c.f(13).to_bits()]);
        w.log.push(v);
    });
    stub!(0x16da10, |c, m, w| {
        let (a0, a3) = (c.gpr(4) as u32, c.gpr(7) as u32);
        let mut v = vec![57, (a0 - obj - 0x220) / 0x40, c.gpr(5) as u32, c.gpr(6) as u32];
        v.extend((0..4).map(|k| m.read_u32(a3 + 4 * k)));
        v.push(c.f(12).to_bits());
        w.log.push(v);
    });

    let mut g = Rng(0x1768_50a1_c4d3_9b27);
    let mut kinds = [0u32; 60];
    for case in 0..30_000 {
        let count = [0usize, 1, 2, 3, 4, 6, 6, 8][g.next() as usize % 8];
        let slots: Vec<u32> = (0..count).map(|_| g.next() % 4).collect();
        let chs: Vec<u32> = (0..count).map(|_| g.next() % 8).collect();
        let table: [u32; 8] = std::array::from_fn(|_| [0u32, 0x382f58, 0x382f60][g.next() as usize % 3]);
        let mut rs = vec![];
        for _ in 0..8 {
            let kind = match g.next() % 4 {
                0 => g.next() % 30,
                1 => [0u32, 10, 11, 12, 13, 13][g.next() as usize % 6],
                _ => [0u32, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21][g.next() as usize % 13],
            };
            rs.push(SceneRec {
                in_use: [0u32, 1, 1, 1][g.next() as usize % 4],
                kind,
                place: [0u32, 1, 5][g.next() as usize % 3],
                from_scene: g.next() % 2,
                pos: [g.f().to_bits(), g.f().to_bits(), g.f().to_bits()],
                heading: g.f().to_bits(),
            });
        }
        for (i, rc) in rs.iter().enumerate() {
            let a = recs + 0x30 * i as u32;
            r.mem.write_u32(a + 0x10, rc.in_use);
            r.mem.write_u32(a + 0x14, rc.kind);
            r.mem.write_u32(a + 0x18, rc.place);
            r.mem.write_u32(a + 0x1c, rc.from_scene);
            for k in 0..3 {
                r.mem.write_u32(a + 0x20 + 4 * k, rc.pos[k as usize]);
            }
            r.mem.write_u32(a + 0x2c, rc.heading);
        }
        r.mem.write_u32(list + 0x88, count as u32);
        r.mem.write_u32(0x32db6c, count as u32);
        for j in 0..8u32 {
            let rd = riders + STRIDE * j;
            r.mem.write_u32(rd + 0x38, slots.get(j as usize).cloned().unwrap_or(9));
            r.mem.write_u32(rd + 0x464, chars + 0x40 * j);
            r.mem.write_u32(chars + 0x40 * j + 0x38, chs.get(j as usize).cloned().unwrap_or(0));
        }
        for k in 0..8 {
            r.mem.write_u32(0x336650 + 4 * k, table[k as usize]);
        }
        let mut o: Vec<u32> = (0..0xf00 / 4).map(|_| g.next() % 4).collect();
        o[0x210 / 4] = g.next() % 2;
        for i in 0..8 {
            o[(0x230 + 0x40 * i) / 4] = g.next() % 3;
        }
        for (i, wd) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *wd);
        }
        {
            let mut wb = w.borrow_mut();
            wb.recs = rs.clone();
            wb.slots = slots.clone();
            wb.chars = chs.clone();
            wb.table = table;
            wb.oracle = (0..400).map(|_| g.next()).collect();
            wb.pos = 0;
            wb.log.clear();
        }
        let step = 0x70_0000 + 0x2c * (g.next() % 9);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, step as u64);
        r.call(0x176850).unwrap();
        let want = std::mem::take(&mut w.borrow_mut().log);
        w.borrow_mut().pos = 0;
        let mut e = U(w.clone());
        start_scene(&mut o, step, &mut e);
        let got = std::mem::take(&mut w.borrow_mut().log);
        assert_eq!(want, got, "case {case} calls (count {count})");
        for (i, wd) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *wd, "case {case} word {:#x}", i * 4);
        }
        for g in &got {
            kinds[g[0] as usize] += 1;
        }
    }
    eprintln!("scene: calls by tag {kinds:?}");
}
