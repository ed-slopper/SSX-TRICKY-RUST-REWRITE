//! Our board orientation and take-off against the original's, bit for bit (board row E8j): the game's own
//! `Boarder_RotateAboutAxis` 0x128e90 (with `Boarder_UpdateOrientation` 0x129008, `Math_SinCos`, `Vec3_Scale`
//! 0x102f80 and `Math_Atan` run as the original) and `Boarder_TakeOff` 0x128470 (its callees recorded) run in
//! the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without it), and
//! `tricky_game::orient` on the same rider.
//!
//! Every case compares the whole first 0x600 bytes of the rider object, not just the fields we expect to change:
//! the original's result must equal the snapshot taken before the call with only our port's outputs written
//! over it, so a write the port does not know about fails too. `planted_*` show the comparison can tell: a
//! one-bit error in one output, or a different order of operations, must be caught.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::orient::{rotate_about_axis, take_off, update_orientation, Orient, TakeOffEnv, TakeOffRider};

type V4 = [f32; 4];

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
    fn unit4(&mut self) -> V4 {
        loop {
            let v = [self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
            if l > 0.1 {
                return [v[0] / l, v[1] / l, v[2] / l, v[3] / l];
            }
        }
    }
}

/// How many words of the rider object are compared.
const WORDS: u32 = 0x180;

struct W {
    r: Runner,
    rd: u32,
    axis: u32,
}

fn world() -> Option<W> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    // pure, run as the original: Math_SinCos, Boarder_UpdateOrientation, Vec3_Scale, Math_Atan
    for a in [0x250c10, 0x129008, 0x102f80, 0x2510f8] {
        r.allow(a);
    }
    let rd = r.mem.alloc(0x6000);
    let axis = r.mem.alloc(0x10);
    Some(W { r, rd, axis })
}

impl W {
    /// Every word of the rider block set to noise (the function must leave what it does not own alone).
    fn noise(&mut self, g: &mut Rng) {
        for i in 0..WORDS {
            let v = g.next() ^ (g.next() << 16);
            self.r.mem.write_u32(self.rd + 4 * i, v);
        }
    }
    fn put4(&mut self, off: u32, v: V4) {
        for (k, x) in v.iter().enumerate() {
            self.r.mem.write_f32(self.rd + off + 4 * k as u32, *x);
        }
    }
    fn block(&mut self) -> Vec<u32> {
        (0..WORDS).map(|i| self.r.mem.read_u32(self.rd + 4 * i)).collect()
    }
}

/// The snapshot with the port's orientation written over it.
fn with_orient(snap: &[u32], o: &Orient) -> Vec<u32> {
    let mut e = snap.to_vec();
    for (off, v) in [(0x170, o.q), (0x180, o.fwd), (0x190, o.side), (0x1a0, o.up)] {
        for k in 0..4 {
            e[off / 4 + k] = v[k].to_bits();
        }
    }
    e[0x1b0 / 4] = o.yaw.to_bits();
    e
}

fn diff(game: &[u32], ours: &[u32]) -> Result<(), String> {
    match (0..game.len()).find(|&i| game[i] != ours[i]) {
        None => Ok(()),
        Some(i) => Err(format!(
            "rider+{:#x}: game {:#010x} ({}) ours {:#010x} ({})",
            i * 4,
            game[i],
            f32::from_bits(game[i]),
            ours[i],
            f32::from_bits(ours[i])
        )),
    }
}

/// Which way `Boarder_UpdateOrientation` took the heading, for the coverage counts.
#[derive(Default, Debug, Clone, Copy)]
struct Heading {
    zero_norm: u32,
    along_y_pos: u32,
    along_y_neg: u32,
    straight_up_down: u32,
    right: u32,
    left_pos: u32,
    left_neg: u32,
}
impl Heading {
    fn add(&mut self, o: &Orient, zero_norm: bool) {
        self.zero_norm += zero_norm as u32;
        let (fx, fy) = (o.fwd[0], o.fwd[1]);
        if fx == 0.0 {
            if fy == 0.0 {
                self.straight_up_down += 1;
            } else if fy > 0.0 {
                self.along_y_pos += 1;
            } else {
                self.along_y_neg += 1;
            }
        } else if fx > 0.0 {
            self.right += 1;
        } else if fy > 0.0 {
            self.left_pos += 1;
        } else {
            self.left_neg += 1;
        }
    }
    fn plus(&mut self, h: Heading) {
        self.zero_norm += h.zero_norm;
        self.along_y_pos += h.along_y_pos;
        self.along_y_neg += h.along_y_neg;
        self.straight_up_down += h.straight_up_down;
        self.right += h.right;
        self.left_pos += h.left_pos;
        self.left_neg += h.left_neg;
    }
}

