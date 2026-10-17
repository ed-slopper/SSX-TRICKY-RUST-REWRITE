//! The board's orientation and the take-off exactly as the original does them (board row E8j):
//! `Boarder_RotateAboutAxis` 0x128e90, the `Boarder_UpdateOrientation` 0x129008 it ends in, and
//! `Boarder_TakeOff` 0x128470. In the machine code's order of operations, so the floats equal the game's bit for
//! bit (`tests/orient.rs`, in the function runner).
//!
//! The rider keeps its attitude as a unit quaternion at rider+0x170 (w, x, y, z). `Boarder_UpdateOrientation`
//! turns the board's own axes by it and stores them as vectors (w lane 0): rider+0x190 is the board's x axis
//! turned (its side), rider+0x180 the y axis (its forward) and rider+0x1a0 the z axis (**the board's up**, proven
//! here; `jump::JumpState::up` and `ground_motion::Rider::board_up` are this vector). rider+0x1b0 is the board's
//! heading, the angle of the forward vector in the x-y plane, from `Math_Atan` 0x2510f8.
//!
//! `placed_attitude`, `rotate_vec_to_vec` and `slerp` take `math::asin`, not `math::asin_ee`: `tests/placement.rs`,
//! `tests/orient.rs` and `tests/spin_enter.rs` run the original with every float rule nearest (as this port's floats
//! are) and feed it arguments beyond 0.7071, where the EE's chopped square root no longer matches (tried
//! 2026-10-17).

use crate::math;

type V4 = [f32; 4];

/// π, as the machine code loads it (`lui 0x4049; ori 0xfdb`)
const PI: f32 = f32::from_bits(0x4049_0fdb);
/// π/2 (`0x3fc90fdb`) for a forward vector straight along ±y
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);

/// The attitude fields of the rider (offsets from the rider object).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orient {
    /// +0x170: the quaternion, w then x, y, z
    pub q: V4,
    /// +0x180: the board's forward (its y axis turned by the quaternion; the w lane is 0)
    pub fwd: V4,
    /// +0x190: the board's side (its x axis turned)
    pub side: V4,
    /// +0x1a0: the board's up (its z axis turned)
    pub up: V4,
    /// +0x1b0: the heading, radians
    pub yaw: f32,
}

/// One of the three axis vectors turned by the quaternion, as `Boarder_UpdateOrientation` does it: with
/// t = u × a (u the quaternion's vector part), the result is `a + t·2w + (u × t)·2`, summed left to right, the
/// two scalings being `Vec3_Scale` 0x102f80 (three lanes, the product with the scale). The products with the
/// axis's zeros are kept (they decide the sign of a zero).
pub(crate) fn turn_axis(q: V4, a: [f32; 3]) -> V4 {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    let t = [y * a[2] - z * a[1], z * a[0] - x * a[2], x * a[1] - y * a[0]];
    let u = [y * t[2] - z * t[1], z * t[0] - x * t[2], x * t[1] - y * t[0]];
    let two_w = w + w;
    let tw = [t[0] * two_w, t[1] * two_w, t[2] * two_w];
    let ut = [u[0] * 2.0, u[1] * 2.0, u[2] * 2.0];
    [(a[0] + tw[0]) + ut[0], (a[1] + tw[1]) + ut[1], (a[2] + tw[2]) + ut[2], 0.0]
}

/// `Boarder_UpdateOrientation` 0x129008: normalises the quaternion (a zero one becomes the identity: only w is
/// set, to 1), then derives the board's side, forward and up and the heading from it.
pub fn update_orientation(o: &mut Orient) {
    let [w, x, y, z] = o.q;
    let norm = (((x * x + y * y) + z * z) + w * w).sqrt();
    if norm == 0.0 {
        o.q[0] = 1.0;
    } else {
        let inv = 1.0 / norm;
        o.q = [w * inv, x * inv, y * inv, z * inv];
    }
    o.side = turn_axis(o.q, [1.0, 0.0, 0.0]);
    o.fwd = turn_axis(o.q, [0.0, 1.0, 0.0]);
    o.up = turn_axis(o.q, [0.0, 0.0, 1.0]);
    let (fx, fy) = (o.fwd[0], o.fwd[1]);
    o.yaw = if fx == 0.0 {
        // straight along y (or up or down: then the heading is the forward's own y, a zero)
        if fy == 0.0 {
            fy
        } else if 0.0 <= fy {
            HALF_PI
        } else {
            -HALF_PI
        }
    } else {
        let a = math::atan(fy / fx);
        if fx < 0.0 {
            if 0.0 < fy {
                a + PI
            } else {
                a - PI
            }
        } else {
            a
        }
    };
}

