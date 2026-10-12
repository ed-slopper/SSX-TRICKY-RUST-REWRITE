//! The announcer's and the riders' speech triggers (board row G10c1): when a race event makes the game post a speech
//! event (`SpeechEvt_Post*` 0x22e4e0..0x2306c0: a 0x50-byte record, the event id then its words, handed to
//! `SpeechLib_PostEvent` 0x2bba80), with the chance rolls (`Speech_RollChance`, the table 0x3a3528). The speech
//! system (`cSpeechSys`, `DAT_00343998`'s) is [`Announcer`]; what the speech library then picks and plays, the banks
//! and the streams are the game's calls ([`SpeechEnv`]). Slot 0 is the announcer, slot 1 the riders' own voices
//! (posted, then `SpeechSys_Trigger` at the rider's position with its voice).
//! Checked in the function runner (`tests/speech.rs`).

/// A speech event as posted: the id and its words (twenty words, zero past the arguments)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub words: [u32; 20],
}

impl Event {
    /// `SpeechEvt_Post<id>`: the record zeroed, the id, then each argument in order (the posts take most of
    /// theirs through pointers; the record is the same)
    pub fn new(id: u32, args: &[u32]) -> Event {
        let mut words = [0; 20];
        words[0] = id;
        words[1..1 + args.len()].copy_from_slice(args);
        Event { words }
    }
}

/// `Speech_RollChance`'s table (0x3a3528): by kind, ten levels, 0..1024 the chance in 1024
pub const CHANCES: [[i32; 10]; 24] = [
    [1024, 1024, 1024, 1024, 1024, 0, 0, 0, 0, 0],
    [1024, 1024, 1024, 1024, 1024, 1024, 1024, 1024, 0, 0],
    [1024; 10],
    [1024; 10],
    [1024, 1024, 512, 512, 512, 0, 0, 0, 0, 0],
    [1024; 10],
    [1024; 10],
    [1024; 10],
    [0, 0, 255, 255, 255, 512, 512, 512, 1024, 1024],
    [1024, 1024, 512, 512, 512, 0, 0, 0, 0, 0],
    [1024, 1024, 512, 512, 512, 0, 0, 0, 0, 0],
    [1024; 10],
    [1024; 10],
    [1024; 10],
    [1024; 10],
    [1024; 10],
    [0, 0, 0, 255, 255, 512, 512, 512, 1024, 1024],
    [1024; 10],
    [1024; 10],
    [1024, 1024, 512, 512, 512, 0, 0, 0, 0, 0],
    [1024; 10],
    [1024; 10],
    [1024; 10],
    [1024; 10],
];

/// The game the triggers ask and tell
pub trait SpeechEnv {
    /// `Rand_Fx` 0x23d4a8
    fn rand_fx(&mut self) -> u32;
    /// `Replay_IsPlaying`
    fn replay_playing(&mut self) -> bool;
    /// `Audio_GetListenerRider`(−1): the rider the sound follows (its id)
    fn listener(&mut self) -> u32;
    /// The game mode word `DAT_0032de8c` (6 the tutorial)
    fn mode(&mut self) -> u32;
    /// `SpeechSys_CanPlay`(slot)
    fn can_play(&mut self, slot: u32) -> bool;
    /// `Audio_FindRiderNear` 0x20f9b0(the radius `DAT_00343998`, the rider's position +0x140): the near listener's
    /// index, below 0 none
    fn find_near(&mut self, rider: u32) -> i32;
    /// `Speech_InterruptIfPlaying` 0x234bb8(voice): a rider line of this voice playing is stopped
    fn interrupt_if_playing(&mut self, voice: u32);
    /// `RelTable_GetRelation`(`DAT_003352b4`, a, b) of two characters
    fn relation(&mut self, a: u32, b: u32) -> i32;
    /// `SpeechLib_PostEvent`: its answer (the trick line keeps it)
    fn post(&mut self, e: Event) -> u32;
    /// `SpeechSys_Trigger` 0x228bd8: the speech library serviced for the slot now; the values it handed the
    /// callback (`Speech_QueueBigScore`, when given) come back in order
    fn trigger(&mut self, t: Trigger) -> Vec<u32>;
    /// The course word `DAT_0032de88`
    fn course(&mut self) -> u32;
    /// gApp+0x58 (what `Speech_GetModeMask` switches on)
    fn app_mode(&mut self) -> u32;
    /// `FUN_00228bb0`(slot, v): the slot's +0x34c
    fn slot_hold(&mut self, slot: u32, v: u32);
    /// `Race_GetRider`(gApp+0x730, i): its id
    fn race_rider(&mut self, i: u32) -> u32;
    /// `SpeechSys_ClearEvent`(slot)
    fn clear_event(&mut self, slot: u32);
    /// The race clock (gApp+0x730 → +0x1c → +0x18, frames)
    fn race_clock(&mut self) -> i32;
    /// The k-th 0x10-byte entry's first word past the career's new-record list: the game reads on into the
    /// pre-race selection object (0x32ecf0) until a negative word
    fn record_after(&mut self, k: usize) -> i32;
}

/// A `SpeechSys_Trigger` call: the gain (f12, 0 keeps the slot's), the slot, the position (a rider's +0x140: the
/// rider's id; none for 0), whether `Speech_QueueBigScore` is the callback, and the voice
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trigger {
    pub gain: f32,
    pub slot: u32,
    pub at: Option<u32>,
    pub callback: bool,
    pub voice: u32,
}

/// A trigger at a rider (gain 0, no callback)
fn at(slot: u32, rider: u32, voice: u32) -> Trigger {
    Trigger { gain: 0.0, slot, at: Some(rider), callback: false, voice }
}

/// What the triggers read of a rider (`cBoarder`): its id (the object, compared with the listener; its position
/// +0x140 for the near test and the trigger), +0x418 the controller (below 0 a player's), +0x41c (1 a local player),
/// +0x38 its race index, +0x118 / +0x120 its rival target (on, which), +0x460 its slot, +0x50 + 0x20 · slot its
/// attitudes to the others (bytes), +0x150.. its velocity (the third word what the air line tests), +0x57f4 its air time, +0x58a8 its tricky count; through its
/// character entry +0x464: +0x38 the character, +100 the voice, +0x74 the level.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpeechRider {
    pub id: u32,
    pub controller: i32,
    pub kind: i32,
    pub index: i32,
    pub rival_on: i32,
    pub rival: i32,
    pub slot: i32,
    pub attitudes: [i8; 8],
    pub air: f32,
    pub tricky: i32,
    /// +0x150..: the velocity
    pub vel: [f32; 3],
    /// +0x370: the heading
    pub heading: f32,
    /// +0x5c + 0x20 · slot: a count kept about each other rider
    pub counts: [i32; 8],
    pub character: u32,
    pub voice: u8,
    pub level: i32,
}

