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

pub trait PlacingChangeEnv: QueueEnv {
    /// `Audio_GetListenerRider` 0x20ed20 (the audio system, −1): the rider the sound follows.
    fn listener(&mut self) -> u32;
    /// `SndSys_IsQueued` 0x2154e0 (the audio system, 0).
    fn is_queued(&mut self) -> bool;
    /// `SndSys_QueueSounds` 0x215998 (the audio system, 0, what `play_pending` returned, `word_80 + 200`, 0).
    fn queue_sounds(&mut self, played: u64, id: u32);
    /// `Speech_OnPlacingChange` 0x2347c8 (the audio system, the rider who passed, the rider passed).
    fn speech_placing(&mut self, a: u32, b: u32);
}

/// `Sfx_OnPlacingChange` 0x219a48 (the audio system, the rider, the rider behind (`next`), the rider ahead (`prev`),
/// the old place, the new): for a local player's rider (+0x41c is 1) or the listener's, taking the lead (`new` 0)
/// queues sound 0x6d (no volume of its own) unless one is queued, played at once with the follow-ups queued after
/// it; then the passing line: the rider over the one now behind when it moved up, else the one ahead over it.
pub fn on_placing_change(q: &mut Queue, word_80: u32, rider: u32, flag_41c: u32, next: u32, prev: u32, old: i64, new: i64, env: &mut dyn PlacingChangeEnv) {
    if flag_41c != 1 && env.listener() != rider {
        return;
    }
    if new == 0 && !env.is_queued() {
        let i = queue_begin(q, 0, 0x6d, None);
        q.a40[i] = env.get_slot(4);
        let played = env.play_pending();
        env.queue_sounds(played, word_80.wrapping_add(200));
    }
    if new < old {
        env.speech_placing(rider, next);
    } else {
        env.speech_placing(prev, rider);
    }
}

pub trait CheerEnv {
    /// `Audio_GetListenerRider` 0x20ed20 (the audio system, the rider's player number).
    fn listener_rider(&mut self, who: u32) -> u32;
    /// `Replay_IsPlaying` 0x21bfd0.
    fn replay_is_playing(&mut self) -> bool;
    /// `Audio_PlayCrowdCheer` 0x221068 (`*0x343050`, 2, who): when the crowd (+0x372c) is there,
    /// `CrowdBox_PlayCheer` 0x2203a8 on its box `who` (0x190 bytes each).
    fn play_crowd_cheer(&mut self, level: u32, who: u32);
}

/// `Audio_CheerForRider` 0x21e528 (the audio system, the rider): when the rider is its player's listener, the
/// crowd's level-2 cheer on that player's box (the first, in a replay).
pub fn cheer_for_rider(rider: u32, player: u32, env: &mut dyn CheerEnv) {
    if env.listener_rider(player) == rider {
        let who = if env.replay_is_playing() { 0 } else { player };
        env.play_crowd_cheer(2, who);
    }
}

/// What the ring of queued sounds asks of the audio system.
pub trait RingEnv {
    /// `BankMonitor_Find` 0x20c200 (the audio system, the handle, 1): whether the sound still plays.
    fn find(&mut self, handle: u32) -> bool;
    /// `SndSys_StopBankSound` 0x20c2a0 (0, the audio system, the handle, 1).
    fn stop(&mut self, handle: u32);
}

/// The audio system's ring of queued sounds: records of four words (the owner, the sound's handle, the tick it ends
/// at, whether to stop it then) in a ring of words (+0x6e24 its size, 0x50 from `cBXAudio::cBXAudio` 0x20db18;
/// +0x6e28 the words, filled with 0xdeadc0ed; +0x6e2c the head, +0x6e30 the tail). One word is always left free.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundRing {
    pub words: Vec<u32>,
    pub head: i32,
    pub tail: i32,
}

impl Default for SoundRing {
    fn default() -> Self {
        SoundRing { words: vec![0xdead_c0ed; 0x50], head: 0, tail: 0 }
    }
}