/// `Boarder_RotateAboutAxis` 0x128e90: turns the board by `angle` radians about `axis` (three lanes read): the
/// rotation's quaternion (cos ½θ, sin ½θ · axis) multiplied in front of the rider's, then
/// `Boarder_UpdateOrientation`. The axis is in the same frame as the vectors (world, z up).
pub fn rotate_about_axis(o: &mut Orient, angle: f32, axis: V4) {
    let (s, c) = math::sin_cos(angle * 0.5);
    let rz = s * axis[2];
    let rx = s * axis[0];
    let ry = s * axis[1];
    let [qw, qx, qy, qz] = o.q;
    let w = ((c * qw - rx * qx) - ry * qy) - rz * qz;
    let x = ((c * qx + rx * qw) + ry * qz) - rz * qy;
    let y = ((c * qy + ry * qw) + rz * qx) - rx * qz;
    let z = ((c * qz + rz * qw) + rx * qy) - ry * qx;
    o.q = [w, x, y, z];
    update_orientation(o);
}

/// `Boarder_TurnFromWall` 0x1257a0(rider, n): with a = unit(up × n) and the forward's angle
/// θ = atan2(fwd · a, fwd · (up × a)) (`jump::atan2_inline`), the board turns about its up (+0x1a0) by −f, where
/// f = 1.9198622 − θ held to [0, 0.34906584] when θ > 0, else −1.9198622 − θ held to [−0.34906584, 0]: at most
/// 20° a call, until the forward is 110° from the wall's direction.
pub fn turn_from_wall(o: &mut Orient, n: V4) {
    // vopmula/vopmsub, the w lane cleared; dots as vmul and (x + y) + 1·z + 1·w
    let cross = |a: V4, b: V4| [a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1], 0.0];
    let dot = |a: V4, b: V4| ((a[0] * b[0] + a[1] * b[1]) + 1.0 * (a[2] * b[2])) + 1.0 * (a[3] * b[3]);
    let c = cross(o.up, n);
    let s = dot(c, c);
    let k = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    let a = c.map(|x| x * k);
    let x = dot(o.fwd, cross(o.up, a));
    let y = dot(o.fwd, a);
    let t = crate::jump::atan2_inline(y, x);
    let f = if 0.0 < t {
        let f = 1.919_862_2 - t;
        if 0.0 <= f { f.min(0.349_065_84) } else { 0.0 }
    } else {
        let f = -1.919_862_2 - t;
        if -0.349_065_84 <= f { if 0.0 < f { 0.0 } else { f } } else { -0.349_065_84 }
    };
    if f != 0.0 {
        let axis = o.up;
        rotate_about_axis(o, -f, axis);
    }
}

/// `Boarder_ResetOrientation` 0x128e20: the quaternion set to (1, 0, 0, 0) (`Vec4_Set` 0x10c120), then turned by
/// −π/2 (`0xbfc90fdb`) about z with `Boarder_RotateAboutAxis`: the board lies flat and points along +x (heading 0).
pub fn reset_orientation(o: &mut Orient) {
    o.q = [1.0, 0.0, 0.0, 0.0];
    rotate_about_axis(o, -HALF_PI, [0.0, 0.0, 1.0, 0.0]);
}

