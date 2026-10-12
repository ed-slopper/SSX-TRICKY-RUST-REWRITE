//! `tricky_game::camera::eval_position` (board row E11a) against `CamCamera_EvalPosition` 0x16e900 in the function
//! runner (TRICKY_ELF; skipped without it), the stage and collision calls logged.

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
        match self.next() % 12 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            5 => (self.next() % 200_000) as f32 / 100.0 - 1000.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn nz(&mut self) -> f32 {
        let v = self.f();
        if v == 0.0 { 0.5 } else { v }
    }
}

#[derive(Default)]
struct Script {
    stage: Option<(bool, [f32; 4], Option<f32>)>,
    collide: Option<([f32; 3], u64)>,
}

struct Env {
    log: Rc<RefCell<Vec<String>>>,
    script: Rc<RefCell<Script>>,
    mats: Vec<[[f32; 4]; 4]>,
}
fn bits(v: &[f32]) -> String {
    v.iter().map(|x| format!("{:08x}", x.to_bits())).collect::<Vec<_>>().join(",")
}
impl CamEnv for Env {
    fn stage_area_to_world(&mut self, area: i32, v: [f32; 4]) -> StageOut {
        self.log.borrow_mut().push(format!("stage {area} {}", bits(&v)));
        let (ret, nv, yaw) = self.script.borrow().stage.unwrap();
        StageOut { ret, v: nv, yaw: yaw.unwrap_or(0.0) }
    }
    fn matrix(&mut self, idx: i32) -> [[f32; 4]; 4] {
        self.mats[idx as usize]
    }
    fn collide_world(&mut self, radius: f32, pos: &mut [f32; 3], target: [f32; 3]) -> u64 {
        self.log.borrow_mut().push(format!("collide {} {} {}", bits(&[radius]), bits(pos), bits(&target)));
        let (np, ret) = self.script.borrow().collide.unwrap();
        *pos = np;
        ret
    }
}