/// Quaternions the random ones never reach: exact axes, exact halves (heading along ±y, forward straight up
/// or down), the zero quaternion, rotations about one axis.
fn special_quats() -> Vec<V4> {
    let h = 0.5;
    let r = 0.70710677;
    let mut v = vec![
        [1.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 0.0, 0.0],
        [-0.0, -0.0, -0.0, -0.0],
        [r, r, 0.0, 0.0],
        [r, -r, 0.0, 0.0],
        [r, 0.0, r, 0.0],
        [r, 0.0, 0.0, r],
    ];
    for sx in [h, -h] {
        for sy in [h, -h] {
            for sz in [h, -h] {
                v.push([h, sx, sy, sz]);
                v.push([-h, sx, sy, sz]);
            }
        }
    }
    v
}

/// Rotations of 0..2π about the x axis (the forward then has x = 0 exactly), about y and about z.
fn single_axis_quat(g: &mut Rng) -> V4 {
    let a = g.f(-3.2, 3.2) * 0.5;
    let (s, c) = (a.sin(), a.cos());
    match g.next() % 3 {
        0 => [c, s, 0.0, 0.0],
        1 => [c, 0.0, s, 0.0],
        _ => [c, 0.0, 0.0, s],
    }
}

#[derive(Clone, Copy, Debug)]
struct Turn {
    q: V4,
    angle: f32,
    axis: V4,
}

fn gen_turn(g: &mut Rng) -> Turn {
    let specials = special_quats();
    let q = match g.next() % 10 {
        0..=3 => g.unit4(),
        4 | 5 => {
            let k = g.f(0.3, 3.0);
            let u = g.unit4();
            [u[0] * k, u[1] * k, u[2] * k, u[3] * k]
        }
        6 => specials[(g.next() as usize) % specials.len()],
        7 => single_axis_quat(g),
        8 => {
            // nearly the identity, nearly the zero quaternion
            [g.f(0.9, 1.0), g.f(-0.01, 0.01), g.f(-0.01, 0.01), g.f(-0.01, 0.01)]
        }
        _ => g.unit4(),
    };
    let pi = std::f32::consts::PI;
    let angle = match g.next() % 14 {
        0 => 0.0,
        1 => -0.0,
        2 => pi,
        3 => -pi,
        4 => 2.0 * pi,
        5 => 1e-7,
        6 => 0.10471976,
        7 => -0.10471976,
        _ => g.f(-4.0, 4.0),
    };
    let axis = match g.next() % 9 {
        0 => [1.0, 0.0, 0.0, 0.0],
        1 => [0.0, 1.0, 0.0, 0.0],
        2 => [0.0, 0.0, 1.0, 0.0],
        3 => [0.0, 0.0, 0.0, 0.0],
        4 => [g.f(-3.0, 3.0), g.f(-3.0, 3.0), g.f(-3.0, 3.0), g.f(-3.0, 3.0)],
        _ => {
            // a unit vector (as the ground tick's ground normal is), the fourth lane noise: only three are read
            let v = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
            [v[0] / l, v[1] / l, v[2] / l, g.f(-50.0, 50.0)]
        }
    };
    Turn { q, angle, axis }
}

type Rotate = fn(&mut Orient, f32, V4);

/// The original's `Boarder_RotateAboutAxis` against `rot` on one case.
fn compare_turn(w: &mut W, g: &mut Rng, t: &Turn, rot: Rotate) -> Result<Heading, String> {
    w.noise(g);
    w.put4(0x170, t.q);
    for (k, x) in t.axis.iter().enumerate() {
        w.r.mem.write_f32(w.axis + 4 * k as u32, *x);
    }
    let snap = w.block();
    w.r.cpu.set_f(12, t.angle);
    w.r.cpu.set_gpr(4, w.rd as u64);
    w.r.cpu.set_gpr(5, w.axis as u64);
    w.r.call(0x128e90).map_err(|e| format!("the original: {e}"))?;
    let game = w.block();
    let mut o = Orient { q: t.q, ..Default::default() };
    rot(&mut o, t.angle, t.axis);
    diff(&game, &with_orient(&snap, &o))?;
    let mut h = Heading::default();
    h.add(&o, false);
    Ok(h)
}

