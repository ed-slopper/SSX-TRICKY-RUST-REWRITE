//! The game's cameras (board rows E11a–c): `CamCamera_EvalPosition` 0x16e900 and `CamCamera_EvalLookAt` 0x1722e8.
//! Checked in the function runner (`tests/camera.rs`).

use crate::math::{angle_diff, asin, atan, sin_cos};

const PI: f32 = 3.141_592_7;
const HALF_PI: f32 = 1.570_796_4;
const TAU: f32 = 6.283_185_5;

/// The camera's parameter block (`*(cam + 0x11c)`), fields named by their byte offset.
#[derive(Clone, Debug, Default)]
pub struct CamP {
    /// +0x00: the camera follows a target (list entry `w04`).
    pub w00: i32,
    pub w04: i32,
    /// +0x10 / +0x14: ease the angles over `f14` ticks.
    pub w10: i32,
    pub f14: f32,
    /// +0x18 / +0x1c and +0x20 / +0x24: two optional words handed on (as `out.w38/3c` and `out.w40/44`).
    pub w18: i32,
    pub w1c: u32,
    pub w20: i32,
    pub w24: u32,
    pub w28: i32,
    /// +0x2c: which of the target's matrices (-1: the rider's own vector, -2: the other matrix).
    pub w2c: i32,
    /// +0x30: the stage area (zero: none).
    pub w30: i32,
    /// +0x3c, +0x68, +0x94: the base position.
    pub base: [f32; 3],
    /// +0xb8 shake amount (counts down 1/60 a call), +0xc4 shake scale.
    pub f_b8: f32,
    pub f_c4: f32,
    /// +0xc8 orbit enable, +0xcc distance, +0xd0 distance per `rider +0x130`.
    pub w_c8: i32,
    pub f_cc: f32,
    pub f_d0: f32,
    pub w_d4: i32,
    pub w_d8: i32,
    /// +0xdc: heading start.
    pub f_dc: f32,
    pub w_e0: i32,
    pub w_f8: i32,
    pub w_fc: i32,
    /// +0x100: pitch start.
    pub f_100: f32,
    pub w_108: i32,
    pub f_10c: f32,
    pub f_110: f32,
    pub f_114: f32,
    pub f_118: f32,
}

/// What a target rider holds that the camera reads.
#[derive(Clone, Debug)]
pub struct TargetRider {
    /// +0x130.
    pub f130: f32,
    /// `*(rider + 0x464) + 0x40` is not zero.
    pub flag40: bool,
    /// +0x2d0.
    pub v2d0: [f32; 3],
    /// +0x4a90.
    pub v4a90: [f32; 3],
    /// The translation of matrix 0 of the table at +0x4498.
    pub t4498: [f32; 3],
}

/// One entry of the camera's target list (0x20 bytes).
#[derive(Clone, Debug)]
pub struct Entry {
    pub rider: Option<TargetRider>,
    /// +4.
    pub w4: f32,
    /// +8, +0xc, +0x10: the target's direction.
    pub v8: [f32; 3],
    /// +0x14, +0x18, +0x1c: the target's position offset.
    pub p14: [f32; 3],
}

/// The camera output record.
#[derive(Clone, Debug, Default)]
pub struct CamOut {
    pub pos: [f32; 3],
    /// +0x24..+0x2c.
    pub v24: [f32; 3],
    pub w30: u32,
    pub w38: u32,
    pub w3c: u32,
    pub w40: u32,
    pub w44: u32,
    /// +0x4c (heading) and +0x50 (pitch), the eased angles.
    pub yaw: f32,
    pub pitch: f32,
}

/// The shake table's two read positions (words 0x335718 and 0x33571c).
#[derive(Clone, Copy, Debug)]
pub struct Shake {
    pub i0: i32,
    pub i1: i32,
}

pub struct StageOut {
    pub ret: bool,
    pub v: [f32; 4],
    /// The stage yaw as the caller's slot holds it after the call.
    pub yaw: f32,
}

/// The calls the position makes.
pub trait CamEnv {
    /// `Cam_StageAreaToWorld` 0x1777d0(area, point (w = 1), yaw slot).
    fn stage_area_to_world(&mut self, area: i32, v: [f32; 4]) -> StageOut;
    /// Row 3 (the translation) of the target's matrix `idx` (the table at rider +0x4494, 0x40 bytes a matrix),
    /// all four rows: `[row][lane]`.
    fn matrix(&mut self, idx: i32) -> [[f32; 4]; 4];
    /// `Camera_CollideWorld` 0x16fe60(radius, position (in/out), target point).
    fn collide_world(&mut self, radius: f32, pos: &mut [f32; 3], target: [f32; 3]) -> u64;
}

pub struct PosResult {
    pub ret: u64,
    /// The stage yaw (stored to the caller's pointer when it gave one).
    pub stage_yaw: f32,
}

/// The matrix at 0x346410 (`StaticInit_Spline2`): the identity.
pub const IDENT3: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

/// The axis-angle matrix the camera builds (sin and cos of `angle` from `Math_SinCos`), nine floats row by row.
pub(crate) fn rot9(ax: f32, ay: f32, az: f32, angle: f32) -> [f32; 9] {
    let (s, c) = sin_cos(angle);
    let t = 1.0 - c;
    let f1 = t * az;
    let f3 = t * ax;
    let f20 = t * ay;
    let f11 = s * az;
    let f12 = s * ax;
    let f8 = f1 * az;
    let f9 = f3 * az;
    let f5 = f1 * ax;
    let f7 = f3 * ax;
    let f2 = f20 * az;
    let f0 = f20 * ax;
    let f6 = s * ay;
    let f1 = f1 * ay;
    let f3 = f3 * ay;
    let f20 = f20 * ay;
    let f5 = f5 + f6;
    let f0 = f0 - f11;
    let f1 = f1 - f12;
    let f8 = f8 + c;
    let f7 = f7 + c;
    let f3 = f3 + f11;
    let f9 = f9 - f6;
    let f20 = f20 + c;
    let f2 = f2 + f12;
    [f7, f0, f5, f3, f20, f1, f9, f2, f8]
}

