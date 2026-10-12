//! `tricky_game::camera::region_is_inside` (board row E11c) against `CamRegion_IsInside` 0x171610 in the function
//! runner (TRICKY_ELF; skipped without it), the stage call and the region's own test logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::camera::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            5 => (self.next() % 200_000) as f32 / 100.0 - 1000.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn pos(&mut self) -> f32 {
        (self.next() % 4000) as f32 / 100.0 - 20.0
    }
}

fn bits(v: &[f32]) -> String {
    v.iter().map(|x| format!("{:08x}", x.to_bits())).collect::<Vec<_>>().join(",")
}

#[derive(Default, Clone)]
struct Script {
    stage: (bool, [f32; 4], Option<f32>),
    cond: u64,
    log: Vec<String>,
}
struct Env(Rc<RefCell<Script>>);
impl RegionEnv for Env {
    fn stage_area_to_world(&mut self, area: i32, v: [f32; 4]) -> StageOut {
        let mut s = self.0.borrow_mut();
        s.log.push(format!("stage {area} {}", bits(&v)));
        StageOut { ret: s.stage.0, v: s.stage.1, yaw: s.stage.2.unwrap_or(0.0) }
    }
    fn check_condition(&mut self) -> u64 {
        let mut s = self.0.borrow_mut();
        s.log.push("cond".to_string());
        s.cond
    }
}

#[test]
fn region_is_inside_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x250c10u32, 0x1741d0, 0x251238, 0x137140, 0x102f80] {
        r.allow(a);
    }
    r.call(0x251238).unwrap();
    let st = Rc::new(RefCell::new(Script::default()));
    let s = st.clone();
    r.stub(0x1777d0, move |c, m| {
        let area = c.gpr(4) as i32;
        let v: Vec<f32> = (0..4).map(|k| m.read_f32(c.gpr(5) as u32 + 4 * k)).collect();
        let mut s = s.borrow_mut();
        s.log.push(format!("stage {area} {}", bits(&v)));
        for k in 0..4 {
            m.write_f32(c.gpr(5) as u32 + 4 * k as u32, s.stage.1[k]);
        }
        if let Some(y) = s.stage.2 {
            m.write_f32(c.gpr(6) as u32, y);
        }
        c.set_gpr(2, s.stage.0 as u64);
    });
    let s = st.clone();
    r.stub(0x171500, move |c, _| {
        let mut s = s.borrow_mut();
        s.log.push("cond".to_string());
        c.set_gpr(2, s.cond);
    });
    let cam = r.mem.alloc(0x20);
    let rec = r.mem.alloc(0x200);
    let list = r.mem.alloc(8 * 0x20);
    r.mem.write_u32(cam + 4, rec);
    let mut g = Rng(0x7777_1234_abcd_0001);
    for case in 0..60_000 {
        let mut ents = [[0.0f32; 3]; 8];
        for k in 0..8 {
            ents[k] = [g.pos(), g.pos(), g.pos()];
            for j in 0..3 {
                r.mem.write_f32(list + 0x20 * k as u32 + 0x14 + 4 * j, ents[k][j as usize]);
            }
        }
        let w154 = (g.next() % 8) as i32;
        let f150 = [0.0f32, 3.0, 10.0, 25.0, g.f().abs()][(g.next() % 5) as usize];
        let e = ents[w154 as usize];
        let near = |g: &mut Rng, c: f32| c + ((g.next() % 4000) as f32 / 4000.0 - 0.5) * (f150 * 2.4 + 0.5);
        let mut p = RegionP {
            pos: if g.next() % 4 == 0 { [g.pos(), g.pos(), g.pos()] } else { [near(&mut g, e[0]), near(&mut g, e[1]), near(&mut g, e[2])] },
            f150,
            w154,
            w174: (g.next() % 3 != 0) as i32,
            f178: [0.0f32, 4.0, 20.0, 60.0, g.f()][(g.next() % 5) as usize],
            f17c: g.f(),
            f180: g.f(),
            w190: if g.next() % 2 == 0 { 0 } else { (g.next() % 6) as i32 + 1 },
        };
        let mut sc = Script::default();
        let sp = p.pos;
        sc.stage = (g.next() % 2 == 0, [sp[0] + g.f(), sp[1] + g.f(), sp[2] + g.f(), g.f()], if g.next() % 2 == 0 { Some(g.f()) } else { None });
        sc.cond = if g.next() % 2 == 0 { 0 } else { (g.next() as u64) << 20 | 1 };
        if p.w174 != 0 && g.next() % 2 == 0 {
            p.f150 = p.f150.max(2.0);
        }
        *st.borrow_mut() = sc.clone();
        for (o, v) in [(0x3cu32, p.pos[0]), (0x68, p.pos[1]), (0x94, p.pos[2]), (0x150, p.f150), (0x178, p.f178), (0x17c, p.f17c), (0x180, p.f180)] {
            r.mem.write_f32(rec + o, v);
        }
        r.mem.write_u32(rec + 0x154, p.w154 as u32);
        r.mem.write_u32(rec + 0x174, p.w174 as u32);
        r.mem.write_u32(rec + 0x190, p.w190 as u32);
        r.mem.write_u32(rec + 0x11c, 0);
        r.cpu.set_gpr(4, cam as u64);
        r.cpu.set_gpr(5, list as u64);
        r.call(0x171610).unwrap();
        let ret = r.cpu.gpr(2);
        let want_log = st.borrow().log.clone();
        assert_eq!(r.mem.read_u32(rec + 0x11c), rec, "case {case} self pointer");
        let es = Rc::new(RefCell::new(sc));
        let got = region_is_inside(&p, &ents, &mut Env(es.clone()));
        let got_log = es.borrow().log.clone();
        assert_eq!(got_log, want_log, "case {case} log {p:?}");
        assert_eq!(got, ret, "case {case} ret {p:?}");
    }
}