#[test]
fn rotate_about_axis_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0xe8_0a_0000_0000_0001);
    let mut cover = Heading::default();
    for n in 0..30000 {
        let t = gen_turn(&mut g);
        let h = compare_turn(&mut w, &mut g, &t, rotate_about_axis).unwrap_or_else(|e| panic!("case {n} {t:?}: {e}"));
        cover.plus(h);
    }
    eprintln!("Boarder_RotateAboutAxis: 30000 cases, all bit-identical; {cover:?}");
    assert!(cover.right > 5000 && cover.left_pos > 2000 && cover.left_neg > 2000, "{cover:?}");
    assert!(cover.along_y_pos > 50 && cover.along_y_neg > 20, "{cover:?}");
}

/// `Boarder_UpdateOrientation` on its own, on quaternions that are not unit length (the game normalises them).
#[test]
fn update_orientation_is_the_originals() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0xe8_0b_0000_0000_0001);
    let specials = special_quats();
    let mut cover = Heading::default();
    for n in 0..30000usize {
        let q = match n % 5 {
            0 => specials[(n / 5) % specials.len()],
            1 => single_axis_quat(&mut g),
            2 => {
                let k = g.f(0.2, 4.0);
                let u = g.unit4();
                [u[0] * k, u[1] * k, u[2] * k, u[3] * k]
            }
            _ => g.unit4(),
        };
        w.noise(&mut g);
        w.put4(0x170, q);
        let snap = w.block();
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.call(0x129008).unwrap_or_else(|e| panic!("case {n}: {e}"));
        let game = w.block();
        let mut o = Orient { q, ..Default::default() };
        update_orientation(&mut o);
        diff(&game, &with_orient(&snap, &o)).unwrap_or_else(|e| panic!("case {n} q {q:?}: {e}"));
        cover.add(&o, q.iter().all(|x| *x == 0.0));
    }
    eprintln!("Boarder_UpdateOrientation: 30000 cases, all bit-identical; {cover:?}");
    assert!(cover.zero_norm > 100, "{cover:?}");
    assert!(cover.along_y_pos > 100 && cover.along_y_neg > 100 && cover.straight_up_down > 50, "{cover:?}");
    assert!(cover.right > 5000 && cover.left_pos > 2000 && cover.left_neg > 2000, "{cover:?}");
}

fn flip(x: f32) -> f32 {
    f32::from_bits(x.to_bits() + 1)
}

/// The port with the quaternion's w summed in a different order: the reordering a tidy-minded rewrite makes.
fn rotate_reordered(o: &mut Orient, angle: f32, axis: V4) {
    let (s, c) = tricky_game::math::sin_cos(angle * 0.5);
    let (rx, ry, rz) = (s * axis[0], s * axis[1], s * axis[2]);
    let [qw, qx, qy, qz] = o.q;
    let w = c * qw - (rx * qx + ry * qy + rz * qz);
    let x = ((c * qx + rx * qw) + ry * qz) - rz * qy;
    let y = ((c * qy + ry * qw) + rz * qx) - rx * qz;
    let z = ((c * qz + rz * qw) + rx * qy) - ry * qx;
    o.q = [w, x, y, z];
    update_orientation(o);
}

/// The port with a fused multiply-add in one product (one rounding instead of two).
fn rotate_fused(o: &mut Orient, angle: f32, axis: V4) {
    let (s, c) = tricky_game::math::sin_cos(angle * 0.5);
    let (rx, ry, rz) = (s * axis[0], s * axis[1], s * axis[2]);
    let [qw, qx, qy, qz] = o.q;
    let w = ((c * qw - rx * qx) - ry * qy) - rz * qz;
    let x = (c.mul_add(qx, rx * qw) + ry * qz) - rz * qy;
    let y = ((c * qy + ry * qw) + rz * qx) - rx * qz;
    let z = ((c * qz + rz * qw) + rx * qy) - ry * qx;
    o.q = [w, x, y, z];
    update_orientation(o);
}