impl SoundRing {
    fn size(&self) -> i32 {
        self.words.len() as i32
    }
    fn used(&self) -> i32 {
        (self.head - self.tail + self.size()) % self.size()
    }
    fn skip(&mut self) {
        if self.head != self.tail {
            self.tail += 1;
            if self.size() <= self.tail {
                self.tail = 0;
            }
        }
    }
    fn pop(&mut self) -> u32 {
        if self.head == self.tail {
            return u32::MAX;
        }
        let v = self.words[self.tail as usize];
        self.skip();
        v
    }
    fn push(&mut self, v: u32) {
        if (self.head + 1) % self.size() != self.tail {
            self.words[self.head as usize] = v;
            self.head += 1;
            if self.size() <= self.head {
                self.head = 0;
            }
        }
    }
    /// The `i`th word from the tail, −1 past the head.
    fn peek(&self, i: i32) -> u32 {
        if self.head == self.tail || self.used() <= i {
            return u32::MAX;
        }
        self.words[((self.tail + i) % self.size()) as usize]
    }

    /// `SndSys_QueueSounds` 0x215998 (the audio system, the owner, the handle, the end tick, the flag): the record
    /// added; a full ring first drops its oldest record, stopping that sound.
    pub fn queue_sounds(&mut self, owner: u32, handle: u32, end: u32, flag: u32, env: &mut dyn RingEnv) {
        if (self.head + 1) % self.size() == self.tail {
            self.skip();
            let h = self.pop();
            env.stop(h);
            self.skip();
            self.skip();
        }
        for v in [owner, handle, end, flag] {
            self.push(v);
        }
    }

    /// `SndSys_IsQueued` 0x2154e0 (the audio system, the owner): whether a record of the owner holds a sound still
    /// playing (or a handle of 0).
    pub fn is_queued(&self, owner: u32, env: &mut dyn RingEnv) -> bool {
        let n = self.used();
        let mut i = 0;
        while i < n {
            if self.peek(i) == owner {
                let h = self.peek(i + 1);
                if h == 0 || env.find(h) {
                    return true;
                }
            }
            i += 4;
        }
        false
    }

    /// `SndSys_UpdateQueuedSounds` 0x215658 (the audio system; `now` its tick, +0x80), from `Audio_InGameUpdate`:
    /// each record is taken off; one whose handle is 0 or still playing goes back while the tick is before its end,
    /// else, with its flag set, its sound is stopped.
    pub fn update(&mut self, now: i32, env: &mut dyn RingEnv) {
        let n = self.used() >> 2;
        for _ in 0..n {
            let owner = self.pop();
            let handle = self.pop() as i32;
            let end = self.pop() as i32;
            let flag = self.pop();
            if handle >= 0 && (handle == 0 || env.find(handle as u32)) {
                if now < end {
                    for v in [owner, handle as u32, end as u32, flag] {
                        self.push(v);
                    }
                } else if flag != 0 {
                    env.stop(handle as u32);
                }
            }
        }
    }
}

/// `DAT_00343998`, the radius (m) the announcer's rider lines are heard within (`Audio_FindRiderNear`'s first argument
/// in `speech`)
pub const SPEECH_NEAR: f32 = 30.0;

/// `Audio_FindRiderNear` 0x20f9b0 (f12 the radius in metres, the audio system, a position): the first view
/// (`Audio_RiderCount` 0x21c078: the race's +0x298, 1 with no race; `Audio_GetRiderPos` 0x21c010: the race's
/// +0xb0 + 0x80 × i) whose position is within the radius (×100 cm, squared) of `at`, −1 for none. `count` and `view`
/// stand for the two calls (asked again each turn, as the original does)
pub fn find_rider_near(radius: f32, count: &mut dyn FnMut() -> i32, view: &mut dyn FnMut(i32) -> [f32; 3], at: [f32; 3]) -> i32 {
    let r = radius * 100.0;
    let lim = r * r;
    let mut i = 0;
    while i < count() {
        let p = view(i);
        let (dx, dy, dz) = (p[0] - at[0], p[1] - at[1], p[2] - at[2]);
        if dx * dx + dy * dy + dz * dz < lim {
            return i;
        }
        i += 1;
    }
    -1
}

