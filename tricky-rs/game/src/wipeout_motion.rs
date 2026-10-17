//! The wipe-out motion (board row G6b): motion 5 and the get-up motion 6 on the wipe-out body (`ragdoll`):
//! `WipeoutMotion_Enter` 0x10c198, `WipeoutMotion_Update` 0x10c2f0 (the tumble and the slides), `WipeoutMotion_Exit`
//! 0x10d680, `Wipeout_StartGetUp` 0x10f178, `Wipeout_PlayImpactClip` 0x10f9a0 and `GetUpMotion_Update` 0x1102c8. In
//! the PS2's floats, in the game's order of operations; checked in the function runner (`tests/wipeout_motion.rs`).
//!
//! The lie-flat turn takes `math::asin`, not `math::asin_ee`: `tests/ragdoll.rs` runs the original with every float
//! rule nearest (as this port's floats are) and feeds it arguments beyond 0.7071, where the EE's chopped square root
//! no longer matches (tried 2026-10-17).

use crate::math::asin;
use crate::orient::{rotate_vec_to_vec, slerp};
use crate::ps2f::F;
use crate::ragdoll::{CastOut, RagEnv, RagRider, Ragdoll};
use crate::rigidbody::{add, cross, dot, ld, mat, scale, st, sub};
use crate::roller::RollerBody;

type V4 = [f32; 4];

/// Clips: the slides on the back and the front, the falling, curled and curled-to-stretched cycles, the fall
/// recovery, the get-ups to riding.
pub const CLIP_BUTTSLIDE: u32 = 0x2dc;
pub const CLIP_FACESLIDE: u32 = 0x2dd;
pub const CLIP_FALLING: u32 = 0x2de;
pub const CLIP_CRUNCH: u32 = 0x2df;
pub const CLIP_CRUNCH2STRETCH: u32 = 0x2e0;
pub const CLIP_FALLRECOVER: u32 = 0x224;

/// The wipe-out motion's state: the body (+8 seconds since the last touch, +0x20 / +0x40 the crash point and
/// normal, +0x70 the rigid body), +0xc seconds down, +0x10 seconds on the ground, +0x14 / +0x18 the stick (the
/// tumble's control), +0x30 the normal when the slide ended, +0x50 the speed then.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Wipeout {
    pub rag: Ragdoll,
    pub t: f32,
    pub t_ground: f32,
    pub stick: [f32; 2],
    pub x30: V4,
    pub speed: f32,
}

/// The rider's further fields the motion uses (beside `RagRider`'s).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct WipeRider {
    /// +0x12c: the time scale (a tick is it / 60 s)
    pub time_scale: f32,
    /// +0x180 the board's forward, +0x190 its side
    pub fwd: V4,
    pub side: V4,
    /// +0x1b4, +0x1dc, +0x1f0
    pub x1b4: u32,
    pub x1dc: u32,
    pub x1f0: u32,
    /// +0x1f4/8, +0x200/4, +0x20c/10, +0x218/c, +0x248/c, +0x26c/70: the pairs the motion sets to (2, 0)
    pub pairs: [(f32, u32); 6],
    /// +0x294: the ride height
    pub x294: f32,
    /// +0x2e4: the stuck counter (1e6 asks for a reset)
    pub stuck: f32,
    /// +0x310: the surface type before the wipe-out
    pub x310: u32,
    /// +0x360: the course point 8 m ahead
    pub course8: V4,
    /// +0x418: below 0 for a human player
    pub player: i32,
    /// +0x420, and whether +0x464's +0x40 is set (switch stance allowed)
    pub x420: u32,
    pub x464: bool,
    /// +0x46d4, +0x46e0: the animation's stance and its angle
    pub x46d4: u32,
    pub x46e0: f32,
    /// the landing prediction (+0x57b0): +0x57b8 seconds to land, +0x57d0 the velocity there, +0x57e0 the normal
    /// there, +0x57f0 the surface type
    pub t_land: f32,
    pub land_v: V4,
    pub land_n: V4,
    pub land_surface: u32,
}