#[test]
fn planted_rotation_bugs_are_caught() {
    let Some(mut w) = world() else { return };
    let mut g = Rng(0x0b09_0a00_0000_0001);
    let turns: Vec<Turn> = (0..4000).map(|_| gen_turn(&mut g)).collect();
    // a one-bit error in an output shows in every case; a different order of operations or a fused multiply-add
    // only in the few inputs where the rounding differs, so those get ten times the cases
    let bugs: [(&str, Rotate, usize); 8] = [
        ("q.w", |o, a, x| {
            rotate_about_axis(o, a, x);
            o.q[0] = flip(o.q[0]);
        }, 400),
        ("fwd.y", |o, a, x| {
            rotate_about_axis(o, a, x);
            o.fwd[1] = flip(o.fwd[1]);
        }, 400),
        ("side.z", |o, a, x| {
            rotate_about_axis(o, a, x);
            o.side[2] = flip(o.side[2]);
        }, 400),
        ("up.x", |o, a, x| {
            rotate_about_axis(o, a, x);
            o.up[0] = flip(o.up[0]);
        }, 400),
        ("yaw", |o, a, x| {
            rotate_about_axis(o, a, x);
            o.yaw = flip(o.yaw);
        }, 400),
        ("up's w lane", |o, a, x| {
            rotate_about_axis(o, a, x);
            o.up[3] = 1.0;
        }, 400),
        ("the order of w's sum", rotate_reordered, 4000),
        ("a fused multiply-add", rotate_fused, 4000),
    ];
    for (name, bug, n) in bugs {
        let mut caught = 0;
        let mut h = Rng(7);
        for t in &turns[..n] {
            if compare_turn(&mut w, &mut h, t, bug).is_err() {
                caught += 1;
            }
        }
        eprintln!("planted error ({name}): caught in {caught} of {n} cases");
        assert!(caught >= 20, "planted error ({name}) was caught in only {caught} of {n} cases");
    }
}

// ---- Boarder_TakeOff ----

/// A call `Boarder_TakeOff` makes, with what it passes (and, for the jump, the crouch the rider has at that moment).
#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Motion(u32),
    Jump { min: u32, crouch: u32 },
    Spin,
    State(u32),
}

/// The port's callees, recording as the stubs do. `after_motion`: what the motion call changes the state to.
struct E {
    log: Vec<Ev>,
    after_motion: Option<u32>,
}
impl TakeOffEnv for E {
    fn set_motion(&mut self, r: &mut TakeOffRider, motion: u32) {
        self.log.push(Ev::Motion(motion));
        if let Some(s) = self.after_motion {
            r.state = s;
        }
    }
    fn jump_impulse(&mut self, r: &mut TakeOffRider, min: f32) {
        self.log.push(Ev::Jump { min: min.to_bits(), crouch: r.crouch.to_bits() });
    }
    fn takeoff_spin_rates(&mut self, _: &mut TakeOffRider) {
        self.log.push(Ev::Spin);
    }
    fn set_state(&mut self, _: &mut TakeOffRider, state: u32) {
        self.log.push(Ev::State(state));
    }
}

struct TW {
    r: Runner,
    rd: u32,
    log: Rc<RefCell<Vec<Ev>>>,
    after_motion: Rc<Cell<Option<u32>>>,
}

fn take_off_world() -> Option<TW> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let rd = r.mem.alloc(0x6000);
    let log = Rc::new(RefCell::new(Vec::new()));
    let after_motion = Rc::new(Cell::new(None::<u32>));
    let (l, a) = (log.clone(), after_motion.clone());
    // Boarder_SetMotion 0x11f610
    r.stub(0x11f610, move |cpu, mem| {
        l.borrow_mut().push(Ev::Motion(cpu.gpr(5) as u32));
        if let Some(s) = a.get() {
            mem.write_u32(rd + 0x428, s);
        }
    });
    let l = log.clone();
    // Jump_ApplyImpulse 0x1284e0 (checked on its own, G1a1)
    r.stub(0x1284e0, move |cpu, mem| {
        assert_eq!(cpu.gpr(4) as u32, rd, "the jump's rider");
        l.borrow_mut().push(Ev::Jump { min: cpu.f(12).to_bits(), crouch: mem.read_u32(rd + 0x1c8) });
    });
    let l = log.clone();
    // Takeoff_SetSpinRates 0x126e30 (checked on its own, G1a2)
    r.stub(0x126e30, move |cpu, _| {
        assert_eq!(cpu.gpr(4) as u32, rd, "the spin rates' rider");
        l.borrow_mut().push(Ev::Spin);
    });
    let l = log.clone();
    // Boarder_SetState 0x11f608
    r.stub(0x11f608, move |cpu, _| {
        assert_eq!(cpu.gpr(4) as u32, rd, "the state's rider");
        l.borrow_mut().push(Ev::State(cpu.gpr(5) as u32));
    });
    Some(TW { r, rd, log, after_motion })
}

