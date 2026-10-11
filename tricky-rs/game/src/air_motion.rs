//! One tick in the air exactly as the original runs it (board row E8a3): `AirMotion_Update` 0x108378, in cm and
//! the game's z up, in the machine code's order. `tests/air_motion.rs` checks it bit for bit against the original
//! in the function runner.
//!
//! Run as ours: the flight step `Air_IntegrateRK4` (`crate::air::integrate_rk4`), `Quat_Multiply`, `Math_SinCos`,
//! `Math_Asin`, `Boarder_UpdateOrientation` (`crate::orient`). Handed to [`AirEnv`], each call in the order the game
//! makes it: the flight prediction's tick and landing (`AirPredict_Tick` 0x123ec8, 0x123f00), the world ray under
//! the board and the wall ray (`World_RayCast`, row E7), `Boarder_ProbeGround` (which needs the world), the landing
//! check and choice (`Landing_CheckAngles`, `Landing_ChooseState`: ported in `crate::landing`, their effects are the
//! environment's), scoring, the wipe-out, clips, sounds and the rider's virtuals.

use crate::ground_motion::CrashRecord;
use crate::orient::{quat_multiply, turn_axis, update_orientation, Orient};
use crate::probe::{add, cross, dot, scale, sub};

type V4 = [f32; 4];

const fn k(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// What a world ray answers: the fraction along it (negative: nothing hit) and the hit record's point (+0x10),
/// normal (+0x20) and surface word (+0x34).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ray {
    pub frac: f32,
    pub point: V4,
    pub normal: V4,
    pub word: u32,
}

/// The rider as `AirMotion_Update` reads and writes it (offsets from the rider).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirRider {
    /// +0x12c: the tick's length in 60ths
    pub timescale: f32,
    /// +0x140, +0x150
    pub pos: V4,
    pub vel: V4,
    /// +0x170..+0x1b0
    pub orient: Orient,
    /// +0x1d0, +0x1d4: spin and flip so far
    pub spin: f32,
    pub flip: f32,
    /// +0x57f0: the surface the prediction says is below (−1 none); +0x290 the surface, +0x29c ticks on it
    pub slope_kind: i32,
    pub surface: u32,
    pub surface_ticks: i32,
    /// +0x4aa0..+0x4ad0: the board's frame (four rows; the third is its up)
    pub frame: [V4; 4],
    /// the pivot bone (rider + +0x44d8 × 0x394 + 0x8c8), x y z
    pub bone: [f32; 3],
    /// +0x2a0 the ground normal, +0x1bc the height over the ground, +0x2d0 the ground point (the probe's)
    pub normal: V4,
    pub height: f32,
    pub ground: V4,
    /// +0x798: the speed into the snow at a landing (negated)
    pub fall: f32,
    /// +0x1b4 riding switch, stats+0x40 the natural stance
    pub switch: u32,
    pub stance: u32,
    /// +0x58a8: the count `Score_LandTrick` raises for a landed trick; +0x28 a 0..3 counter of celebrations
    pub tricks: i32,
    pub x28: i32,
    /// stats+0x74: the rider's celebration level (index into [`CELEBRATE`])
    pub stats74: u32,
    /// +0x494
    pub mask: u32,
}

/// The landing score each celebration level must beat (0x3625c8).
pub const CELEBRATE: [f32; 10] = [0.75, k(0x3f2e_147b), k(0x3f19_999a), k(0x3f0c_cccd), 0.5, k(0x3ee6_6666), k(0x3ecc_cccd), k(0x3eb3_3333), k(0x3e99_999a), 0.25];