/// What the motion asks of the animation, the rider and the world.
pub trait WipeEnv: RagEnv {
    /// `AnimCtl_GetClip`(rider+0x46c0, 0), `AnimCtl_GetClipType`
    fn clip(&self) -> u32;
    fn clip_type(&self) -> u32;
    /// `AnimCtl_PlayClip`(rider+0x46c0, clip, 0, 5)
    fn play_clip(&mut self, clip: u32);
    /// `AnimCtl_BlendOutAll`
    fn blend_out_all(&mut self);
    /// `AnimCtl_GetClipDisplacement`(rider+0x46c0, 0)
    fn displacement(&mut self) -> V4;
    /// `Boarder_ProbeGround` 0x128ae8
    fn probe_ground(&mut self, r: &mut RagRider, w: &mut WipeRider) -> bool;
    /// `Boarder_RotateAboutAxis` 0x128e90(angle, rider, axis)
    fn rotate_about_axis(&mut self, r: &mut RagRider, w: &mut WipeRider, angle: f32, axis: V4);
    /// the surface table's gravity (race +0x24, 100 bytes a surface)
    fn surface_g(&self, surface: u32) -> f32;
    /// the word at 0x31d310 in rider+0x494 while `Boarder_InstanceCollisions` and `Boarder_WallCollide` run
    fn collide_world(&mut self, r: &mut RagRider, w: &mut WipeRider);
    /// `Boarder_Translate` 0x1267a0
    fn translate(&mut self, r: &mut RagRider, d: V4);
    /// `AirPredict_AtTakeoff` (true) / `AirPredict_Tick` on rider+0x57b0, and `FUN_00123f00` (its reset)
    fn predict(&mut self, r: &RagRider, w: &mut WipeRider, takeoff: bool);
    fn predict_reset(&mut self);
    /// `Boarder_SetMotion`, `Boarder_SetState`
    fn set_motion(&mut self, m: u32);
    fn set_state(&mut self, s: u32);
    /// the sounds: `FUN_00218318` (entering), `Sfx_OnWipeoutImpact`, `Sfx_PlayWipeoutSlide`, `Sfx_StopWipeoutLoop`,
    /// `FUN_00218b68` (leaving, unless into motion 6), and the rider's virtual +0x9c on leaving
    fn sfx_enter(&mut self);
    fn sfx_impact(&mut self);
    fn sfx_slide(&mut self);
    fn sfx_stop(&mut self);
    fn sfx_leave(&mut self);
    fn rider_leave(&mut self);
    /// Not a call of the game's: the rider's forward and side (+0x180, +0x190) as `Boarder_UpdateOrientation` 0x129008
    /// last wrote them inside `Ragdoll_Step` (which hands it only the [`RagRider`]), into the motion's copy. The game
    /// reads those fields where they are; a host that keeps them apart copies them here. Nothing by default.
    fn sync(&mut self, w: &mut WipeRider) {
        let _ = w;
    }
}

const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const PI: f32 = f32::from_bits(0x4049_0fdb);
const SLOPE: f32 = 0.34906584;
const RIGHTING: f32 = 3.3475056;

fn unit(v: [F; 4]) -> [F; 4] {
    scale(v, F(1.0) / dot(v, v).sqrt())
}

/// (c + 2w(q × c)) + 2(q × (q × c)), w 0
fn turn(q: V4, c: [F; 4]) -> [F; 4] {
    let [w, x, y, z] = ld(q);
    let t = [y * c[2] - z * c[1], z * c[0] - x * c[2], x * c[1] - y * c[0]];
    let u = [y * t[2] - z * t[1], z * t[0] - x * t[2], x * t[1] - y * t[0]];
    let s = w + w;
    let two = F(2.0);
    [(c[0] + t[0] * s) + u[0] * two, (c[1] + t[1] * s) + u[1] * two, (c[2] + t[2] * s) + u[2] * two, F(0.0)]
}

/// 1/m as the motion takes it: −1 when the inverse mass is 0
fn mass(im: f32) -> F {
    if im == 0.0 {
        F(-1.0)
    } else {
        F(1.0) / F(im)
    }
}

fn bad(surface: u32) -> bool {
    surface == 6 || surface == 10
}