/// The audio system's event sound (+0x7420 pending, +0x7424 its handle, +0x7428 its kind): a loop the rider's event
/// bits ask for (`cBoarder::vf10` 0x11a348: bits 0xe, 0x17, 0x18 → kinds 0, 1, 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSound {
    pub pending: u32,
    pub handle: u32,
    pub kind: u32,
}

impl Default for EventSound {
    /// `cBXAudio::cBXAudio` 0x20db18: the handle −1 (0x20dbdc), none pending (0x20dc18)
    fn default() -> Self {
        EventSound { pending: 0, handle: u32::MAX, kind: 0 }
    }
}

/// The banks the event sound loads: kind 0 (and any kind over 2) the tunnel's (0x39f1c0), kind 1 the river's (0x39f1d8)
pub const EVENT_BANKS: [&str; 2] = ["|data\\audio\\Tunnel1.bnk", "|data\\audio\\RiverSmall.bnk"];

pub trait EventSoundEnv: QueueEnv {
    /// `Audio_FindBankName` 0x21a640 then `Audio_LoadAmbientBank` 0x21a548 (the audio system, the name found): 1 when
    /// the bank is in
    fn load_ambient_bank(&mut self, name: &str) -> i64;
    /// `SndSys_StopBankSound` 0x20c2a0(f12 2.0, the audio object, the handle, 1)
    fn stop_bank_sound(&mut self, handle: u32);
}

impl EventSound {
    /// `Audio_QueueEventSound` 0x21d0b0: the kind kept, the sound pending
    pub fn queue(&mut self, kind: u32) {
        self.kind = kind;
        self.pending = 1;
    }

    /// `Audio_StopEventSound` 0x21d5c8: a handle kept (not negative) is stopped; none kept after
    pub fn stop(&mut self, env: &mut dyn EventSoundEnv) {
        if (self.handle as i32) >= 0 {
            env.stop_bank_sound(self.handle);
        }
        self.handle = u32::MAX;
    }

    /// `Audio_PlayEventSound` 0x21d0c0 (from `Audio_InGameUpdate` 0x20ee90 at 0x20ef24): a pending sound played.
    /// Kinds 0 and 1 load their bank (nothing more when it is not in, still pending) and play its sound 0 as kind 6
    /// at volume 0x23; kind 2 plays sound 9 as kind 2. The handle is kept and nothing is pending after.
    pub fn update(&mut self, q: &mut Queue, env: &mut dyn EventSoundEnv) {
        if self.pending == 0 {
            return;
        }
        if self.kind == 2 {
            queue_begin(q, 2, 9, None);
        } else {
            let name = EVENT_BANKS[(self.kind == 1) as usize];
            if env.load_ambient_bank(name) != 1 {
                return;
            }
            queue_begin(q, 6, 0, Some(0x23));
        }
        let i = q.count as usize;
        q.a40[i] = env.get_slot(4);
        self.handle = env.play_pending() as u32;
        self.pending = 0;
    }
}

/// The audio system's ambient ("swap") banks: the names `Audio_LoadInGameConfig` 0x211098 lists from BANKS.INF's
/// `SWAP` keys (+0x38e0, 0x40 bytes each, at most 200; their count +0x38dc) and a loaded flag for each (+0x6ae0).
/// One of them at a time sits in the bank manager's slot 6.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmbientBanks {
    pub names: Vec<Vec<u8>>,
    pub loaded: [u32; 200],
}

impl Default for AmbientBanks {
    fn default() -> Self {
        AmbientBanks { names: Vec::new(), loaded: [0; 200] }
    }
}

/// The bank manager's slot 6 (the system +0xa3c, 0x60 bytes a slot).
pub trait AmbientEnv {
    /// the slot's state (+0x240: 0 empty, 1 in, 2 loading)
    fn state(&mut self) -> i32;
    /// `BankManager_UnloadBank` 0x20a508 (the system, 6)
    fn unload(&mut self);
    /// `BankManager_LoadBank` 0x20a520 (the system, 6, the name, 0x10)
    fn load(&mut self, name: &[u8]);
}