/// The board's attitude at a placement (board row G20c3a): `Boarder_PlaceAt` 0x11cae8 from its call of
/// `Boarder_ResetOrientation` to the end of its ray's branch. The board is turned by the heading `heading` (its
/// first argument, radians: the angle of the board's forward from +x) about z; when the placement's ray (`cWorldLine`
/// 0x25d7f0, `World_RayCast` 0x25aff8, 2 cm above to `height` + 200 below the place) hits, with the hit's normal `hit`,
/// the board is turned further by −asin(s) about the axis (hit × board up) / s, s the length of that cross product
/// (the rotation is skipped when s is 1e-5 or less, and s is held to 0.99999 for the asin, but not for the axis).
/// With no hit the heading's turn is all there is. The caller (`Boarder_PlaceAt` with its last argument 0) passes
/// the ray's normal in the same space as the quaternion (z up).
pub fn placed_attitude(heading: f32, hit: Option<V4>) -> Orient {
    let mut o = Orient::default();
    reset_orientation(&mut o);
    rotate_about_axis(&mut o, heading, [0.0, 0.0, 1.0, 0.0]);
    if let Some(n) = hit {
        // vopmula / vopmsub on (normal, up), w cleared
        let c = crate::probe::cross(n, o.up);
        let s = crate::jump::dot(c, c).sqrt();
        if f32::from_bits(0x3727_c5ac) < s {
            // vdiv Q = 1 / s, every lane times Q
            let k = 1.0 / s;
            let axis = [c[0] * k, c[1] * k, c[2] * k, c[3] * k];
            let top = f32::from_bits(0x3f7f_ff58);
            let s = if top < s { top } else { s };
            rotate_about_axis(&mut o, -math::asin(s), axis);
        }
    }
    o
}

/// What `Boarder_TakeOff` reads and writes of the rider itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TakeOffRider {
    /// the control state, rider+0x428 (0xa is the jump control, `cJumpControl`)
    pub state: u32,
    /// rider+0x1c8: the crouch the jump's strength is taken from (`jump::JumpState::crouch`)
    pub crouch: f32,
    /// rider+0x208: the crouch amount taken off it (`jump::JumpState::x208`)
    pub x208: f32,
}

/// What `Boarder_TakeOff` calls. Every method is one call of the original, at the point it makes it; they may
/// change the rider (the game's callees do).
pub trait TakeOffEnv {
    /// `Boarder_SetMotion` 0x11f610(rider, motion)
    fn set_motion(&mut self, r: &mut TakeOffRider, motion: u32);
    /// `Jump_ApplyImpulse` 0x1284e0(rider, min): `jump::jump_impulse` with the smallest jump `min` (cm/s)
    fn jump_impulse(&mut self, r: &mut TakeOffRider, min: f32);
    /// `Takeoff_SetSpinRates` 0x126e30(rider): `jump::takeoff_spin_rates` (a tail call: the function ends there)
    fn takeoff_spin_rates(&mut self, r: &mut TakeOffRider);
    /// `Boarder_SetState` 0x11f608(rider, state) (a tail call too)
    fn set_state(&mut self, r: &mut TakeOffRider, state: u32);
}

/// `Boarder_TakeOff` 0x128470: the rider leaves the snow. Air motion first; the state is read after that call
/// (it may have changed it). From the jump control (state 0xa) the jump's own strength is used and the spin and
/// flip rates are set; from any other state the crouch is first made equal to rider+0x208 (so the jump is the
/// smallest possible, 0 over its crouch) and the air state 0xd is entered.
pub fn take_off(r: &mut TakeOffRider, env: &mut dyn TakeOffEnv) {
    env.set_motion(r, 1);
    if r.state != 0xa {
        r.crouch = r.x208;
        env.jump_impulse(r, 0.0);
        env.set_state(r, 0xd);
    } else {
        env.jump_impulse(r, 0.0);
        env.takeoff_spin_rates(r);
    }
}

/// `Quat_Multiply` 0x102e80: p · q (w, x, y, z), each lane summed left to right as the machine code does.
pub fn quat_multiply(p: V4, q: V4) -> V4 {
    let [p0, p1, p2, p3] = p;
    let [q0, q1, q2, q3] = q;
    [
        ((p0 * q0 - p1 * q1) - p2 * q2) - p3 * q3,
        ((p0 * q1 + p1 * q0) + p2 * q3) - p3 * q2,
        ((p0 * q2 + p2 * q0) + p3 * q1) - p1 * q3,
        ((p0 * q3 + p3 * q0) + p1 * q2) - p2 * q1,
    ]
}