/// What `AirMotion_Update` calls that is other rows' work, in its order. Each may change the rider, as the game's
/// callees do.
pub trait AirEnv {
    /// `AirPredict_Tick` 0x123ec8(rider+0x57b0)
    fn predict_tick(&mut self, r: &mut AirRider);
    /// `cWorldLine`(0.5, from, to, 1), `World_RayCast`(the world, line, hit, rider+0x430)
    fn down_ray(&mut self, r: &mut AirRider, from: V4, to: V4) -> Ray;
    /// `Boarder_ProbeGround` 0x128ae8: true when it found ground
    fn probe_ground(&mut self, r: &mut AirRider) -> bool;
    /// 0x123f00(rider+0x57b0): the prediction told of the landing
    fn predict_landed(&mut self, r: &mut AirRider);
    /// `AirMotion_SetFallScale` 0x1049a0(vn, manager+0x6f0)
    fn set_fall_scale(&mut self, r: &mut AirRider, vn: f32);
    /// the rider's virtual +0x6c(vn × −0.00072000007)
    fn virtual_6c(&mut self, r: &mut AirRider, x: f32);
    /// `AnimCtl_GetClipType`(rider+0x46c0, 1)
    fn clip_type_layer1(&mut self, r: &mut AirRider) -> u32;
    /// `AnimCtl_BlendOutAll`(rider+0x46c0)
    fn blend_out_all(&mut self, r: &mut AirRider);
    /// `Landing_CheckAngles` 0x12ba78(rider, +0x2a0): its two angles, or `None` (it returned 0: a crash)
    fn check_angles(&mut self, r: &mut AirRider) -> Option<(f32, f32)>;
    /// the rider's virtual +0x64(point, velocity, normal): spray
    fn crash_virtual(&mut self, r: &mut AirRider, point: V4, vel: V4, normal: V4);
    /// `Boarder_Wipeout` 0x11d830(rider, record)
    fn wipeout(&mut self, r: &mut AirRider, rec: &CrashRecord);
    /// `Score_Crash` 0x156de0(rider+0x5820, 0)
    fn score_crash(&mut self, r: &mut AirRider) -> f32;
    /// `Boarder_AddMeter` 0x11b018(amount, rider)
    fn add_meter(&mut self, r: &mut AirRider, amount: f32);
    /// `Landing_ChooseState` 0x109308(angle 1, angle 2, vn)
    fn choose_state(&mut self, r: &mut AirRider, a1: f32, a2: f32, vn: f32);
    /// `Score_LandTrick` 0x1569b0(rider+0x5820, 0, riding switch, normal z < 0.7)
    fn land_trick(&mut self, r: &mut AirRider, switch: bool, steep: bool) -> f32;
    /// the rider's virtual +0x84(2.0)
    fn virtual_84(&mut self, r: &mut AirRider, x: f32);
    /// `Boarder_StartCelebrate` 0x11f3b8(amount, rider, 1)
    fn start_celebrate(&mut self, r: &mut AirRider, amount: f32);
    /// `Boarder_IsTricky` 0x11f430
    fn is_tricky(&mut self, r: &mut AirRider) -> bool;
    /// `Speech_OnAirTime` 0x233448(sound, rider)
    fn speech_air_time(&mut self, r: &mut AirRider);
    /// `Boarder_SpawnLandingPuffs` 0x136e90(vn, manager)
    fn spawn_landing_puffs(&mut self, r: &mut AirRider, vn: f32);
    /// `Sfx_UpdateAir` 0x217d50(score, sound, rider)
    fn sfx_update_air(&mut self, r: &mut AirRider, score: f32);
    /// the rider's virtual +0x9c
    fn virtual_9c(&mut self, r: &mut AirRider);
    /// `Rail_TryCapture` 0x125cb8 (`ground_tick::rail_try_capture`)
    fn rail_try_capture(&mut self, r: &mut AirRider);
    /// `Boarder_InstanceCollisions` 0x125088
    fn instance_collisions(&mut self, r: &mut AirRider);
    /// `cWorldBoarder`(rider+0x470, 0) and `World_RayCast`(the world, it, hit, 0), with rider+0x494 = 0xcf
    fn wall_ray(&mut self, r: &mut AirRider) -> Ray;
    /// `Boarder_Translate` 0x1267a0(rider, by)
    fn translate(&mut self, r: &mut AirRider, by: V4);
    /// `cWorldBoarder::vf10`(2), after rider+0x494 = −1
    fn wall_done(&mut self, r: &mut AirRider);
}

