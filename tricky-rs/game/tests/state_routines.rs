//! The small state routines against the original's (board row G18a): each enter and exit routine
//! `states::run_routine` ports, run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without
//! it) on random rider fields, every field it can touch and every call it makes compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::orient::{update_orientation, Orient};
use tricky_game::states::{run_routine, RoutineCall, StateObj, StateRider};

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
}

/// Each ported routine and where its object keeps the rider.
const ROUTINES: [(u32, u32); 29] = [
    (0x1049f8, 0xc),
    (0x108368, 0x20), (0x108310, 0x20), (0x10aec8, 0xc), (0x109858, 0xc), (0x10b090, 0x10), (0x10b068, 0x10),
    (0x10d680, 0x60), (0x110290, 0xc), (0x110260, 0xc), (0x104388, 0x10), (0x104190, 0x10), (0x1033c0, 0x14),
    (0x102fb0, 0x14), (0x104178, 0), (0x103eb8, 0), (0x104688, 0), (0x104790, 0), (0x104e38, 0xc), (0x1055a8, 0xc),
    (0x105070, 0xc), (0x1075f0, 0x14), (0x108280, 0), (0x106a48, 0x18), (0x106600, 0x10), (0x105d80, 0),
    (0x1000b8, 0xc), (0x108110, 0), (0x106500, 0xc),
];

fn put4(m: &mut r5900::Mem, at: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}
fn get4(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}

fn write(m: &mut r5900::Mem, rd: u32, r: &StateRider) {
    m.write_f32(rd + 0x138, r.x138);
    m.write_f32(rd + 0x13c, r.x13c);
    put4(m, rd + 0x150, r.vel);
    put4(m, rd + 0x170, r.orient.q);
    put4(m, rd + 0x180, r.orient.fwd);
    put4(m, rd + 0x190, r.orient.side);
    put4(m, rd + 0x1a0, r.orient.up);
    m.write_f32(rd + 0x1b0, r.orient.yaw);
    m.write_u32(rd + 0x1b4, r.stance);
    m.write_f32(rd + 0x1cc, r.x1cc);
    m.write_u32(rd + 0x1dc, r.kind);
    m.write_u32(rd + 0x1e4, r.x1e4);
    m.write_u32(rd + 0x1e8, r.x1e8);
    m.write_u32(rd + 0x1ec, r.x1ec);
    for k in 0..6 {
        m.write_f32(rd + 0x1fc + 4 * k as u32, r.x1fc[k]);
    }
    m.write_f32(rd + 0x214, r.steer);
    m.write_f32(rd + 0x218, r.x218);
    m.write_f32(rd + 0x21c, r.x21c);
    m.write_f32(rd + 0x244, r.lean);
    for k in 0..3 {
        m.write_f32(rd + 0x268 + 4 * k as u32, r.x268[k]);
    }
    m.write_f32(rd + 0x284, r.x284);
    m.write_f32(rd + 0x288, r.x288);
    m.write_f32(rd + 0x28c, r.x28c);
    m.write_f32(rd + 0x448, r.x448);
    m.write_f32(rd + 0x44c, r.x44c);
    m.write_u32(rd + 0x46d4, r.x46d4);
    m.write_f32(rd + 0x46e0, r.x46e0);
    m.write_u32(rd + 0x5ae4, r.x5ae4);
    m.write_u32(rd + 0x5af0, r.x5af0);
    m.write_u32(rd + 0x46dc, r.x46dc);
    m.write_u32(rd + 0x38, r.cam as u32);
    m.write_u32(rd + 0x420, r.class);
    m.write_u32(rd + 0x428, r.state);
}

fn read(m: &mut r5900::Mem, rd: u32, base: &StateRider) -> StateRider {
    let mut r = *base;
    r.x138 = m.read_f32(rd + 0x138);
    r.x13c = m.read_f32(rd + 0x13c);
    r.orient = Orient { q: get4(m, rd + 0x170), fwd: get4(m, rd + 0x180), side: get4(m, rd + 0x190), up: get4(m, rd + 0x1a0), yaw: m.read_f32(rd + 0x1b0) };
    r.stance = m.read_u32(rd + 0x1b4);
    r.x1cc = m.read_f32(rd + 0x1cc);
    r.kind = m.read_u32(rd + 0x1dc);
    r.x1e4 = m.read_u32(rd + 0x1e4);
    r.x1e8 = m.read_u32(rd + 0x1e8);
    r.x1ec = m.read_u32(rd + 0x1ec);
    for k in 0..6 {
        r.x1fc[k] = m.read_f32(rd + 0x1fc + 4 * k as u32);
    }
    r.steer = m.read_f32(rd + 0x214);
    r.x218 = m.read_f32(rd + 0x218);
    r.x21c = m.read_f32(rd + 0x21c);
    r.lean = m.read_f32(rd + 0x244);
    for k in 0..3 {
        r.x268[k] = m.read_f32(rd + 0x268 + 4 * k as u32);
    }
    r.x284 = m.read_f32(rd + 0x284);
    r.x288 = m.read_f32(rd + 0x288);
    r.x28c = m.read_f32(rd + 0x28c);
    r.x448 = m.read_f32(rd + 0x448);
    r.x44c = m.read_f32(rd + 0x44c);
    r.x46d4 = m.read_u32(rd + 0x46d4);
    r.x46e0 = m.read_f32(rd + 0x46e0);
    r.x5ae4 = m.read_u32(rd + 0x5ae4);
    r.x5af0 = m.read_u32(rd + 0x5af0);
    r.x46dc = m.read_u32(rd + 0x46dc);
    r
}