impl SpeechRider {
    fn player(&self) -> bool {
        self.controller < 0
    }
}

/// `Speech_CharacterBit` 0x234698 (and `Speech_RiderMask` 0x234730, the same): the voice's bit in the speech
/// library's character masks
pub fn character_bit(voice: u8) -> u32 {
    match voice {
        0 => 0x40,
        1 => 0x10,
        2 => 0x20,
        3 => 1,
        4 => 2,
        5 => 8,
        6 => 0x400,
        7 => 4,
        8 => 0x100,
        9 => 0x800,
        10 => 0x80,
        11 => 0x200,
        _ => 0,
    }
}

/// `Speech_WipeoutChance` 0x233b70: the wipe-out line's grade by the rider's level (+0x464 → +0x74)
pub fn wipeout_grade(level: i32) -> u32 {
    if level < 5 {
        if 1 < level {
            10
        } else {
            0xc
        }
    } else {
        9
    }
}

/// The speech system's state the triggers read (the audio object `DAT_00343050`): +0x383c the level (a float),
/// +0x6e68 its cap, +0x6e50, the checkpoints passed by race index (+0x6e6c), the big-score ring (+0x7dd4 → its
/// size, buffer, write and read indices) and +0x7dd8 (a big score queued by the library's callback)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Announcer {
    pub level: f32,
    pub cap: i32,
    pub resets_on: u32,
    pub checkpoints: [i32; 8],
    pub ring: Vec<u32>,
    pub ring_write: i32,
    pub ring_read: i32,
    pub big_queued: u32,
    /// +0x7dcc: bit 0 the result line spoken, bit 4 the race-end lines queued
    pub end_flags: u32,
    /// +0x7ddc: the race clock of the last placing line (they wait 600 frames)
    pub placing_time: i32,
    /// +0x6e00: trick lines spoken (one per landing until cleared)
    pub tricks_spoken: i32,
    /// +0x6e04: the last trick line's post answer (0 stops them)
    pub trick_ok: u32,
}

impl Announcer {
    /// `Speech_RollChance` 0x235d58(kind): one `Rand_Fx`, its low ten bits below the table at the level
    pub fn roll(&self, env: &mut dyn SpeechEnv, kind: usize) -> bool {
        let r = env.rand_fx();
        crate::fxrand::speech_roll(r, self.level, self.cap, kind, CHANCES.as_flattened()).unwrap_or(false)
    }

    /// The common guard: a player's rider, then (in the game's order) the roll, not the tutorial, no replay
    fn roll_first(&self, env: &mut dyn SpeechEnv, r: &SpeechRider, kind: usize) -> bool {
        r.player() && self.roll(env, kind) && env.mode() != 6 && !env.replay_playing()
    }

    /// The other order: a player's rider, no replay, not the tutorial, then the roll
    fn replay_first(&self, env: &mut dyn SpeechEnv, r: &SpeechRider, kind: usize) -> bool {
        r.player() && !env.replay_playing() && env.mode() != 6 && self.roll(env, kind)
    }

    /// `Speech_OnWipeout` 0x233d38: the listener's own wipe-out (roll 8: event 0x100202e with its grade; roll 0x10
    /// in show-off 3 or 5: 0x1002054 with 2)
    pub fn on_wipeout(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if r.player() && !env.replay_playing() && env.mode() != 6 && r.id == env.listener() {
            if self.roll(env, 8) {
                env.post(Event::new(0x100202e, &[wipeout_grade(r.level)]));
            }
            if self.roll(env, 0x10) {
                let m = env.mode();
                if m == 5 || m == 3 {
                    env.post(Event::new(0x1002054, &[2]));
                }
            }
        }
    }

    /// `Speech_OnBoost` 0x233e78: roll 10, 0x1002033 with 2
    pub fn on_boost(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.roll_first(env, r, 10) {
            env.post(Event::new(0x1002033, &[2]));
        }
    }

    /// `FUN_00233e10`: roll 9, 0x1002046 with 1
    pub fn on_boost_full(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.roll_first(env, r, 9) {
            env.post(Event::new(0x1002046, &[1]));
        }
    }

    /// `Speech_OnTrickLanded` 0x2333e0: roll 4, 0x100201a
    pub fn on_trick_landed(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.roll_first(env, r, 4) {
            env.post(Event::new(0x100201a, &[]));
        }
    }

    /// `Speech_OnAirTime` 0x233448: roll 0x17, 0x100204d
    pub fn on_air_time(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.roll_first(env, r, 0x17) {
            env.post(Event::new(0x100204d, &[]));
        }
    }

    /// `Speech_OnTenSecondsLeft` 0x233ff0: roll 0xc, 0x1002036
    pub fn on_ten_seconds_left(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.roll_first(env, r, 0xc) {
            env.post(Event::new(0x1002036, &[]));
        }
    }

    /// `FUN_00234058`: roll 0xd (no rider), 0x1002037
    pub fn on_race_line(&self, env: &mut dyn SpeechEnv) {
        if self.roll(env, 0xd) && env.mode() != 6 && !env.replay_playing() {
            env.post(Event::new(0x1002037, &[]));
        }
    }

    /// `Speech_OnRiderCollision` 0x233cb8: roll 7, the listener, 0x100202c
    pub fn on_rider_collision(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.roll_first(env, r, 7) && r.id == env.listener() {
            env.post(Event::new(0x100202c, &[]));
        }
    }

    /// `Speech_OnGrudge` 0x2336b8(rider, other): roll 0x11 and the other a local player, 0x100204f
    pub fn on_grudge(&self, env: &mut dyn SpeechEnv, r: &SpeechRider, other: &SpeechRider) {
        if self.replay_first(env, r, 0x11) && other.kind == 1 {
            env.post(Event::new(0x100204f, &[]));
        }
    }

    /// `FUN_00233740`: roll 0x12, the listener, 0x1002056
    pub fn on_listener_line(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.replay_first(env, r, 0x12) && r.id == env.listener() {
            env.post(Event::new(0x1002056, &[]));
        }
    }

    /// `Speech_OnTricky` 0x2337c8: roll 0x13, the listener, under six trickies (+0x58a8), 0x100204c
    pub fn on_tricky(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if self.replay_first(env, r, 0x13) && r.id == env.listener() && r.tricky < 6 {
            env.post(Event::new(0x100204c, &[]));
        }
    }

