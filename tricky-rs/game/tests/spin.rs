//! Our air spin and flip against the original's (board row G1a5): the game's own `SpinState_Update` 0x100908 run
//! in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), with its maths run as the
//! original (`Math_SinCos`, `Quat_Multiply`, `Vec3_Scale`, `Vec4_Scale`) and every other call answered and
//! recorded (scores, orientation, clips, lean, trick clips, start from the stick, landing alignment, the wind-up
//! mask, reset), and `tricky_game::spin::spin_update` on the same spin object and rider: every field and every
//! call bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::spin::{spin_update, Anim, SpinCall, SpinObj, SpinRider};

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

/// One of the choices, picked at random; only the chosen one is evaluated.
macro_rules! pick {
    ($g:expr, $($x:expr),+ $(,)?) => {{
        let n = [$(stringify!($x)),+].len() as u32;
        let mut i = $g.next() % n;
        let mut out = None;
        $( if i == 0 && out.is_none() { out = Some($x); } i = i.wrapping_sub(1); )+
        let _ = i;
        out.unwrap()
    }};
}

const BONE: u32 = 2;

fn put(m: &mut r5900::Mem, at: u32, v: &[f32]) {
    for (k, x) in v.iter().enumerate() {
        m.write_f32(at + 4 * k as u32, *x);
    }
}

fn get(m: &mut r5900::Mem, at: u32) -> [f32; 4] {
    [m.read_f32(at), m.read_f32(at + 4), m.read_f32(at + 8), m.read_f32(at + 12)]
}

fn write(m: &mut r5900::Mem, st: u32, rd: u32, stats: u32, s: &SpinObj, r: &SpinRider) {
    m.write_u32(st + 8, s.mode);
    for (o, v) in [(0xc, s.phase), (0x10, s.owed), (0x14, s.spin_hi), (0x18, s.spin_lo), (0x1c, s.spin_floor), (0x20, s.flip_hi), (0x24, s.flip_lo), (0x28, s.flip_floor), (0x2c, s.w_spin), (0x30, s.w_flip), (0x34, s.align), (0x50, s.emax_spin), (0x54, s.emax_flip)] {
        m.write_f32(st + o, v);
    }
    put(m, st + 0x40, &s.align_axis);
    for (o, v) in [(0x58, s.x58), (0x5c, s.x5c), (0x60, s.x60), (0x64, s.x64)] {
        m.write_u32(st + o, v);
    }
    m.write_u32(st + 0x70, rd);
    for (o, v) in [(0x12c, r.timescale), (0x57b8, r.land), (0x1d0, r.spin), (0x1d4, r.flip), (0x274, r.wind_spin), (0x278, r.wind_flip), (0x1f0, r.x1f0), (0x1f4, r.x1f4), (0x1f8, r.x1f8), (0x46e0, r.x46e0), (0x5800, r.x5800)] {
        m.write_f32(rd + o, v);
    }
    put(m, rd + 0x170, &r.quat);
    put(m, rd + 0x57d0, &r.hp_face);
    put(m, rd + 0x57e0, &r.hp_axis);
    put(m, rd + 0x4ab0, &r.hp_at);
    put(m, rd + 0x150, &r.vel);
    put(m, rd + 0x140, &r.pos);
    put(m, rd + BONE * 0x394 + 0x8c8, &r.bone);
    for (o, v) in [(0x1b4, r.switch), (0x420, r.class), (0x46d4, r.x46d4), (0x44d8, BONE)] {
        m.write_u32(rd + o, v);
    }
    m.write_u32(rd + 0x464, stats);
    m.write_u8(stats + 0x1a, r.tricks);
    m.write_u8(stats + 0x10, r.stat10);
    m.write_u32(stats + 0x40, r.mirror);
}