fn bits(r: &StateRider) -> String {
    // compare floats by their bits (NaN-safe), through the debug form of every field as bits
    let f = |v: f32| v.to_bits();
    format!(
        "{:x?}",
        (
            (f(r.x138), f(r.x13c), r.orient.q.map(f), r.orient.fwd.map(f), r.orient.side.map(f), r.orient.up.map(f), f(r.orient.yaw)),
            (r.stance, f(r.x1cc), r.kind, r.x1e4, r.x1e8, r.x1ec, r.x1fc.map(f)),
            (f(r.steer), f(r.x218), f(r.x21c), f(r.lean), r.x268.map(f), f(r.x284), f(r.x288), f(r.x28c)),
            (f(r.x448), f(r.x44c), r.x46d4, f(r.x46e0), r.x5ae4, r.x5af0, r.x46dc),
        )
    )
}

#[test]
fn state_routines_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x218b68u32, 0x11f440, 0x128e90, 0x129008, 0x250c10, 0x2510f8, 0x102f80, 0x102f50] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<RoutineCall>>> = Rc::default();
    let rider = r.mem.alloc(0x6000);
    let (app, game, stats, vt, obj) = (r.mem.alloc(0x800), r.mem.alloc(0x400), r.mem.alloc(0x100), r.mem.alloc(0x100), r.mem.alloc(0x100));
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(game + 0x38, 0x4848);
    for i in 0..4u32 {
        r.mem.write_u32(game + 0xa0 + 0x80 * i, 0xc000 + i);
    }
    r.mem.write_u32(0x343050, 0x5f5f);
    r.mem.write_u32(rider, vt);
    r.mem.write_u16(vt + 0x98, 0);
    r.mem.write_u32(vt + 0x9c, 0x1000);
    r.mem.write_u32(rider + 0x464, stats);
    let cam_now = Rc::new(std::cell::Cell::new(0i32));
    let stubs: Vec<(u32, Box<dyn Fn(&r5900::Cpu, &mut r5900::Mem) -> Option<RoutineCall>>)> = vec![
        (0x123b10, Box::new(move |c, _| (c.gpr(4) as u32 == rider + 0x57b0).then_some(RoutineCall::AirPredictAtTakeoff))),
        (0x2174f8, Box::new(move |c, _| (c.gpr(4) == 0x5f5f && c.gpr(5) as u32 == rider).then_some(RoutineCall::SfxOnAirEnter(c.gpr(6) != 0)))),
        (0x218b20, Box::new(move |c, _| (c.gpr(4) == 0x5f5f && c.gpr(5) as u32 == rider).then_some(RoutineCall::SfxStopWipeoutLoop))),
        (0x1000, Box::new(move |c, _| (c.gpr(4) as u32 == rider).then_some(RoutineCall::Virtual9c))),
        (0x128ae8, Box::new(move |c, _| (c.gpr(4) as u32 == rider).then_some(RoutineCall::ProbeGround))),
        (0x11d038, Box::new(move |c, _| (c.gpr(4) as u32 == rider && c.gpr(5) == 0 && c.gpr(6) == 0).then_some(RoutineCall::BoostOff))),
        (0x107628, Box::new(move |c, _| (c.gpr(4) as u32 == obj).then_some(RoutineCall::PlayQueuedRailClip))),
        (0x11f3f8, Box::new(move |c, _| (c.gpr(4) as u32 == rider && c.gpr(5) == 1).then_some(RoutineCall::StartDisappointed))),
        (0x1a7860, Box::new(move |c, _| (c.gpr(4) == 0x4848 && c.gpr(5) == 0xc).then_some(RoutineCall::HideHud))),
        (0x175de0, {
            let cn = cam_now.clone();
            Box::new(move |c, _| (c.gpr(4) as u32 == 0xc000 + cn.get() as u32 && c.gpr(5) == 0x360630 && c.gpr(6) == 0).then_some(RoutineCall::StartCamScript))
        }),
        (0x15f050, Box::new(move |c, _| (c.gpr(4) as u32 == rider + 0x46c0).then_some(RoutineCall::BlendOutAll))),
        (0x15fcf8, Box::new(move |c, m| (c.gpr(4) as u32 == rider + 0x46c0 && c.gpr(6) == 0 && c.gpr(7) == 5).then(|| {
            let clip = c.gpr(5) as u32;
            if (0x20c..=0x20e).contains(&clip) { RoutineCall::PlayClipMasked(clip, m.read_u32(rider + 0x46dc)) } else { RoutineCall::PlayClip(clip) }
        }))),
        (0x15eef8, Box::new(move |c, _| (c.gpr(4) as u32 == rider + 0x46c0 && c.f(12) == std::f32::consts::PI).then_some(RoutineCall::TurnAllLayers))),
    ];
    for (a, f) in stubs {
        let l = log.clone();
        r.stub(a, move |c, m| {
            let call = f(c, m).unwrap_or_else(|| panic!("call to {a:#x} with unexpected arguments"));
            l.borrow_mut().push(call);
        });
    }
    let mut g = Rng(0x7e15_c0a2_93db_4f68);
    let mut ran = 0;
    for case in 0..20_000 {
        let (addr, slot) = ROUTINES[g.next() as usize % ROUTINES.len()];
        let mut o = Orient { q: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)], ..Default::default() };
        update_orientation(&mut o);
        let mut fs = || g.f(-9.0, 9.0);
        let mut sr = StateRider {
            x138: fs(), x13c: fs(), vel: [0.0; 4], orient: o, stance: 0, x1cc: fs(), kind: 0, x1e4: 0, x1e8: 0, x1ec: 0,
            x1fc: [fs(), fs(), fs(), fs(), fs(), fs()], steer: fs(), x218: fs(), x21c: fs(), lean: fs(), x268: [fs(), fs(), fs()],
            x284: fs(), x288: fs(), x28c: fs(), x448: fs(), x44c: fs(), x46d4: 0, x46e0: fs(), x5ae4: 0, x5af0: 0, x46dc: 0, bone_b50: None,
            cam: 0, class: 0, state: 0, stats40: 0,
        };
        let sp = [g.f(0.0, 270.0), 277.77777, g.f(280.0, 3000.0)][g.next() as usize % 3];
        sr.vel = [sp, 0.0, 0.0, 0.0];
        sr.stance = g.next() % 2;
        sr.kind = g.next() % 5;
        (sr.x1e4, sr.x1e8, sr.x1ec, sr.x46d4) = (g.next(), g.next(), g.next(), g.next());
        sr.x5ae4 = g.next() % 2;
        sr.x5af0 = g.next() % 9;
        sr.cam = (g.next() % 4) as i32 - 1;
        sr.class = g.next() % 3;
        sr.state = [0xd, 3, 0x10][g.next() as usize % 3];
        sr.stats40 = g.next() % 2;
        sr.x46dc = g.next();
        let bone = (g.next() % 6) as i32 - 1;
        sr.bone_b50 = if bone < 0 { None } else { Some([g.next() % 20, g.next()][g.next() as usize % 2]) };
        if g.next() % 2 == 0 {
            sr.vel = [g.f(-3000.0, 3000.0), g.f(-3000.0, 3000.0), g.f(-500.0, 500.0), 0.0];
        }
        cam_now.set(sr.cam);
        let a1 = [6u32, 2, 0x16, 3][g.next() as usize % 4];
        let mut so = StateObj { w8: if g.next() % 2 == 0 { g.f(0.0, 1.0).to_bits() } else { g.next() }, wc: g.next(), w10: g.next() };
        write(&mut r.mem, rider, &sr);
        r.mem.write_u32(stats + 0x40, sr.stats40);
        r.mem.write_u32(rider + 0x44d8, bone as u32);
        if let Some(b) = sr.bone_b50 {
            r.mem.write_u32(rider + bone as u32 * 0x394 + 0xb50, b);
        }
        put4(&mut r.mem, rider + 0x150, sr.vel);
        for w in 0..0x1a {
            r.mem.write_u32(obj + 4 * w, 0xdead_0000 + w);
        }
        r.mem.write_u32(obj + 8, so.w8);
        r.mem.write_u32(obj + 0xc, so.wc);
        r.mem.write_u32(obj + 0x10, so.w10);
        r.mem.write_u32(obj + slot, rider);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, a1 as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let input = sr;
        let calls = run_routine(addr, &mut sr, &mut so, a1).expect("ported");
        let got = read(&mut r.mem, rider, &input);
        let ctx = format!("case {case}: {addr:#x} a1 {a1:#x} {input:?}");
        assert_eq!(bits(&got), bits(&sr), "{ctx}: fields");
        assert_eq!(*log.borrow(), calls, "{ctx}: calls");
        let words = [(8u32, so.w8), (0xc, so.wc), (0x10, so.w10)];
        for (off, v) in words {
            if off != slot {
                assert_eq!(r.mem.read_u32(obj + off), v, "{ctx}: object +{off:#x}");
            }
        }
        ran += 1;
    }
    assert_eq!(ran, 20_000);
}
