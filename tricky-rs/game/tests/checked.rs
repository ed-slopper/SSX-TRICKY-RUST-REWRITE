//! Our ground forces against the original's, bit for bit (board row F4d): the game's own
//! `Boarder_ForwardDrag` 0x109cb8 and `Boarder_SideFriction` 0x109ef8, run in the function runner on the
//! player's SLUS_203.26 (TRICKY_ELF; skipped without it), and `tricky_game::ground` on the same inputs.
//!
//! The runner rounds every operation to nearest here, as IEEE f32 does: that is what matched the game in
//! PCSX2 for add, subtract and multiply (tricky-rs/docs/checking.md, F4f and F4g); for divide it is assumed
//! until row F4c settles how exact floats must be.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::air::integrate_rk4;
use tricky_game::ground::{forward_drag, ground_thrust, side_friction, spring_force, DragState, FrictionState, ThrustState};

/// The surface table's drag and grip terms of a few real rows (SURFACES in rider.rs: +4, +8, +0xc, +0x10).
const ROWS: [[f32; 4]; 4] = [
    [0.00204, 0.00196, 0.00751, 1.2017],
    [0.00606, 0.0061, 0.00884, 1.50908],
    [1.0, 1.0, 1.0, 1.0],
    [0.0127, 0.02, 0.05, 0.2],
];

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
    fn byte(&mut self) -> u8 {
        self.next() as u8
    }
}

/// Memory laid out as the original reads it: boarder (+0xc → rider), rider (+0x464 → stats), surface row.
struct Layout {
    boarder: u32,
    rider: u32,
    stats: u32,
    row: u32,
}

fn layout(r: &mut Runner) -> Layout {
    let stats = r.mem.alloc(0x50);
    let rider = r.mem.alloc(0x5000); // up to the clip queue at +0x46c0
    let boarder = r.mem.alloc(0x10);
    let row = r.mem.alloc(0x64);
    r.mem.write_u32(boarder + 0xc, rider);
    r.mem.write_u32(rider + 0x464, stats);
    Layout { boarder, rider, stats, row }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn forward_drag_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let m = layout(&mut r);
    let mut rng = Rng(0x5eed_d4a9);
    let mut checked = 0;
    for case in 0..3000 {
        let row = ROWS[case % ROWS.len()];
        let powder = case % 3 == 0;
        let (speed, edging, cubic) = (rng.byte(), rng.byte(), rng.byte());
        let s = DragState {
            speed: speed as f32,
            edging: edging as f32,
            cubic: cubic as f32,
            class: rng.next() % 3,
            switch: rng.next() % 2 == 0,
            powder: powder.then(|| (rng.f(-8.0, 3.0), rng.f(2.0, 30.0))),
            crouch: rng.f(0.0, 1.0),
            boost_level: [0.0, 0.25, 0.6013, 1.0][(rng.next() % 4) as usize],
            brake: rng.f(0.0, 1.0),
        };
        let (vf, load) = (rng.f(-3400.0, 3400.0), rng.f(0.0, 4.0));

        r.mem.write_u8(m.stats + 0x0e, speed);
        r.mem.write_u8(m.stats + 0x11, edging);
        r.mem.write_u8(m.stats + 0x19, cubic);
        r.mem.write_u32(m.stats + 0x40, 0);
        r.mem.write_u32(m.rider + 0x1b4, s.switch as u32);
        r.mem.write_u32(m.rider + 0x420, s.class);
        let (h, d2) = s.powder.unwrap_or((0.0, 1.0));
        r.mem.write_u32(m.rider + 0x290, if powder { 3 } else { 1 });
        r.mem.write_f32(m.rider + 0x1bc, h);
        r.mem.write_f32(m.rider + 0x298, d2);
        r.mem.write_f32(m.rider + 0x208, s.crouch);
        r.mem.write_f32(m.rider + 0x130, s.boost_level);
        r.mem.write_f32(m.rider + 0x1fc, s.brake);
        for (i, v) in row[..3].iter().enumerate() {
            r.mem.write_f32(m.row + 4 + 4 * i as u32, *v);
        }
        r.cpu.set_f(12, vf);
        r.cpu.set_f(13, load);
        r.cpu.set_gpr(4, m.boarder as u64);
        r.cpu.set_gpr(5, m.row as u64);
        r.call(0x109cb8).unwrap_or_else(|e| panic!("case {case}: {e}"));

        let ours = forward_drag(vf, load, [row[0], row[1], row[2]], &s);
        assert_eq!(
            ours.to_bits(),
            r.cpu.f(0).to_bits(),
            "case {case}: ours {ours} vs the game's {} for vf {vf} load {load} {s:?}",
            r.cpu.f(0)
        );
        checked += 1;
    }
    eprintln!("Boarder_ForwardDrag: {checked} cases, all bit-identical");
}

