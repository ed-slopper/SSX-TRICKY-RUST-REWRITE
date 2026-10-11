//! One air tick against the original's (board row E8a3): `AirMotion_Update` 0x108378 run in the function runner on
//! the player's SLUS_203.26 (TRICKY_ELF; skipped without it), the real flight step, quaternion and orientation code
//! under it, every other call stubbed, recorded with its arguments and answered from a script, against
//! `tricky_game::air_motion::update` with an environment answering the same script.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::air_motion::{update, AirEnv, AirRider, Ray};
use tricky_game::ground_motion::CrashRecord;
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::surface::SURFACE_TABLE;

type V4 = [f32; 4];
type Log = Vec<(&'static str, Vec<u32>)>;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
    fn unit(&mut self) -> V4 {
        let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        [v[0] / l, v[1] / l, v[2] / l, 0.0]
    }
}

fn put4(m: &mut r5900::Mem, at: u32, v: V4) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> V4 {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}
fn b4(v: V4) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}
fn rec_words(r: &CrashRecord) -> Vec<u32> {
    let mut w = b4(r.pos);
    w.extend(b4(r.dir));
    w.extend(b4(r.normal));
    w.push(r.along.to_bits());
    w.extend(b4(r.dir));
    w
}

/// One tick's answers.
#[derive(Clone, Debug, Default)]
struct Script {
    down: Ray,
    wall: Ray,
    probe: bool,
    probe_normal: V4,
    probe_height: f32,
    probe_ground: V4,
    clip1: u32,
    angles: Option<(f32, f32)>,
    crash_score: f32,
    land_score: f32,
    land_bumps: bool,
    tricky: bool,
}

/// Ours: answers the script and records the calls as the runner's stubs record them.
struct Env<'a> {
    s: &'a Script,
    log: Log,
}

impl AirEnv for Env<'_> {
    fn predict_tick(&mut self, _r: &mut AirRider) {
        self.log.push(("predict_tick", vec![]));
    }
    fn down_ray(&mut self, _r: &mut AirRider, from: V4, to: V4) -> Ray {
        let mut w = b4(from);
        w.extend(b4(to));
        self.log.push(("down_ray", w));
        self.s.down
    }
    fn probe_ground(&mut self, r: &mut AirRider) -> bool {
        self.log.push(("probe", b4(r.pos)));
        if self.s.probe {
            r.normal = self.s.probe_normal;
            r.height = self.s.probe_height;
            r.ground = self.s.probe_ground;
        }
        self.s.probe
    }
    fn predict_landed(&mut self, _r: &mut AirRider) {
        self.log.push(("predict_landed", vec![]));
    }
    fn set_fall_scale(&mut self, _r: &mut AirRider, vn: f32) {
        self.log.push(("fall_scale", vec![vn.to_bits()]));
    }
    fn virtual_6c(&mut self, _r: &mut AirRider, x: f32) {
        self.log.push(("v6c", vec![x.to_bits()]));
    }
    fn clip_type_layer1(&mut self, _r: &mut AirRider) -> u32 {
        self.log.push(("clip_type1", vec![]));
        self.s.clip1
    }
    fn blend_out_all(&mut self, _r: &mut AirRider) {
        self.log.push(("blend_out", vec![]));
    }
    fn check_angles(&mut self, _r: &mut AirRider) -> Option<(f32, f32)> {
        self.log.push(("check_angles", vec![]));
        self.s.angles
    }
    fn crash_virtual(&mut self, _r: &mut AirRider, point: V4, vel: V4, normal: V4) {
        let mut w = b4(point);
        w.extend(b4(vel));
        w.extend(b4(normal));
        self.log.push(("v64", w));
    }
    fn wipeout(&mut self, _r: &mut AirRider, rec: &CrashRecord) {
        self.log.push(("wipeout", rec_words(rec)));
    }
    fn score_crash(&mut self, _r: &mut AirRider) -> f32 {
        self.log.push(("score_crash", vec![]));
        self.s.crash_score
    }
    fn add_meter(&mut self, _r: &mut AirRider, amount: f32) {
        self.log.push(("add_meter", vec![amount.to_bits()]));
    }
    fn choose_state(&mut self, _r: &mut AirRider, a1: f32, a2: f32, vn: f32) {
        self.log.push(("choose_state", vec![a1.to_bits(), a2.to_bits(), vn.to_bits()]));
    }
    fn land_trick(&mut self, r: &mut AirRider, switch: bool, steep: bool) -> f32 {
        self.log.push(("land_trick", vec![switch as u32, steep as u32]));
        if self.s.land_bumps {
            r.tricks += 1;
        }
        self.s.land_score
    }
    fn virtual_84(&mut self, _r: &mut AirRider, x: f32) {
        self.log.push(("v84", vec![x.to_bits()]));
    }
    fn start_celebrate(&mut self, _r: &mut AirRider, amount: f32) {
        self.log.push(("celebrate", vec![amount.to_bits()]));
    }
    fn is_tricky(&mut self, _r: &mut AirRider) -> bool {
        self.log.push(("is_tricky", vec![]));
        self.s.tricky
    }
    fn speech_air_time(&mut self, _r: &mut AirRider) {
        self.log.push(("speech", vec![]));
    }
    fn spawn_landing_puffs(&mut self, _r: &mut AirRider, vn: f32) {
        self.log.push(("puffs", vec![vn.to_bits()]));
    }
    fn sfx_update_air(&mut self, _r: &mut AirRider, score: f32) {
        self.log.push(("sfx_air", vec![score.to_bits()]));
    }
    fn virtual_9c(&mut self, _r: &mut AirRider) {
        self.log.push(("v9c", vec![]));
    }
    fn rail_try_capture(&mut self, _r: &mut AirRider) {
        self.log.push(("rail_capture", vec![]));
    }
    fn instance_collisions(&mut self, _r: &mut AirRider) {
        self.log.push(("instance_collisions", vec![]));
    }
    fn wall_ray(&mut self, r: &mut AirRider) -> Ray {
        self.log.push(("wall_ray", vec![r.mask]));
        self.s.wall
    }
    fn translate(&mut self, _r: &mut AirRider, by: V4) {
        self.log.push(("translate", b4(by)));
    }
    fn wall_done(&mut self, r: &mut AirRider) {
        self.log.push(("wall_done", vec![r.mask]));
    }
}