/// `r` times the constant matrix `b`, the vector through the product (the order the original adds in).
fn rot_apply(a: [f32; 3], r: [f32; 9], b: [f32; 9]) -> [f32; 3] {
    let m =
        |i: usize, j: usize| (b[j] * r[i * 3] + b[3 + j] * r[i * 3 + 1]) + b[6 + j] * r[i * 3 + 2];
    [
        (m(0, 0) * a[0] + m(1, 0) * a[1]) + m(2, 0) * a[2],
        (m(0, 1) * a[0] + m(1, 1) * a[1]) + m(2, 1) * a[2],
        (m(0, 2) * a[0] + m(1, 2) * a[1]) + m(2, 2) * a[2],
    ]
}

/// A direction through the vector unit: `m` rows weighted by (0, 100, 0, 0): vmulax, vmadday, vmaddaz, vmaddw.
fn up100(m: [[f32; 4]; 4]) -> [f32; 3] {
    let lane = |l: usize| ((m[0][l] * 0.0 + m[1][l] * 100.0) + m[2][l] * 0.0) + m[3][l] * 0.0;
    [lane(0), lane(1), lane(2)]
}

/// The angle of a vector (z against its flat length) as the camera takes it: atan(z / flat) with the flat length
/// zero going to ±π/2.
fn lift_angle(flat: f32, z: f32) -> f32 {
    if flat == 0.0 {
        if z == 0.0 {
            return z;
        }
        return if 0.0 <= z { HALF_PI } else { -HALF_PI };
    }
    let a = atan(z / flat);
    if flat < 0.0 {
        return if 0.0 < z { a + PI } else { a - PI };
    }
    a
}