    /// `Speech_OnRiderHit` 0x233858(hitter, hit): roll 0x14, the hitter the listener: 0x1002050 with the two
    /// characters' relation (1 → 4, 0 → 1, 2 → 2, else 0)
    pub fn on_rider_hit(&self, env: &mut dyn SpeechEnv, hitter: &SpeechRider, hit: &SpeechRider) {
        if hit.player() && !env.replay_playing() && env.mode() != 6 && self.roll(env, 0x14) && hitter.id == env.listener() {
            let rel = env.relation(hitter.character, hit.character);
            let a = match rel {
                1 => 4,
                0 => 1,
                2 => 2,
                _ => 0,
            };
            env.post(Event::new(0x1002050, &[a]));
        }
    }

    /// `Speech_OnAirEnter` 0x233950(rider, kind): roll 0x15, +0x158 (the velocity's third word) at 1341.12 or more, the listener: 0x1002030
    /// with 2 for kind 1, else 1
    pub fn on_air_enter(&self, env: &mut dyn SpeechEnv, r: &SpeechRider, kind: i64) {
        if self.replay_first(env, r, 0x15) && 1341.12 <= r.vel[2] && r.id == env.listener() {
            env.post(Event::new(0x1002030, &[if kind != 1 { 1 } else { 2 }]));
        }
    }

    /// `Speech_OnRiderReset` 0x233a18: the listener, while +0x6e50 is set, roll 0x16: 0x1002052
    pub fn on_rider_reset(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if r.player() && !env.replay_playing() && env.mode() != 6 && r.id == env.listener() && self.resets_on != 0 && self.roll(env, 0x16) {
            env.post(Event::new(0x1002052, &[]));
        }
    }

    /// `Speech_OnCheckpoint` 0x233ee0: a local player's checkpoint counted (by race index); for the listener, roll
    /// 0xb in modes 2..5: 0x1002034 with 1 and the count's bit (1 << (count + 1)) | 1
    pub fn on_checkpoint(&mut self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if r.player() && !env.replay_playing() && env.mode() != 6 && r.kind == 1 {
            let i = r.index as usize;
            self.checkpoints[i] = self.checkpoints[i].wrapping_add(1);
            if r.id == env.listener() && self.roll(env, 0xb) {
                let m = env.mode() as i32;
                if 1 < m && m < 6 {
                    let bit = 1u32 << ((self.checkpoints[i] as u32).wrapping_add(1) & 31) | 1;
                    env.post(Event::new(0x1002034, &[1, bit]));
                }
            }
        }
    }

    /// `FUN_00234558`(rider, kind): a local player's, roll 0xe: 0x1002051 with 2 for kind 1, else 1
    pub fn on_local_line(&self, env: &mut dyn SpeechEnv, r: &SpeechRider, kind: i64) {
        if !env.replay_playing() && r.player() && r.kind == 1 && env.mode() != 6 && self.roll(env, 0xe) {
            env.post(Event::new(0x1002051, &[if kind == 1 { 2 } else { 1 }]));
        }
    }

    /// `FUN_00234610`: a local player's, roll 0xf: 0x1002053
    pub fn on_local_line2(&self, env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if !env.replay_playing() && r.player() && r.kind == 1 && env.mode() != 6 && self.roll(env, 0xf) {
            env.post(Event::new(0x1002053, &[]));
        }
    }
}

/// `Speech_IsRivalOrGrudge` 0x234c38(a, b): their relation 2, or 0..1 with a set on b (+0x118, +0x120 = b's slot)
/// or a's attitude to b above 60
pub fn is_rival_or_grudge(env: &mut dyn SpeechEnv, a: &SpeechRider, b: &SpeechRider) -> bool {
    let rel = env.relation(a.character, b.character);
    if rel < 0 {
        return false;
    }
    if rel < 2 {
        let att = a.attitudes[b.slot as usize] as i32 as f32;
        if (a.rival_on == 0 || a.rival != b.slot) && att <= 60.0 {
            return false;
        }
    } else if rel != 2 {
        return false;
    }
    true
}

/// The riders' own voices (slot 1)
pub mod voices {
    use super::*;

    /// `Speech_OnCrashNearby` 0x235090: a player's rider crashing near the listener: its voice and the announcer's
    /// (12) interrupted; 0x3006023 or 0x3006024 (`Rand_Fx` bit 0) with its bit and 0x40, at its position, voice 13
    pub fn on_crash_nearby(env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if !r.player() {
            return;
        }
        env.interrupt_if_playing(r.voice as u32);
        env.interrupt_if_playing(0xc);
        if env.can_play(1) && 0 <= env.find_near(r.id) {
            let args = [character_bit(r.voice), 0x40];
            let id = if env.rand_fx() & 1 == 0 { 0x3006024 } else { 0x3006023 };
            env.post(Event::new(id, &args));
            env.trigger(at(1, r.id, 0xd));
        }
    }

    /// `Speech_OnStumble` 0x235398(rider, kind): kind 0 is a crash nearby; else 0x3006022 with its bit, voice 13
    pub fn on_stumble(env: &mut dyn SpeechEnv, r: &SpeechRider, kind: i64) {
        if !r.player() {
            return;
        }
        env.interrupt_if_playing(r.voice as u32);
        if env.can_play(1) && 0 <= env.find_near(r.id) {
            if kind == 0 {
                on_crash_nearby(env, r);
            } else {
                env.post(Event::new(0x3006022, &[character_bit(r.voice)]));
                env.trigger(at(1, r.id, 0xd));
            }
        }
    }

    /// `Speech_RiderHitLine` 0x234d00(a, b, which, crash): two players' riders colliding near the listener
    pub fn rider_hit_line(env: &mut dyn SpeechEnv, a: &SpeechRider, b: &SpeechRider, which: i64, crash: i64) {
        if !a.player() || !b.player() {
            return;
        }
        env.interrupt_if_playing(b.voice as u32);
        if !env.can_play(1) {
            return;
        }
        let nb = env.find_near(b.id);
        let na = env.find_near(a.id);
        if nb < 0 && na < 0 {
            return;
        }
        if crash != 0 && 0 <= nb {
            on_crash_nearby(env, b);
            return;
        }
        let pick = if 0 <= nb && 0 <= na { env.rand_fx() & 1 } else { 0 };
        let l = env.listener();
        if a.kind != 1 && b.kind != 1 && a.id != l && b.id != l {
            if pick == 0 {
                on_crash_nearby(env, a);
            } else {
                on_crash_nearby(env, b);
            }
            return;
        }
        let mut mask = 0x40;
        if pick == 0 {
            let rival = is_rival_or_grudge(env, a, b);
            let bit = character_bit(a.voice);
            mask |= match (rival, which != 0) {
                (false, false) => 0x10,
                (false, true) => 2,
                (true, false) => 0x20,
                (true, true) => 1,
            };
            env.post(Event::new(0x3006036, &[bit, mask]));
            env.trigger(at(1, a.id, a.voice as u32));
            return;
        }
        let rival = is_rival_or_grudge(env, b, a);
        let bit = character_bit(b.voice);
        mask |= if rival { 4 } else { 8 };
        env.post(Event::new(0x3006036, &[bit, mask]));
        env.trigger(at(1, b.id, 0xc));
    }

