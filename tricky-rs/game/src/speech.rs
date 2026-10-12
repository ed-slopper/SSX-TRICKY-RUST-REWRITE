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
    /// `SpeechLib_PostEvent`
    fn post(&mut self, e: Event);
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
/// attitudes to the others (bytes), +0x158 its speed, +0x57f4 its air time, +0x58a8 its tricky count; through its
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
    pub speed: f32,
    pub air: f32,
    pub tricky: i32,
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

    /// `Speech_OnAirEnter` 0x233950(rider, kind): roll 0x15, at 1341.12 cm/s or more, the listener: 0x1002030
    /// with 2 for kind 1, else 1
    pub fn on_air_enter(&self, env: &mut dyn SpeechEnv, r: &SpeechRider, kind: i64) {
        if self.replay_first(env, r, 0x15) && 1341.12 <= r.speed && r.id == env.listener() {
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