impl AmbientBanks {
    /// `Audio_FindBankName` 0x21a640: the index of the list entry equal to `name` (`strcmp` over all 200, whatever the
    /// count), −1 when none
    pub fn find(&self, name: &[u8]) -> i32 {
        (0..200).find(|&i| self.names.get(i).map_or(name.is_empty(), |n| n.as_slice() == name)).map_or(-1, |i| i as i32)
    }

    /// `Audio_ClearAmbientLoaded` 0x21a468: every flag 0
    pub fn clear_loaded(&mut self) {
        self.loaded = [0; 200];
    }

    /// `Audio_LoadAmbientBank` 0x21a548 (the system, the index): 1 when that bank is in. Not marked loaded: slot 6
    /// emptied first (flags cleared, unloaded) unless it is loading (−1 then). Then slot 6 in: 1; empty: the bank
    /// marked and its load asked for, −1; loading: −1. (An index of −1 would read +0x6adc, the word before the flags;
    /// every course of the disc's BANKS.INF lists the event sound's banks.)
    pub fn load(&mut self, index: usize, env: &mut dyn AmbientEnv) -> i64 {
        if self.loaded[index] == 0 {
            let s = env.state();
            if (0..2).contains(&s) {
                self.clear_loaded();
                env.unload();
            } else if s == 2 {
                return -1;
            }
        }
        match env.state() {
            1 => 1,
            0 => {
                self.loaded[index] = 1;
                let name = self.names.get(index).cloned().unwrap_or_default();
                env.load(&name);
                -1
            }
            _ => -1,
        }
    }
}

/// `SndPlayOpts_Init` 0x2d4a60: a play record's defaults (volume 0x7f, pan 0x40, 0x7f, root 0x3c, 0x7f, then the
/// half-words 0, 0, 0x1000, 0x1000, 0x1000, 0, 0xffff, 0; bytes 5..7 left as they were). `cSndPlayOpts::cSndPlayOpts`
/// 0x226848 (built under every audio system, from `cMonitor::cMonitor` 0x222b10) runs it on the stack's first record
/// and copies that into the system's header (+8, 0x18 bytes), the record `queue_begin` copies into each sound.
pub fn play_opts_init(r: &mut [u8; 24]) {
    r[..5].copy_from_slice(&[0x7f, 0x40, 0x7f, 0x3c, 0x7f]);
    for (k, h) in [0u16, 0, 0x1000, 0x1000, 0x1000, 0, 0xffff, 0].into_iter().enumerate() {
        r[8 + 2 * k..10 + 2 * k].copy_from_slice(&h.to_le_bytes());
    }
}

/// The system's header (+8) as `cSndPlayOpts::cSndPlayOpts` 0x226848 leaves it: [`play_opts_init`] on a record the
/// allocator filled with 0xdeadc0ed (`EAMem_Set`), so bytes 5..7 are 0xc0, 0xad, 0xde.
pub fn play_opts_header() -> [u8; 24] {
    let mut r = [0u8; 24];
    for (k, b) in r.iter_mut().enumerate() {
        *b = 0xdead_c0ed_u32.to_le_bytes()[k % 4];
    }
    play_opts_init(&mut r);
    r
}

// ---- The rider's sound loops (board row E1b2g7d5g) ----

/// The rider values the loops' starts read and write.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopRider {
    /// The rider's address (the instance's owner, +0x64; +0x140 is the emitter).
    pub addr: u32,
    /// +0x41c: 1 for the player's own rider.
    pub flag_41c: u32,
    /// +0x424: the rider's motion (2 air, 3 a rail).
    pub motion: u32,
    /// +0x788 / +0x78c: the motion the loops last saw.
    pub w788: u32,
    pub w78c: u32,
    /// +0x790 / +0x794: the surface sound type the loops last saw.
    pub w790: u32,
    pub w794: u32,
    /// +0x5ae4: the sound flag (the loops silent while set).
    pub w5ae4: u32,
    /// +0x5af0: the loops' fade-in count (`Boarder_TickSfxFadeIn` 0x11f468 counts it down).
    pub w5af0: i32,
}

pub trait RiderLoopEnv: QueueEnv {
    /// `Sfx_SurfaceSoundType` 0x20f908 (the audio system, the rider).
    fn surface_sound_type(&mut self, rider: u32) -> u32;
    /// `SndSys_SetPendingVolume` 0x226dd8 (the volume, -1.0, the audio system).
    fn set_pending_volume(&mut self, a: f32, b: f32);
}