/// The pivot's height turned back by the trick's spin and flip, as seen from the rider's attitude without them:
/// the offset the down ray and the probe start from (0x1085c8..0x108920).
fn pivot_offset(r: &AirRider) -> V4 {
    let (s1, c1) = crate::math::sin_cos(r.spin * -0.5);
    let (s2, c2) = crate::math::sin_cos(r.flip * -0.5);
    let t = quat_multiply([c1, 0.0, 0.0, s1], [c2, s2, 0.0, 0.0]);
    let q = quat_multiply(r.orient.q, t);
    let zero = 0.0f32;
    let bz = r.bone[2];
    let (w, x, y, z) = (t[0], t[1], t[2], t[3]);
    let tv = [y * bz - z * zero, z * zero - x * bz, x * zero - y * zero];
    let u = [y * tv[2] - z * tv[1], z * tv[0] - x * tv[2], x * tv[1] - y * tv[0]];
    let two_w = w + w;
    let p = [zero, zero, bz, zero];
    let back = [(p[0] - tv[0] * two_w) + u[0] * 2.0, (p[1] - tv[1] * two_w) + u[1] * 2.0, (p[2] - tv[2] * two_w) + u[2] * 2.0, zero];
    let e = sub(back, p);
    let wv = turn_axis(q, [e[0], e[1], e[2]]);
    [wv[0], wv[1], wv[2], zero]
}

/// A crash: the spray virtual, the wipe-out record (the hit point, the unit velocity, the normal, the speed into
/// it), `Boarder_Wipeout`, `Score_Crash` into `Boarder_AddMeter`. (On a landing crash the game leaves the record's
/// second copy of the direction, +0x40, unwritten: whatever its stack held; here it is the direction.)
fn crash(r: &mut AirRider, env: &mut dyn AirEnv, point: V4, normal: V4, along: f32) {
    let dir = scale(r.vel, 1.0 / dot(r.vel, r.vel).sqrt());
    let rec = CrashRecord { pos: point, dir, normal, along };
    let vel = r.vel;
    env.crash_virtual(r, point, vel, normal);
    env.wipeout(r, &rec);
    let s = env.score_crash(r);
    env.add_meter(r, s);
}