    /// `Speech_OnAITaunt` 0x234f68(a, b): two players' riders, one a local player or the listener, both near the
    /// same listener: 0x300600f with a's bit and b's, a's voice
    pub fn on_ai_taunt(env: &mut dyn SpeechEnv, a: &SpeechRider, b: &SpeechRider) {
        if a.player() && b.player() && env.can_play(1) {
            let l = env.listener();
            if a.kind == 1 || b.kind == 1 || a.id == l || b.id == l {
                let na = env.find_near(a.id);
                if 0 <= na && na == env.find_near(b.id) {
                    env.post(Event::new(0x300600f, &[character_bit(a.voice), character_bit(b.voice)]));
                    env.trigger(at(1, a.id, a.voice as u32));
                }
            }
        }
    }

    /// `Speech_OnAirNearby` 0x235170(rider, kind): three seconds of air or more (+0x57f4), a local player or the
    /// listener: kind 0 0x3006026, 1 0x3006025 (other kinds only the trigger), its voice
    pub fn on_air_nearby(env: &mut dyn SpeechEnv, r: &SpeechRider, kind: i64) {
        if !(r.player() && 3.0 <= r.air) {
            return;
        }
        if r.kind != 1 && env.listener() != r.id {
            return;
        }
        if env.can_play(1) {
            let bit = character_bit(r.voice);
            if 0 <= env.find_near(r.id) {
                if kind == 0 {
                    env.post(Event::new(0x3006026, &[bit]));
                } else if kind == 1 {
                    env.post(Event::new(0x3006025, &[bit]));
                }
                env.trigger(at(1, r.id, r.voice as u32));
            }
        }
    }

    /// `FUN_00235298`(grade, rider): a local player or the listener: 0x3006028 at 0.65 or more, else 0x3006027
    pub fn on_graded_line(env: &mut dyn SpeechEnv, grade: f32, r: &SpeechRider) {
        if r.player() && (r.kind == 1 || env.listener() == r.id) && env.can_play(1) {
            let bit = character_bit(r.voice);
            if 0 <= env.find_near(r.id) {
                let id = if 0.65 <= grade { 0x3006028 } else { 0x3006027 };
                env.post(Event::new(id, &[bit]));
                env.trigger(at(1, r.id, r.voice as u32));
            }
        }
    }

    /// `FUN_00235468`: a local player or the listener: 0x300602b with its bit
    pub fn on_rider_line(env: &mut dyn SpeechEnv, r: &SpeechRider) {
        if r.player() && (r.kind == 1 || env.listener() == r.id) && env.can_play(1) {
            let bit = character_bit(r.voice);
            if 0 <= env.find_near(r.id) {
                env.post(Event::new(0x300602b, &[bit]));
                env.trigger(at(1, r.id, r.voice as u32));
            }
        }
    }
}

/// `Speech_CourseMask` 0x2307f0(course): a course's bit (0 past 11)
pub fn course_mask(course: u64) -> u32 {
    const M: [u32; 12] = [0x10, 0x100, 0x40, 1, 0x800, 4, 0x20, 2, 0x400, 8, 0x80, 0x200];
    if course < 12 {
        M[course as usize]
    } else {
        0
    }
}

/// `Speech_CharMaskOf` 0x230880: the same bits, 0x2000 past 11
pub fn char_mask_of(c: u64) -> u32 {
    if c < 12 {
        course_mask(c)
    } else {
        0x2000
    }
}

/// `Speech_CurrentCourseMask` 0x230910: the course word's bit in the narration's course masks
pub fn current_course_mask(course: u32) -> u32 {
    match course {
        1 => 2,
        2 => 8,
        3 => 4,
        4 => 1,
        5 => 0x40,
        6 => 0x80,
        7 => 0x200,
        8 => 0x20,
        0xb => 0x100,
        _ => 0x10,
    }
}

/// `Speech_GetModeMask` 0x2309a8: gApp+0x58 as a bit (2 → 2, 3 → 8, 4 → 4, else 1)
pub fn mode_mask(app_mode: u32) -> u32 {
    match app_mode {
        2 => 2,
        3 => 8,
        4 => 4,
        _ => 1,
    }
}

/// `Speech_PickRaceGoLine` 0x233138(event mode, course, mode): the start line's bits
pub fn race_go_line(event: i32, course: i32, mode: i64) -> u32 {
    let v = match mode as i32 {
        2 | 3 | 7 => 2,
        4 => {
            let v = match event {
                1 => 4,
                0 => 2,
                2 => {
                    if course == 5 {
                        0x10
                    } else {
                        8
                    }
                }
                _ => 0,
            };
            if mode != 4 {
                return v | 1;
            }
            v | 0x200
        }
        5 => {
            let v = if course == 6 { 0x40 } else { 0x20 };
            if mode != 5 {
                return v | 1;
            }
            v | 0x400
        }
        _ => 0,
    };
    v | 1
}

impl Announcer {
    /// The ring's push (`Speech_QueueBigScore`'s and `Speech_OnBigScore`'s): kept unless full
    fn ring_push(&mut self, v: u32) {
        let n = self.ring.len() as i32;
        if (self.ring_write + 1) % n != self.ring_read {
            self.ring[self.ring_write as usize] = v;
            self.ring_write += 1;
            if n <= self.ring_write {
                self.ring_write = 0;
            }
        }
    }

    /// `Speech_QueueBigScore` 0x235530(v): the library's callback: v queued, slot 1's hold off, +0x7dd8 set
    pub fn queue_big_score(&mut self, env: &mut dyn SpeechEnv, v: u32) {
        self.ring_push(v);
        env.slot_hold(1, 0);
        self.big_queued = 1;
    }