#[test]
fn side_friction_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let m = layout(&mut r);
    let mut rng = Rng(0xf1c7_10e5);
    let mut checked = 0;
    for case in 0..3000 {
        let row = ROWS[case % ROWS.len()];
        let grip_byte = rng.byte();
        let s = FrictionState {
            grip: grip_byte as f32,
            class: rng.next() % 3,
            switch: rng.next() % 2 == 0,
            boost_level: [0.0, 0.25, 0.6013, 1.0][(rng.next() % 4) as usize],
        };
        // full lock now and then, so the rider+0x214 term is exercised
        let x214 = if case % 5 == 0 { [1.0, -1.0][case % 2] } else { rng.f(-1.0, 1.0) };
        let (vr, vf) = (rng.f(-1500.0, 1500.0), rng.f(-3400.0, 3400.0));

        r.mem.write_u8(m.stats + 0x13, grip_byte);
        r.mem.write_u32(m.stats + 0x40, 0);
        r.mem.write_u32(m.rider + 0x1b4, s.switch as u32);
        r.mem.write_u32(m.rider + 0x420, s.class);
        r.mem.write_f32(m.rider + 0x130, s.boost_level);
        r.mem.write_f32(m.row + 0x10, row[3]);
        r.cpu.set_f(12, vr);
        r.cpu.set_f(13, vf);
        r.cpu.set_f(14, x214);
        r.cpu.set_gpr(4, m.boarder as u64);
        r.cpu.set_gpr(5, m.row as u64);
        r.call(0x109ef8).unwrap_or_else(|e| panic!("case {case}: {e}"));

        let ours = side_friction(vr, vf, x214, row[3], &s);
        assert_eq!(
            ours.to_bits(),
            r.cpu.f(0).to_bits(),
            "case {case}: ours {ours} vs the game's {} for vr {vr} vf {vf} x214 {x214} {s:?}",
            r.cpu.f(0)
        );
        checked += 1;
    }
    eprintln!("Boarder_SideFriction: {checked} cases, all bit-identical");
}

#[test]
fn spring_force_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let m = layout(&mut r);
    let mut rng = Rng(0x5971_96f0);
    let mut checked = 0;
    for case in 0..3000 {
        let d1 = rng.f(0.3, 5.0);
        let d2 = d1 + rng.f(0.5, 35.0);
        // above the snow, just in it, and deeper than the band (all three of the game's cases)
        let h = match case % 3 { 0 => rng.f(0.001, 15.0), 1 => -rng.f(0.0, d1 * 0.999), _ => -rng.f(d1, d2 + 10.0) };
        let vn = rng.f(-500.0, 500.0);
        let g = [989.826, 1300.85, 1200.45][case % 3];
        let damping = rng.f(0.0, 50.0);

        r.mem.write_f32(m.rider + 0x294, d1);
        r.mem.write_f32(m.rider + 0x298, d2);
        r.mem.write_f32(m.row, g);
        r.mem.write_f32(m.row + 0x24, damping);
        r.cpu.set_f(12, h);
        r.cpu.set_f(13, vn);
        r.cpu.set_gpr(4, m.boarder as u64);
        r.cpu.set_gpr(5, m.row as u64);
        r.call(0x109878).unwrap_or_else(|e| panic!("case {case}: {e}"));

        let ours = spring_force(h, vn, d1, d2, g, damping);
        assert_eq!(ours.to_bits(), r.cpu.f(0).to_bits(),
            "case {case}: ours {ours} vs the game's {} for h {h} vn {vn} d1 {d1} d2 {d2} g {g} damping {damping}", r.cpu.f(0));
        checked += 1;
    }
    eprintln!("Boarder_GroundSpringForce: {checked} cases, all bit-identical");
}

