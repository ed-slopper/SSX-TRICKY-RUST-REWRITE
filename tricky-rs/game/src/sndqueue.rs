//! The audio system's pending-sound queue and the handlers that fill it (board row G10a2), checked in the function
//! runner (`tests/sndqueue.rs`). `Sfx_UpdateAir` 0x217d50 is the first handler.
//!
//! The queue is the audio system's `count` (+0x20) and a set of parallel arrays indexed by it (the entry being
//! `++count`): the 24-byte header copy (+0x24 points to the records), the emitter pointer (inline at +0x28), 0x38,
//! 0x3c (volume), 0x40 (the sound manager's slot), 0x44 (8 bytes each, inline), 0x64, 0x68 (the listener), 0x6c,
//! 0x70, 0x74 (16-byte rows), 0x78, 0x7c, 0xa40 (the kind), 0xa44 (the sound id).

use crate::jump::dot;

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
    fn play_pending(&mut self) -> u64;
}

/// The entry every handler builds (inlined in each): a default record with the `kind` (+0xa40) and sound `id`
/// (+0xa44); `volume` (clamped to the byte 0..127 into the record and kept in +0x3c) when the handler gives one;
/// `emitter` when the sound is the rider's (its pointer is stored and the listener asked for); then the slot the
/// manager gives. The caller plays the pending sounds (`update_air` and the others do it here, `play`).
pub fn queue_sound(q: &mut Queue, kind: u32, id: u32, volume: Option<i64>, emitter: Option<u32>, env: &mut dyn QueueEnv) {
    let i = queue_begin(q, kind, id, volume);
    let slot = env.get_slot(4);
    if let Some(e) = emitter {
        q.emitter[i] = e;
        q.a40[i] = slot;
        let l = env.rider_listener();
        q.a68[i] = l;
    } else {
        q.a40[i] = slot;
    }
    env.play_pending();
}

/// `SndSys_PushEntry` 0x2210e8: a new default entry (no kind or id yet): the new entry's index.
pub fn queue_push(q: &mut Queue) -> usize {
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
    q.a7c[i] = 1;
    i
}

/// The entry's volume: kept whole in +0x3c, clamped to the byte 0..127 into the record.
pub fn queue_volume(q: &mut Queue, i: usize, volume: i64) {
    q.a3c[i] = volume as u32;
    let v = volume as i32;
    q.record[i][0] = if v < 0 { 0 } else if v < 0x7f { v as u8 } else { 0x7f };
}

/// The part of the entry before the sound manager is asked for a slot: the new entry's index.
pub fn queue_begin(q: &mut Queue, kind: u32, id: u32, volume: Option<i64>) -> usize {
    let i = queue_push(q);
    q.kind[i] = kind;
    q.id[i] = id;
    if let Some(volume) = volume {
        queue_volume(q, i, volume);
    }
    i
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
    queue_sound(q, 1, id, Some(vol), Some(r.emitter), env);
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
    queue_sound(q, 1, surface * 8 + 2, Some(vol as i64), Some(r.emitter), env);
}

pub trait WipeoutEnv {
    fn replay_is_playing(&mut self) -> bool;
    fn listener_rider(&mut self, who: u32) -> i64;
    /// 0x2210c8 (the audio system, who).
    fn crowd_off(&mut self, who: u32);
    /// `CrowdSys_StartBoxCheer` 0x221088 (the audio system, level 0..2, who).
    fn crowd_level(&mut self, level: u32, who: u32);
    /// 0x233d38 (the audio system, the rider).
    fn wipeout_voice(&mut self);
}

/// `Sfx_OnWipeout` 0x2181e8: the crowd's cheer for the listener's rider after a wipeout, by `level` (f12).
pub fn on_wipeout(level: f32, rider_id: i64, r: &AirRider, env: &mut dyn WipeoutEnv) {
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
        env.wipeout_voice();
    }
}

pub trait WipeoutImpactEnv: QueueEnv {
    /// 0x235090 (no arguments).
    fn prelude(&mut self);
    fn find_rider_near(&mut self, radius: f32, emitter: u32) -> i64;
    fn eval_curve(&mut self, slot: u32, x: i32) -> i64;
    /// The audio system's random function (its object's virtual +0x24).
    fn random(&mut self) -> u32;
}