    /// `Speech_OnBigScore` 0x235ba8(rider or none, score, kind): a score of 1000 or more posts 0x500a000 with it and
    /// speaks it at the rider (race rider 0 when none and kind ≠ 0; nowhere for kind 0) at gain 1500; with kind ≠ 0
    /// slot 1 is held, the library may queue through the callback, else −1 is queued; then −2; the hold off.
    pub fn on_big_score(&mut self, env: &mut dyn SpeechEnv, rider: Option<u32>, score: i64, kind: i64) {
        if score < 1000 {
            return;
        }
        env.post(Event::new(0x500a000, &[score as u32]));
        let pos = match rider {
            Some(r) => Some(r),
            None if kind == 0 => None,
            None => Some(env.race_rider(0)),
        };
        if kind == 0 {
            env.trigger(Trigger { gain: 1500.0, slot: 1, at: pos, callback: false, voice: 0 });
            return;
        }
        env.slot_hold(1, u32::MAX);
        self.big_queued = 0;
        for v in env.trigger(Trigger { gain: 1500.0, slot: 1, at: pos, callback: true, voice: 0 }) {
            self.queue_big_score(env, v);
        }
        if self.big_queued == 0 {
            self.ring_push(0xffff_ffff);
        }
        self.ring_push(0xffff_fffe);
        env.slot_hold(1, 0);
    }

    /// `Speech_CheckBigScore` 0x2356f0(score): over 999, `Speech_OnBigScore`(none, score, 1)
    pub fn check_big_score(&mut self, env: &mut dyn SpeechEnv, score: i64) {
        if 999 < score {
            self.on_big_score(env, None, score, 1);
        }
    }

    /// `Speech_OnRaceGo` 0x2334b0: roll 5, not the tutorial, no replay: 0x100204a with the mode's bits, the
    /// course's bit and the mode's kind, then 0x1002020 (a start line with bit 2) or 0x100201f with the line
    pub fn on_race_go(&self, env: &mut dyn SpeechEnv, career: &crate::career::Career) {
        if !(self.roll(env, 5) && env.mode() != 6 && !env.replay_playing()) {
            return;
        }
        let mode = env.mode();
        let (m, k) = match mode {
            2 | 7 => (2, 1),
            3 => (2, 2),
            4 => (4, 1),
            5 => (4, 2),
            _ => (0, 0),
        };
        let m = m | 1;
        let course = env.course();
        let c = match course {
            0 => 0x400,
            1 => 1,
            2 => 2,
            3 => 4,
            4 => 8,
            5 => 0x20,
            6 => 0x40,
            7 => 0x80,
            8 => 0x10,
            0xb => 0x800,
            _ => 0,
        };
        let line = race_go_line(career.event_mode() as i32, course as i32, mode as i32 as i64);
        env.post(Event::new(0x100204a, &[m, c, k]));
        if line & 2 == 0 {
            env.post(Event::new(0x100201f, &[m, k, line]));
        } else {
            env.post(Event::new(0x1002020, &[m, c, k]));
        }
    }

    /// `Speech_OnScoreMessage` 0x233220(rider, message, kind): the listener, a player, not the tutorial or mode 1,
    /// no replay, roll 3: messages 4, 0xb, 0x10 (and 0xc with kind 2, 3, 5) post 0x100204e with their bit; 0xe and
    /// 0xf post 0x1002031 with the kind's (1 → 1, 2 → 2, 3 → 4, 0 → 0, else 8)
    pub fn on_score_message(&self, env: &mut dyn SpeechEnv, r: Option<&SpeechRider>, message: i64, kind: i64) {
        let Some(r) = r else { return };
        if !r.player() {
            return;
        }
        let mode = env.mode();
        if mode == 6 || mode == 1 || r.id != env.listener() || env.replay_playing() || !self.roll(env, 3) {
            return;
        }
        let bit = match message {
            4 => 0x40,
            0xb => 4,
            0xc => match kind {
                3 => 0x10,
                2 => 8,
                5 => 0x20,
                _ => 0,
            },
            0x10 => 1,
            _ => 0,
        };
        if bit != 0 {
            env.post(Event::new(0x100204e, &[bit]));
        }
        if 1 < (message as i32).wrapping_sub(0xe) as u32 {
            return;
        }
        let v = match kind {
            1 | 2 => kind as u32,
            0 => 0,
            3 => 4,
            _ => 8,
        };
        env.post(Event::new(0x1002031, &[v]));
    }

    /// `Speech_OnNextRace` 0x231ff8(career): slot 0 serviced; for a single profile in a race (2) or circuit race
    /// (4): when slot 0 cannot play, 0x4008015 with 2; then 0x4008017 with how the computer riders feel about the
    /// player's rider (above 80: two or more 4 (and 0x10 with two or more at 20 or under), none 2; at 20 or under two
    /// or more 1, none 8) and the mode bit
    pub fn on_next_race(&self, env: &mut dyn SpeechEnv, c: &crate::career::Career) {
        let none = Trigger { gain: 0.0, slot: 0, at: None, callback: false, voice: 0 };
        env.trigger(none);
        let mode = env.mode();
        if mode != 4 && mode != 2 {
            return;
        }
        if 1 < c.count_profiles() {
            return;
        }
        if !env.can_play(0) {
            env.post(Event::new(0x4008015, &[2]));
            env.trigger(none);
        }
        let mm = mode_mask(env.app_mode());
        let me = c.r8(0x30fc) as i8;
        let n = c.r32(0x27dc) as i32;
        let mut mine = -1i32;
        for i in 0..n.max(0) {
            if c.r8(0x2845 + 0x84 * i as usize) as i8 == me {
                mine = i;
                break;
            }
        }
        let (mut fond, mut cold) = (0, 0);
        for i in 0..n.max(0) as usize {
            if c.r8(0x2845 + 0x84 * i) as i8 == -1 {
                let at = (0x27dc + 0x24 + 0x84 * i as isize + 4 * mine as isize) as usize;
                let v = c.r8(at) as i8 as i32 as f32;
                if 80.0 < v {
                    fond += 1;
                } else if v <= 20.0 {
                    cold += 1;
                }
            }
        }
        let mut v = 0;
        if fond < 2 {
            if fond == 0 {
                v = 2;
            }
        } else {
            v = if 1 < cold { 0x10 } else { 0 } | 4;
        }
        if cold < 2 {
            if cold == 0 {
                v |= 8;
            }
        } else {
            v |= 1;
        }
        env.post(Event::new(0x4008017, &[v, mm]));
        env.trigger(none);
    }
}