/// `CamCamera_EvalPosition` 0x16e900(cam, out, list, cut, stage out): `p` is the cam's block, `entries` the target
/// list. Returns what `Camera_CollideWorld` returned (0 when it did not run) and the stage yaw.
pub fn eval_position(
    p: &mut CamP,
    entries: &[Entry],
    out: &mut CamOut,
    cut: bool,
    env: &mut dyn CamEnv,
    shake: &mut Shake,
    shake_table: &[f32],
) -> PosResult {
    let mut acc = [0.0f32; 3];
    let mut base = p.base;
    let mut stage_yaw = 0.0f32;
    if p.w30 != 0 {
        let r = env.stage_area_to_world(p.w30, [base[0], base[1], base[2], 1.0]);
        stage_yaw = r.yaw;
        if r.ret {
            base = [r.v[0], r.v[1], r.v[2]];
        }
    }
    out.w30 = 0;
    let e = |p: &CamP| entries.get(p.w04 as usize);
    if p.w_c8 != 0 {
        let mut s2 = false;
        let mut d = p.f_cc;
        if p.w00 != 0 {
            if let Some(r) = e(p).and_then(|x| x.rider.as_ref()) {
                d += p.f_d0 * r.f130;
            }
        }
        acc = [acc[0] + 0.0, acc[1] + (-d), acc[2] + 0.0];

        // the pitch
        let mut pitch = p.f_100;
        if p.w_f8 != 0 && p.w00 != 0 {
            if let Some(en) = e(p) {
                if en.rider.is_some() {
                    out.w30 = 1;
                    out.v24 = en.v8;
                    let flat = (en.v8[0] * en.v8[0] + en.v8[1] * en.v8[1]).sqrt();
                    let z = en.v8[2];
                    let a = if z == 0.0 {
                        if flat == 0.0 {
                            flat
                        } else if 0.0 <= flat {
                            HALF_PI
                        } else {
                            -HALF_PI
                        }
                    } else {
                        let mut a = atan(flat / z);
                        if z < 0.0 {
                            a = if 0.0 < flat { a + PI } else { a - PI };
                        }
                        a
                    };
                    let div = if z < 0.0 { p.f_114 } else { p.f_118 };
                    pitch += (HALF_PI - a) / div;
                    if p.w_108 != 0 {
                        if pitch < p.f_10c {
                            pitch = p.f_10c;
                        } else if p.f_110 < pitch {
                            pitch = p.f_110;
                        }
                    }
                    s2 = true;
                }
            }
        }
        if p.w_fc != 0 && p.w00 != 0 && p.w2c != -1 && p.w2c != -2 {
            if e(p).map_or(false, |x| x.rider.is_some()) {
                let m = env.matrix(p.w2c);
                let v = up100(m);
                let flat = (v[0] * v[0] + v[1] * v[1]).sqrt();
                if flat == 0.0 {
                    if v[2] == 0.0 {
                        pitch += v[2];
                    } else if 0.0 <= v[2] {
                        pitch += HALF_PI;
                    } else {
                        pitch += -HALF_PI;
                    }
                } else {
                    pitch += lift_angle(flat, v[2]);
                }
                s2 = true;
            }
        }
        // ease and output the pitch
        let neg;
        if !s2 || cut || p.w10 == 0 || p.f14 == 0.0 {
            neg = -pitch;
        } else {
            let old = out.pitch;
            pitch = old + (pitch - old) / p.f14;
            neg = -pitch;
        }
        out.pitch = pitch;
        s2 = false;
        acc = rot_apply(acc, rot9(1.0, 0.0, 0.0, neg), IDENT3);

        // the heading
        let mut yaw = p.f_dc;
        if p.w_e0 != 0 {
            let r = e(p).and_then(|x| x.rider.as_ref()).expect("target rider");
            if r.flag40 {
                yaw = angle_diff(PI, yaw) + PI;
            }
        }
        if p.w_d4 != 0 && p.w00 != 0 {
            if let Some(en) = e(p).filter(|x| x.rider.is_some()) {
                let (x8, xc) = (en.v8[0], en.v8[1]);
                let f0 = if x8 == 0.0 && xc == 0.0 {
                    en.w4
                } else if xc == 0.0 {
                    if 0.0 <= x8 {
                        -HALF_PI
                    } else {
                        HALF_PI
                    }
                } else {
                    let mut a = atan(x8 / xc);
                    if xc < 0.0 {
                        a = if 0.0 < x8 { a + PI } else { a - PI };
                    }
                    -a
                };
                yaw += f0;
                s2 = true;
                if yaw < 0.0 {
                    yaw += TAU;
                }
            }
        }
        if p.w_d8 != 0 && p.w00 != 0 {
            if let Some(en) = e(p).filter(|x| x.rider.is_some()) {
                let _ = en;
                if p.w2c != -1 && p.w2c != -2 {
                    let m = env.matrix(p.w2c);
                    let v = up100(m);
                    let (x, y) = (v[0], v[1]);
                    if y == 0.0 {
                        if x == 0.0 {
                            yaw -= x;
                        } else if 0.0 <= x {
                            yaw -= HALF_PI;
                        } else {
                            yaw -= -HALF_PI;
                        }
                    } else {
                        let mut a = atan(x / y);
                        if y < 0.0 {
                            a = if 0.0 < x { a + PI } else { a - PI };
                        }
                        yaw -= a;
                    }
                    s2 = true;
                }
            }
        }
        if p.w30 != 0 && p.w_d4 == 0 && p.w_d8 == 0 {
            yaw = angle_diff(yaw + stage_yaw, 0.0);
        }
        if s2 && !cut && p.w10 != 0 && p.f14 != 0.0 {
            let old = out.yaw;
            yaw = old + angle_diff(yaw, old) / p.f14;
        }
        out.yaw = yaw;
        acc = rot_apply(acc, rot9(0.0, 0.0, 1.0, -yaw), IDENT3);
    }
    acc = [acc[0] + base[0], acc[1] + base[1], acc[2] + base[2]];
    let mut w = [0.0f32; 3];
    if p.w00 != 0 {
        let en = &entries[p.w04 as usize];
        let v = if p.w2c == -2 {
            let r = en.rider.as_ref().expect("target rider");
            [r.v2d0[0] + 0.0, r.v2d0[1] + 0.0, r.v2d0[2] + 0.0]
        } else {
            [en.p14[0] + 0.0, en.p14[1] + 0.0, en.p14[2] + 0.0]
        };
        w = v;
        acc = [acc[0] + v[0], acc[1] + v[1], acc[2] + v[2]];
        if p.w28 != 0 {
            if let Some(r) = en.rider.as_ref() {
                if p.w2c == -1 {
                    let q = r.v4a90;
                    let dz = [q[0] - en.p14[0], q[1] - en.p14[1], q[2] - en.p14[2]];
                    acc = [acc[0] + dz[0], acc[1] + dz[1], acc[2] + dz[2]];
                } else {
                    let q = if p.w2c == -2 {
                        r.t4498
                    } else {
                        let m = env.matrix(p.w2c);
                        [m[3][0], m[3][1], m[3][2]]
                    };
                    acc = [acc[0] + q[0], acc[1] + q[1], acc[2] + q[2]];
                    acc = [acc[0] - en.p14[0], acc[1] - en.p14[1], acc[2] - en.p14[2]];
                }
            }
        }
    }
    if p.w18 != 0 {
        out.w38 = 1;
        out.w3c = p.w1c;
    } else {
        out.w38 = 0;
    }
    if p.w20 != 0 {
        out.w40 = 1;
        out.w44 = p.w24;
    } else {
        out.w40 = 0;
    }
    let mut ret = 0u64;
    if p.w_c8 != 0 {
        ret = env.collide_world(p.f_cc, &mut acc, w);
    }
    out.pos = acc;
    if 0.0 < p.f_b8 {
        let (a1, t0) = (shake.i0, shake.i1);
        let dx = shake_table[a1 as usize] * p.f_c4;
        let dz = shake_table[t0 as usize] * p.f_c4;
        out.pos = [acc[0] + dx, acc[1] + 0.0, acc[2] + dz];
        shake.i0 = if a1 + 3 < 0xbd { a1 + 3 } else { 0 };
        shake.i1 = if t0 + 3 < 0xbd { t0 + 3 } else { 0 };
        p.f_b8 -= 0.016_666_668;
    }
    PosResult { ret, stage_yaw }
}

// ---- CamCamera_EvalLookAt 0x1722e8 ----

/// The look-at block (`*(cam + 0x44)`), fields named by byte offset.
#[derive(Clone, Debug, Default)]
pub struct LookP {
    /// +0x00: a target (list entry `w04`) is followed.
    pub w00: i32,
    pub w04: i32,
    /// +0x08, +0x0c, +0x10: the look-at point's offset.
    pub off: [f32; 3],
    /// +0x14, +0x18, +0x1c: heading, pitch and roll added.
    pub f14: f32,
    pub f18: f32,
    pub f1c: f32,
    /// +0x20: the field of view; +0x24: its floor 1.0 applies to `out.f48`.
    pub f20: f32,
    pub f24: f32,
    /// +0x28: the roll's divisor when `p7 == 0`.
    pub f28: f32,
    pub w2c: i32,
    pub w30: i32,
    /// +0x34: which matrix (-1, -2: the rider's own).
    pub w34: i32,
    pub w38: i32,
    pub f3c: f32,
    /// +0x40: look back (pitch negated, heading + π).
    pub w40: i32,
}