/// The update callbacks the loops' entries carry (+0x44: a pointer to member, no this-adjust, no vtable index, the
/// function): `Sfx_UpdateAirLoop` 0x21b440, `Sfx_UpdateGlideLoop` 0x21b508, `Sfx_UpdateAiGlideLoop` 0x21b628,
/// `Sfx_UpdateCarveLoop` 0x21b718.
pub const AIR_LOOP_UPDATE: u32 = 0x21b440;
pub const GLIDE_LOOP_UPDATE: u32 = 0x21b508;
pub const AI_GLIDE_LOOP_UPDATE: u32 = 0x21b628;
pub const CARVE_LOOP_UPDATE: u32 = 0x21b718;

/// One loop's entry: kind 1, the id, volume 0, the slot, the rider's emitter and update callback, the rider as the
/// owner, the listener when `listen`, +0x7c cleared, the volume `vol` (-1.0) and played (the starts write these in
/// different orders; only the calls' order shows).
fn start_loop(q: &mut Queue, r: &LoopRider, id: u32, update: u32, listen: bool, vol: f32, env: &mut dyn RiderLoopEnv) {
    let i = queue_begin(q, 1, id, Some(0));
    q.a40[i] = env.get_slot(4);
    q.emitter[i] = r.addr.wrapping_add(0x140);
    q.a44[i] = [0xffff_0000, update];
    q.a64[i] = r.addr;
    if listen {
        q.a68[i] = env.rider_listener();
    }
    q.a7c[i] = 0;
    env.set_pending_volume(vol, -1.0);
    env.play_pending();
}

/// `Sfx_StartAirLoop` 0x20fce0 (the audio system, the rider): the player's rider's air loop, sound 0.
pub fn start_air_loop(q: &mut Queue, r: &LoopRider, env: &mut dyn RiderLoopEnv) {
    if r.flag_41c == 1 {
        start_loop(q, r, 0, AIR_LOOP_UPDATE, true, 40.0, env);
    }
}

/// `Sfx_StartGlideLoop` 0x210048 (the audio system, the rider, the surface sound type): the glide loop, sound
/// `surface * 8 + 4` heard by the rider's listener for the player's rider, else `surface * 8 + 6` (the AI's, louder,
/// no listener).
pub fn start_glide_loop(q: &mut Queue, r: &LoopRider, surface: u32, env: &mut dyn RiderLoopEnv) {
    if r.flag_41c == 1 {
        start_loop(q, r, surface.wrapping_mul(8).wrapping_add(4), GLIDE_LOOP_UPDATE, true, 40.0, env);
    } else {
        start_loop(q, r, surface.wrapping_mul(8).wrapping_add(6), AI_GLIDE_LOOP_UPDATE, false, 50.0, env);
    }
}

/// `Sfx_StartCarveLoop` 0x2106c0 (the audio system, the rider, the surface sound type): the player's rider's carve
/// loop, sound `surface * 8 + 3`.
pub fn start_carve_loop(q: &mut Queue, r: &LoopRider, surface: u32, env: &mut dyn RiderLoopEnv) {
    if r.flag_41c == 1 {
        start_loop(q, r, surface.wrapping_mul(8).wrapping_add(3), CARVE_LOOP_UPDATE, true, 40.0, env);
    }
}

/// `Sfx_StartRiderLoops` 0x210ad0 (the audio system, the rider): the motion and surface remembered, the three loops
/// started.
pub fn start_rider_loops(q: &mut Queue, r: &mut LoopRider, env: &mut dyn RiderLoopEnv) {
    let motion = r.motion;
    let surface = env.surface_sound_type(r.addr);
    r.w78c = motion;
    r.w788 = motion;
    r.w790 = surface;
    r.w794 = surface;
    start_air_loop(q, r, env);
    start_glide_loop(q, r, surface, env);
    start_carve_loop(q, r, surface, env);
}