/// `FUN_002315e0` (the race stats mask): for each race rider of `player` (+0x69), the course's thresholds (fast and
/// slow time, two counts) against its run (+0x3300 the time, +0x32ac and +0x32b0 the counts, +0x32dc, 0xbc apart):
/// fast 0x10c, slow 0x230, the counts 0x240 / 0x280, +0x32dc none 0x101, more than one 0x202; nothing 0x300
pub fn race_stats_mask(c: &crate::career::Career, course: u32, player: i8) -> u32 {
    let n = c.r32(0x27dc) as i32;
    let mut v = 0u32;
    for i in 0..n.max(0) as usize {
        if c.r8(0x2845 + 0x84 * i) as i8 != player {
            continue;
        }
        let (fast, slow, a, b) = match course {
            0 => (30000, 90000, 5, 10),
            1 => (25000, 75000, 4, 8),
            3 => (27000, 81000, 4, 9),
            0xb => (0x86c4, 0x1944c, 5, 0xb),
            6 | 9 | 7 => (0x1e23, 0x5a69, 1, 2),
            8 | 4 => (32000, 96000, 5, 10),
            2 => (0x9664, 0x1c32c, 6, 0xc),
            5 => (21000, 63000, 3, 7),
            _ => (0, 0, 0, 0),
        };
        let o = 0xbc * i;
        let t = c.rf(0x3300 + o);
        if t <= fast as f32 {
            v |= 0x10c;
        }
        if slow as f32 <= t {
            v |= 0x230;
        }
        if a <= c.r32(0x32ac + o) as i32 {
            v |= 0x240;
        }
        if b <= c.r32(0x32b0 + o) as i32 {
            v |= 0x280;
        }
        let k = c.r32(0x32dc + o) as i32;
        if k == 0 {
            v |= 0x101;
        }
        if 1 < k {
            v |= 0x202;
        }
        if v == 0 {
            v = 0x300;
        }
    }
    v
}

/// The medal as the result lines' bit (gold 8, silver 4, bronze 2, else 1)
fn medal_bit(m: u32) -> u32 {
    match m {
        1 => 4,
        0 => 8,
        2 => 2,
        _ => 1,
    }
}

const SLOT0: Trigger = Trigger { gain: 0.0, slot: 0, at: None, callback: false, voice: 0 };

impl Announcer {
    /// `FUN_00231800` (the placing line): once (+0x7dcc bit 0), no replay: the first player rider in the places, its
    /// place's bit (1, 2, 4, 8, 0x10; last 0x20), whether it set a new record (the list after the block: 2, else 1;
    /// | 4), the mode bit and its stats: 0x4008007; in a circuit race at its final (career event mode 2) also
    /// 0x4008009 with the place's medal bit
    pub fn place_line(&mut self, env: &mut dyn SpeechEnv, c: &crate::career::Career) -> bool {
        if self.end_flags & 1 != 0 || env.replay_playing() {
            return false;
        }
        let n = c.r32(0x27dc) as i32;
        let pl = |i: i32| c.r8(0x2845 + 0x84 * c.r32(0x3264 + 4 * i as usize) as usize) as i8;
        let (mut who, mut place) = (-1i32, -1i32);
        for i in 0..n.max(0) {
            let p = pl(i);
            if 0 <= p {
                who = p as i32;
                place = i;
                break;
            }
        }
        let bit = if place == 0 || place != n - 1 {
            match place {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                4 => 0x10,
                _ => 0,
            }
        } else {
            0x20
        };
        let mut rec = 1;
        let mut k = 0;
        loop {
            let e = if k < 5 { c.r32(0x3910 + 0x10 * k) as i32 } else { env.record_after(k - 5) };
            if e < 0 {
                break;
            }
            k += 1;
            if e == who {
                rec = 2;
                break;
            }
        }
        let rec = rec | 4;
        let mm = mode_mask(env.app_mode());
        let stats = race_stats_mask(c, env.course(), who as i8);
        env.post(Event::new(0x4008007, &[bit, rec, mm, stats, 1]));
        if env.mode() == 4 && c.event_mode() == 2 {
            let b = match place {
                1 => 4,
                0 => 8,
                2 => 2,
                _ => 1,
            };
            env.post(Event::new(0x4008009, &[b, 2, mm, stats, 1]));
        }
        env.trigger(SLOT0);
        self.end_flags |= 1;
        true
    }

    /// `FUN_00231af8` (the medal line): once, no replay, one rider: rider 0's show-off medal (`Circuit_GetMedal` of
    /// the score) as a bit, slot 0 cleared, 0x4008013 with it, 3, the mode bit, the stats' 0x3c0 bits, 0x11, 1
    pub fn medal_line(&mut self, env: &mut dyn SpeechEnv, c: &crate::career::Career) -> bool {
        if self.end_flags & 1 != 0 {
            return false;
        }
        if env.replay_playing() || 1 < c.r32(0x27dc) as i32 {
            return false;
        }
        let course = env.course();
        let medal = crate::courses::medal(c.r32(0x3310) as i32, course);
        let mm = mode_mask(env.app_mode());
        let stats = race_stats_mask(c, course, c.r8(0x2845) as i8) & 0x3c0;
        env.clear_event(0);
        env.post(Event::new(0x4008013, &[medal_bit(medal), 3, mm, stats, 0x11, 1]));
        env.trigger(SLOT0);
        self.end_flags |= 1;
        true
    }

    /// `Speech_QueueResultLine` 0x231548: once, no replay: the placing line in the races (2, 4), the medal line in
    /// the show-offs (3, 5)
    pub fn queue_result_line(&mut self, env: &mut dyn SpeechEnv, c: &crate::career::Career) -> bool {
        if self.end_flags & 1 != 0 || env.replay_playing() {
            return false;
        }
        match env.mode() {
            2 | 4 => self.place_line(env, c),
            3 | 5 => self.medal_line(env, c),
            _ => false,
        }
    }

    /// `Speech_OnHiScore` 0x232240(rank): when no result line was spoken: 0x400800b with the mode's bits (show-off
    /// 0x11, circuit race 0xb, race 9, else 1), the rank's (top ten in a show-off 5, top five in a race 3, else 1)
    /// and the mode bit
    pub fn on_hi_score(&mut self, env: &mut dyn SpeechEnv, c: &crate::career::Career, rank: i64) {
        if self.queue_result_line(env, c) {
            return;
        }
        let mm = mode_mask(env.app_mode());
        let (mut k, mut r) = (1, 1);
        match env.mode() {
            3 | 5 => {
                k = 0x11;
                if rank < 10 {
                    r = 5;
                }
            }
            m @ (2 | 4) => {
                k = if m == 4 { 3 } else { 1 } | 8;
                if rank < 5 {
                    r = 3;
                }
            }
            _ => {}
        }
        env.post(Event::new(0x400800b, &[k, r, mm]));
        env.trigger(SLOT0);
    }