/// `Sfx_OnWipeoutImpact` 0x218328: the thud of a wipeout, volume from curve 7 of the rider's air speed, the sound one of
/// three picked at random (0x30..0x32) unless the rider's +0x310 is 3 or 4 (then 0x40).
pub fn on_wipeout_impact(q: &mut Queue, state_310: u32, r: &AirRider, env: &mut dyn WipeoutImpactEnv) {
    env.prelude();
    if r.flag_41c != 1 && env.find_rider_near(40.0, r.emitter) < 0 {
        return;
    }
    let curve = env.eval_curve(7, r.air as i32);
    let mut vol = if curve < 0x80 { curve } else { 0x7f };
    if vol <= -1 {
        vol = 0;
    }
    let id = if state_310.wrapping_sub(3) < 2 {
        0x40
    } else {
        let u = env.random();
        ((u & 0x7fff) * 3) / 0x7fff + 0x30
    };
    queue_sound(q, 0, id, Some(vol), Some(r.emitter), env);
}

pub trait RiderResetEnv: QueueEnv {
    /// 0x233a18 (no arguments).
    fn prelude(&mut self);
    /// `StreamSys_StopStream` (the audio system, 3).
    fn stop_stream(&mut self, which: u32);
    /// 0x21c4d0 (the audio system).
    fn after_a(&mut self);
    /// 0x21d5c8 (the audio system).
    fn after_b(&mut self);
}

/// `Sfx_OnRiderReset` 0x218eb0: a rider is put back on the course: the reset sound (id 0x7b, no emitter), then the
/// streams and voices of the old run are stopped.
pub fn on_rider_reset(q: &mut Queue, env: &mut dyn RiderResetEnv) {
    env.prelude();
    queue_sound(q, 0, 0x7b, None, None, env);
    env.stop_stream(3);
    env.after_a();
    env.after_b();
}

pub trait TrickBookEnv {
    /// `SndMgr_GetSlot` (the audio system, 4).
    fn get_slot(&mut self) -> u32;
    /// The global at 0x39ecf0.
    fn global(&mut self) -> u64;
    /// `SndSys_ScheduleEvent` (the audio system, `delay`, `global`, `slot`, 0, 0x5f, 0x7f, 0).
    fn schedule(&mut self, delay: u32, global: u64, slot: u32);
}

/// `Sfx_OnTrickBookDone` 0x2190f0: the trick book fanfare, three events 250, 500 and 750 ms later.
pub fn on_trick_book_done(env: &mut dyn TrickBookEnv) {
    let slot = env.get_slot();
    let g = env.global();
    env.schedule(0xfa, g, slot);
    let slot = env.get_slot();
    env.schedule(500, g, slot);
    let slot = env.get_slot();
    env.schedule(0x2ee, g, slot);
}

pub trait WallHitEnv: QueueEnv {
    /// 0x2154e0 (the audio system, the rider): non-zero while a wall hit sound is already queued.
    fn is_queued(&mut self) -> bool;
    /// `SndSys_SetPendingVolume` 0x226dd8 (40.0, -1.0, the audio system).
    fn set_pending_volume(&mut self, a: f32, b: f32);
    /// `Audio_IsNearerPlayer2` 0x20fb68 (the audio system, the emitter): which player's listener.
    fn is_nearer_player2(&mut self, emitter: u32) -> u32;
    /// `SndSys_QueueSounds` 0x215998 (the audio system, the rider, what `play_pending` returned, `word_80 + 0x32`, 0).
    fn queue_sounds(&mut self, played: u64, id: u32);
}

/// `Sfx_OnWallHit` 0x2162f0: a rider hits a wall: unless a hit sound is already queued, the scrape (id 0x31, no
/// volume of its own, the manager's pending volume set to 40, -1) for whichever player is nearer, played at once,
/// then the follow-up sounds queued after it. `word_80` is the audio system's +0x80.
pub fn on_wall_hit(q: &mut Queue, word_80: u32, emitter: u32, env: &mut dyn WallHitEnv) {
    if env.is_queued() {
        return;
    }
    let i = queue_begin(q, 0, 0x31, None);
    let slot = env.get_slot(4);
    q.a40[i] = slot;
    env.set_pending_volume(40.0, -1.0);
    let l = env.is_nearer_player2(emitter);
    q.a68[i] = l;
    q.emitter[i] = emitter;
    let played = env.play_pending();
    env.queue_sounds(played, word_80.wrapping_add(0x32));
}