#[test]
fn eval_position_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x250c10u32, 0x2510f8, 0x1741d0, 0x251238] {
        r.allow(a);
    }
    r.call(0x251238).unwrap();
    for (k, v) in tricky_game::camera::IDENT3.iter().enumerate() {
        assert_eq!(r.mem.read_f32(0x346410 + 4 * k as u32), *v, "ident {k}");
    }
    let table: Vec<f32> = (0..0xbd + 3).map(|k| r.mem.read_f32(0x337798 + 4 * k)).collect();
    let obj = r.mem.alloc(0x200);
    let pblk = r.mem.alloc(0x140);
    r.mem.write_u32(obj + 0x11c, pblk);
    let list = r.mem.alloc(0x20 * 4);
    let out = r.mem.alloc(0x80);
    let stage_ptr = r.mem.alloc(0x10);
    let riders: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x4b00)).collect();
    let stats: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x100)).collect();
    let o44: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x20)).collect();
    let o48: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x20)).collect();
    let mb: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x40 * 8)).collect();
    let mb2: Vec<u32> = (0..4).map(|_| r.mem.alloc(0x80)).collect();
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let script = Rc::new(RefCell::new(Script::default()));
    let (l, sc) = (log.clone(), script.clone());
    r.stub(0x1777d0, move |c, m| {
        let area = c.gpr(4) as i32;
        let mut v = [0.0f32; 4];
        for k in 0..4 {
            v[k] = m.read_f32(c.gpr(5) as u32 + 4 * k as u32);
        }
        l.borrow_mut().push(format!("stage {area} {}", bits(&v)));
        let (ret, nv, yaw) = sc.borrow().stage.unwrap();
        for k in 0..4 {
            m.write_f32(c.gpr(5) as u32 + 4 * k as u32, nv[k]);
        }
        if let Some(y) = yaw {
            m.write_f32(c.gpr(6) as u32, y);
        }
        c.set_gpr(2, ret as u64);
    });
    let (l, sc) = (log.clone(), script.clone());
    r.stub(0x16fe60, move |c, m| {
        let mut pos = [0.0f32; 3];
        let mut tg = [0.0f32; 3];
        for k in 0..3 {
            pos[k] = m.read_f32(c.gpr(4) as u32 + 4 * k as u32);
            tg[k] = m.read_f32(c.gpr(5) as u32 + 4 * k as u32);
        }
        l.borrow_mut().push(format!("collide {} {} {}", bits(&[c.f(12)]), bits(&pos), bits(&tg)));
        let (np, ret) = sc.borrow().collide.unwrap();
        for k in 0..3 {
            m.write_f32(c.gpr(4) as u32 + 4 * k as u32, np[k]);
        }
        c.set_gpr(2, ret);
    });
    let mut g = Rng(0x9e37_79b9_7f4a_7c15);
    for case in 0..60_000 {
        let flag = |g: &mut Rng, pct: u32| (g.next() % 100 < pct) as i32;
        let mut p = CamP {
            w00: flag(&mut g, 80),
            w04: (g.next() % 4) as i32,
            w10: flag(&mut g, 50),
            f14: if g.next() % 6 == 0 { 0.0 } else { (g.next() % 50) as f32 / 4.0 + 0.5 },
            w18: flag(&mut g, 50),
            w1c: g.next(),
            w20: flag(&mut g, 50),
            w24: g.next(),
            w28: flag(&mut g, 60),
            w2c: [-2, -1, 0, 1, 2, 3, 4, 5][g.next() as usize % 8],
            w30: flag(&mut g, 30) * (1 + (g.next() % 5) as i32),
            base: [g.f(), g.f(), g.f()],
            f_b8: if g.next() % 3 == 0 { g.f().abs() / 4.0 } else { 0.0 },
            f_c4: g.f(),
            w_c8: flag(&mut g, 85),
            f_cc: g.f(),
            f_d0: g.f(),
            w_d4: flag(&mut g, 40),
            w_d8: flag(&mut g, 40),
            f_dc: g.f(),
            w_e0: flag(&mut g, 30),
            w_f8: flag(&mut g, 60),
            w_fc: flag(&mut g, 50),
            f_100: g.f(),
            w_108: flag(&mut g, 50),
            f_10c: g.f() - 1.0,
            f_110: g.f() + 1.0,
            f_114: g.nz(),
            f_118: g.nz(),
        };
        // null riders only where the original never reads them unchecked
        let unchecked = p.w_e0 != 0 || (p.w00 != 0 && p.w2c == -2);
        // memory
        let w = |r: &mut Runner, a: u32, v: u32| r.mem.write_u32(a, v);
        w(&mut r, pblk, p.w00 as u32);
        w(&mut r, pblk + 4, p.w04 as u32);
        w(&mut r, pblk + 0x10, p.w10 as u32);
        r.mem.write_f32(pblk + 0x14, p.f14);
        w(&mut r, pblk + 0x18, p.w18 as u32);
        w(&mut r, pblk + 0x1c, p.w1c);
        w(&mut r, pblk + 0x20, p.w20 as u32);
        w(&mut r, pblk + 0x24, p.w24);
        w(&mut r, pblk + 0x28, p.w28 as u32);
        w(&mut r, pblk + 0x2c, p.w2c as u32);
        w(&mut r, pblk + 0x30, p.w30 as u32);
        r.mem.write_f32(pblk + 0x3c, p.base[0]);
        r.mem.write_f32(pblk + 0x68, p.base[1]);
        r.mem.write_f32(pblk + 0x94, p.base[2]);
        r.mem.write_f32(pblk + 0xb8, p.f_b8);
        r.mem.write_f32(pblk + 0xc4, p.f_c4);
        w(&mut r, pblk + 0xc8, p.w_c8 as u32);
        r.mem.write_f32(pblk + 0xcc, p.f_cc);
        r.mem.write_f32(pblk + 0xd0, p.f_d0);
        w(&mut r, pblk + 0xd4, p.w_d4 as u32);
        w(&mut r, pblk + 0xd8, p.w_d8 as u32);
        r.mem.write_f32(pblk + 0xdc, p.f_dc);
        w(&mut r, pblk + 0xe0, p.w_e0 as u32);
        w(&mut r, pblk + 0xf8, p.w_f8 as u32);
        w(&mut r, pblk + 0xfc, p.w_fc as u32);
        r.mem.write_f32(pblk + 0x100, p.f_100);
        w(&mut r, pblk + 0x108, p.w_108 as u32);
        r.mem.write_f32(pblk + 0x10c, p.f_10c);
        r.mem.write_f32(pblk + 0x110, p.f_110);
        r.mem.write_f32(pblk + 0x114, p.f_114);
        r.mem.write_f32(pblk + 0x118, p.f_118);
        let mut entries = vec![];
        let mut mats_all: Vec<[[f32; 4]; 4]> = vec![];
        for i in 0..4 {
            let has_rider = unchecked || g.next() % 5 != 0;
            let ent = list + 0x20 * i as u32;
            let rider = if has_rider {
                let b = riders[i];
                let tr = TargetRider {
                    f130: g.f(),
                    flag40: g.next() % 2 == 0,
                    v2d0: [g.f(), g.f(), g.f()],
                    v4a90: [g.f(), g.f(), g.f()],
                    t4498: [0.0; 3],
                };
                r.mem.write_f32(b + 0x130, tr.f130);
                w(&mut r, b + 0x464, stats[i]);
                w(&mut r, stats[i] + 0x40, tr.flag40 as u32);
                for k in 0..3 {
                    r.mem.write_f32(b + 0x2d0 + 4 * k, tr.v2d0[k as usize]);
                    r.mem.write_f32(b + 0x4a90 + 4 * k, tr.v4a90[k as usize]);
                }
                w(&mut r, b + 0x4494, o44[i]);
                w(&mut r, o44[i] + 4, mb[i]);
                w(&mut r, b + 0x4498, o48[i]);
                w(&mut r, o48[i] + 4, mb2[i]);
                let mut t = tr;
                for k in 0..4 {
                    let v = g.f();
                    r.mem.write_f32(mb2[i] + 0x30 + 4 * k, v);
                    if k < 3 {
                        t.t4498[k as usize] = v;
                    }
                }
                w(&mut r, ent, b);
                Some(t)
            } else {
                w(&mut r, ent, 0);
                None
            };
            let w4 = g.f();
            let mut v8 = [g.f(), g.f(), g.f()];
            if g.next() % 5 == 0 { v8[0] = 0.0; }
            if g.next() % 5 == 0 { v8[1] = 0.0; }
            if g.next() % 6 == 0 { v8[2] = 0.0; }
            let p14 = [g.f(), g.f(), g.f()];
            r.mem.write_f32(ent + 4, w4);
            for k in 0..3 {
                r.mem.write_f32(ent + 8 + 4 * k, v8[k as usize]);
                r.mem.write_f32(ent + 0x14 + 4 * k, p14[k as usize]);
            }
            entries.push(Entry { rider, w4, v8, p14 });
        }
        // the matrices (the rider of the targeted entry's table; the port asks by index)
        let tgt = p.w04 as usize;
        let mut mats = vec![];
        for idx in 0..8 {
            let mut m = [[0.0f32; 4]; 4];
            for row in 0..4 {
                for l in 0..4 {
                    m[row][l] = if g.next() % 4 == 0 { 0.0 } else { g.f() };
                    r.mem.write_f32(mb[tgt] + 0x40 * idx as u32 + 0x10 * row as u32 + 4 * l as u32, m[row][l]);
                }
            }
            mats.push(m);
        }
        mats_all.extend(mats.iter().cloned());
        // other state
        let mut o = CamOut { yaw: g.f(), pitch: g.f(), v24: [f32::from_bits(0xdead_0009), f32::from_bits(0xdead_000a), f32::from_bits(0xdead_000b)], ..Default::default() };
        r.mem.write_f32(out + 0x4c, o.yaw);
        r.mem.write_f32(out + 0x50, o.pitch);
        for k in 0..0x80 / 4 {
            if k != 0x13 && k != 0x14 {
                w(&mut r, out + 4 * k, 0xdead_0000 + k);
            }
        }
        let mut shake = Shake { i0: (g.next() % 0xbd) as i32, i1: (g.next() % 0xbd) as i32 };
        w(&mut r, 0x335718, shake.i0 as u32);
        w(&mut r, 0x33571c, shake.i1 as u32);
        let cut = g.next() % 3 == 0;
        let want_stage = g.next() % 2 == 0;
        *script.borrow_mut() = Script {
            stage: Some((g.next() % 2 == 0, [g.f(), g.f(), g.f(), g.f()], if g.next() % 3 == 0 { None } else { Some(g.f()) })),
            collide: Some(([g.f(), g.f(), g.f()], ((g.next() as u64) << 32) | g.next() as u64)),
        };
        let p0 = p.clone();
        log.borrow_mut().clear();
        w(&mut r, stage_ptr, 0x1234_5678);
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, out as u64);
        r.cpu.set_gpr(6, list as u64);
        r.cpu.set_gpr(7, cut as u64);
        r.cpu.set_gpr(8, if want_stage { stage_ptr } else { 0 } as u64);
        r.call(0x16e900).unwrap_or_else(|e| panic!("call {case}: {e}"));
        let want_log = log.borrow().clone();
        log.borrow_mut().clear();
        let mut env = Env { log: log.clone(), script: script.clone(), mats };
        let res = eval_position(&mut p, &entries, &mut o, cut, &mut env, &mut shake, &table);
        let ctx = format!("case {case}: {p0:?} cut {cut}");
        assert_eq!(*log.borrow(), want_log, "calls {ctx}");
        assert_eq!(r.cpu.gpr(2), res.ret, "ret {ctx}");
        if want_stage {
            assert_eq!(r.mem.read_f32(stage_ptr).to_bits(), res.stage_yaw.to_bits(), "stage yaw {ctx}");
        } else {
            assert_eq!(r.mem.read_u32(stage_ptr), 0x1234_5678);
        }
        let rf = |r: &mut Runner, off: u32| r.mem.read_f32(out + off).to_bits();
        for k in 0..3 {
            assert_eq!(rf(&mut r, 4 * k), o.pos[k as usize].to_bits(), "pos {k} {ctx}");
            assert_eq!(rf(&mut r, 0x24 + 4 * k), o.v24[k as usize].to_bits(), "v24 {k} {ctx}");
        }
        for (off, v) in [(0x30u32, o.w30), (0x38, o.w38), (0x3c, o.w3c), (0x40, o.w40), (0x44, o.w44)] {
            let _ = (off, v);
        }
        assert_eq!(r.mem.read_u32(out + 0x30), o.w30, "w30 {ctx}");
        assert_eq!(r.mem.read_u32(out + 0x38), o.w38, "w38 {ctx}");
        if o.w38 != 0 { assert_eq!(r.mem.read_u32(out + 0x3c), o.w3c, "w3c {ctx}"); }
        assert_eq!(r.mem.read_u32(out + 0x40), o.w40, "w40 {ctx}");
        if o.w40 != 0 { assert_eq!(r.mem.read_u32(out + 0x44), o.w44, "w44 {ctx}"); }
        assert_eq!(rf(&mut r, 0x4c), o.yaw.to_bits(), "yaw {ctx}");
        assert_eq!(rf(&mut r, 0x50), o.pitch.to_bits(), "pitch {ctx}");
        assert_eq!(r.mem.read_u32(0x335718) as i32, shake.i0, "shake0 {ctx}");
        assert_eq!(r.mem.read_u32(0x33571c) as i32, shake.i1, "shake1 {ctx}");
        assert_eq!(r.mem.read_f32(pblk + 0xb8).to_bits(), p.f_b8.to_bits(), "b8 {ctx}");
        let _ = &mats_all;
    }
}