    /// `Speech_QueueRaceEndLines` 0x231c50: once (+0x7dcc bit 4): the result line (and 0x4008015 with 2 after it);
    /// in a circuit race whose first player rider placed in the top half: 0x400800c (0xb, the event's bit, the mode
    /// bit, 2), then the rider among the top places (half, all at the final) with the most of the player's count
    /// (+0x3348 + 4 · the players placed before) from 2 up names its character for 0x400800a (0x4008015 first when
    /// slot 0 cannot play); in a show-off with a medal: 0x400800c (0x11, 4, the mode bit, 2)
    pub fn queue_race_end_lines(&mut self, env: &mut dyn SpeechEnv, c: &crate::career::Career) {
        if self.end_flags & 0x10 != 0 {
            return;
        }
        self.end_flags |= 0x10;
        if self.queue_result_line(env, c) {
            env.post(Event::new(0x4008015, &[2]));
            env.trigger(SLOT0);
        }
        let mm = mode_mask(env.app_mode());
        let n = c.r32(0x27dc) as i32;
        let place = |i: i32| c.r32(0x3264 + 4 * i as usize) as usize;
        let mode = env.mode();
        if mode == 4 {
            let mut first = -1i8;
            let mut top = false;
            for i in 0..n.max(0) {
                let p = c.r8(0x2845 + 0x84 * place(i)) as i8;
                if 0 <= p {
                    if first < 0 {
                        first = p;
                    }
                    if i < n / 2 {
                        top = true;
                        break;
                    }
                }
            }
            if top {
                let ev = c.event_mode();
                let e = match ev {
                    1 => 9,
                    0 => 3,
                    2 => 4,
                    _ => 1,
                };
                env.post(Event::new(0x400800c, &[0xb, e, mm, 2]));
                env.trigger(SLOT0);
                let mut before = 0;
                for i in 0..n.max(0) as usize {
                    let p = c.r8(0x2845 + 0x84 * i) as i8;
                    if 0 <= p {
                        before += (p < first) as usize;
                    }
                }
                let upto = if ev == 2 { n } else { n / 2 };
                let mut best = 2;
                let mut mask = 0;
                for k in 0..upto.max(0) {
                    let r = place(k);
                    let v = c.r32(0x3348 + 0xbc * r + 4 * before) as i32;
                    if best <= v {
                        mask = course_mask(c.r32(0x2818 + 0x84 * r) as u64);
                        best = v;
                    }
                }
                if mask != 0 {
                    if !env.can_play(0) {
                        // (the caller passes the mode bit too; the post takes one word)
                        env.post(Event::new(0x4008015, &[2]));
                        env.trigger(SLOT0);
                    }
                    env.post(Event::new(0x400800a, &[mask, mm]));
                    env.trigger(SLOT0);
                }
            }
        } else if mode == 5 && crate::courses::medal(c.r32(0x3310) as i32, env.course()) != 3 {
            env.post(Event::new(0x400800c, &[0x11, 4, mm, 2]));
            env.trigger(SLOT0);
        }
        env.trigger(SLOT0);
    }

    /// `Speech_QueueUnlockLines` 0x232330 (from `cUnlockedOverlay::Reset`): the result line (0x4008015 with 2 after
    /// it), then for each unlock of kind 0..2 in the circuit's list 0x400800d with its course's bit and the mode bit
    pub fn queue_unlock_lines(&mut self, env: &mut dyn SpeechEnv, c: &crate::career::Career, circuit: &crate::circuit::Circuit) {
        if self.queue_result_line(env, c) {
            env.post(Event::new(0x4008015, &[2]));
            env.trigger(SLOT0);
        }
        let mm = mode_mask(env.app_mode());
        for u in &circuit.unlocks {
            if (0..3).contains(&(u.kind as i32)) {
                let b = match u.value {
                    0 => 0x10,
                    1 => 2,
                    2 => 8,
                    3 => 4,
                    4 => 1,
                    5 => 0x40,
                    6 => 0x80,
                    7 => 0x200,
                    8 => 0x20,
                    0xb => 0x100,
                    _ => 0,
                };
                env.post(Event::new(0x400800d, &[b, mm]));
            }
        }
        env.trigger(SLOT0);
    }

    /// `Speech_OnPlacingChange` 0x2347c8(a, b): 600 frames after the last (+0x7ddc), two players' riders, one a
    /// local player or the listener, slot 1 free, both near the same listener: a speaks if it is a local player, the
    /// listener, b's relation 1, or fond of b (above 60) with a count; b likewise (its relation to a, or fond of a);
    /// both: a `Rand_Fx` bit. When the two riders' speeds along b's heading part by 670.56..1788.16: the speaker
    /// posts 0x3006019 / 0x300601a (a, rival or not) or 0x300601b / 0x300601c (b) with its bit and the other's, at
    /// its position in its voice
    pub fn on_placing_change(&mut self, env: &mut dyn SpeechEnv, a: &SpeechRider, b: &SpeechRider) {
        use crate::ps2f::F;
        if env.race_clock().wrapping_sub(self.placing_time) < 600 {
            return;
        }
        if !a.player() || !b.player() {
            return;
        }
        let l = env.listener();
        if a.kind != 1 && b.kind != 1 && a.id != l && b.id != l {
            return;
        }
        if !env.can_play(1) {
            return;
        }
        let na = env.find_near(a.id);
        if na < 0 || na != env.find_near(b.id) {
            return;
        }
        let a_speaks = a.kind == 1
            || a.id == l
            || env.relation(a.character, b.character) == 1
            || (60.0 < a.attitudes[b.slot as usize] as i32 as f32 && 0 < a.counts[b.slot as usize]);
        let b_speaks = b.kind == 1
            || b.id == l
            || env.relation(b.character, a.character) == 1
            || 60.0 < b.attitudes[a.slot as usize] as i32 as f32;
        if !(a_speaks || b_speaks) {
            return;
        }
        let pick = if b_speaks { if a_speaks { env.rand_fx() & 1 } else { 0 } } else { 1 };
        let (sn, cs) = crate::math::sin_cos(b.heading);
        let along = |r: &SpeechRider| F(cs) * F(r.vel[0]) + F(sn) * F(r.vel[1]) + F(r.vel[2]) * F(0.0);
        let d = (along(a) - along(b)).0;
        if 670.56 < d && d < 1788.16 {
            if pick == 0 {
                let rival = is_rival_or_grudge(env, b, a);
                env.post(Event::new(if rival { 0x300601b } else { 0x300601c }, &[character_bit(b.voice), character_bit(a.voice)]));
                env.trigger(at(1, b.id, b.voice as u32));
            } else {
                let rival = is_rival_or_grudge(env, a, b);
                env.post(Event::new(if rival { 0x3006019 } else { 0x300601a }, &[character_bit(a.voice), character_bit(b.voice)]));
                env.trigger(at(1, a.id, a.voice as u32));
            }
        }
    }
}