#[test]
fn ground_thrust_is_the_originals() {
    let Some(mut r) = runner() else { return };
    r.allow(0x15fdd0); // the clip getter runs as the original
    let m = layout(&mut r);
    let mut rng = Rng(0x7a7e_5ee0);
    let mut checked = 0;
    for case in 0..3000 {
        let speed_byte = rng.byte();
        let s = ThrustState {
            brake: if case % 10 == 0 { rng.f(0.01, 1.0) } else { 0.0 },
            board_yaw: rng.f(-12.0, 12.0),
            course_yaw: rng.f(-12.0, 12.0),
            boost_level: [0.0, 0.25, 0.6013, 1.0][(rng.next() % 4) as usize],
            speed_timer: if case % 4 == 0 { rng.f(0.0, 3.0) } else { 0.0 },
            steer: if case % 2 == 0 { rng.f(-0.1, 0.1) } else { rng.f(-1.0, 1.0) },
            forward_up: rng.f(-1.0, 1.0),
            vel: [rng.f(-2000.0, 2000.0), rng.f(-2000.0, 2000.0), rng.f(-600.0, 600.0), 0.0],
            class: rng.next() % 3,
            speed: speed_byte as f32,
            skating: case % 3 == 0,
        };
        let (push_target, thrust_scale) = [(51.8501, 2.00119), (45.6771, 1.81067), (0.0, 0.0), (140.0, 1.0)][case % 4];

        r.mem.write_u8(m.stats + 0x0e, speed_byte);
        r.mem.write_f32(m.rider + 0x1fc, s.brake);
        r.mem.write_f32(m.rider + 0x1b0, s.board_yaw);
        r.mem.write_f32(m.rider + 0x370, s.course_yaw);
        r.mem.write_f32(m.rider + 0x130, s.boost_level);
        r.mem.write_f32(m.rider + 0x134, s.speed_timer);
        r.mem.write_f32(m.rider + 0x214, s.steer);
        r.mem.write_f32(m.rider + 0x328, s.forward_up);
        for (i, v) in s.vel.iter().enumerate() {
            r.mem.write_f32(m.rider + 0x150 + 4 * i as u32, *v);
        }
        r.mem.write_u32(m.rider + 0x420, s.class);
        r.mem.write_u32(m.rider + 0x46c0 + 8, if s.skating { 0x221 } else { 0x200 });
        r.mem.write_f32(m.row + 0x2c, push_target);
        r.mem.write_f32(m.row + 0x30, thrust_scale);
        r.cpu.set_gpr(4, m.boarder as u64);
        r.cpu.set_gpr(5, m.row as u64);
        r.call(0x109950).unwrap_or_else(|e| panic!("case {case}: {e}"));

        let ours = ground_thrust(&s, push_target, thrust_scale);
        assert_eq!(ours.to_bits(), r.cpu.f(0).to_bits(),
            "case {case}: ours {ours} vs the game's {} for {s:?} target {push_target} scale {thrust_scale}", r.cpu.f(0));
        checked += 1;
    }
    eprintln!("Boarder_GroundThrust: {checked} cases, all bit-identical");
}

#[test]
fn air_rk4_is_the_originals() {
    let Some(mut r) = runner() else { return };
    let m = layout(&mut r);
    let (pos, vel) = (r.mem.alloc(16), r.mem.alloc(16));
    let mut rng = Rng(0xa1b4_7e11);
    let mut checked = 0;
    for case in 0..3000 {
        // a riding frame most of the time, a flight prediction (in 0.2 s steps) now and then
        let substeps = case % 4 == 3;
        let dt = match case % 4 {
            0 => 1.0 / 60.0,
            1 => rng.f(0.001, 0.05),
            2 => 2.0 / 60.0,
            _ => rng.f(0.01, 1.5),
        };
        let mut p = [rng.f(-50000.0, 50000.0), rng.f(-50000.0, 50000.0), rng.f(-20000.0, 20000.0), 1.0];
        // around the cap now and then, and sometimes nearly still on the way up or down
        let s = if case % 5 == 0 { 3400.0 } else { 2500.0 };
        let mut v = [rng.f(-s, s), rng.f(-s, s), rng.f(-s, s), 0.0];
        if case % 7 == 0 {
            v[2] = rng.f(-5.0, 5.0);
        }
        for i in 0..4 {
            r.mem.write_f32(pos + 4 * i as u32, p[i]);
            r.mem.write_f32(vel + 4 * i as u32, v[i]);
        }
        r.cpu.set_f(12, dt);
        r.cpu.set_gpr(4, m.rider as u64);
        r.cpu.set_gpr(5, pos as u64);
        r.cpu.set_gpr(6, vel as u64);
        r.cpu.set_gpr(7, substeps as u64);
        r.call(0x12b340).unwrap_or_else(|e| panic!("case {case}: {e}"));

        let (p0, v0) = (p, v);
        integrate_rk4(dt, &mut p, &mut v, substeps);
        for i in 0..4 {
            let (gp, gv) = (r.mem.read_f32(pos + 4 * i as u32), r.mem.read_f32(vel + 4 * i as u32));
            assert_eq!((p[i].to_bits(), v[i].to_bits()), (gp.to_bits(), gv.to_bits()),
                "case {case} lane {i}: ours {p:?} {v:?} vs the game's {gp} {gv} for dt {dt} substeps {substeps} from {p0:?} {v0:?}");
        }
        checked += 1;
    }
    eprintln!("Air_IntegrateRK4: {checked} cases, all bit-identical");
}