/// What the look-at reads from a rider.
#[derive(Clone, Debug)]
pub struct LookRider {
    /// +0x424.
    pub motion: i32,
    /// +0x57b8.
    pub f57b8: f32,
    /// +0x57d0.
    pub v57d0: [f32; 3],
    /// +0x57e0 (four lanes).
    pub v57e0: [f32; 4],
    /// +0x290.
    pub w290: i32,
    /// +0x2a0.
    pub v2a0: [f32; 3],
    /// +0x2d0.
    pub v2d0: [f32; 3],
    /// `*(rider + 0x464) + 0x60`.
    pub f464_60: f32,
    /// +0x4a90.
    pub v4a90: [f32; 3],
    /// The translation of matrix 0 of the table at +0x4498.
    pub t4498: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct LookEntry {
    pub rider: Option<LookRider>,
    /// +0x14, +0x18, +0x1c.
    pub p14: [f32; 3],
}

/// The view record: the camera position in, the angles out.
#[derive(Clone, Debug, Default)]
pub struct LookView {
    pub pos: [f32; 3],
    /// +0x0c heading, +0x10 pitch, +0x14 roll, +0x18 field of view, +0x48.
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fov: f32,
    pub f48: f32,
}

/// The last flat direction to the target (words 0x335724 and 0x335728), kept while the target is on top of the
/// camera.
#[derive(Clone, Copy, Debug)]
pub struct LastDir {
    pub x: f32,
    pub y: f32,
}

pub trait LookEnv {
    /// Row 3 (xyz) of the target's matrix `idx` (rider +0x4494 table).
    fn translation(&mut self, idx: i32) -> [f32; 3];
}

/// The R5900's divide: a zero divisor gives the largest float of the quotient's sign (0/0 included).
fn ps2_div(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        let neg = a.is_sign_negative() != b.is_sign_negative();
        return if neg { -f32::MAX } else { f32::MAX };
    }
    a / b
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `Vec3_Div` after a zero test: the vector over its length, or itself when the length is zero.
fn unit_or_same(v: [f32; 3]) -> [f32; 3] {
    let len = dot3(v, v).sqrt();
    if len == 0.0 {
        return v;
    }
    let inv = 1.0 / len;
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

/// A direction normalised by the vector unit (the sum of four squares, `vrsqrt`, `vmulq`).
fn vu_unit(v: [f32; 4]) -> [f32; 3] {
    let m = [v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]];
    let s = ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3];
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    [v[0] * q, v[1] * q, v[2] * q]
}

/// The flat angle of (x, y) as the look-at takes it: atan(x / y) with y zero going to ±π/2.
fn flat_angle(x: f32, y: f32) -> f32 {
    if y == 0.0 {
        if x == 0.0 {
            return x;
        }
        return if 0.0 <= x { HALF_PI } else { -HALF_PI };
    }
    let a = atan(x / y);
    if y < 0.0 {
        return if 0.0 < x { a + PI } else { a - PI };
    }
    a
}

/// `CamCamera_EvalLookAt` 0x1722e8(extra, cam, view, list, —, timer, p7, p8): the heading, pitch, roll and field
/// of view of the view from `view.pos` to the target.
#[allow(clippy::too_many_arguments)]
pub fn eval_look_at(
    p: &LookP,
    entries: &[LookEntry],
    v: &mut LookView,
    last: &mut LastDir,
    timer: &mut f32,
    p7: i64,
    p8: i64,
    extra: f32,
    env: &mut dyn LookEnv,
) {
    let from = v.pos;
    let mut fov_t: Option<f32> = None;
    let mut t = [0.0f32 + p.off[0], 0.0f32 + p.off[1], 0.0f32 + p.off[2]];
    if p.w00 == 0 {
        v.yaw = p.f14;
        v.pitch = p.f18;
        v.roll = p.f1c;
    } else {
        let en = &entries[p.w04 as usize];
        let add = if p.w34 == -2 {
            en.rider.as_ref().expect("target rider").v2d0
        } else {
            en.p14
        };
        t = [t[0] + add[0], t[1] + add[1], t[2] + add[2]];
        if p.w30 != 0 {
            if let Some(r) = en.rider.as_ref() {
                match p.w34 {
                    -1 => {
                        let q = r.t4498;
                        let d = [q[0] - en.p14[0], q[1] - en.p14[1], q[2] - en.p14[2]];
                        t = [t[0] + d[0], t[1] + d[1], t[2] + d[2]];
                    }
                    -2 => {
                        let q = r.v4a90;
                        let k = r.f464_60 * 195.0 * -0.4;
                        let z = r.v2d0[2] - k;
                        t = [t[0] + q[0], t[1] + q[1], t[2] + z];
                        t = [t[0] - r.v2d0[0], t[1] - r.v2d0[1], t[2] - r.v2d0[2]];
                    }
                    idx => {
                        let q = env.translation(idx);
                        t = [
                            (t[0] + q[0]) - en.p14[0],
                            (t[1] + q[1]) - en.p14[1],
                            (t[2] + q[2]) - en.p14[2],
                        ];
                    }
                }
            }
        }
        let dx = t[0] - from[0];
        let dy = t[1] - from[1];
        let mut flat = (dx * dx + dy * dy).sqrt();
        if flat < 0.01 {
            flat = (last.x * last.x + last.y * last.y).sqrt();
        } else {
            last.x = dx;
            last.y = dy;
        }
        let h = flat_angle(last.x, last.y);
        v.yaw = (HALF_PI - h) + p.f14;
        if let Some(r) = en.rider.as_ref() {
            if r.motion == 1 && r.f57b8 < 0.033_333_335 {
                let n = vu_unit(r.v57e0);
                let u = [-r.v57d0[0], -r.v57d0[1], -r.v57d0[2]];
                let dot = (u[0] * n[0] + u[1] * n[1]) + u[2] * n[2];
                if 1000.0 < dot {
                    *timer = 0.0;
                }
            } else if p8 == 1 && r.motion == 2 && *timer < 0.083_333_336 {
                let n = vu_unit(r.v57e0);
                let u = [-r.v57d0[0], -r.v57d0[1], -r.v57d0[2]];
                let dot = (u[0] * n[0] + u[1] * n[1]) + u[2] * n[2];
                let mut f = (dot - 1000.0) / 3000.0;
                if 1.0 < f {
                    f = 1.0;
                }
                let s = r.w290;
                let k = if s == 4 {
                    125.0
                } else if s == 3 {
                    166.0
                } else if s < 5 || s >= 15 || s < 12 {
                    200.0
                } else {
                    250.0
                };
                t[2] -= k * f;
            }
        }
        let dz = t[2] - from[2];
        let pitch = if flat == 0.0 {
            if dz == 0.0 {
                dz
            } else if 0.0 <= dz {
                HALF_PI
            } else {
                -HALF_PI
            }
        } else {
            atan(dz / flat)
        };
        v.roll = 0.0;
        v.pitch = pitch + p.f18;
        if let Some(r) = en.rider.as_ref() {
            if (p.w2c != 0 || p7 == 1) && !matches!(r.motion, 1 | 3 | 5 | 6) {
                let up = r.v2a0;
                let d = [t[0] - from[0], t[1] - from[1], t[2] - from[2]];
                let nd = unit_or_same(d);
                let k1 = (up[0] * nd[0] + up[1] * nd[1]) + up[2] * nd[2];
                let s1 = [nd[0] * k1, nd[1] * k1, nd[2] * k1];
                let w = [up[0] - s1[0], up[1] - s1[1], up[2] - s1[2]];
                let wn = unit_or_same(w);
                let k2 = (0.0 * nd[0] + 0.0 * nd[1]) + 1.0 * nd[2];
                let s2 = [nd[0] * k2, nd[1] * k2, nd[2] * k2];
                let vv = [0.0 - s2[0], 0.0 - s2[1], 1.0 - s2[2]];
                let vn = unit_or_same(vv);
                let mut dv = (vn[0] * wn[0] + vn[1] * wn[1]) + vn[2] * wn[2];
                if dv < -1.0 || 1.0 < dv {
                    dv = 1.0;
                }
                let a = asin(dv);
                let c = [
                    nd[1] * vn[2] - nd[2] * vn[1],
                    nd[2] * vn[0] - nd[0] * vn[2],
                    nd[0] * vn[1] - nd[1] * vn[0],
                ];
                let sg = (c[0] * wn[0] + c[1] * wn[1]) + c[2] * wn[2];
                let mut ang = HALF_PI - a;
                if sg < 0.0 {
                    ang = -ang;
                }
                let div = if p7 != 0 { 1.0 } else { p.f28 };
                v.roll = ps2_div(ang, div);
            }
        }
        v.roll += p.f1c;
        if p.w38 != 0 {
            let d = [t[0] - from[0], t[1] - from[1], t[2] - from[2]];
            let len = dot3(d, d).sqrt();
            let mut f = ps2_div(p.f3c, len * p.f20);
            if f < 0.25 {
                f = 0.25;
            }
            if 2.0 < f {
                f = 2.0;
            }
            fov_t = Some(f);
        }
    }
    if p.w40 != 0 {
        v.pitch = -v.pitch;
        v.yaw += PI;
    }
    v.fov = fov_t.unwrap_or(p.f20);
    v.yaw += extra;
    v.f48 = p.f24;
    if p.f24 < 1.0 {
        v.f48 = 1.0;
    }
}