pub trait CollisionEnv: QueueEnv {
    /// The audio system's random function (its object's virtual +0x24).
    fn random(&mut self) -> u32;
}

/// `Sfx_OnRiderCollision` 0x216960: two riders bump (one of them the player's): at most one bump sound per 30 frames
/// (`last_bump` is the global at 0x343118, `frame` the app's +0x10), id 0x5a or 0x5b by chance, at the first rider.
pub fn on_rider_collision(q: &mut Queue, last_bump: &mut i32, frame: i32, flag_a: u32, flag_b: u32, emitter: u32, env: &mut dyn CollisionEnv) {
    if (flag_a == 1 || flag_b == 1) && 0x1e < frame.wrapping_sub(*last_bump) {
        let u = env.random();
        queue_sound(q, 0, ((u & 0x7fff) << 1) / 0x7fff + 0x5a, None, Some(emitter), env);
        *last_bump = frame;
    }
}

pub trait WipeoutLoopEnv {
    /// 0x20c2a0 (0.75, the global at 0x343050, the handle, 1): stop a voice with a fade.
    fn stop_voice(&mut self, fade: f32, handle: u32);
}

/// `Sfx_StopWipeoutLoop` 0x218b20: stop the rider's wipeout loop voice (the handle at the rider's +0x5ae8, negative
/// for none) and forget it.
pub fn stop_wipeout_loop(handle: &mut i32, env: &mut dyn WipeoutLoopEnv) {
    if -1 < *handle {
        env.stop_voice(f32::from_bits(0x3f40_0000), *handle as u32);
        *handle = -1;
    }
}

/// `Sfx_PlayMenuSelect` 0x2138f0: the menu click (id 0x70, byte 1 of the record 0x5c); the sound it started is kept in
/// the audio system's +0x6e48.
pub fn play_menu_select(q: &mut Queue, w6e48: &mut u32, env: &mut dyn QueueEnv) {
    let i = queue_begin(q, 0, 0x70, None);
    let slot = env.get_slot(4);
    q.a40[i] = slot;
    q.record[i][1] = 0x5c;
    *w6e48 = env.play_pending() as u32;
}

pub trait BoostEnv: QueueEnv {
    fn replay_is_playing(&mut self) -> bool;
    /// `SndSys_CancelEvents` 0x222df0 (the audio system, the global at 0x39eb58).
    fn cancel_events(&mut self);
}

/// `Sfx_UpdateBoost` 0x2192a8: each tick of a player's boost: the earlier boost events are cancelled, and on the
/// first tick (`w6e3c` 0) the boost start sound (id 0x78 at over two thirds of the boost bar, 0x79 over one third, else
/// 0x7a) is queued and its handle kept in `w6e38`; with `extra` (the caller's third argument) a second, 0x30 lower,
/// sound goes with it every tick. `w6e3c` counts the ticks.
pub fn update_boost(q: &mut Queue, w6e38: &mut u32, w6e3c: &mut u32, flag_41c: u32, boost: f32, extra: bool, env: &mut dyn BoostEnv) {
    if env.replay_is_playing() || flag_41c != 1 {
        return;
    }
    env.cancel_events();
    let mut id = 0x7a;
    if f32::from_bits(0x3f2a_7e78) < boost {
        id = 0x78;
    } else if f32::from_bits(0x3eaa_d18b) < boost {
        id = 0x79;
    }
    if *w6e3c == 0 {
        let i = queue_begin(q, 0, id, None);
        let slot = env.get_slot(4);
        q.a40[i] = slot;
        *w6e38 = env.play_pending() as u32;
    }
    if extra {
        let i = queue_begin(q, 0, id - 0x30, None);
        let slot = env.get_slot(4);
        q.a40[i] = slot;
        env.play_pending();
    }
    *w6e3c = w6e3c.wrapping_add(1);
}