/// The slides' and the get-up's step (fixed 1/60 s): the position by the velocity, gravity along the ground
/// (the surface's), × 0.98333; then (the caller's probe between) held to the ground by at most 8.333333 cm and the
/// velocity into the ground taken out.
fn slide_move(r: &mut RagRider, w: &WipeRider, env: &impl WipeEnv) {
    let g = F(env.surface_g(r.surface));
    let f = F(r.ground[2]) * g;
    let a = [F(r.ground[0]) * f, F(r.ground[1]) * f, f - g, F(0.0)];
    let dt = F(f32::from_bits(0x3c88_8889));
    let _ = w;
    r.pos = st(add(ld(r.pos), scale(ld(r.vel), dt)));
    r.vel = st(add(ld(r.vel), scale(a, dt)));
    r.vel = st(scale(ld(r.vel), F(f32::from_bits(0x3f7b_bbbc))));
}

fn hold_to_ground(r: &mut RagRider, w: &WipeRider) {
    let h = F(r.height) + F(w.x294);
    let s = if F(-8.333333) <= h { if F(8.333333) < h { F(8.333333) } else { h } } else { F(-8.333333) };
    r.pos = st(sub(ld(r.pos), scale(ld(r.ground), s)));
    r.height = (F(r.height) - s).0;
    let n = ld(r.ground);
    r.vel = st(sub(ld(r.vel), scale(n, dot(ld(r.vel), n))));
}

impl Wipeout {
    /// `WipeoutMotion_Enter` 0x10c198: the body made (`Ragdoll_InitFromBoarder`); the stance and its angle
    /// (switch: π, wrapped to −π..π); the clips blended out; the timers 0; the surface kept at +0x310 and taken as
    /// 10 until something is touched; an impact clip; the six pairs (2, 0); the sound.
    pub fn enter(&mut self, r: &mut RagRider, w: &mut WipeRider, env: &mut impl WipeEnv) {
        self.rag.init(r);
        let u = (w.x464 && w.x420 == 2) as u32;
        w.x1b4 = u;
        let ang = if u != 0 { F(f32::from_bits(0x4049_0fdb)) } else { F(0.0) };
        let f6 = ang * F(0.15915494) + F(0.5);
        let mut f5 = F(f6.0 as i32 as f32);
        if f6 < f5 {
            f5 = f5 - F(1.0);
        }
        w.x46d4 = u;
        w.x46e0 = (ang - f5 * F(6.2831855)).0;
        env.blend_out_all();
        self.rag.t_air = 0.0;
        self.t = 0.0;
        self.t_ground = 0.0;
        w.x310 = r.surface;
        r.surface = 10;
        w.x1dc = 0;
        play_impact_clip(r, w, env);
        w.pairs = [(2.0, 0); 6];
        env.sfx_enter();
    }

    /// `WipeoutMotion_Exit` 0x10d680(the next motion)
    pub fn exit(&mut self, next: u32, env: &mut impl WipeEnv) {
        env.sfx_stop();
        if next != 6 {
            env.sfx_leave();
        }
        env.rider_leave();
    }

    /// `WipeoutMotion_Update` 0x10c2f0: a slide (clips 0x2dc / 0x2dd) or the tumble.
    pub fn update(&mut self, r: &mut RagRider, w: &mut WipeRider, rollers: &mut [(u32, RollerBody)], env: &mut impl WipeEnv) {
        let dt = F(w.time_scale) * F(0.016666668);
        self.t = (F(self.t) + dt).0;
        let clip = env.clip();
        if clip.wrapping_sub(CLIP_BUTTSLIDE) < 2 {
            self.slide(clip, dt, r, w, env);
        } else {
            self.tumble(clip, dt, r, w, rollers, env);
        }
    }

    fn slide(&mut self, clip: u32, dt: F, r: &mut RagRider, w: &mut WipeRider, env: &mut impl WipeEnv) {
        self.t_ground = (F(self.t_ground) + dt).0;
        slide_move(r, w, env);
        hold_to_ground(r, w);
        let probe = crate::ragdoll::ground_probe(r, env);
        if !probe || r.ground[2] < 0.3 || bad(r.surface) {
            // off the snow: back to tumbling, the body made again from the rider
            env.play_clip(CLIP_FALLING);
            self.rag.crash_at = r.pos;
            self.x30 = r.ground;
            self.rag.crash_n = r.ground;
            let v = ld(r.vel);
            self.speed = dot(v, v).sqrt().0;
            self.rag.init(r);
        }
        // lie flat: turn the board's forward toward the ground plane by 1/12 of the angle a tick
        let mut a = cross(ld(r.ground), ld(w.fwd));
        if clip == CLIP_FACESLIDE {
            a = scale(a, F(-1.0));
        }
        let mut s = dot(a, a).sqrt();
        if F(1e-5) < s {
            let axis = scale(a, F(1.0) / s);
            if F(0.99999) < s {
                s = F(0.99999);
            }
            let ang = F(asin(s.0));
            env.rotate_about_axis(r, w, (-ang * F(0.083333336)).0, st(axis));
        }
        let mut limit = F(self.t_ground) * F(694.4444) + F(833.3333);
        if F(2222.2222) < limit {
            limit = F(2222.2222);
        }
        if s < F(0.1) {
            let v = ld(r.vel);
            if dot(v, v).sqrt() < limit && F(0.9) < F(dot(ld(w.fwd), ld(r.ground)).0.abs()) && env.probe_ground(r, w) {
                self.start_get_up(r, w, env);
                return;
            }
        }
        env.collide_world(r, w);
    }

