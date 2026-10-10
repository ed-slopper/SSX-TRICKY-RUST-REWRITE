//! The next targets of the checking design (board row F4e): `AI_RubberBandSpeedScale`, `Takeoff_SetSpinRates`,
//! `Jump_ApplyImpulse` and `Air_IntegrateRK4`, run to the end on inputs built from the port notes
//! (`tricky-rs/docs/original-rules.md`) and checked against the notes' formulas where there is one. Their small
//! pure callees run as the original (sine, cosine, vector scale, a table getter); callees with side effects are
//! stubs that record the call. Skips when TRICKY_ELF is not set.

use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;

const SIN: u32 = 0x250d60;
const COS: u32 = 0x250e98;
const VEC4_SCALE: u32 = 0x102f50;
const TABLE_GET: u32 = 0x15fdd0;

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    for f in [SIN, COS, VEC4_SCALE, TABLE_GET] {
        r.allow(f);
    }
    Some(r)
}

fn close(what: &str, got: f64, want: f64, rel: f64) {
    let err = (got - want).abs();
    assert!(err <= want.abs() * rel + 1e-4, "{what}: runner {got} vs notes {want}");
}

fn vec4(r: &mut Runner, at: u32, v: [f32; 4]) {
    for (i, x) in v.iter().enumerate() {
        r.mem.write_f32(at + 4 * i as u32, *x);
    }
}

fn read_vec(r: &mut Runner, at: u32) -> [f64; 3] {
    [r.mem.read_f32(at) as f64, r.mem.read_f32(at + 4) as f64, r.mem.read_f32(at + 8) as f64]
}

/// Records each call of a stubbed function: its first float argument and its first integer argument.
fn recorder(r: &mut Runner, addr: u32, f0: f32) -> Rc<RefCell<Vec<(f32, u64, u64)>>> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let c = calls.clone();
    r.stub(addr, move |cpu, _| {
        c.borrow_mut().push((cpu.f(12), cpu.gpr(4), cpu.gpr(5)));
        cpu.set_f(0, f0);
    });
    calls
}

/// The rubber band (`AI_RubberBandSpeedScale` 0x13a0f0; notes: "AI, race, camera").
fn rubber_band_formula(dist: f64, angle: f64, heading: f64, min: f64, max: f64) -> f64 {
    let d = -dist * (angle - heading).cos();
    let m = if d < min {
        ((d - min) / min + 1.0) * 1.1359849
    } else if d > max {
        2.0487268 / (d - max)
    } else {
        1.0
    };
    m.clamp(0.70005476, 1.5022597)
}

#[test]
fn rubber_band() {
    let Some(mut r) = runner() else { return };
    let (min, max) = (-1101.80f32, 5509.69f32); // the single-race window of rider slot 1
    for (target, dist, angle) in [
        (-1, 0.0f32, 0.0f32),       // no target: 1
        (0, -500.0, 0.0),           // inside the window: 1
        (0, 1367.4, 0.5),           // behind: unclamped, about 1.24
        (2, 2000.0, 0.0),           // far behind: clamped to 1.5023
        (1, -5512.19, 0.0),         // just ahead: unclamped, about 0.82
        (3, -6000.0, 0.2),          // far ahead: clamped to 0.70005
    ] {
        let ai = r.mem.alloc(0x380);
        r.mem.write_u32(ai + 0x100, target as u32);
        if target >= 0 {
            let e = ai + 0x40 + 0x20 * target as u32;
            r.mem.write_f32(e, dist);
            r.mem.write_f32(e + 4, angle + 0.25);
        }
        r.mem.write_f32(ai + 0x370, 0.25); // heading
        r.mem.write_f32(ai + 0x104, min);
        r.mem.write_f32(ai + 0x108, max);
        let holder = r.mem.alloc(4);
        r.mem.write_u32(holder, ai);
        r.cpu.set_gpr(4, holder as u64);
        r.call(0x13a0f0).unwrap_or_else(|e| panic!("{e}"));
        let want = if target < 0 { 1.0 } else { rubber_band_formula(dist as f64, angle as f64 + 0.25, 0.25, min as f64, max as f64) };
        close("rubber band", r.cpu.f(0) as f64, want, 1e-5);
    }
}

/// A rider with the fields the boarder code reads, zero elsewhere.
fn rider(r: &mut Runner, jump_stat: u8, tricks_stat: u8) -> (u32, u32) {
    let stats = r.mem.alloc(0x50);
    r.mem.write_u8(stats + 0x17, jump_stat);
    r.mem.write_u8(stats + 0x1a, tricks_stat);
    let rider = r.mem.alloc(0x6000);
    r.mem.write_u32(rider + 0x464, stats);
    (rider, stats)
}