pub trait InitRiderSoundsEnv: RiderLoopEnv {
    /// `Audio_CancelRaceEvents` 0x210b50.
    fn cancel_race_events(&mut self);
    /// The race's rider count (`gApp` 0x337c58, +0x730 → +0x1c → +0x88), read before each rider.
    fn rider_count(&mut self) -> i32;
    /// `Race_GetRider` (the race, the index): the rider's address.
    fn get_rider(&mut self, i: i32) -> u32;
}

/// `Audio_InitRiderSounds` 0x210a38 (the audio system): the race's events cancelled, every rider's loops started
/// (`riders` found by address), then the audio system's +0x38a0 cleared.
pub fn init_rider_sounds(q: &mut Queue, w38a0: &mut u32, riders: &mut [LoopRider], env: &mut dyn InitRiderSoundsEnv) {
    env.cancel_race_events();
    let mut i = 0;
    while i < env.rider_count() {
        let a = env.get_rider(i);
        let r = riders.iter_mut().find(|r| r.addr == a).expect("a rider the race gives");
        start_rider_loops(q, r, env);
        i += 1;
    }
    *w38a0 = 0;
}

// ---- The rider's sound loops' updates (board row E1b2g7d5h) ----

/// The snow program tables (`cBXAudio` + table + surface · 0x50, one 0x50-byte record each, `Sfx_RunSnowProgram`
/// 0x21ae90): SNOW.INF's GLIDE, AIGLIDE and CARVE programs.
pub const GLIDE_PROGRAMS: u32 = 0x745c;
pub const AI_GLIDE_PROGRAMS: u32 = 0x777c;
pub const CARVE_PROGRAMS: u32 = 0x7a9c;

/// A loop's bank instance as its update sees it: its address and its volume (+0x64); the rider is the instance's
/// +0x80.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopInstance {
    pub addr: u32,
    pub volume: i32,
}

pub trait LoopUpdateEnv: RiderLoopEnv {
    /// `Sfx_ImpactSpeed` 0x21a6a8 (the rider).
    fn impact_speed(&mut self, rider: u32) -> i32;
    /// `Sfx_EvalCurve` 0x20d928 (the curve slot, the impact speed).
    fn eval_curve(&mut self, slot: u32, x: i32) -> i64;
    /// `Music_CancelBreak` 0x2257d8 (the audio system, 0).
    fn cancel_break(&mut self);
    /// `BankInstance_Reset` 0x20a9d8 (the instance).
    fn reset_instance(&mut self, inst: u32);
    /// `Sfx_RunSnowProgram` 0x21ae90 (the record at the audio system + `table` + `surface` · 0x50, the rider, the
    /// volume's and the bend's words; `sfxcalc::run_snow_program`).
    fn run_snow_program(&mut self, table: u32, surface: u32, rider: u32, vol: &mut i32, bend: &mut i32);
    /// `BankInstance_SetPitchBend` 0x21e7e0 (the instance, the bend; `bankmon::set_pitch_bend`).
    fn set_pitch_bend(&mut self, inst: u32, bend: i64);
}

/// The end the updates share: silent while the rider's sound flag is set, the volume divided by the fade-in count
/// while it counts down (`Boarder_TickSfxFadeIn` 0x11f468), then the volume stored and the bend sent.
fn finish_loop(inst: &mut LoopInstance, r: &mut LoopRider, mut vol: i32, mut bend: i64, env: &mut dyn LoopUpdateEnv) {
    if r.w5ae4 != 0 {
        vol = 0;
        bend = 0;
    }
    let fade = crate::control::tick_sfx_fade_in(&mut r.w5af0);
    if fade > 0 {
        vol = vol.wrapping_div(fade);
    }
    inst.volume = vol;
    env.set_pitch_bend(inst.addr, bend);
}

/// `Sfx_UpdateAirLoop` 0x21b440 (the audio system, the instance): in the air (+0x424 = 2) the volume and bend are
/// curves 5 and 6 of the impact speed, else silent with bend 64.
pub fn update_air_loop(inst: &mut LoopInstance, r: &mut LoopRider, env: &mut dyn LoopUpdateEnv) {
    let (mut vol, mut bend) = (0, 0x40);
    if r.motion == 2 {
        let x = env.impact_speed(r.addr);
        vol = env.eval_curve(5, x) as i32;
        bend = env.eval_curve(6, x);
    }
    finish_loop(inst, r, vol, bend, env);
}