#[derive(Clone, Copy, Debug)]
struct Lift {
    state: u32,
    crouch: f32,
    x208: f32,
    after_motion: Option<u32>,
}

fn gen_lift(g: &mut Rng) -> Lift {
    let states = [0xa, 0xa, 0xa, 0xd, 3, 1, 0xe, 4, 0x10, 0xb, 5, 0xc];
    let crouch = g.f(0.0, 1.0);
    let x208 = match g.next() % 5 {
        0 => crouch,
        1 => 0.0,
        _ => g.f(0.0, 0.8),
    };
    let after_motion = if g.next() % 3 == 0 { Some(states[(g.next() as usize) % states.len()]) } else { None };
    Lift { state: states[(g.next() as usize) % states.len()], crouch, x208, after_motion }
}

type TakeOff = fn(&mut TakeOffRider, &mut dyn TakeOffEnv);

fn compare_lift(w: &mut TW, g: &mut Rng, l: &Lift, port: TakeOff) -> Result<(bool, bool), String> {
    for i in 0..WORDS {
        let v = g.next() ^ (g.next() << 16);
        w.r.mem.write_u32(w.rd + 4 * i, v);
    }
    w.r.mem.write_u32(w.rd + 0x428, l.state);
    w.r.mem.write_f32(w.rd + 0x1c8, l.crouch);
    w.r.mem.write_f32(w.rd + 0x208, l.x208);
    let snap: Vec<u32> = (0..WORDS).map(|i| w.r.mem.read_u32(w.rd + 4 * i)).collect();
    w.log.borrow_mut().clear();
    w.after_motion.set(l.after_motion);
    w.r.cpu.set_gpr(4, w.rd as u64);
    w.r.call(0x128470).map_err(|e| format!("the original: {e}"))?;
    let game: Vec<u32> = (0..WORDS).map(|i| w.r.mem.read_u32(w.rd + 4 * i)).collect();
    let their_log = w.log.borrow().clone();

    let mut r = TakeOffRider { state: l.state, crouch: l.crouch, x208: l.x208 };
    let mut env = E { log: Vec::new(), after_motion: l.after_motion };
    port(&mut r, &mut env);
    if their_log != env.log {
        return Err(format!("calls differ\n game {their_log:?}\n ours {:?}", env.log));
    }
    let mut want = snap;
    want[0x1c8 / 4] = r.crouch.to_bits();
    want[0x428 / 4] = r.state;
    diff(&game, &want)?;
    let from_jump_control = their_log.iter().any(|e| *e == Ev::Spin);
    Ok((from_jump_control, l.after_motion.is_some_and(|s| (s == 0xa) != (l.state == 0xa))))
}

#[test]
fn take_off_is_the_originals() {
    let Some(mut w) = take_off_world() else { return };
    let mut g = Rng(0xe8_0c_0000_0000_0001);
    let (mut jump_control, mut other, mut flipped) = (0, 0, 0);
    for n in 0..20000 {
        let l = gen_lift(&mut g);
        let (jc, fl) = compare_lift(&mut w, &mut g, &l, take_off).unwrap_or_else(|e| panic!("case {n} {l:?}: {e}"));
        if jc {
            jump_control += 1;
        } else {
            other += 1;
        }
        flipped += fl as u32;
    }
    eprintln!("Boarder_TakeOff: 20000 cases, all identical; from the jump control {jump_control}, from other states {other}, branch flipped by the motion call {flipped}");
    assert!(jump_control > 3000 && other > 3000 && flipped > 500);
}

fn lift_crouch_after_jump(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 1);
    if r.state != 0xa {
        env.jump_impulse(r, 0.0);
        r.crouch = r.x208;
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}
fn lift_state_read_early(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    let s = r.state;
    env.set_motion(r, 1);
    if s != 0xa {
        r.crouch = r.x208;
        env.jump_impulse(r, 0.0);
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}
fn lift_negative_zero_min(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 1);
    if r.state != 0xa {
        r.crouch = r.x208;
        env.jump_impulse(r, -0.0);
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, -0.0);
        env.takeoff_spin_rates(r);
    }
}
fn lift_wrong_state(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 1);
    if r.state != 0xa {
        r.crouch = r.x208;
        env.jump_impulse(r, 0.0);
        env.set_state(r, 0xc);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}