/// `Quat_RotateVecToVec` 0x2508b8: `q` turned further by the rotation taking `a` to `b` (both unit length, four
/// lanes): the axis a × b (vopmula / vopmsub) and its length s (vsqrt; 0: `q` as it was), the axis divided by s
/// (vdiv, vmulq), the angle asin(min(s, 0.99999)), or π less it when a · b < 0, then (cos ½θ, sin ½θ · axis) · q
/// (`Math_SinCos`, `Quat_Multiply`).
pub fn rotate_vec_to_vec(q: V4, a: V4, b: V4) -> V4 {
    let c = crate::probe::cross(a, b);
    let s = crate::jump::dot(c, c).sqrt();
    if s == 0.0 {
        return q;
    }
    let k = 1.0 / s;
    let axis = [c[0] * k, c[1] * k, c[2] * k, c[3] * k];
    let s = if 0.99999 < s { 0.99999 } else { s };
    let angle = if crate::jump::dot(a, b) < 0.0 { PI - math::asin(s) } else { math::asin(s) };
    let (sin, cos) = math::sin_cos(angle * 0.5);
    quat_multiply([cos, sin * axis[0], sin * axis[1], sin * axis[2]], q)
}

/// `Quat_Slerp` 0x250a48: from `a` to `b` by `t` along the shorter way (`b` negated when a · b < 0, the dot summed
/// x, y, z then w); the angle π/2 − asin(dot) and its sines (`Math_Asin`, `Math_Sin`), or straight 1 − t and t
/// when 1 − dot is 0.001 or less.
pub fn slerp(t: f32, a: V4, b: V4) -> V4 {
    let mut d = ((a[1] * b[1] + a[2] * b[2]) + a[3] * b[3]) + a[0] * b[0];
    let mut b = b;
    if d < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        d = -d;
    }
    let (wa, wb) = if 0.001 < 1.0 - d {
        let angle = HALF_PI - math::asin(d);
        let s = math::sin(angle);
        (math::sin((1.0 - t) * angle) / s, math::sin(t * angle) / s)
    } else {
        (1.0 - t, t)
    };
    [wa * a[0] + wb * b[0], wa * a[1] + wb * b[1], wa * a[2] + wb * b[2], wa * a[3] + wb * b[3]]
}

/// `Mat4_ToPosQuat` 0x2504f0: a matrix (rows) to its position (row 3, w kept) and rotation (w, x, y, z). With a
/// positive trace t (m00 + m11 + m22) s = √(t + 1), w = s / 2 and the rest (m12 − m21, m20 − m02, m01 − m10) · ½/s;
/// else from the largest diagonal entry i (then j, k after it): s = √(m_ii − m_jj − m_kk + 1), q_i = s / 2, and
/// with f = ½/s (0 when s is 0) w = (m_jk − m_kj) f, q_j = (m_ij + m_ji) f, q_k = (m_ik + m_ki) f.
pub fn mat4_to_pos_quat(m: [V4; 4]) -> (V4, V4) {
    let e = |i: usize, j: usize| m[i][j];
    let t = (e(0, 0) + e(1, 1)) + e(2, 2);
    if 0.0 < t {
        let s = (t + 1.0).sqrt();
        let f = 0.5 / s;
        return (m[3], [s * 0.5, (e(1, 2) - e(2, 1)) * f, (e(2, 0) - e(0, 2)) * f, (e(0, 1) - e(1, 0)) * f]);
    }
    let mut i = (e(0, 0) < e(1, 1)) as usize;
    if e(i, i) < e(2, 2) {
        i = 2;
    }
    let j = (i + 1) % 3;
    let k = (j + 1) % 3;
    let s = ((e(i, i) - e(j, j)) - e(k, k) + 1.0).sqrt();
    let mut q = [0.0f32; 4];
    q[i] = s * 0.5;
    let f = if s != 0.0 { 0.5 / s } else { s };
    q[3] = (e(j, k) - e(k, j)) * f;
    q[j] = (e(i, j) + e(j, i)) * f;
    q[k] = (e(i, k) + e(k, i)) * f;
    (m[3], [q[3], q[0], q[1], q[2]])
}