/// `AirMotion_Update` 0x108378 on `r`, with the surface table `table` (`crate::surface::SURFACE_TABLE`).
pub fn update(r: &mut AirRider, table: &[[f32; 25]; 20], env: &mut dyn AirEnv) {
    crate::air::integrate_rk4(r.timescale * k(0x3c88_8889), &mut r.pos, &mut r.vel, false);
    env.predict_tick(r);
    if r.slope_kind != -1 {
        if r.surface == r.slope_kind as u32 {
            if -1 < r.surface_ticks {
                r.surface_ticks = 0;
            }
        } else {
            r.surface = r.slope_kind as u32;
            r.surface_ticks = 0x8000_0001u32 as i32;
        }
    }
    let up = r.frame[2];
    // the pivot in the world: the frame applied to minus the bone (ACC: rows × x, y, z, then + row 3 × 1)
    let b = [-r.bone[0], -r.bone[1], -r.bone[2], 1.0];
    let f = r.frame;
    let mut piv = [0.0f32; 4];
    for i in 0..4 {
        piv[i] = ((f[0][i] * b[0] + f[1][i] * b[1]) + f[2][i] * b[2]) + f[3][i] * b[3];
    }
    let from = add(piv, scale(up, -100.0));
    let to = add(piv, scale(up, 200.0));
    let mut flying = true;
    let mut vn = 0.0f32;
    let down = env.down_ray(r, from, to);
    if 0.0 <= down.frac {
        r.normal = down.normal;
        vn = dot(r.normal, r.vel);
        if vn < 0.0 {
            let saved = r.pos;
            let off = pivot_offset(r);
            r.pos = add(r.pos, off);
            let found = env.probe_ground(r);
            r.pos = saved;
            if found {
                vn = dot(r.normal, r.vel);
                if r.height < table[r.surface as usize][6] * 0.5 {
                    flying = false;
                    if r.height < 0.0 {
                        r.pos = sub(r.ground, off);
                        r.height = 0.0;
                    }
                }
            }
        }
    }
    if flying {
        env.rail_try_capture(r);
        env.instance_collisions(r);
        r.mask = 0xcf;
        let wall = env.wall_ray(r);
        if 0.0 < wall.frac {
            let push = scale(scale(wall.normal, k(0x3f8c_cccd)), wall.frac);
            env.translate(r, push);
            r.pos = add(r.pos, push);
            let vn2 = dot(wall.normal, r.vel);
            if vn2 < 0.0 {
                let x = vn2 * k(0xb9eb_edfb);
                let lim = if 1.0 < x { k(0x3f66_6667) } else { x * k(0x3e99_999a) + k(0x3f19_999a) };
                if lim < dot(wall.normal, up) {
                    let c = cross(up, wall.normal);
                    let len = dot(c, c).sqrt();
                    if k(0x3727_c5ac) < len.abs() {
                        let inv = 1.0 / len;
                        let axis = [c[0] * inv, c[1] * inv, c[2] * inv, c[3] * inv];
                        let mut a = crate::math::asin(len);
                        if k(0x3dd6_7750) < a {
                            a = k(0x3dd6_7750);
                        }
                        let (s, co) = crate::math::sin_cos(a * 0.5);
                        r.orient.q = quat_multiply([co, s * axis[0], s * axis[1], s * axis[2]], r.orient.q);
                        update_orientation(&mut r.orient);
                    }
                    r.vel = sub(r.vel, scale(wall.normal, vn2));
                } else {
                    crash(r, env, wall.point, wall.normal, vn2);
                }
            }
        }
        r.mask = 0xffff_ffff;
        env.wall_done(r);
        return;
    }
    // landing
    env.predict_landed(r);
    env.set_fall_scale(r, vn);
    env.virtual_6c(r, vn * k(0xba3c_be63));
    if env.clip_type_layer1(r) == 0x1a {
        env.blend_out_all(r);
    }
    match env.check_angles(r) {
        None => {
            r.fall = -vn;
            crash(r, env, down.point, down.normal, vn);
        }
        Some((a1, a2)) => {
            r.fall = -vn;
            let mut switch = r.switch != r.stance;
            if k(0x3fc9_0fdb) < a1.abs() {
                switch = !switch;
            }
            env.choose_state(r, a1, a2, vn);
            let lo = k(0x3e86_0a92);
            let x = a2.abs();
            let g = if x < lo { lo } else if k(0x3f5f_66f3) < x { k(0x3f5f_66f3) } else { x };
            if lo < g {
                r.vel = scale(r.vel, g * k(0xbe7b_4efa) + k(0x3f88_395b));
            }
            let lo = k(0x3edf_66f3);
            let x = a1.abs();
            let g = if x < lo { lo } else if k(0x3fb2_b8c2) < x { k(0x3fb2_b8c2) } else { x };
            if lo < g {
                r.vel = scale(r.vel, g * k(0xbe85_4942) + k(0x3f8e_8a0d));
            }
            let before = r.tricks;
            let steep = r.normal[2] < k(0x3f33_3333);
            let score = env.land_trick(r, switch, steep);
            env.add_meter(r, score);
            if before < r.tricks {
                env.virtual_84(r, 2.0);
                env.start_celebrate(r, 100.0);
                if env.is_tricky(r) && before < 6 {
                    env.speech_air_time(r);
                }
                r.x28 = r.x28.wrapping_add(1) % 4;
            } else if CELEBRATE[r.stats74 as usize] < score {
                env.start_celebrate(r, score);
            }
            env.spawn_landing_puffs(r, vn);
            env.sfx_update_air(r, score);
        }
    }
    env.virtual_9c(r);
}
