//! The audio system's pending-sound queue and the handlers that fill it (board row G10a2), checked in the function
//! runner (`tests/sndqueue.rs`). `Sfx_UpdateAir` 0x217d50 is the first handler.
//!
//! The queue is the audio system's `count` (+0x20) and a set of parallel arrays indexed by it (the entry being
//! `++count`): the 24-byte header copy (+0x24 points to the records), the emitter pointer (inline at +0x28), 0x38,
//! 0x3c (volume), 0x40 (the sound manager's slot), 0x44 (8 bytes each, inline), 0x64, 0x68 (the listener), 0x6c,
//! 0x70, 0x74 (16-byte rows), 0x78, 0x7c, 0xa40 (the kind), 0xa44 (the sound id).

/// The queue arrays, each indexed by the entry.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Queue {
    /// +0x08..+0x20 of the audio system, copied into each entry's record.
    pub header: [u8; 24],
    /// +0x20.
    pub count: i32,
    pub record: Vec<[u8; 24]>,
    pub emitter: Vec<u32>,
    pub a38: Vec<u32>,
    pub a3c: Vec<u32>,
    pub a40: Vec<u32>,
    pub a44: Vec<[u32; 2]>,
    pub a64: Vec<u32>,
    pub a68: Vec<u32>,
    pub a6c: Vec<u32>,
    pub a70: Vec<u32>,
    pub a74: Vec<[u32; 4]>,
    pub a78: Vec<u32>,
    pub a7c: Vec<u32>,
    pub kind: Vec<u32>,
    pub id: Vec<u32>,
}

impl Queue {
    pub fn with_capacity(n: usize) -> Queue {
        Queue {
            header: [0; 24],
            count: 0,
            record: vec![[0; 24]; n],
            emitter: vec![0; n],
            a38: vec![0; n],
            a3c: vec![0; n],
            a40: vec![0; n],
            a44: vec![[0; 2]; n],
            a64: vec![0; n],
            a68: vec![0; n],
            a6c: vec![0; n],
            a70: vec![0; n],
            a74: vec![[0; 4]; n],
            a78: vec![0; n],
            a7c: vec![0; n],
            kind: vec![0; n],
            id: vec![0; n],
        }
    }
}

/// What the queue's users call out to.
pub trait QueueEnv {
    /// `SndMgr_GetSlot` 0x211f38 (the audio system, 4).
    fn get_slot(&mut self, which: u32) -> u32;
    /// `Audio_GetRiderListener` 0x20fc60 (the audio system, the rider).
    fn rider_listener(&mut self) -> u32;
    /// `SndSys_PlayPending` 0x20c948.
    fn play_pending(&mut self);
}

/// The entry every handler builds (inlined in each): a default record with `volume` (clamped to the byte 0..127),
/// the `kind` (+0xa40) and sound `id` (+0xa44), the `emitter` pointer and the slot and listener the manager gives,
/// then plays the pending sounds.
pub fn queue_sound(q: &mut Queue, kind: u32, id: u32, volume: i64, emitter: u32, env: &mut dyn QueueEnv) {
    q.count += 1;
    let i = q.count as usize;
    q.record[i] = q.header;
    q.emitter[i] = 0;
    q.a38[i] = 0;
    q.a3c[i] = q.record[i][0] as i8 as i32 as u32;
    q.a40[i] = 0;
    q.a44[i] = [0; 2];
    q.a68[i] = 0;
    q.a6c[i] = 0;
    q.a70[i] = 0;
    q.a74[i][1] = 0;
    q.a74[i][0] = 100;
    q.a64[i] = 0;
    q.a74[i][2] = 0x5a;
    q.a74[i][3] = 0x32;
    q.a78[i] = 0;
    q.kind[i] = kind;
    q.a7c[i] = 1;
    q.id[i] = id;
    q.a3c[i] = volume as u32;
    let v = volume as i32;
    q.record[i][0] = if v < 0 { 0 } else if v < 0x7f { v as u8 } else { 0x7f };
    let slot = env.get_slot(4);
    q.emitter[i] = emitter;
    q.a40[i] = slot;
    let l = env.rider_listener();
    q.a68[i] = l;
    env.play_pending();
}

/// The rider values `Sfx_UpdateAir` reads.
#[derive(Clone, Copy, Debug)]
pub struct AirRider {
    /// +0x41c: 1 for a rider the player's (the human) own.
    pub flag_41c: u32,
    /// +0x38: the player number.
    pub player: u32,
    /// +0x798: the rider's air speed (as an integer to the curve).
    pub air: f32,
    /// The rider's address + 0x140 (the emitter).
    pub emitter: u32,
}