fn lift_wrong_test_state(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 1);
    if r.state != 0xb {
        r.crouch = r.x208;
        env.jump_impulse(r, 0.0);
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}
fn lift_wrong_crouch(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 1);
    if r.state != 0xa {
        r.crouch = f32::from_bits(r.x208.to_bits() + 1);
        env.jump_impulse(r, 0.0);
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}
fn lift_wrong_motion(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 2);
    take_off_rest(r, env);
}
fn take_off_rest(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    if r.state != 0xa {
        r.crouch = r.x208;
        env.jump_impulse(r, 0.0);
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}

#[test]
fn planted_take_off_bugs_are_caught() {
    let Some(mut w) = take_off_world() else { return };
    let mut g = Rng(0x0b09_0c00_0000_0001);
    let lifts: Vec<Lift> = (0..400).map(|_| gen_lift(&mut g)).collect();
    let bugs: [(&str, TakeOff); 6] = [
        ("the crouch written after the jump", lift_crouch_after_jump),
        ("the state read before the motion call", lift_state_read_early),
        ("a negative zero for the smallest jump", lift_negative_zero_min),
        ("state 0xc instead of 0xd", lift_wrong_state),
        ("state 0xb for the jump control", lift_wrong_test_state),
        ("the crouch one bit off", lift_wrong_crouch),
    ];
    for (name, bug) in bugs {
        let mut caught = 0;
        let mut h = Rng(9);
        for l in &lifts {
            if compare_lift(&mut w, &mut h, l, bug).is_err() {
                caught += 1;
            }
        }
        eprintln!("planted error ({name}): caught in {caught} of 400 cases");
        assert!(caught >= 20, "planted error ({name}) was caught in only {caught} of 400 cases");
    }
    let mut caught = 0;
    let mut h = Rng(9);
    for l in &lifts {
        if compare_lift(&mut w, &mut h, l, lift_wrong_motion).is_err() {
            caught += 1;
        }
    }
    assert!(caught >= 200, "motion 2 instead of 1 was caught in only {caught} of 400 cases");
}

/// `Boarder_TurnFromWall` 0x1257a0 against `orient::turn_from_wall`: attitudes from the original's own
/// `Boarder_UpdateOrientation`, wall normals of every kind (along the up, flat, facing up or down, zero).
#[test]
fn turn_from_wall_is_the_originals() {
    let Some(mut w) = world() else { return };
    w.r.allow(0x128e90);
    let mut g = Rng(0xe7_c3_0c00_0000_0001);
    let (mut turned, mut kept) = (0u32, 0u32);
    for n in 0..30000 {
        let mut o = Orient { q: g.unit4(), ..Default::default() };
        tricky_game::orient::update_orientation(&mut o);
        let wall = match g.next() % 8 {
            0 => o.up,
            1 => [0.0; 4],
            2 => {
                let k = g.f(-1.0, 1.0);
                [o.fwd[0] * k, o.fwd[1] * k, o.fwd[2] * k, 0.0]
            }
            _ => {
                let v = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
                [v[0] / l, v[1] / l, v[2] / l, if g.next() % 4 == 0 { g.f(-1.0, 1.0) } else { 0.0 }]
            }
        };
        w.noise(&mut g);
        for (off, v) in [(0x170, o.q), (0x180, o.fwd), (0x190, o.side), (0x1a0, o.up)] {
            w.put4(off, v);
        }
        w.r.mem.write_f32(w.rd + 0x1b0, o.yaw);
        for (k, x) in wall.iter().enumerate() {
            w.r.mem.write_f32(w.axis + 4 * k as u32, *x);
        }
        let snap = w.block();
        w.r.cpu.set_gpr(4, w.rd as u64);
        w.r.cpu.set_gpr(5, w.axis as u64);
        w.r.call(0x1257a0).unwrap_or_else(|e| panic!("case {n}: {e}"));
        let game = w.block();
        let before = o;
        tricky_game::orient::turn_from_wall(&mut o, wall);
        diff(&game, &with_orient(&snap, &o)).unwrap_or_else(|e| panic!("case {n} q {:?} n {wall:?}: {e}", before.q));
        if o == before {
            kept += 1;
        } else {
            turned += 1;
        }
    }
    eprintln!("Boarder_TurnFromWall: 30000 cases, all bit-identical; {turned} turned, {kept} kept");
    assert!(turned > 3000 && kept > 3000, "{turned} {kept}");
}