// ---- Camera_CollideWorld 0x16fe60 ----

/// The ray test's result: the hit point and the surface normal (four lanes each; the original reads the
/// fourth lane of the sum and difference vectors).
#[derive(Clone, Copy, Debug, Default)]
pub struct RayRes {
    pub hit: [f32; 4],
    pub normal: [f32; 4],
}

/// A world line, as the original builds it on the stack: its constructor's object (`main`) and the words the
/// copy into the call's own buffer skips (`holes`, left from the first line built there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub main: u32,
    pub holes: u32,
}

/// The world queries `Camera_CollideWorld` makes (all on the world at 0x346488).
pub trait WorldEnv {
    /// `cWorldLine::cWorldLine` 0x25d7f0 with a zero first float; returns the object's identity.
    fn line(&mut self, a: [f32; 4], b: [f32; 4], mode: i32, f12: f32) -> u32;
    /// `World_RayCast` 0x25aff8.
    fn ray_cast(&mut self, line: Line, res: &mut RayRes) -> f32;
    /// `World_IsBlocked` 0x25c690.
    fn is_blocked(&mut self, line: Line, res: &mut RayRes) -> bool;
    /// `Camera_CollidePush` 0x16fb08(k, pos, target).
    fn push(&mut self, k: f32, pos: &mut [f32; 3], target: [f32; 3]);
}

/// `Camera_CollideWorld` 0x16fe60(radius, pos, target): keeps the camera at `pos` out of the world when it moves
/// along `target`; returns 1 when the position was moved off a wall. `counter` is the call count at 0x335720.
pub fn collide_world(
    radius: f32,
    pos: &mut [f32; 3],
    target: [f32; 3],
    counter: &mut u32,
    env: &mut dyn WorldEnv,
) -> u32 {
    *counter = counter.wrapping_add(1);
    if target[0] == 0.0 && target[1] == 0.0 && target[2] == 0.0 {
        return 0;
    }
    let mut tgt = target;
    let mut a = [pos[0], pos[1], pos[2], 1.0];
    let c1 = env.line(
        [tgt[0], tgt[1], tgt[2], 1.0],
        [tgt[0] + 0.0, tgt[1] + 0.0, tgt[2] + 150.0, 1.0 + 0.0],
        2,
        0.0,
    );
    let mut line = Line {
        main: c1,
        holes: c1,
    };
    let mut res = RayRes::default();
    let r = env.ray_cast(line, &mut res);
    if 0.0 < r && 0.0 < res.normal[2] {
        tgt = [
            res.hit[0] + res.normal[0] * 20.0,
            res.hit[1] + res.normal[1] * 20.0,
            res.hit[2] + res.normal[2] * 20.0,
        ];
    }
    let c2 = env.line([tgt[0], tgt[1], tgt[2], 1.0], a, 2, 0.0);
    line = Line {
        main: c2,
        holes: c1,
    };
    let far = 500.0 < radius;
    if env.is_blocked(line, &mut res) {
        let d = [
            a[0] - res.hit[0],
            a[1] - res.hit[1],
            a[2] - res.hit[2],
            a[3] - res.hit[3],
        ];
        let n = res.normal;
        let k = ((d[0] * n[0] + d[1] * n[1]) + 1.0 * (d[2] * n[2])) + 1.0 * (d[3] * n[3]);
        let sc = [n[0] * k, n[1] * k, n[2] * k];
        *pos = [pos[0] - sc[0], pos[1] - sc[1], pos[2] - sc[2]];
        a = [pos[0], pos[1], pos[2], 1.0];
        let c3 = env.line([tgt[0], tgt[1], tgt[2], 1.0], a, 2, 0.0);
        line = Line {
            main: c3,
            holes: c1,
        };
        if env.is_blocked(line, &mut res) {
            *pos = [res.hit[0], res.hit[1], res.hit[2]];
        }
        let m = if far { 60.0 } else { 40.0 };
        let n = res.normal;
        *pos = [pos[0] + n[0] * m, pos[1] + n[1] * m, pos[2] + n[2] * m];
        return 1;
    }
    env.push(if far { 100.0 } else { 40.0 }, pos, tgt);
    0
}