    fn tumble(&mut self, clip: u32, dt: F, r: &mut RagRider, w: &mut WipeRider, rollers: &mut [(u32, RollerBody)], env: &mut impl WipeEnv) {
        let mut damp = true;
        // lifted by the ride height while stepping
        let mut lift = (F(w.x294) + F(10.0)) * F(0.5);
        lift = if r.ground[2] < 0.0 { lift * F(0.0) } else { lift * F(r.ground[2]) };
        let mut up = [F(0.0); 4];
        if lift <= F(0.0) {
            lift = F(0.0);
        } else {
            up = scale(ld(r.ground), lift);
            env.translate(r, st(up));
        }
        let mut rec = CastOut::default();
        self.rag.step(dt.0, r, &mut rec, rollers, env);
        env.sync(w);
        if F(0.0) < lift {
            env.translate(r, st(scale(up, F(-1.0))));
        }
        let probe = crate::ragdoll::ground_probe(r, env);
        r.ground_kind = rec.kind;
        if rec.kind == 3 {
            let mut ta = self.rag.t_air;
            if 0.1 < ta {
                if clip == CLIP_CRUNCH {
                    env.play_clip(CLIP_CRUNCH2STRETCH);
                    env.predict(r, w, true);
                } else {
                    env.predict(r, w, false);
                }
                if 0.1 < w.t_land {
                    if bad(w.land_surface) {
                        self.t_ground = 0.0;
                    } else {
                        let zn = dot(ld(r.up), ld(w.land_n));
                        if self.stick[0].abs() < 0.5 && self.stick[1].abs() < 0.5 {
                            // right the body toward the landing
                            let ln = ld(w.land_n);
                            let lv = ld(w.land_v);
                            let d = unit(sub(lv, scale(ln, dot(lv, ln))));
                            let xd = dot(ld(w.fwd), d);
                            let m = mass(self.rag.rb.inv_mass);
                            let a = scale(cross(ld(r.up), ln), (dt * F(250951.34)) * m);
                            let m = mass(self.rag.rb.inv_mass);
                            let k = if F(0.0) < xd { F(125475.67) } else { F(-125475.67) };
                            let b = scale(cross(ld(w.fwd), d), (dt * k) * m);
                            let a = add(a, b);
                            damp = false;
                            let l = add(scale(ld(self.rag.rb.l), F(1.0) - dt * F(RIGHTING)), a);
                            self.rag.rb.l = st(l);
                            self.rag.rb.w = st(mat(&self.rag.rb.inv_inertia, l, F(0.0)));
                            let om = ld(self.rag.rb.w);
                            if F(0.9) < zn && dot(om, om).sqrt() < F(4.0) {
                                w.x1f0 = 0;
                                w.pairs[0] = (0.0, 0);
                                env.play_clip(CLIP_FALLRECOVER);
                                env.set_motion(1);
                                env.set_state(0xd);
                                return;
                            }
                        }
                        self.t_ground = 0.0;
                    }
                } else {
                    self.t_ground = 0.0;
                }
                ta = self.rag.t_air;
            } else if !probe {
                self.t_ground = 0.0;
            } else {
                r.ground_kind = 0;
                self.t_ground = (F(self.t_ground) + dt).0;
            }
            self.rag.t_air = (F(ta) + dt).0;
        } else {
            if !probe || r.ground[2] < rec.normal[2] {
                r.ground = rec.normal;
            }
            r.surface = if rec.kind == 0 || rec.kind == 2 { rec.x30[1] } else { 10 };
            if (self.t_ground == 0.0 && clip == CLIP_CRUNCH) || clip == CLIP_FALLING || clip == CLIP_CRUNCH2STRETCH {
                play_impact_clip(r, w, env);
                env.predict_reset();
            }
            self.rag.t_air = 0.0;
            self.t_ground = (F(self.t_ground) + dt).0;
        }
        if probe && 0.25 < self.t_ground && !bad(r.surface) {
            let xn = dot(ld(w.fwd), ld(r.ground));
            env.sfx_slide();
            if F(SLOPE) < F(r.ground[2]) {
                if F(0.75) < xn {
                    env.play_clip(CLIP_BUTTSLIDE);
                } else if xn < F(-0.75) {
                    env.play_clip(CLIP_FACESLIDE);
                } else {
                    // rolled onto the back or the front
                    let a = cross(ld(w.fwd), ld(r.ground));
                    let m = mass(self.rag.rb.inv_mass);
                    let k = if xn < F(0.0) { F(-400000.0) } else { F(400000.0) };
                    let l = add(ld(self.rag.rb.l), scale(a, (dt * k) * m));
                    self.rag.rb.l = st(l);
                    self.rag.rb.w = st(mat(&self.rag.rb.inv_inertia, l, F(0.0)));
                }
            }
        }
        // straight back up
        if 0.5 < self.t && 0.1 < self.t_ground && !bad(r.surface) {
            let om = ld(self.rag.rb.w);
            if dot(om, om).sqrt() < F(7.0) && F(SLOPE) < F(r.ground[2]) {
                let d = unit(sub(ld(w.course8), ld(r.pos)));
                if F(0.8) < F(dot(d, ld(w.fwd)).0.abs()) && F(0.85) < dot(ld(r.up), ld(r.ground)) && env.probe_ground(r, w) && !bad(r.surface) {
                    env.play_clip(if env.clip_type() == 0x11 { 0x250 } else { 0x251 });
                    env.set_motion(2);
                    env.set_state(3);
                    return;
                }
            }
        }
        // the stick turns the body
        let m = mass(self.rag.rb.inv_mass);
        let a = scale(ld(w.side), ((dt * F(75285.4)) * m) * F(self.stick[0]));
        let m = mass(self.rag.rb.inv_mass);
        let b = scale(ld(w.fwd), ((dt * F(-125475.67)) * m) * F(self.stick[1]));
        let a = add(a, b);
        let l = if damp { add(scale(ld(self.rag.rb.l), F(1.0) - dt * F(RIGHTING)), a) } else { add(ld(self.rag.rb.l), a) };
        self.rag.rb.l = st(l);
        self.rag.rb.w = st(mat(&self.rag.rb.inv_inertia, l, F(0.0)));
        if self.t_ground < 3.0 && self.t <= 7.0 {
            return;
        }
        // timed out: a player is reset, the computer gets up where it is
        if w.player < 0 {
            w.stuck = 1e6;
            return;
        }
        env.play_clip(if F(0.0) < dot(ld(w.fwd), ld(r.ground)) { CLIP_BUTTSLIDE } else { CLIP_FACESLIDE });
        self.start_get_up(r, w, env);
    }