#[test]
#[allow(unused_variables)]
fn air_tick_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x12b340u32, 0x102e80, 0x250c10, 0x250fd0, 0x129008, 0x2510f8, 0x102f50, 0x102f80] {
        r.allow(a);
    }
    let rider = r.mem.alloc(0x6000);
    let (vt, stats, mgr, app, game, table, sound) = (r.mem.alloc(0x200), r.mem.alloc(0x100), r.mem.alloc(0x1000), r.mem.alloc(0x800), r.mem.alloc(0x100), r.mem.alloc(20 * 100), r.mem.alloc(0x100));
    r.mem.write_u32(rider, vt);
    for off in [0x60u32, 0x68, 0x80, 0x98] {
        r.mem.write_u16(vt + off, 0);
    }
    r.mem.write_u32(vt + 0x64, 0x1000);
    r.mem.write_u32(vt + 0x6c, 0x1010);
    r.mem.write_u32(vt + 0x84, 0x1020);
    r.mem.write_u32(vt + 0x9c, 0x1030);
    r.mem.write_u32(rider + 0x464, stats);
    r.mem.write_u32(rider + 0x5ae0, mgr);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x24, table);
    for (s, row) in SURFACE_TABLE.iter().enumerate() {
        for (k, v) in row.iter().enumerate() {
            r.mem.write_f32(table + 100 * s as u32 + 4 * k as u32, *v);
        }
    }
    r.mem.write_u32(0x343050, sound);
    let script: Rc<RefCell<Script>> = Rc::default();
    let log: Rc<RefCell<Log>> = Rc::default();
    let rays = Rc::new(RefCell::new(Vec::<(V4, V4)>::new()));
    macro_rules! stub {
        ($addr:expr, |$c:ident, $m:ident, $s:ident, $l:ident| $body:block) => {{
            stub!($addr, |$c, $m, $s, $l, _rs| $body)
        }};
        ($addr:expr, |$c:ident, $m:ident, $s:ident, $l:ident, $rs:ident| $body:block) => {{
            let (sc, lg, rs2) = (script.clone(), log.clone(), rays.clone());
            r.stub($addr, move |$c, $m| {
                let $s = sc.borrow().clone();
                let $l = &mut *lg.borrow_mut();
                let $rs = &rs2;
                $body
            });
        }};
    }
    stub!(0x123ec8, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider + 0x57b0); l.push(("predict_tick", vec![])); });
    {
        let rs = rays.clone();
        r.stub(0x25d7f0, move |c, m| {
            assert_eq!((c.f(12), c.gpr(7)), (0.5, 1));
            rs.borrow_mut().push((get4(m, c.gpr(5) as u32), get4(m, c.gpr(6) as u32)));
        });
    }
    {
        let lg = log.clone();
        r.stub(0x257810, move |c, m| {
            assert_eq!((c.gpr(5) as u32, c.gpr(6)), (rider + 0x470, 0));
            lg.borrow_mut().push(("wall_ray", vec![m.read_u32(rider + 0x494)]));
        });
    }
    stub!(0x25aff8, |c, m, s, l, rs| {
        assert_eq!(c.gpr(4), 0x346488);
        let down = c.gpr(7) != 0;
        let ray = if down {
            assert_eq!(c.gpr(7) as u32, rider + 0x430);
            let (a, b) = rs.borrow_mut().pop().expect("a line");
            let mut w = b4(a);
            w.extend(b4(b));
            l.push(("down_ray", w));
            s.down
        } else {
            s.wall
        };
        let out = c.gpr(6) as u32;
        put4(m, out + 0x10, ray.point);
        put4(m, out + 0x20, ray.normal);
        m.write_u32(out + 0x34, ray.word);
        c.set_f(0, ray.frac);
    });
    stub!(0x128ae8, |c, m, s, l| {
        l.push(("probe", b4(get4(m, rider + 0x140))));
        if s.probe {
            put4(m, rider + 0x2a0, s.probe_normal);
            m.write_f32(rider + 0x1bc, s.probe_height);
            put4(m, rider + 0x2d0, s.probe_ground);
        }
        c.set_gpr(2, s.probe as u64);
    });
    stub!(0x123f00, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider + 0x57b0); l.push(("predict_landed", vec![])); });
    stub!(0x1049a0, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, mgr + 0x6f0); l.push(("fall_scale", vec![c.f(12).to_bits()])); });
    stub!(0x1010, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("v6c", vec![c.f(12).to_bits()])); });
    stub!(0x15eec0, |c, m, s, l| { assert_eq!((c.gpr(4) as u32, c.gpr(5)), (rider + 0x46c0, 1)); l.push(("clip_type1", vec![])); c.set_gpr(2, s.clip1 as u64); });
    stub!(0x15f050, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider + 0x46c0); l.push(("blend_out", vec![])); });
    stub!(0x12ba78, |c, m, s, l| {
        assert_eq!((c.gpr(4) as u32, c.gpr(5) as u32), (rider, rider + 0x2a0));
        l.push(("check_angles", vec![]));
        match s.angles {
            Some((a1, a2)) => {
                m.write_f32(c.gpr(6) as u32, a1);
                m.write_f32(c.gpr(7) as u32, a2);
                c.set_gpr(2, 1);
            }
            None => c.set_gpr(2, 0),
        }
    });
    stub!(0x1000, |c, m, s, l| {
        assert_eq!((c.gpr(4) as u32, c.gpr(6) as u32), (rider, rider + 0x150));
        let mut w = b4(get4(m, c.gpr(5) as u32));
        w.extend(b4(get4(m, rider + 0x150)));
        w.extend(b4(get4(m, c.gpr(7) as u32)));
        l.push(("v64", w));
    });
    stub!(0x11d830, |c, m, s, l| {
        assert_eq!(c.gpr(4) as u32, rider);
        let a = c.gpr(5) as u32;
        let mut w = b4(get4(m, a));
        w.extend(b4(get4(m, a + 0x10)));
        w.extend(b4(get4(m, a + 0x20)));
        w.push(m.read_u32(a + 0x30));
        w.extend(b4(get4(m, a + 0x40)));
        l.push(("wipeout", w));
    });
    stub!(0x156de0, |c, m, s, l| { assert_eq!((c.gpr(4) as u32, c.gpr(5)), (rider + 0x5820, 0)); l.push(("score_crash", vec![])); c.set_f(0, s.crash_score); });
    stub!(0x11b018, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("add_meter", vec![c.f(12).to_bits()])); });
    stub!(0x109308, |c, m, s, l| { l.push(("choose_state", vec![c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits()])); });
    stub!(0x1569b0, |c, m, s, l| {
        assert_eq!((c.gpr(4) as u32, c.gpr(5)), (rider + 0x5820, 0));
        l.push(("land_trick", vec![c.gpr(6) as u32, c.gpr(7) as u32]));
        if s.land_bumps {
            let t = m.read_u32(rider + 0x58a8);
            m.write_u32(rider + 0x58a8, t.wrapping_add(1));
        }
        c.set_f(0, s.land_score);
    });
    stub!(0x1020, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("v84", vec![c.f(12).to_bits()])); });
    stub!(0x11f3b8, |c, m, s, l| { assert_eq!((c.gpr(4) as u32, c.gpr(5)), (rider, 1)); l.push(("celebrate", vec![c.f(12).to_bits()])); });
    stub!(0x11f430, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("is_tricky", vec![])); c.set_gpr(2, s.tricky as u64); });
    stub!(0x233448, |c, m, s, l| { assert_eq!((c.gpr(4) as u32, c.gpr(5) as u32), (sound, rider)); l.push(("speech", vec![])); });
    stub!(0x136e90, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, mgr); l.push(("puffs", vec![c.f(12).to_bits()])); });
    stub!(0x217d50, |c, m, s, l| { assert_eq!((c.gpr(4) as u32, c.gpr(5) as u32), (sound, rider)); l.push(("sfx_air", vec![c.f(12).to_bits()])); });
    stub!(0x1030, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("v9c", vec![])); });
    stub!(0x125cb8, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("rail_capture", vec![])); });
    stub!(0x125088, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("instance_collisions", vec![])); });
    stub!(0x1267a0, |c, m, s, l| { assert_eq!(c.gpr(4) as u32, rider); l.push(("translate", b4(get4(m, c.gpr(5) as u32)))); });
    stub!(0x257ca0, |c, m, s, l| { assert_eq!(c.gpr(5), 2); l.push(("wall_done", vec![m.read_u32(rider + 0x494)])); });

    let mut g = Rng(0x93a1_5f0c_7e28_d4b6);
    let mut seen = [0u32; 5];
    for case in 0..20_000 {
        let mut o = Orient { q: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)], ..Default::default() };
        update_orientation(&mut o);
        let frame = [g.unit(), g.unit(), g.unit(), [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-900.0, 900.0), 1.0]];
        let mut ar = AirRider {
            timescale: [1.0, g.f(0.5, 2.0)][g.next() as usize % 2],
            pos: [frame[3][0] + g.f(-50.0, 50.0), frame[3][1] + g.f(-50.0, 50.0), frame[3][2] + g.f(-50.0, 50.0), 1.0],
            vel: [g.f(-2500.0, 2500.0), g.f(-2500.0, 2500.0), g.f(-1500.0, 800.0), 0.0],
            orient: o,
            spin: [0.0, g.f(-12.0, 12.0)][g.next() as usize % 2],
            flip: [0.0, g.f(-12.0, 12.0)][g.next() as usize % 2],
            slope_kind: (g.next() % 21) as i32 - 1,
            surface: g.next() % 20,
            surface_ticks: [-5, 0, 7][g.next() as usize % 3],
            frame,
            bone: [g.f(-30.0, 30.0), g.f(-30.0, 30.0), g.f(0.0, 120.0)],
            normal: g.unit(),
            height: g.f(-50.0, 50.0),
            ground: [g.f(-9.0, 9.0); 4],
            fall: g.f(-9.0, 9.0),
            switch: g.next() % 2,
            stance: g.next() % 2,
            tricks: (g.next() % 9) as i32,
            x28: (g.next() % 4) as i32,
            stats74: g.next() % 10,
            mask: 0x77,
        };
        if ar.slope_kind >= 20 {
            ar.slope_kind = -1;
        }
        let up_n = [g.f(-0.3, 0.3), g.f(-0.3, 0.3), 1.0, 0.0];
        let s = Script {
            down: Ray { frac: [-1.0, 0.0, g.f(0.0, 1.0)][g.next() as usize % 3], point: [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-900.0, 900.0), 1.0], normal: if g.next() % 2 == 0 { up_n } else { g.unit() }, word: g.next() },
            wall: Ray { frac: [-1.0, 0.0, g.f(0.0, 1.0)][g.next() as usize % 3], point: [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-900.0, 900.0), 1.0], normal: if g.next() % 2 == 0 { frame[2] } else { g.unit() }, word: g.next() },
            probe: g.next() % 4 != 0,
            probe_normal: if g.next() % 2 == 0 { up_n } else { g.unit() },
            probe_height: [g.f(-30.0, 0.0), g.f(0.0, 60.0), g.f(60.0, 500.0)][g.next() as usize % 3],
            probe_ground: [g.f(-9000.0, 9000.0), g.f(-9000.0, 9000.0), g.f(-900.0, 900.0), 1.0],
            clip1: [0x1a, 3][g.next() as usize % 2],
            angles: if g.next() % 4 == 0 { None } else { Some((g.f(-3.2, 3.2), g.f(-1.5, 1.5))) },
            crash_score: g.f(-1.0, 1.0),
            land_score: g.f(0.0, 1.2),
            land_bumps: g.next() % 3 == 0,
            tricky: g.next() % 2 == 0,
        };
        *script.borrow_mut() = s.clone();
        log.borrow_mut().clear();
        rays.borrow_mut().clear();
        let m = &mut r.mem;
        m.write_f32(rider + 0x12c, ar.timescale);
        put4(m, rider + 0x140, ar.pos);
        put4(m, rider + 0x150, ar.vel);
        put4(m, rider + 0x170, o.q);
        put4(m, rider + 0x180, o.fwd);
        put4(m, rider + 0x190, o.side);
        put4(m, rider + 0x1a0, o.up);
        m.write_f32(rider + 0x1b0, o.yaw);
        m.write_f32(rider + 0x1d0, ar.spin);
        m.write_f32(rider + 0x1d4, ar.flip);
        m.write_u32(rider + 0x57f0, ar.slope_kind as u32);
        m.write_u32(rider + 0x290, ar.surface);
        m.write_u32(rider + 0x29c, ar.surface_ticks as u32);
        for (k, row) in frame.iter().enumerate() {
            put4(m, rider + 0x4aa0 + 0x10 * k as u32, *row);
        }
        let bi = g.next() % 8;
        m.write_u32(rider + 0x44d8, bi);
        let b = rider + bi * 0x394 + 0x8c8;
        m.write_f32(b, ar.bone[0]);
        m.write_f32(b + 4, ar.bone[1]);
        m.write_f32(b + 8, ar.bone[2]);
        put4(m, rider + 0x2a0, ar.normal);
        m.write_f32(rider + 0x1bc, ar.height);
        put4(m, rider + 0x2d0, ar.ground);
        m.write_f32(rider + 0x798, ar.fall);
        m.write_u32(rider + 0x1b4, ar.switch);
        m.write_u32(stats + 0x40, ar.stance);
        m.write_u32(rider + 0x58a8, ar.tricks as u32);
        m.write_u32(rider + 0x28, ar.x28 as u32);
        m.write_u32(stats + 0x74, ar.stats74);
        m.write_u32(rider + 0x494, ar.mask);
        r.cpu.set_gpr(4, (rider + 0x480) as u64);
        // the motion object: its +0x20 is the rider
        r.mem.write_u32(rider + 0x480 + 0x20, rider);
        r.call(0x108378).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let input = ar;
        let mut env = Env { s: &s, log: Vec::new() };
        update(&mut ar, &SURFACE_TABLE, &mut env);
        let ctx = format!("case {case}: {input:?}\n{s:?}");
        // a crash on landing leaves the record's second copy of the direction (+0x40) unwritten (stale stack)
        let trim = |l: &Log| -> Log {
            let landing = l.iter().any(|c| c.0 == "check_angles");
            l.iter().map(|(n, w)| (*n, if *n == "wipeout" && landing { w[..13].to_vec() } else { w.clone() })).collect()
        };
        assert_eq!(trim(&log.borrow()), trim(&env.log), "{ctx}: calls");
        let m = &mut r.mem;
        let theirs = (
            (b4(get4(m, rider + 0x140)), b4(get4(m, rider + 0x150)), b4(get4(m, rider + 0x170)), b4(get4(m, rider + 0x180)), b4(get4(m, rider + 0x190)), b4(get4(m, rider + 0x1a0)), m.read_u32(rider + 0x1b0)),
            (m.read_u32(rider + 0x290), m.read_u32(rider + 0x29c), b4(get4(m, rider + 0x2a0)), m.read_u32(rider + 0x1bc), m.read_u32(rider + 0x798)),
            (m.read_u32(rider + 0x58a8), m.read_u32(rider + 0x28), m.read_u32(rider + 0x494)),
        );
        let ours = (
            (b4(ar.pos), b4(ar.vel), b4(ar.orient.q), b4(ar.orient.fwd), b4(ar.orient.side), b4(ar.orient.up), ar.orient.yaw.to_bits()),
            (ar.surface, ar.surface_ticks as u32, b4(ar.normal), ar.height.to_bits(), ar.fall.to_bits()),
            (ar.tricks as u32, ar.x28 as u32, ar.mask),
        );
        assert_eq!(theirs, ours, "{ctx}: fields");
        let lg = env.log;
        let kind = if lg.iter().any(|c| c.0 == "land_trick") { 0 } else if lg.iter().any(|c| c.0 == "wipeout") && lg.iter().any(|c| c.0 == "check_angles") { 1 } else if lg.iter().any(|c| c.0 == "wipeout") { 2 } else if lg.iter().any(|c| c.0 == "translate") { 3 } else { 4 };
        seen[kind] += 1;
    }
    println!("landed, crashed landing, hit a wall hard, pushed off a wall, flew: {seen:?}");
    assert!(seen.iter().all(|&n| n > 300), "{seen:?}");
}