// ---- Camera_CollidePush 0x16fb08 ----

fn v4_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    let m = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((m[0] + m[1]) + 1.0 * m[2]) + 1.0 * m[3]
}

fn v4_unit(v: [f32; 4]) -> [f32; 4] {
    let s = v4_dot(v, v);
    let q = if s == 0.0 { f32::MAX } else { 1.0 / s.sqrt() };
    v.map(|x| x * q)
}

/// `Camera_CollidePush` 0x16fb08(k, pos, target): looks for a wall to the side of the line from `pos` to
/// `target` (a sideways line of length k, then its turn about the line); when one is hit, `pos` goes to the hit
/// point pushed k off the wall. Returns 1 when moved.
pub fn collide_push(k: f32, pos: &mut [f32; 3], target: [f32; 3], env: &mut dyn WorldEnv) -> u32 {
    let a = [pos[0], pos[1], pos[2], 1.0];
    let b = [target[0], target[1], target[2], 1.0];
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2], b[3] - a[3]];
    let u = v4_unit(d);
    let z = [0.0, 0.0, 1.0, 0.0];
    let kz = v4_dot(z, u);
    let su = u.map(|x| x * kz);
    let e = [z[0] - su[0], z[1] - su[1], z[2] - su[2], z[3] - su[3]];
    let p = v4_unit(e);
    let pk = p.map(|x| x * k);
    let plus = |v: [f32; 4]| [a[0] + v[0], a[1] + v[1], a[2] + v[2], a[3] + v[3]];
    let minus = |v: [f32; 4]| [a[0] - v[0], a[1] - v[1], a[2] - v[2], a[3] - v[3]];
    let c1 = env.line(plus(pk), minus(pk), 1, 0.5);
    let mut res = RayRes::default();
    let mut line = Line {
        main: c1,
        holes: c1,
    };
    if !(0.0 < env.ray_cast(line, &mut res)) {
        let c = [
            pk[1] * u[2] - u[1] * pk[2],
            pk[2] * u[0] - u[2] * pk[0],
            pk[0] * u[1] - u[0] * pk[1],
            0.0,
        ];
        let c2 = env.line(plus(c), minus(c), 1, 0.5);
        line = Line {
            main: c2,
            holes: c1,
        };
        if !(0.0 < env.ray_cast(line, &mut res)) {
            return 0;
        }
    }
    let n = res.normal;
    *pos = [
        res.hit[0] + n[0] * k,
        res.hit[1] + n[1] * k,
        res.hit[2] + n[2] * k,
    ];
    1
}

// ---- CamRegion_IsInside 0x171610 ----

/// The camera region record (`*(cam + 4)`), fields named by byte offset.
#[derive(Clone, Debug, Default)]
pub struct RegionP {
    /// +0x3c, +0x68, +0x94: the camera's position.
    pub pos: [f32; 3],
    /// +0x150: the reach.
    pub f150: f32,
    /// +0x154: the entry the region is placed on.
    pub w154: i32,
    /// +0x174: the region is a box turned about two axes (otherwise a cube of side 2·`f150`).
    pub w174: i32,
    /// +0x178: the box's length.
    pub f178: f32,
    /// +0x17c, +0x180: the box's two turns.
    pub f17c: f32,
    pub f180: f32,
    /// +0x190: the stage area the position is taken from (0: none).
    pub w190: i32,
}

pub trait RegionEnv {
    /// `Cam_StageAreaToWorld` 0x1777d0 on the region's position.
    fn stage_area_to_world(&mut self, area: i32, v: [f32; 4]) -> StageOut;
    /// `CamRegion_CheckCondition` 0x171500 (the region's own test).
    fn check_condition(&mut self) -> u64;
}

/// `m(i, j)` of the product `b` · `r` as the camera adds it (the same products `rot_apply` takes).
pub(crate) fn mat_prod(b: [f32; 9], r: [f32; 9]) -> [f32; 9] {
    let m = |i: usize, j: usize| (b[j] * r[i * 3] + b[3 + j] * r[i * 3 + 1]) + b[6 + j] * r[i * 3 + 2];
    [m(0, 0), m(0, 1), m(0, 2), m(1, 0), m(1, 1), m(1, 2), m(2, 0), m(2, 1), m(2, 2)]
}