    /// `Wipeout_StartGetUp` 0x10f178: the get-up clip by the side lain on (the clip playing) and where the course
    /// is (8 m ahead) against the board's up and side, the board turned to match; the rider moved by the hip's
    /// place less the clip's displacement (both turned); motion 6, state 0x14.
    pub fn start_get_up(&mut self, r: &mut RagRider, w: &mut WipeRider, env: &mut impl WipeEnv) {
        let hip = turn(r.q, [F(r.hip[0]), F(r.hip[1]), F(r.hip[2]), F(0.0)]);
        let d = unit(sub(ld(w.course8), ld(r.pos)));
        let fz = dot(ld(r.up), d);
        let fs = dot(ld(w.side), d);
        let c = F(0.70710677);
        let switch = |w: &WipeRider| !(w.x464 && w.x420 == 2);
        let clip = if env.clip() == CLIP_FACESLIDE {
            let side = w.side;
            env.rotate_about_axis(r, w, HALF_PI, side);
            if c < fz {
                0x24e
            } else if c < fs {
                Self::get_up_sideways(r, w, env, 0x253, 0x24f)
            } else if fz < -c {
                let u = r.up;
                env.rotate_about_axis(r, w, PI, u);
                0x24d
            } else {
                let u = r.up;
                let a = if switch(w) { HALF_PI } else { -HALF_PI };
                env.rotate_about_axis(r, w, a, u);
                0x24f
            }
        } else {
            let side = w.side;
            env.rotate_about_axis(r, w, -HALF_PI, side);
            if c < fz {
                let u = r.up;
                env.rotate_about_axis(r, w, PI, u);
                0x24a
            } else if c < fs {
                Self::get_up_sideways(r, w, env, 0x252, 0x24c)
            } else if -c <= fz {
                let u = r.up;
                let a = if switch(w) { HALF_PI } else { -HALF_PI };
                env.rotate_about_axis(r, w, a, u);
                0x24c
            } else {
                0x24b
            }
        };
        env.play_clip(clip);
        let disp = ld(env.displacement());
        let v = turn(r.q, disp);
        r.pos = st(add(ld(r.pos), sub(hip, v)));
        env.set_motion(6);
        env.set_state(0x14);
    }