pub trait ScriptSoundEnv: QueueEnv {
    /// 0x2154e0 (the audio system, the node): non-zero while its sound is already queued.
    fn is_queued(&mut self) -> bool;
    /// The audio system's virtual +0x6c (the audio system, 2, 0x7a): the special play for sound 0x7a.
    fn play_special(&mut self) -> u64;
    /// `SndSys_SetPendingVolume` 0x226dd8 (31.0, -1.0, the audio system).
    fn set_pending_volume(&mut self, a: f32, b: f32);
    /// `Audio_IsNearerPlayer2` 0x20fb68 (the audio system, the position): which player's listener.
    fn is_nearer_player2(&mut self, pos: [u32; 4]) -> u32;
    /// `SndSys_QueueSounds` 0x215998 (the audio system, the node, what was played, `word_80 + 0x32`, 0).
    fn queue_sounds(&mut self, played: u64, id: u32);
}

/// `Sfx_ScriptSoundPlay` 0x2165c8: a trigger script plays sound `id` at a node's position `pos` (its +0x30..0x40): unless the
/// node's sound is already queued, either (in game mode 4, for sound 0x7a) a special play, or a queue entry
/// (kind 2, flag +0x38 = 1) whose emitter is a copy of the position on the caller's stack (`local`), with the
/// pending volume set to 31, -1 and the listener of the nearer player; then the follow-up sounds are queued.
pub fn script_sound_play(q: &mut Queue, word_80: u32, mode_is_4: bool, id: i64, pos: [u32; 4], local: u32, env: &mut dyn ScriptSoundEnv) {
    if env.is_queued() {
        return;
    }
    if mode_is_4 && id == 0x7a {
        let played = env.play_special();
        env.queue_sounds(played, word_80.wrapping_add(0x32));
        return;
    }
    let i = queue_begin(q, 2, id as u32, None);
    let slot = env.get_slot(4);
    q.a40[i] = slot;
    q.a38[i] = 1;
    q.emitter[i] = local;
    env.set_pending_volume(f32::from_bits(0x41f0_0000), -1.0);
    let l = env.is_nearer_player2(pos);
    q.a68[i] = l;
    let played = env.play_pending();
    env.queue_sounds(played, word_80.wrapping_add(0x32));
}


pub trait InstanceCollisionEnv {
    /// `Audio_FindRiderNear` 0x20f9b0 (40.0, the audio system, the rider's position): negative when none.
    fn find_rider_near(&mut self, radius: f32) -> i64;
    /// 0x2154e0 (the audio system, the instance): non-zero while its sound is already queued.
    fn is_queued(&mut self) -> bool;
    /// `Sfx_EvalCurve` 0x20d928 (2, the speed as an integer).
    fn eval_curve(&mut self, slot: u32, x: i32) -> i64;
    /// 0x22d508 (the object at 0x34370c, the instance, its sound's word, the rider, the volume): the sound started,
    /// negative for none.
    fn start_instance_sound(&mut self, sound: u32, volume: i64) -> i64;
    /// `SndSys_QueueSounds` 0x215998 (the audio system, the instance, what was started, `word_80 + 0x32`, 0).
    fn queue_sounds(&mut self, started: i64, id: u32);
}

/// `Sfx_InstanceCollision` 0x216c50: a rider hits a world instance that has a sound (`sound`, the word its +0xf4
/// record points to; `None` for no record): the volume follows the rider's speed (curve 2, 0..127), and the sound is
/// started and its follow-ups queued unless a rider is out of earshot or the instance's sound is already queued.
pub fn instance_collision(vel: [f32; 4], sound: Option<u32>, word_80: u32, env: &mut dyn InstanceCollisionEnv) {
    let Some(sound) = sound else { return };
    if env.find_rider_near(40.0) < 0 {
        return;
    }
    if env.is_queued() {
        return;
    }
    let speed = dot(vel, vel).sqrt();
    let curve = env.eval_curve(2, speed as i32);
    let mut vol = if curve < 0x80 { curve } else { 0x7f };
    if vol <= -1 {
        vol = 0;
    }
    let started = env.start_instance_sound(sound, vol);
    if started < 0 {
        return;
    }
    env.queue_sounds(started, word_80.wrapping_add(0x32));
}