/// `CamRegion_IsInside` 0x171610: is the camera inside the region placed on `entries[w154]`? Returns the
/// region's own test (`check_condition`) when it is, else 0.
pub fn region_is_inside(p: &RegionP, entries: &[[f32; 3]], env: &mut dyn RegionEnv) -> u64 {
    let mut a = p.pos;
    let mut yaw = 0.0f32;
    if p.w190 != 0 {
        let o = env.stage_area_to_world(p.w190, [a[0], a[1], a[2], 0.0]);
        yaw = o.yaw;
        if o.ret {
            a = [o.v[0], o.v[1], o.v[2]];
        }
    }
    let e = entries[p.w154 as usize];
    let mut d = [a[0] - e[0], a[1] - e[1], a[2] - e[2]];
    if p.w174 == 0 {
        let h = p.f150;
        if !(d[0].abs() <= h) || !(d[1].abs() <= h) || !(d[2].abs() <= h) {
            return 0;
        }
    } else {
        let half = p.f178 * 0.5;
        let h = if half < p.f150 { p.f150 } else { half };
        if !(d[0].abs() <= h) || !(d[1].abs() <= h) || !(d[2].abs() <= h) {
            return 0;
        }
        let mut ang = p.f17c;
        if p.w190 != 0 {
            ang = angle_diff(ang + yaw, 0.0);
        }
        let m1 = mat_prod(IDENT3, rot9(0.0, 0.0, 1.0, ang));
        let m2 = mat_prod(m1, rot9(0.0, 1.0, 0.0, p.f180));
        let z = 0.0f32;
        let v = [
            (m2[0] * half + m2[3] * z) + m2[6] * z,
            (m2[1] * half + m2[4] * z) + m2[7] * z,
            (m2[2] * half + m2[5] * z) + m2[8] * z,
        ];
        let q = ps2_div((d[0] * v[0] + d[1] * v[1]) + d[2] * v[2], half * half);
        if !(q.abs() <= 1.0) {
            return 0;
        }
        let s = [v[0] * q, v[1] * q, v[2] * q];
        d = [d[0] - s[0], d[1] - s[1], d[2] - s[2]];
    }
    if dot3(d, d).sqrt() < p.f150 {
        return env.check_condition();
    }
    0
}

// ---- CamRegion_CheckCondition 0x171500 ----

/// What the region's own test reads: the region's `+0x184`, `+0x18c`, `+0x188` and the entry it is placed on.
#[derive(Clone, Debug, Default)]
pub struct CondP {
    pub w184: i32,
    pub w18c: i32,
    pub f188: f32,
}

/// The rider of the entry the region is placed on.
#[derive(Clone, Debug)]
pub struct CondRider {
    /// +0x424.
    pub motion: i32,
    /// +0x57b8.
    pub f57b8: f32,
    /// +0x57c0.
    pub v57c0: [f32; 3],
}

/// `CamRegion_CheckCondition` 0x171500: with `+0x184` set, the rider must be moving (+0x57b8 over 0.33) and at
/// least `f188` from the entry's offset; otherwise a region without `+0x18c` always holds, and one with it
/// holds while the rider's motion is 5 or 6.
pub fn region_check_condition(p: &CondP, rider: Option<&CondRider>, p14: [f32; 3]) -> u64 {
    if p.w184 == 0 {
        if p.w18c == 0 {
            return 1;
        }
        return match rider {
            None => 0,
            Some(r) => (r.motion == 5 || r.motion == 6) as u64,
        };
    }
    let Some(r) = rider else { return 0 };
    if !(f32::from_bits(0x3ea8_f5c3) < r.f57b8) {
        return 0;
    }
    let d = [r.v57c0[0] - p14[0], r.v57c0[1] - p14[1], r.v57c0[2] - p14[2]];
    (p.f188 <= dot3(d, d).sqrt()) as u64
}

// ---- Camera_TrackRider 0x173270 ----

/// The tracker (`param_1`): `+4` the speed taken from the rider, `+8` the smoothed direction, `+0x14` the
/// rider's marker position.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    pub speed: f32,
    pub dir: [f32; 3],
    pub marker: [f32; 3],
}

/// What the tracker reads from its rider.
#[derive(Clone, Debug)]
pub struct TrackRider {
    /// +0x424, +0x428.
    pub w424: i32,
    pub w428: i32,
    /// +0x150: the velocity (four lanes).
    pub v150: [f32; 4],
    /// +0x180: the direction it is heading.
    pub v180: [f32; 4],
    /// +0x1b0.
    pub f1b0: f32,
    /// +0x4ad0.
    pub v4ad0: [f32; 3],
}