fn read(m: &mut r5900::Mem, st: u32, rd: u32, s0: &SpinObj, r0: &SpinRider) -> (SpinObj, SpinRider) {
    let f = |m: &mut r5900::Mem, a: u32| m.read_f32(a);
    let s = SpinObj {
        mode: m.read_u32(st + 8),
        phase: f(m, st + 0xc),
        owed: f(m, st + 0x10),
        spin_hi: f(m, st + 0x14),
        spin_lo: f(m, st + 0x18),
        spin_floor: f(m, st + 0x1c),
        flip_hi: f(m, st + 0x20),
        flip_lo: f(m, st + 0x24),
        flip_floor: f(m, st + 0x28),
        w_spin: f(m, st + 0x2c),
        w_flip: f(m, st + 0x30),
        align: f(m, st + 0x34),
        align_axis: [f(m, st + 0x40), f(m, st + 0x44), f(m, st + 0x48)],
        align_w: f(m, st + 0x4c),
        emax_spin: f(m, st + 0x50),
        emax_flip: f(m, st + 0x54),
        x58: m.read_u32(st + 0x58),
        x5c: m.read_u32(st + 0x5c),
        x60: m.read_u32(st + 0x60),
        x64: m.read_u32(st + 0x64),
    };
    let b = rd + BONE * 0x394 + 0x8c8;
    let r = SpinRider {
        land: f(m, rd + 0x57b8),
        quat: get(m, rd + 0x170),
        spin: f(m, rd + 0x1d0),
        flip: f(m, rd + 0x1d4),
        switch: m.read_u32(rd + 0x1b4),
        wind_spin: f(m, rd + 0x274),
        wind_flip: f(m, rd + 0x278),
        x1f0: f(m, rd + 0x1f0),
        x1f4: f(m, rd + 0x1f4),
        x1f8: f(m, rd + 0x1f8),
        vel: get(m, rd + 0x150),
        pos: get(m, rd + 0x140),
        bone: [f(m, b), f(m, b + 4), f(m, b + 8)],
        x46d4: m.read_u32(rd + 0x46d4),
        x46e0: f(m, rd + 0x46e0),
        ..*r0
    };
    let _ = s0;
    (s, r)
}

fn bits_s(s: &SpinObj) -> Vec<u32> {
    [s.phase, s.owed, s.spin_hi, s.spin_lo, s.spin_floor, s.flip_hi, s.flip_lo, s.flip_floor, s.w_spin, s.w_flip, s.align, s.emax_spin, s.emax_flip]
        .iter()
        .chain(s.align_axis.iter())
        .map(|x| x.to_bits())
        .chain([s.mode, s.x58, s.x5c, s.x60, s.x64])
        .collect()
}

fn bits_r(r: &SpinRider) -> Vec<u32> {
    [r.land, r.spin, r.flip, r.wind_spin, r.wind_flip, r.x1f0, r.x1f4, r.x1f8, r.x46e0]
        .iter()
        .chain(r.quat.iter())
        .chain(r.vel.iter())
        .chain(r.pos.iter())
        .chain(r.bone.iter())
        .map(|x| x.to_bits())
        .chain([r.switch, r.x46d4])
        .collect()
}