#[test]
fn takeoff_spin_rates() {
    let Some(mut r) = runner() else { return };
    for (class, fakie, ws, wf, t) in [(1u32, 0u32, 0.8f32, 0.0f32, 128u8), (2, 1, 0.5, -0.6, 200), (0, 0, -1.0, 1.0, 0)] {
        let takeoff_setup = recorder(&mut r, 0x12bee0, 0.0);
        let pick_clip = recorder(&mut r, 0x126fc0, 0.0);
        let set_motion = recorder(&mut r, 0x11f608, 0.0);
        let (rd, _) = rider(&mut r, 0, t);
        let motion = r.mem.alloc(0x800);
        r.mem.write_u32(rd + 0x5ae0, motion);
        r.mem.write_u32(rd + 0x46c0 + 8, 0x200); // the clip playing: not 0x218 or 0x260/0x261
        r.mem.write_u32(rd + 0x420, class);
        r.mem.write_u32(rd + 0x1b4, fakie);
        r.mem.write_f32(rd + 0x220, ws);
        r.mem.write_f32(rd + 0x22c, wf);
        r.cpu.set_gpr(4, rd as u64);
        r.call(0x126e30).unwrap_or_else(|e| panic!("{e}"));
        // notes: K = 11.517(0.5449 + 0.6742T); spin = K·ws (×2/3 alpine), flip = (2/3)K·wf, flip negated riding fakie
        let k = (t as f64 * 0.6741781 / 255.0 + 0.5449139) * 11.5171995;
        let spin = k * ws as f64 * if class == 2 { 0.6666667 } else { 1.0 };
        let flip = k * 0.6666667 * wf as f64 * if fakie != 0 { -1.0 } else { 1.0 };
        close("spin rate", r.mem.read_f32(motion + 0x76c) as f64, spin, 1e-5);
        close("flip rate", r.mem.read_f32(motion + 0x770) as f64, flip, 1e-5);
        assert_eq!(r.mem.read_f32(rd + 0x220), 0.0, "wind-up cleared");
        assert_eq!(takeoff_setup.borrow().len(), 1);
        assert_eq!(pick_clip.borrow().len(), 1);
        assert_eq!(set_motion.borrow().iter().map(|c| c.2).collect::<Vec<_>>(), vec![0x10], "into motion 0x10");
    }
}

#[test]
fn jump_impulse() {
    let Some(mut r) = runner() else { return };
    for on_rail in [false, true] {
        let score = recorder(&mut r, 0x156618, 0.25);
        let meter = recorder(&mut r, 0x11b018, 0.0);
        let (rd, _) = rider(&mut r, 128, 0);
        vec4(&mut r, rd + 0x150, [1000.0, 0.0, 0.0, 0.0]); // velocity
        vec4(&mut r, rd + 0x1a0, [0.0, 0.0, 1.0, 0.0]); // board up: the rail jump's direction
        vec4(&mut r, rd + 0x320, [1.0, 0.0, 0.0, 0.0]); // board forward: F in the notes (0x1286e8)
        vec4(&mut r, rd + 0x2a0, [0.0, 0.0, 1.0, 0.0]); // ground normal: flat
        r.mem.write_f32(rd + 0x1c8, 1.0); // fully crouched
        r.mem.write_u32(rd + 0x424, if on_rail { 3 } else { 1 });
        let before = read_vec(&mut r, rd + 0x150);
        r.cpu.set_f(12, 630.88); // the smallest jump (notes: Δv = max(630.88, ...))
        r.cpu.set_gpr(4, rd as u64);
        r.call(0x1284e0).unwrap_or_else(|e| panic!("{e}"));
        let after = read_vec(&mut r, rd + 0x150);
        let dv = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2) + (after[2] - before[2]).powi(2)).sqrt();
        eprintln!("jump on {}: velocity {before:?} -> {after:?}, |Δv| {dv:.2}", if on_rail { "rail" } else { "snow" });
        // c² (0.0985 + 0.787 J) S(v) = 1 · (0.0985 + 0.787 · 128/255) · 899.87 = 444 < 630.88, so Δv is the minimum
        close("|Δv|", dv, 630.88, 1e-4);
        // direction: on snow normalize(N + 0.2F) (rider+0x2a0, rider+0x320); on a rail the board's up (rider+0x1a0)
        let dir = if on_rail { [0.0, 0.0, 1.0] } else { [0.2 / 1.04f64.sqrt(), 0.0, 1.0 / 1.04f64.sqrt()] };
        for i in 0..3 {
            close("Δv direction", (after[i] - before[i]) / dv, dir[i], 1e-4);
        }
        assert_eq!(score.borrow().len(), 1, "the jump is scored");
        assert_eq!(meter.borrow().len(), 1, "and the points fill the meter");
        assert_eq!(meter.borrow()[0].0, 0.25, "with what scoring returned");
    }
}

#[test]
fn air_step() {
    let Some(mut r) = runner() else { return };
    let (rd, _) = rider(&mut r, 0, 0);
    r.mem.write_f32(rd + 0x1c4, 3347.2); // speed cap
    let pos = r.mem.alloc(16);
    let vel = r.mem.alloc(16);
    vec4(&mut r, pos, [0.0, 0.0, 0.0, 1.0]);
    vec4(&mut r, vel, [1000.0, 200.0, 500.0, 0.0]);
    let dt = 1.0f32 / 60.0;
    r.cpu.set_f(12, dt);
    r.cpu.set_gpr(4, rd as u64);
    r.cpu.set_gpr(5, pos as u64);
    r.cpu.set_gpr(6, vel as u64);
    r.cpu.set_gpr(7, 0);
    r.call(0x12b340).unwrap_or_else(|e| panic!("{e}"));
    let (p, v) = (read_vec(&mut r, pos), read_vec(&mut r, vel));
    eprintln!("air step: position {p:?}, velocity {v:?}");
    // notes: gravity −850.24 cm/s² rising, horizontal drag −0.20002v
    let dt = dt as f64;
    close("vx", v[0], 1000.0 * (1.0 - 0.20002478 * dt), 1e-4);
    close("vy", v[1], 200.0 * (1.0 - 0.20002478 * dt), 1e-4);
    close("vz", v[2], 500.0 - 850.24 * dt, 1e-4);
    close("x", p[0], (1000.0 + v[0]) / 2.0 * dt, 1e-3);
    close("z", p[2], (500.0 + v[2]) / 2.0 * dt, 1e-3);
}
