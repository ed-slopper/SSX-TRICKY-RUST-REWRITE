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
fn turn_axis(q: V4, a: [f32; 3]) -> V4 {
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