/// The trick's name byte as the narration's trick id (`Speech_AnnounceTrick`'s switch on the trick word's bytes 1
/// and 2): 1..0x65, else −1
pub fn trick_name_id(b: u8) -> u32 {
    const T: [u32; 29] = [0xf, 0x15, 0x10, 0x2c, 0x19, 7, 0x11, 4, 10, 0x12, 0x13, 5, 3, 1, 0x16, 0x17, 0xb, 9, 0x68, 0xc, 0x18, 8, 6, 0x14, 0xe, 2, 0x6f, 0x70, 0x23];
    match b {
        1..=29 => T[b as usize - 1],
        30..=100 => 0x71 + (b as u32 - 30),
        101 => 0x1e,
        _ => u32::MAX,
    }
}

/// The trick's byte 0 (its grab or move) as the narration's id: 1..43, 44 (',') 0x3d, else −1
pub fn trick_move_id(b: u8) -> u32 {
    const T: [u32; 43] = [
        0x67, 0x69, 0x6a, 0x6b, 0x6c, 0x48, 0x44, 0x45, 0x3a, 0x46, 0x49, 0x24, 0x3b, 0x25, 0x34, 0x26, 0x28, 0x3c, 0x2f, 0x41, 0x35, 0x36, 0x42, 0x40,
        0x47, 0x4e, 0x4d, 0x4c, 0x4b, 0x2a, 0x27, 0x4f, 0x37, 0x38, 0x31, 0x2e, 0x30, 0x3d, 0x3f, 0x3e, 0x4a, 0x43, 0x29,
    ];
    match b {
        1..=43 => T[b as usize - 1],
        44 => 0x3d,
        _ => u32::MAX,
    }
}

/// The score bands `Speech_AnnounceTrick` rolls by (0x3a38e8: 0, 4000, 10000): the band index, −1 below 0
pub const TRICK_BANDS: [i64; 3] = [0, 4000, 10000];

impl Announcer {
    /// `Speech_AnnounceTrick` 0x232498(rider, the trick's two words, score): no replay, a line allowed (+0x6e04),
    /// a player's rider: roll by the score's band: 0x100201c (4, 2, 2, under 4000 1 else 2); then, the first since
    /// the count was cleared and a trick with parts: 0x100200f with its parts (rotation bits, stance, grabs, flips,
    /// moves) and the name and move ids; the post's answer kept, and when not 0 counted and the second name byte
    /// as 0x1002010. Last (also when it stopped early), roll 0x10 in show-off 3 or 5: 0x1002054 with 1.
    pub fn announce_trick(&mut self, env: &mut dyn SpeechEnv, r: &SpeechRider, trick: [u32; 2], score: i32) {
        if env.replay_playing() || self.trick_ok == 0 || !r.player() {
            return;
        }
        let mut band = 2i32;
        if (score as i64) < TRICK_BANDS[2] {
            band = 1;
            while 0 <= band && (score as i64) < TRICK_BANDS[band as usize] {
                band -= 1;
            }
        }
        if self.roll_band(env, band) {
            env.post(Event::new(0x100201c, &[4, 2, 2, if score < 4000 { 1 } else { 2 }]));
            if self.tricks_spoken == 0 {
                self.trick_line(env, trick);
            }
        }
        if self.roll(env, 0x10) {
            let m = env.mode();
            if m == 5 || m == 3 {
                env.post(Event::new(0x1002054, &[1]));
            }
        }
    }

    /// The roll for a band (−1, a score below 0, reads the ten words before the table: the exe's string data there)
    fn roll_band(&self, env: &mut dyn SpeechEnv, band: i32) -> bool {
        if band < 0 {
            const BEFORE: [i32; 10] = [0x536c6f72, 0x65746174, 0, 0, 0x53633731, 0x436e6970, 0x72746e6f, 0x74536c6f, 0x657461, 0];
            let r = env.rand_fx();
            return crate::fxrand::speech_roll(r, self.level, self.cap, 0, &BEFORE).unwrap_or(false);
        }
        self.roll(env, band as usize)
    }

    fn trick_line(&mut self, env: &mut dyn SpeechEnv, trick: [u32; 2]) {
        let (w0, w1) = (trick[0], trick[1]);
        let mut a = match (w1 >> 16) & 0xf {
            0 | 1 => 2,
            2 | 3 => 1,
            _ => 0,
        };
        if w1 & 0xf0f0 == 0 && w0 & 0xf00_0000 == 0 && w1 & 0xf00 == 0 && w0 & 0xffff == 0 {
            return;
        }
        let mut f = match (w1 >> 27) & 7 {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        match (w1 >> 12) & 0xf {
            0 => a = 2,
            1 => {
                a = 4;
                f |= 4;
            }
            2..=4 => {
                a = 1;
                f |= 4;
            }
            _ => {}
        }
        let bits = |n: u32| if n <= 10 { 1 << n } else { 0 };
        let mut t = bits((w0 >> 24) & 0xf);
        let e = match (w1 >> 24) & 7 {
            1 => 2,
            0 => 1,
            2 => 4,
            3 => 8,
            _ => 0,
        };
        let (mut b, mut c) = (0, 0);
        match (w1 >> 8) & 0xf {
            1 => b = 1,
            2 => b = 2,
            3 => c = 1,
            4 => c = 2,
            _ => {}
        }
        let name = trick_name_id((w0 >> 8) as u8);
        let mv = trick_move_id(w0 as u8);
        if w0 & 0xf000_0000 != 0 {
            let n = w0 >> 28;
            if n <= 10 {
                t = 1 << n;
            }
        }
        let ok = env.post(Event::new(0x100200f, &[t, a, b, c, e, name, mv, f]));
        self.trick_ok = ok;
        if ok != 0 {
            self.tricks_spoken += 1;
            let second = (w0 >> 16) as u8;
            if second != 0 {
                env.post(Event::new(0x1002010, &[trick_name_id(second)]));
            }
        }
    }
}