    /// The course to the side: the board turned a quarter (the stance flipped when switch is not allowed), the
    /// switch clip or the other with the animation's stance set.
    fn get_up_sideways(r: &mut RagRider, w: &mut WipeRider, env: &mut impl WipeEnv, sw: u32, other: u32) -> u32 {
        let u = r.up;
        if !(w.x464 && w.x420 == 2) {
            env.rotate_about_axis(r, w, -HALF_PI, u);
            w.x1b4 = 1;
            w.x46e0 = f32::from_bits(0xc049_0fdb);
        } else {
            env.rotate_about_axis(r, w, HALF_PI, u);
            w.x1b4 = 0;
            w.x46e0 = 0.0;
        }
        if w.x420 == 2 {
            sw
        } else {
            w.x46d4 = 1;
            other
        }
    }
}

/// `Wipeout_PlayImpactClip` 0x10f9a0: the impact clip by which way the body meets the ground (board up, side,
/// forward against the ground's normal), curled (clip type 0x11: cmCI_) or stretched (cmI_); the sound first.
pub fn play_impact_clip(r: &RagRider, w: &WipeRider, env: &mut impl WipeEnv) {
    let curled = env.clip_type() == 0x11;
    let n = ld(r.ground);
    let zn = dot(ld(r.up), n);
    env.sfx_impact();
    let pick = |a: u32, b: u32| if curled { a } else { b };
    let clip = if zn < F(-0.7) {
        pick(0x2e4, 0x2eb)
    } else if F(0.7) < zn {
        pick(0x2e3, 0x2ea)
    } else {
        let sn = dot(ld(w.side), n);
        if F(0.7) < sn {
            pick(0x2e5, 0x2ec)
        } else if F(-0.7) <= sn {
            if F(0.0) < dot(ld(w.fwd), n) {
                pick(0x2e1, 0x2e8)
            } else {
                pick(0x2e2, 0x2e9)
            }
        } else {
            pick(0x2e6, 0x2ed)
        }
    };
    env.play_clip(clip);
}

/// `GetUpMotion_Update` 0x1102c8 (motion 6): the slide's step on the ground (`Boarder_ProbeGround` between), the
/// board's up turned to the ground's normal and its forward toward the course along it, the rider's orientation
/// slerped 1/12 of the way there; `Boarder_UpdateOrientation`; the world's collisions.
pub fn get_up_update(r: &mut RagRider, w: &mut WipeRider, env: &mut impl WipeEnv) {
    slide_move(r, w, env);
    env.probe_ground(r, w);
    hold_to_ground(r, w);
    let q1 = rotate_vec_to_vec([1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], r.ground);
    let n = ld(r.ground);
    let d = sub(ld(w.course8), ld(r.pos));
    let d = unit(sub(d, scale(n, dot(d, n))));
    let y = turn(q1, [F(0.0), F(1.0), F(0.0), F(0.0)]);
    let q2 = rotate_vec_to_vec(q1, st(y), st(d));
    r.q = slerp(f32::from_bits(0x3daa_aaab), r.q, q2);
    env.update_orientation(r);
    env.sync(w);
    env.collide_world(r, w);
}
