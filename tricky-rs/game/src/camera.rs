//! The game's cameras (board rows E11a–c): `CamCamera_EvalPosition` 0x16e900. Checked in the function runner
//! (`tests/camera.rs`).

use crate::math::{angle_diff, atan, sin_cos};

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
fn rot9(ax: f32, ay: f32, az: f32, angle: f32) -> [f32; 9] {
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
    let m = |i: usize, j: usize| (b[j] * r[i * 3] + b[3 + j] * r[i * 3 + 1]) + b[6 + j] * r[i * 3 + 2];
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
                    if 0.0 <= x8 { -HALF_PI } else { HALF_PI }
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