/// `Camera_TrackRider` 0x173270(tracker, force): eases the tracker's direction toward the rider's velocity (or
/// heading) and then makes it a unit vector (or leaves it when it has no length).
pub fn track_rider(t: &mut Tracker, rider: Option<&TrackRider>, force: i64) {
    if let Some(r) = rider {
        let mut force = force != 0;
        let d = t.dir;
        if d[0] == 0.0 && d[1] == 0.0 {
            force = true;
        } else if dot3(d, d).sqrt() < 0.5 {
            force = true;
        }
        let k = |bits: u32| f32::from_bits(bits);
        let sum = |a: [f32; 3], b: [f32; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
        let speed4 = v4_dot(r.v150, r.v150).sqrt();
        let eased = |t: &Tracker, kd: f32, ks: f32| {
            // direction · kd · speed + velocity · ks
            let u = unit_or_same(t.dir);
            let a = [u[0] * kd * speed4, u[1] * kd * speed4, u[2] * kd * speed4];
            let b = [r.v150[0] * ks, r.v150[1] * ks, r.v150[2] * ks];
            sum(a, b)
        };
        let v150 = [r.v150[0], r.v150[1], r.v150[2]];
        if r.w428 != 0x12 && r.w424 == 5 {
            t.dir = if force { v150 } else { eased(t, k(0x3f7d_70a4), k(0x3c23_d70a)) };
        } else if r.w428 != 0x12 && r.w424 == 6 {
            t.dir = if force { v150 } else { eased(t, k(0x3f73_3333), k(0x3d4c_cccd)) };
        } else {
            t.speed = r.f1b0;
            let near = speed4 < 10.0;
            t.dir = if force {
                if near { [r.v180[0], r.v180[1], r.v180[2]] } else { v150 }
            } else {
                let u = unit_or_same(t.dir);
                let a = [u[0] * k(0x3f73_3333), u[1] * k(0x3f73_3333), u[2] * k(0x3f73_3333)];
                let w = if near { r.v180 } else { v4_unit(r.v150) };
                let c = k(0x3d4c_cccd);
                sum(a, [w[0] * c, w[1] * c, w[2] * c])
            };
        }
        t.marker = r.v4ad0;
    }
    t.dir = unit_or_same(t.dir);
}

// ---- Camera_SmoothOutput 0x174238 ----

/// The calls `Camera_SmoothOutput` makes besides the angle maths.
pub trait SmoothEnv {
    /// `Camera_CollidePush` 0x16fb08(k, pos, target).
    fn collide_push(&mut self, k: f32, pos: &mut [f32; 3], target: [f32; 3]);
}

/// The camera object as the smoothing sees it: its words by byte offset (`o[off / 4]`), at least 0x4e8 bytes.
struct Words<'a>(&'a mut [u32]);
impl Words<'_> {
    fn i(&self, off: usize) -> u32 {
        self.0[off / 4]
    }
    fn f(&self, off: usize) -> f32 {
        f32::from_bits(self.0[off / 4])
    }
    fn set(&mut self, off: usize, v: u32) {
        self.0[off / 4] = v;
    }
    fn setf(&mut self, off: usize, v: f32) {
        self.0[off / 4] = v.to_bits();
    }
    fn v3(&self, off: usize) -> [f32; 3] {
        [self.f(off), self.f(off + 4), self.f(off + 8)]
    }
    /// Words `0x42c..0x484` and `0x484..0x4dc` are the "now" and the "wanted" copies of the same record.
    fn latch(&mut self) {
        for k in 0..22 {
            self.0[(0x484 + 4 * k) / 4] = self.0[(0x42c + 4 * k) / 4];
        }
    }
    fn unlatch(&mut self) {
        for k in 0..22 {
            self.0[(0x42c + 4 * k) / 4] = self.0[(0x484 + 4 * k) / 4];
        }
    }
    fn push(&mut self, env: &mut dyn SmoothEnv) {
        if 0.0 < self.f(0x20) {
            let mut p = self.v3(0x42c);
            env.collide_push(f32::from_bits(0x4248_0000), &mut p, self.v3(0x4a8));
            for k in 0..3 {
                self.setf(0x42c + 4 * k, p[k]);
            }
        }
    }
    /// Flips the heading by half a turn when it points against the one wanted (`+0x4d0`).
    fn flip(&mut self) {
        if angle_diff(self.f(0x490), self.f(0x4d0)) < 0.0 {
            let pi = f32::from_bits(0x4049_0fdb);
            let a = angle_diff(self.f(0x490) + pi, 0.0);
            self.setf(0x490, a);
            let b = angle_diff(pi, self.f(0x494));
            self.setf(0x494, b);
        }
    }
}

/// `Camera_SmoothOutput` 0x174238: eases the camera's position and angles (`0x42c…`) toward the wanted ones
/// (`0x484…`), or latches them when the camera has just been cut.
pub fn smooth_output(o: &mut [u32], env: &mut dyn SmoothEnv) {
    let mut w = Words(o);
    let pi = f32::from_bits(0x4049_0fdb);
    w.set(0x1c, 1);
    let state = w.i(0xc) as i32;
    let mut l = w.v3(0x484);
    if state < 3 {
        w.set(0x460, 0);
    }
    if w.i(4) == 0 && w.i(0x4d8) == 0 {
        // the plain easing
        let blend = if w.i(0x18) == 0 {
            true
        } else if w.i(0x4bc) == 0 {
            false
        } else {
            0 < w.i(0x4c0) as i32
        };
        if !blend {
            for k in 0..3 {
                w.set(0x4dc + 4 * k, w.i(0x484 + 4 * k));
            }
        } else {
            let a = w.v3(0x4dc).map(|x| x * f32::from_bits(0x3f4c_cccd));
            let b = w.v3(0x484).map(|x| x * f32::from_bits(0x3e4c_cccd));
            for k in 0..3 {
                w.setf(0x4dc + 4 * k, a[k] + b[k]);
            }
            l = w.v3(0x4dc);
        }
        let mut f21 = w.f(0x4cc);
        if f21 < 50.0 && 0.0 < w.f(0x20) {
            f21 = w.f(0x20) * 20.0 + 10.0;
        }
        if state < 3 {
            let k = 1.0f32 / 1.0;
            for i in 0..3 {
                let p = w.f(0x42c + 4 * i);
                let d = l[i] - p;
                w.setf(0x42c + 4 * i, p + d * k);
            }
        }
        if w.i(0x4b4) != 0 {
            w.push(env);
            w.flip();
        }
        if state == 3 {
            w.latch();
        }
        let a = angle_diff(w.f(0x490), w.f(0x438));
        w.setf(0x438, w.f(0x438) + ps2_div(a, f21));
        let a = angle_diff(w.f(0x494), w.f(0x43c));
        w.setf(0x43c, w.f(0x43c) + ps2_div(a, f21));
        let a = angle_diff(w.f(0x498), w.f(0x440));
        let f1 = w.f(0x49c) - w.f(0x444);
        w.setf(0x440, w.f(0x440) + ps2_div(a, 25.0));
        w.setf(0x444, w.f(0x444) + ps2_div(f1, f21));
    } else {
        // a cut: take the wanted record at once
        w.set(0x4d8, 0);
        if w.i(0x4b4) != 0 {
            w.flip();
        }
        if state == 3 {
            w.latch();
        }
        w.unlatch();
        for k in 0..3 {
            w.set(0x4dc + 4 * k, w.i(0x484 + 4 * k));
        }
        if w.i(0x4b4) != 0 {
            w.push(env);
        }
        w.setf(0x10, 500.0);
    }
    let _ = pi;
    if w.i(8) == 0 {
        w.set(4, 0);
    }
    w.set(0x18, 0);
    w.set(8, 0);
}