/// `Sfx_UpdateGlideLoop` 0x21b508 (the audio system, the instance): a changed motion is remembered (+0x788) and,
/// unless it is 1, the music's break cancelled; off motion 1, a changed surface restarts the glide loop (the
/// instance reset), else the surface's GLIDE program gives the volume and bend.
pub fn update_glide_loop(q: &mut Queue, inst: &mut LoopInstance, r: &mut LoopRider, env: &mut dyn LoopUpdateEnv) {
    let motion = r.motion;
    if motion != r.w788 {
        r.w788 = motion;
        if motion != 1 {
            env.cancel_break();
        }
    }
    let (mut vol, mut bend) = (0, 0x40);
    if motion != 1 {
        let surface = env.surface_sound_type(r.addr);
        if surface != r.w790 {
            r.w790 = surface;
            env.reset_instance(inst.addr);
            start_glide_loop(q, r, surface, env);
        } else {
            env.run_snow_program(GLIDE_PROGRAMS, surface, r.addr, &mut vol, &mut bend);
        }
    }
    finish_loop(inst, r, vol, bend as i64, env);
}

/// `Sfx_UpdateAiGlideLoop` 0x21b628 (the audio system, the instance): the motion remembered (+0x788); on the
/// ground or a rail (+0x424 2 or 3) a changed surface restarts the glide loop (the instance reset) and the surface's
/// AIGLIDE program gives the volume and bend; no fade-in, no sound flag.
pub fn update_ai_glide_loop(q: &mut Queue, inst: &mut LoopInstance, r: &mut LoopRider, env: &mut dyn LoopUpdateEnv) {
    let motion = r.motion;
    if motion != r.w788 {
        r.w788 = motion;
    }
    let (mut vol, mut bend) = (0, 0x40);
    if (motion as i32) < 4 && 1 < (motion as i32) {
        let surface = env.surface_sound_type(r.addr);
        if surface != r.w790 {
            r.w790 = surface;
            env.reset_instance(inst.addr);
            let surface = env.surface_sound_type(r.addr);
            start_glide_loop(q, r, surface, env);
        }
        let surface = env.surface_sound_type(r.addr);
        env.run_snow_program(AI_GLIDE_PROGRAMS, surface, r.addr, &mut vol, &mut bend);
    }
    inst.volume = vol;
    env.set_pitch_bend(inst.addr, bend as i64);
}

/// `Sfx_UpdateCarveLoop` 0x21b718 (the audio system, the instance): off motion 1, a changed surface (+0x794)
/// restarts the carve loop (the instance reset), else the surface's CARVE program gives the volume and bend.
pub fn update_carve_loop(q: &mut Queue, inst: &mut LoopInstance, r: &mut LoopRider, env: &mut dyn LoopUpdateEnv) {
    let (mut vol, mut bend) = (0, 0x40);
    if r.motion != 1 {
        let surface = env.surface_sound_type(r.addr);
        if surface != r.w794 {
            r.w794 = surface;
            env.reset_instance(inst.addr);
            let surface = env.surface_sound_type(r.addr);
            start_carve_loop(q, r, surface, env);
        } else {
            let surface = env.surface_sound_type(r.addr);
            env.run_snow_program(CARVE_PROGRAMS, surface, r.addr, &mut vol, &mut bend);
        }
    }
    finish_loop(inst, r, vol, bend as i64, env);
}

/// The update an entry's callback word (+0x44) names, run on its instance.
pub fn update_loop(update: u32, q: &mut Queue, inst: &mut LoopInstance, r: &mut LoopRider, env: &mut dyn LoopUpdateEnv) {
    match update {
        AIR_LOOP_UPDATE => update_air_loop(inst, r, env),
        GLIDE_LOOP_UPDATE => update_glide_loop(q, inst, r, env),
        AI_GLIDE_LOOP_UPDATE => update_ai_glide_loop(q, inst, r, env),
        CARVE_LOOP_UPDATE => update_carve_loop(q, inst, r, env),
        _ => {}
    }
}