#[test]
fn spin_is_the_originals() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x250c10, 0x102e80, 0x102f80, 0x102f50] {
        r.allow(f);
    }
    let anim = Rc::new(RefCell::new(Anim::default()));
    let log = Rc::new(RefCell::new(Vec::<SpinCall>::new()));
    let rec = |r: &mut Runner, addr: u32, f: fn(&r5900::Cpu) -> SpinCall, log: &Rc<RefCell<Vec<SpinCall>>>| {
        let l = log.clone();
        r.stub(addr, move |cpu, _| l.borrow_mut().push(f(cpu)));
    };
    rec(&mut r, 0x155e00, |c| SpinCall::ScoreSpin(c.gpr(5) as i32), &log);
    rec(&mut r, 0x155e68, |c| SpinCall::ScoreFlip(c.gpr(5) as i32), &log);
    rec(&mut r, 0x129008, |_| SpinCall::Orientation, &log);
    rec(&mut r, 0x102800, |_| SpinCall::ExitClip, &log);
    rec(&mut r, 0x127918, |c| SpinCall::Lean(c.f(12).to_bits(), c.f(13).to_bits()), &log);
    rec(&mut r, 0x15fcf8, |c| SpinCall::PlayClip(c.gpr(5) as u32), &log);
    rec(&mut r, 0x15eef8, |c| SpinCall::TurnLayers(c.f(12).to_bits()), &log);
    rec(&mut r, 0x11db00, |_| SpinCall::LandingAlign, &log);
    rec(&mut r, 0x127d08, |_| SpinCall::WindMask, &log);
    rec(&mut r, 0x118f10, |c| SpinCall::Reset(c.f(12).to_bits()), &log);
    let (l, a) = (log.clone(), anim.clone());
    r.stub(0x1272e8, move |c, _| {
        l.borrow_mut().push(SpinCall::TrickClips(c.gpr(5) as u32, c.gpr(6) as i32));
        c.set_gpr(2, a.borrow().trick_clips as u64);
    });
    let (l, a) = (log.clone(), anim.clone());
    r.stub(0x102158, move |c, _| {
        l.borrow_mut().push(SpinCall::StartFromStick);
        c.set_gpr(2, a.borrow().start_from_stick as u64);
    });
    let a = anim.clone();
    r.stub(0x15fdd0, move |c, _| c.set_gpr(2, a.borrow().clip as u64));
    let a = anim.clone();
    r.stub(0x15eec0, move |c, _| c.set_gpr(2, a.borrow().clip_type as u64));
    let a = anim.clone();
    r.stub(0x15fe70, move |c, _| c.set_f(0, a.borrow().phase));
    let st = r.mem.alloc(0x80);
    let rd = r.mem.alloc(0x6000);
    let stats = r.mem.alloc(0x50);
    let pad = r.mem.alloc(4);
    let mut g = Rng(0x5b1e_0000_0000_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..40000 {
        let ang = |g: &mut Rng| match g.next() % 4 { 0 => pick!(g, 0.0, std::f32::consts::PI, -std::f32::consts::PI) + g.f(-0.3, 0.3), _ => g.f(-20.0, 20.0) };
        let small = |g: &mut Rng| match g.next() % 3 { 0 => g.f(-0.6, 0.6), _ => g.f(-9.0, 9.0) };
        let v = |g: &mut Rng, k: f32| [g.f(-k, k), g.f(-k, k), g.f(-k, k), pick!(g, 0.0, 1.0, g.f(-1.0, 1.0))];
        let s0 = SpinObj {
            mode: pick!(g, 0, 1, 2, 2, 2, 3),
            phase: g.f(0.0, 1.0),
            owed: pick!(g, 0.0, std::f32::consts::PI, g.f(-4.0, 4.0)),
            spin_hi: small(&mut g),
            spin_lo: small(&mut g),
            spin_floor: pick!(g, 0.0, g.f(0.0, 1.4)),
            flip_hi: small(&mut g),
            flip_lo: small(&mut g),
            flip_floor: pick!(g, 0.0, g.f(0.0, 1.4)),
            w_spin: small(&mut g),
            w_flip: small(&mut g),
            align: pick!(g, 0.0, 0.0, g.f(-1.5, 1.5)),
            align_axis: [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)],
            align_w: 0.0,
            emax_spin: pick!(g, std::f32::consts::TAU, g.f(0.0, 4.0)),
            emax_flip: pick!(g, std::f32::consts::TAU, g.f(0.0, 4.0)),
            x58: g.next() % 2,
            x5c: pick!(g, 0, 0, 1),
            x60: g.next() % 2,
            x64: g.next() % 2,
        };
        let r0 = SpinRider {
            timescale: pick!(g, 1.0, 1.0, g.f(0.3, 1.5)),
            land: pick!(g, 0.0, -0.5, g.f(0.0, 1.5), g.f(0.0, 0.02)),
            quat: { let q = v(&mut g, 1.0); [q[0], q[1], q[2], g.f(-1.0, 1.0)] },
            spin: ang(&mut g),
            flip: ang(&mut g),
            tricks: g.next() as u8,
            stat10: g.next() as u8,
            mirror: g.next() % 2,
            switch: g.next() % 2,
            wind_spin: pick!(g, 0.0, g.f(-1.0, 1.0)),
            wind_flip: pick!(g, 0.0, g.f(-1.0, 1.0)),
            x1f0: pick!(g, 0.0, 0.0, g.f(0.0, 1.0)),
            x1f4: g.f(0.0, 4.0),
            x1f8: g.f(0.0, 1.0),
            class: pick!(g, 0, 1, 2),
            hp_face: v(&mut g, 1.0),
            hp_axis: v(&mut g, 1.0),
            hp_at: v(&mut g, 2000.0),
            vel: v(&mut g, 3000.0),
            pos: v(&mut g, 100000.0),
            bone: [g.f(-50.0, 50.0), g.f(-50.0, 50.0), g.f(-50.0, 150.0)],
            x46d4: g.next() % 2,
            x46e0: g.f(-4.0, 4.0),
            x5800: g.f(-3.0, 3.0),
        };
        let an = Anim {
            clip: pick!(g, 0x23c, 0x23d, 0x225, 0x223, 0x223, 0x200, 0x201),
            clip_type: pick!(g, 0xc, 0x2, 0x8),
            phase: g.f(0.0, 1.0),
            trick_clips: pick!(g, 0, 0, 0, 1),
            start_from_stick: pick!(g, 0, 0, 0, 1),
        };
        let p = g.next() & !0x100 | if g.next() % 8 == 0 { 0x100 } else { 0 };
        *anim.borrow_mut() = an;
        log.borrow_mut().clear();
        write(&mut r.mem, st, rd, stats, &s0, &r0);
        r.mem.write_u32(pad, p);
        r.cpu.set_gpr(4, st as u64);
        r.cpu.set_gpr(5, pad as u64);
        r.call(0x100908).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
        let (sg, rg) = read(&mut r.mem, st, rd, &s0, &r0);
        let (mut so, mut ro) = (s0, r0);
        let calls = spin_update(p, &mut so, &mut ro, &an);
        let got = log.borrow().clone();
        if calls.contains(&SpinCall::TurnLayers(std::f32::consts::PI.to_bits())) {
            *seen.entry("half-pipe turn").or_default() += 1;
        }
        if calls.contains(&SpinCall::PlayClip(0x222)) {
            *seen.entry("late wind-up start").or_default() += 1;
        }
        if calls.contains(&SpinCall::ExitClip) {
            *seen.entry("let go").or_default() += 1;
        }
        if calls.contains(&SpinCall::Orientation) && s0.align != 0.0 {
            *seen.entry("landing align").or_default() += 1;
        }
        if an.start_from_stick != 0 && calls.last() == Some(&SpinCall::StartFromStick) {
            *seen.entry("restarted from the stick").or_default() += 1;
        }
        if s0.mode == 2 && an.clip.wrapping_sub(0x23c) < 2 {
            *seen.entry("half-pipe clip phase").or_default() += 1;
        }
        if s0.x5c != 0 && calls.contains(&SpinCall::WindMask) && (ro.wind_spin != r0.wind_spin || ro.wind_flip != r0.wind_flip) {
            *seen.entry("late wind-up moving").or_default() += 1;
        }
        if an.clip == 0x223 && calls.contains(&SpinCall::WindMask) && s0.x5c == 0 {
            *seen.entry("clip 0x223 lean").or_default() += 1;
        }
        if s0.mode == 1 && an.trick_clips != 0 {
            *seen.entry("trick clip took over").or_default() += 1;
        }
        assert_eq!(got, calls, "case {case}: calls\n{s0:?}\n{r0:?}\n{an:?} pad {p:#x}");
        assert_eq!(bits_s(&sg), bits_s(&so), "case {case}: spin object\ngame {sg:?}\nours {so:?}\n{s0:?}\n{r0:?}\n{an:?} pad {p:#x}");
        assert_eq!(bits_r(&rg), bits_r(&ro), "case {case}: rider\ngame {rg:?}\nours {ro:?}\n{s0:?}\n{r0:?}\n{an:?} pad {p:#x}");
    }
    eprintln!("branches reached: {seen:?}");
    for k in ["half-pipe turn", "late wind-up start", "let go", "landing align", "restarted from the stick", "half-pipe clip phase", "late wind-up moving", "clip 0x223 lean", "trick clip took over"] {
        assert!(seen.get(k).copied().unwrap_or(0) > 20, "too few cases reach {k}: {seen:?}");
    }
}