pub trait AirEnv: QueueEnv {
    fn replay_is_playing(&mut self) -> bool;
    /// `Audio_GetListenerRider` 0x20ed20 (the audio system, the player or -1): the listener's rider.
    fn listener_rider(&mut self, who: u32) -> i64;
    /// 0x2210c8 (the audio system, who).
    fn crowd_off(&mut self, who: u32);
    /// 0x221068 (the audio system, level 0..2, who).
    fn crowd_level(&mut self, level: u32, who: u32);
    /// `Audio_FindRiderNear` 0x20f9b0 (40.0, the audio system, the emitter): negative when none.
    fn find_rider_near(&mut self, radius: f32, emitter: u32) -> i64;
    /// `Sfx_EvalCurve` 0x20d928 (the curve slot 0 or 1, the speed as an integer).
    fn eval_curve(&mut self, slot: u32, x: i32) -> i64;
    /// `Sfx_SurfaceSoundType` 0x20f908 (the audio system, the rider).
    fn surface_sound_type(&mut self) -> u32;
}

/// `Sfx_UpdateAir` 0x217d50: keep the air whoosh of a rider going, and the crowd's level for the listener.
/// `rider` is the rider's address as the listener returns it, `level` the crowd's excitement (f12).
pub fn update_air(q: &mut Queue, level: f32, rider_id: i64, r: &AirRider, env: &mut dyn AirEnv) {
    let mut who = 0xffff_ffffu32;
    if !env.replay_is_playing() {
        if r.flag_41c == 1 {
            who = r.player;
        }
    } else {
        who = 0;
    }
    if env.listener_rider(who) == rider_id {
        if level <= 0.0 {
            env.crowd_off(who);
        } else if level < 0.25 {
            env.crowd_level(0, who);
        } else if level < f32::from_bits(0x3f26_6666) {
            env.crowd_level(1, who);
        } else {
            env.crowd_level(2, who);
        }
    }
    if r.flag_41c != 1 && env.find_rider_near(40.0, r.emitter) < 0 {
        return;
    }
    let own = r.flag_41c == 1;
    let x = r.air as i32;
    let curve = env.eval_curve((!own) as u32, x);
    let mut vol = if curve < 0x80 { curve } else { 0x7f };
    if vol <= -1 {
        vol = 0;
    }
    let surface = env.surface_sound_type();
    let id = surface * 8 + if own { 1 } else { 5 };
    queue_sound(q, 1, id, vol, r.emitter, env);
}

/// The speed fraction of a rider's impact for the volume: `impact * 127 / 1000` through a float.
fn impact_volume(impact: i32) -> i32 {
    let q = (impact << 7).wrapping_sub(impact) / 1000;
    (q as f32) as i32
}

pub trait AirEnterEnv: QueueEnv {
    /// 0x233950 (no arguments), called when the rider's +0x57f4 is above 5.
    fn big_landing(&mut self);
    /// 0x235170 (the audio system, the rider, the caller's third argument).
    fn on_air_enter_prelude(&mut self);
    fn replay_is_playing(&mut self) -> bool;
    fn listener_rider(&mut self, who: u32) -> i64;
    /// 0x2210a8 (the audio system, who), when +0x57f4 is above 2.
    fn crowd_stop(&mut self, who: u32);
    /// 0x225768 (the audio system, 0), when +0x57f4 is above 1.5.
    fn crowd_clear(&mut self);
    /// `Sfx_ImpactSpeed` 0x21a6a8 (the rider).
    fn impact_speed(&mut self) -> i32;
    fn surface_sound_type(&mut self) -> u32;
}

/// `Sfx_OnAirEnter` 0x2174f8: a rider leaves the ground: the crowd reacts to how high it went (`height`, the rider's
/// +0x57f4), and for the listener's rider a landing-out sound is queued whose volume follows the impact speed (at
/// least 64, at most 127: the original evaluates `clamp` as nested conditionals that come to this).
pub fn on_air_enter(q: &mut Queue, height: f32, rider_id: i64, r: &AirRider, env: &mut dyn AirEnterEnv) {
    if 5.0 < height {
        env.big_landing();
    }
    env.on_air_enter_prelude();
    let mut who = 0xffff_ffffu32;
    if !env.replay_is_playing() {
        if r.flag_41c == 1 {
            who = r.player;
        }
    } else {
        who = 0;
    }
    if env.listener_rider(who) != rider_id {
        return;
    }
    if 2.0 < height {
        env.crowd_stop(who);
    }
    if 1.5 < height {
        env.crowd_clear();
    }
    let x = impact_volume(env.impact_speed());
    let vol = if x < 0x40 { 0x40 } else if x < 0x7f { x } else { 0x7f };
    let surface = env.surface_sound_type();
    queue_sound(q, 1, surface * 8 + 2, vol as i64, r.emitter, env);
}
