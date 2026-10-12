//! The Trick Tutorial's lesson manager (board row C11b): `cLessonMan` (`LessonMan_Init` 0x14aaa0) and its eleven
//! state handlers, read from SLUS_203.26 (`docs/port-notes/tutorial.md`). The manager is a state machine: a requested
//! state is switched in at the start of its tick (`LessonMan_SwitchState` 0x14bb08: previous handler `Exit`, new handler
//! `Enter`), then the handler's `Update` runs (`cLessonMan::vf1` 0x14b9b8).
//!
//! Everything the handlers touch outside the manager (the pad, the HUD, speech, the camera, the scene, the rider,
//! the recorded input) goes through [`Env`]; the port's screens and the `.inp` player (C11c, C11d) implement it.
//! Checked by the unit tests below, and against the original in the function runner (`tests/lesson.rs`: the real
//! `cLessonMan::vf1` 0x14b9b8, its handlers and helpers on a lesson object, 200,000 steps of random pad edges, flags
//! and state requests, every outside call, the manager's fields and the recording position compared).
//!
//! The race object's own states drive the lesson from outside (`Race_CreateHandlers` 0x1125c8): 6 LoadLesson, 7
//! PreLesson, 8 RestartLesson, 9 SelectLesson, 10 EndLesson (and 1..5, 11 for races). `cLoadLessonHandler::Update`
//! 0x14ecb0 waits for the course's textures (`FUN_00112920`), then `Lesson_RestartRider`, lesson state Intro (2) and
//! race state 7 ([`Lesson::on_load_done`]). `Env::set_mode(3)` is race state 3 (Countdown: the race clock counts up
//! 2.0 a tick to 0, then race state 4, the ride), `set_mode(8)` is RestartLesson (its Enter puts the lesson in
//! Intro, its Update moves the race to 9, SelectLesson, which does nothing). Race states 7 and 9 do nothing, the
//! lesson manager is in charge; `cEndLessonHandler` (10) only counts the clock up 1/60 a tick. (C11b2.) The race side
//! is [`RaceLesson`] (C11b4), checked in `tests/lesson.rs` as well.

/// Button masks of the newly pressed buttons (`Lesson_HandleMenuInput` 0x14d6e0 and the Wait handler).
pub mod btn {
    pub const UP: u32 = 0x10_0008;
    pub const DOWN: u32 = 0x8_0004;
    pub const ACCEPT: u32 = 0x800;
    pub const BACK: u32 = 0x20;
    /// Up / down in the action menu (a second mask pair).
    pub const MENU_UP: u32 = 0x2_0001;
    pub const MENU_DOWN: u32 = 0x4_0002;
}

/// The manager's states (`+0xc`); 0 is none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum State {
    #[default]
    None = 0,
    Init = 1,
    Intro = 2,
    Start = 3,
    Wait = 4,
    Preview = 5,
    PreviewEnd = 6,
    Perform = 7,
    Practice = 8,
    PracticeEnd = 9,
    End = 10,
    Exit = 11,
}

impl State {
    pub fn from_u32(n: u32) -> State {
        use State::*;
        [None, Init, Intro, Start, Wait, Preview, PreviewEnd, Perform, Practice, PracticeEnd, End, Exit]
            .get(n as usize)
            .copied()
            .unwrap_or(None)
    }
}

/// Strings (`Loc_GetString` ids) of the menus.
pub mod text {
    pub const PAGE_TITLE: u32 = 0x22d;
    pub const AFTER_TITLE: u32 = 0x22c;
    pub const ACTIONS: [u32; 4] = [0x199, 0x195, 0x198, 0x18e];
    /// After a demonstration (`FUN_0014d948`).
    pub const PREVIEW_END: [u32; 4] = [0x22c, 0x195, 0x191, 0x18e];
    /// After practice (`FUN_0014d9c8`, three items).
    pub const PRACTICE_END: [u32; 3] = [0x22c, 0x191, 0x18e];
    /// The exit question's three strings (`Dialog_Set`).
    pub const EXIT_DIALOG: [u32; 3] = [0x146, 0x1d6, 0x188];
}

/// The camera script the intro starts (0x36e550).
pub const INTRO_CAM_SCRIPT: &str = "RaceInBlocks2";
/// The three camera picks of practice (words at 0x36e560): `Rand_Fx % 3` chooses one for `lesson+0xac`.
pub const PRACTICE_CAMS: [u32; 3] = [577, 579, 306];
/// The panel position set on entering the Wait menu (0x43d98000) and the end menus (0x43700000), `lesson+0x53c`.
pub const WAIT_PANEL: f32 = 435.0;
pub const END_PANEL: f32 = 240.0;
/// The result menu's panel (`cLessonEndHandler::Enter`, 0x43a50000).
pub const RESULT_PANEL: f32 = 330.0;
/// Pages of the trick book (6 x 5 tricks).
pub const PAGES: i32 = 6;

/// The world outside the manager.
pub trait Env {
    /// The dialog box is open (`hud+0x328`): the Wait handler only polls it.
    fn dialog_active(&self) -> bool;
    /// `cMenuManager::IsActive`, or the HUD's `+0x42c` dialog (`vf1` does nothing while either is up).
    fn blocked(&self) -> bool;
    /// `CamScript_IsRunning` on the game camera.
    fn cam_running(&self) -> bool;
    /// `Dialog_PollButtons`: 1 = confirmed.
    fn dialog_poll(&mut self) -> i64;
    fn dialog_set(&mut self, strings: [u32; 3]);
    /// The buttons newly pressed this tick (the pad's edge mask, `Lesson_HandleMenuInput`).
    fn pressed(&mut self) -> u32;
    fn nav_sound(&mut self);
    fn back_sound(&mut self);
    fn select_sound(&mut self);
    /// `TrickBook_PageFirstTrick(page)`; `page` may be 6 (the end).
    fn page_first_trick(&self, page: i32) -> i32;
    fn hud_show(&mut self, item: u32);
    fn hud_hide(&mut self, item: u32);
    fn speech_clear(&mut self);
    fn speech_line(&mut self, n: u32);
    fn cam_script(&mut self, name: &str);
    /// `Camera_ForceCutFade`.
    fn cam_cut_fade(&mut self);
    /// `Lesson_ResetScene` 0x14d7d0 (delete the node lists, reset the camera, the world and the rider).
    fn reset_scene(&mut self);
    /// The race object's state change (`gApp+0x730 → +0x1c`, slot +0x54): 3 Countdown, 8 RestartLesson (see the
    /// module doc; the lesson's side of 8, going to Intro, is done by [`Lesson::retry_menu`]).
    fn set_mode(&mut self, mode: u32);
    /// `Lesson_RestartRider` 0x14d898: the rider back at the lesson's start point (`Boarder_SetRestartPoint`,
    /// `Boarder_StartLessonWait`).
    fn restart_rider(&mut self);
    /// `FUN_001117f0` + clearing `+0x2c` of the rider's controller + `Boarder_Finish`.
    fn rider_finish(&mut self);
    /// The rider's speed fields (`Lesson_SetupRider` 0x14b768): page 5 gives (1.0, 20.0), else (0, 0).
    fn set_rider_speed(&mut self, a: f32, b: f32);
    /// Swap to the lesson pad map (`true`, +0x2a4 ..) or back to the player's (`false`, +0xdc ..).
    fn pad_map(&mut self, lesson: bool);
    /// Load `data\tutorial\<board><trick+1:02>.inp` into the recording buffer and rewind it; returns its length.
    fn load_recording(&mut self, rider: usize, trick: i32) -> u32;
    /// `Replay_Clear` when the replay exists.
    fn replay_clear(&mut self);
    fn rand_fx(&mut self) -> u32;
    /// The practice handler's test of the trick text (`FUN_00157978(...+0x5820)` non-empty).
    fn trick_text_nonempty(&self) -> bool;
    /// The game object's "done" flag (`gApp+0x730 → +0x54`), set by the Exit handler.
    fn set_done(&mut self);
    /// One step of the lesson course's load (`FUN_00112920` 0x112920: the texture files bound as they arrive, the
    /// speech system ready): true when everything is loaded.
    fn load_step(&mut self) -> bool;
    /// `cEndLessonHandler::Enter` 0x14ed40: the pad device's key (+0x2c) cleared.
    fn clear_pad_key(&mut self);
}

/// The race object's side of the tutorial (`cAI` 0x364738, `Race_CreateHandlers` 0x1125c8): its mode (+0x1c) picks the
/// handler (+0x64); lesson modes are 6 LoadLesson, 7 PreLesson, 8 RestartLesson, 9 SelectLesson, 10 EndLesson (1..5
/// and 11 are the races' and not here). `cAI::vf1` 0x1126f0 runs the handler's `Exit` and the new one's `Enter` when
/// the mode changed, then `Update`, every tick; `cAI::vf9` 0x1125a0 is [`RaceLesson::set_mode`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceLesson {
    /// +0x1c
    pub mode: u32,
    /// +0x20: the mode the handlers last ran for
    pub ran: u32,
    /// +0x64: the handler (by its mode number; 0 none)
    pub handler: u32,
    /// +0x60: the one before, until its `Exit` has run
    pub prev_handler: u32,
    /// +0x18
    pub ticks: u32,
    /// +0x14: the race clock (the end handler counts it up)
    pub clock: f32,
    /// +0x4f8: the course is loading
    pub loading: bool,
}

/// `cEndLessonHandler::Update` 0x14ed68: 1/60 as the game's float.
pub const END_CLOCK_STEP: f32 = 0.016666668;

impl RaceLesson {
    /// `cAI::vf9` 0x1125a0: a new mode is taken at the next tick.
    pub fn set_mode(&mut self, m: u32) {
        if m != self.mode {
            self.ran = self.mode;
            self.prev_handler = self.handler;
            self.create_handlers(m);
        }
    }

    /// `Race_CreateHandlers` 0x1125c8: modes 1..=11 have a handler, any other clears the mode and the handler.
    fn create_handlers(&mut self, m: u32) {
        if (1..=11).contains(&m) {
            self.handler = m;
            self.mode = m;
        } else {
            self.mode = 0;
            self.handler = 0;
        }
    }

    /// `cAI::vf1` 0x1126f0. Only the lesson modes (6..10) do anything here; a race mode's handler is not ported.
    pub fn tick(&mut self, lesson: &mut Lesson, env: &mut dyn Env) {
        if self.ran != self.mode {
            if self.prev_handler != 0 {
                self.exit(self.prev_handler);
            }
            self.enter(lesson, env, self.handler);
            self.prev_handler = 0;
            self.ran = self.mode;
        }
        self.update(lesson, env, self.handler);
        self.ticks += 1;
    }

    fn exit(&mut self, _h: u32) {
        // every lesson handler's Exit does nothing
    }

    fn enter(&mut self, lesson: &mut Lesson, env: &mut dyn Env, h: u32) {
        match h {
            // cLoadLessonHandler::Enter 0x14eca0
            6 => self.loading = true,
            // cRestartLessonHandler::Enter 0x14ec48
            8 => lesson.set_state(env, State::Intro),
            // cEndLessonHandler::Enter 0x14ed40
            10 => env.clear_pad_key(),
            _ => {}
        }
    }

    fn update(&mut self, lesson: &mut Lesson, env: &mut dyn Env, h: u32) {
        match h {
            // cLoadLessonHandler::Update 0x14ecb0: the load step (HUD item 8 shows until it is done), then
            // the rider to the start, the lesson to Intro, the race to PreLesson
            6 => {
                if self.loading {
                    if env.load_step() {
                        env.hud_hide(8);
                        self.loading = false;
                    } else {
                        env.hud_show(8);
                    }
                }
                if !self.loading {
                    env.restart_rider();
                    lesson.set_state(env, State::Intro);
                    // (`Race_CreateHandlers(race, 7)` itself, not `vf9`: the handlers last run for are not noted)
                    self.create_handlers(7);
                }
            }
            // cRestartLessonHandler::Update 0x14ec60: on to SelectLesson
            8 => self.set_mode(9),
            // cEndLessonHandler::Update 0x14ed68
            10 => self.clock += END_CLOCK_STEP,
            // PreLesson (7) and SelectLesson (9) do nothing: the lesson manager is in charge
            _ => {}
        }
    }
}

/// `cLessonMan`'s state (offsets in the comments).
#[derive(Clone, Debug)]
pub struct Lesson {
    pub state: State,             // +0xc
    pub requested: State,         // +0x10
    pub prev: State,              // +0x14
    pub ticks: u32,               // +0x50
    pub attempt_done: bool,       // +0x54
    pub choice: i32,              // +0x58
    pub count: i32,               // +0x5c
    pub last_action: i32,         // +0x60
    pub trick_seen: bool,         // +0x64
    pub items: [u32; 7],          // +0x68, 7 string ids (0 = none)
    pub cam_pick: u32,            // +0xac
    pub menu_level: i32,          // +0xb0: 0 pages, 1 tricks, 2 actions
    pub win_pos: i32,             // +0xb4
    pub win_top: i32,             // +0xb8
    pub trick: i32,               // +0x4e8
    pub rider: usize,             // +0x510
    pub page: i32,                // +0x50c
    pub panel: f32,               // +0x53c
    pub rec_len: u32,             // the recording buffer's end (+0x46c word 1)
    pub rec_pos: u32,             // and its position (word 0)
    /// The handler that is in (Enter has run) and the one before it, as states.
    handler: State,
    prev_handler: State,
}

impl Lesson {
    /// `LessonMan_Init` 0x14aaa0 + `FUN_0014b5a8` 0x14b5a8: all zero, then `LessonMan_SetState(1)`; the menu
    /// window starts at 0, the rider is the player's.
    pub fn new(rider: usize) -> Self {
        let mut l = Lesson {
            state: State::None,
            requested: State::None,
            prev: State::None,
            ticks: 0,
            attempt_done: false,
            choice: 0,
            count: 0,
            last_action: 0,
            trick_seen: false,
            items: [0; 7],
            cam_pick: 0,
            menu_level: 0,
            win_pos: 0,
            win_top: 0,
            trick: 0,
            rider,
            page: 0,
            panel: 0.0,
            rec_len: 0,
            rec_pos: 0,
            handler: State::None,
            prev_handler: State::None,
        };
        l.requested = State::Init;
        l
    }

    /// `LessonMan_SetState` 0x14baa8. State 11 cuts the camera and hides HUD item 10 first.
    pub fn set_state(&mut self, env: &mut dyn Env, s: State) {
        if s == State::Exit {
            env.cam_cut_fade();
            env.hud_hide(10);
        }
        self.requested = s;
    }

    /// `LessonMan_SwitchState` 0x14bb08.
    fn switch(&mut self, s: State) {
        if s == self.state {
            return;
        }
        self.prev = self.state;
        self.prev_handler = self.handler;
        self.handler = s;
        self.state = s;
    }

    /// `cLessonMan::vf1` 0x14b9b8: one tick.
    pub fn tick(&mut self, env: &mut dyn Env) {
        if env.blocked() {
            return;
        }
        self.ticks += 1;
        if self.requested != State::None {
            let r = self.requested;
            self.switch(r);
            self.requested = State::None;
        }
        if self.prev != self.state {
            if self.prev_handler != State::None {
                let h = self.prev_handler;
                self.exit(env, h);
            }
            self.ticks = 0;
            let h = self.handler;
            self.enter(env, h);
            self.prev_handler = State::None;
            self.prev = self.state;
        }
        let h = self.handler;
        self.update(env, h);
    }

    // ---- shared pieces ----

    /// `Lesson_SetupRider` 0x14b768.
    fn setup_rider(&mut self, env: &mut dyn Env) {
        self.attempt_done = false;
        self.trick_seen = false;
        self.cam_pick = 0;
        if self.page == 5 {
            env.set_rider_speed(1.0, 20.0);
        } else {
            env.set_rider_speed(0.0, 0.0);
        }
    }

    /// `FUN_0014daa8` 0x14daa8: the menu of the current level.
    fn refresh_menu(&mut self) {
        match self.menu_level {
            // (these two leave the other strings as they were: the list is cleared only for the action menu)
            0 | 1 => {
                self.choice = 0;
                self.count = 1;
                self.items[0] = text::PAGE_TITLE;
            }
            2 => {
                self.count = 4;
                self.choice = self.last_action;
                self.items = [0; 7];
                for (i, s) in text::ACTIONS.iter().enumerate() {
                    self.items[i] = *s;
                }
            }
            _ => {}
        }
    }

    fn end_menu(&mut self, strings: &[u32]) {
        self.choice = 0;
        self.count = strings.len() as i32;
        self.items = [0; 7];
        for (i, s) in strings.iter().enumerate() {
            self.items[i] = *s;
        }
    }

    /// `Lesson_HandleMenuInput` 0x14d6e0: returns the newly pressed buttons; moves the choice in the action menu.
    fn menu_input(&mut self, env: &mut dyn Env) -> u32 {
        let b = env.pressed();
        if self.menu_level == 2 {
            if b & btn::MENU_UP != 0 {
                if self.choice < 1 {
                    return b;
                }
                env.nav_sound();
                self.choice -= 1;
            } else if b & btn::MENU_DOWN != 0 {
                if self.count - 1 <= self.choice {
                    return b;
                }
                env.nav_sound();
                self.choice += 1;
            }
        }
        b
    }

    /// The HUD and control mode every attempt starts with (Preview, Perform, Practice Enter; EndAttempt).
    fn attempt_hud(&mut self, env: &mut dyn Env) {
        env.hud_hide(10);
        env.hud_show(0xe);
        env.set_mode(3);
    }

    /// `LessonMan_EndAttempt` 0x14b638.
    pub fn end_attempt(&mut self, env: &mut dyn Env) {
        env.reset_scene();
        self.rec_pos = 0;
        env.hud_hide(10);
        env.hud_show(0xe);
        env.set_mode(3);
        match self.state {
            State::Preview | State::Perform | State::Practice => self.setup_rider(env),
            State::PreviewEnd => self.set_state(env, State::Preview),
            State::PracticeEnd => self.set_state(env, State::Practice),
            State::End => self.set_state(env, State::Perform),
            _ => self.set_state(env, State::Preview),
        }
    }

    // ---- the handlers ----

    fn enter(&mut self, env: &mut dyn Env, h: State) {
        match h {
            State::Intro => {
                env.speech_clear();
                env.speech_line(0);
                env.cam_script(INTRO_CAM_SCRIPT);
                env.hud_show(10);
            }
            State::Wait => {
                self.refresh_menu();
                self.panel = WAIT_PANEL;
            }
            State::Preview => {
                env.pad_map(true);
                self.setup_rider(env);
                env.speech_clear();
                env.speech_line(1);
                env.select_sound();
                self.rec_len = env.load_recording(self.rider, self.trick);
                self.rec_pos = 0;
                self.attempt_hud(env);
            }
            State::PreviewEnd => {
                env.hud_show(10);
                env.hud_hide(0xe);
                env.rider_finish();
                self.end_menu(&text::PREVIEW_END);
                self.panel = END_PANEL;
            }
            State::Perform => {
                self.setup_rider(env);
                env.select_sound();
                env.speech_clear();
                env.speech_line(2);
                self.rec_len = env.load_recording(self.rider, self.trick);
                self.rec_pos = 0;
                self.attempt_hud(env);
                env.replay_clear();
            }
            State::Practice => {
                self.setup_rider(env);
                let r = env.rand_fx();
                self.cam_pick = PRACTICE_CAMS[(r % 3) as usize];
                env.speech_clear();
                env.hud_hide(10);
                env.hud_show(0xe);
                env.set_mode(3);
            }
            State::PracticeEnd => {
                env.hud_show(10);
                env.hud_hide(0xe);
                env.rider_finish();
                self.end_menu(&text::PRACTICE_END);
                self.panel = END_PANEL;
                env.replay_clear();
            }
            State::End => {
                env.hud_show(10);
                env.hud_hide(0xe);
                env.rider_finish();
                self.end_menu(&text::PRACTICE_END);
                self.panel = RESULT_PANEL;
            }
            // Init, Start, Exit: nothing
            _ => {}
        }
    }

    fn exit(&mut self, env: &mut dyn Env, h: State) {
        match h {
            State::Preview => env.pad_map(false),
            // `cLessonPerformHandler::Exit` 0x14e6a8: one line if the trick was seen, another if not
            State::Perform => env.speech_line(if self.trick_seen { 4 } else { 5 }),
            _ => {}
        }
    }

    fn update(&mut self, env: &mut dyn Env, h: State) {
        match h {
            State::Intro => self.set_state(env, State::Start),
            State::Wait => self.update_wait(env),
            State::Preview => {
                if self.rec_pos >= self.rec_len {
                    self.attempt_done = true;
                }
                if self.attempt_done {
                    self.set_state(env, State::PreviewEnd);
                }
            }
            State::PreviewEnd => {
                let b = self.menu_input(env);
                if b & btn::ACCEPT == 0 {
                    return;
                }
                match self.choice {
                    0 => self.retry_menu(env),
                    1 => {
                        env.select_sound();
                        env.reset_scene();
                        self.rec_pos = 0;
                        env.hud_hide(10);
                        env.hud_show(0xe);
                        env.set_mode(3);
                        self.set_state(env, State::Perform);
                    }
                    2 => {
                        env.select_sound();
                        self.end_attempt(env);
                    }
                    3 => {
                        env.select_sound();
                        self.set_state(env, State::Exit);
                    }
                    _ => {}
                }
            }
            State::Start => {
                if !env.cam_running() {
                    self.set_state(env, State::Wait);
                }
            }
            State::Perform => {
                if self.attempt_done {
                    self.set_state(env, State::End);
                }
            }
            State::Practice => {
                if self.attempt_done {
                    self.set_state(env, State::PracticeEnd);
                    return;
                }
                if env.trick_text_nonempty() {
                    self.trick_seen = true;
                }
            }
            State::PracticeEnd => {
                let b = self.menu_input(env);
                if b & btn::ACCEPT == 0 {
                    return;
                }
                match self.choice {
                    0 => self.retry_menu(env),
                    1 => {
                        env.select_sound();
                        self.end_attempt(env);
                    }
                    2 => {
                        env.select_sound();
                        self.set_state(env, State::Exit);
                    }
                    _ => {}
                }
            }
            State::End => {
                let b = self.menu_input(env);
                if b & btn::ACCEPT == 0 {
                    return;
                }
                match self.choice {
                    0 => self.retry_menu(env),
                    1 => {
                        env.select_sound();
                        self.end_attempt(env);
                    }
                    2 => {
                        env.select_sound();
                        self.set_state(env, State::Exit);
                    }
                    _ => {}
                }
            }
            State::Exit => {
                env.set_done();
                env.hud_hide(10);
                env.hud_hide(0xe);
            }
            _ => {}
        }
    }

    /// `cLoadLessonHandler::Update` 0x14ecb0, once the course has loaded: rider to the start, lesson to Intro.
    pub fn on_load_done(&mut self, env: &mut dyn Env) {
        env.restart_rider();
        self.set_state(env, State::Intro);
    }

    /// Choice 0 of the end menus: reset the scene, back to the trick list (menu level 1), race state 8
    /// (`cRestartLessonHandler::Enter` 0x14ec48 puts the lesson in Intro).
    fn retry_menu(&mut self, env: &mut dyn Env) {
        env.select_sound();
        env.reset_scene();
        self.menu_level = 1;
        env.set_mode(8);
        self.set_state(env, State::Intro);
    }

    /// `cLessonWaitHandler::Update` 0x14dcc0.
    fn update_wait(&mut self, env: &mut dyn Env) {
        if env.dialog_active() {
            if env.dialog_poll() == 1 {
                self.set_state(env, State::Exit);
            }
            return;
        }
        let b = self.menu_input(env);
        match self.menu_level {
            1 => {
                if b & btn::UP != 0 {
                    if self.trick <= env.page_first_trick(self.page) {
                        return;
                    }
                    env.nav_sound();
                    self.trick -= 1;
                    if self.win_pos > 0 {
                        self.win_pos -= 1;
                    } else {
                        self.win_top -= 1;
                    }
                    return;
                }
                if b & btn::DOWN != 0 {
                    if env.page_first_trick(self.page + 1) - 1 <= self.trick {
                        return;
                    }
                    env.nav_sound();
                    self.trick += 1;
                    if self.win_pos < 4 {
                        self.win_pos += 1;
                    } else {
                        self.win_top += 1;
                    }
                    return;
                }
                if b & btn::ACCEPT != 0 {
                    env.nav_sound();
                    self.menu_level = 2;
                    self.refresh_menu();
                    return;
                }
                if b & btn::BACK == 0 {
                    return;
                }
                env.back_sound();
                self.menu_level = 0;
                self.refresh_menu();
            }
            0 => {
                let np = if b & btn::UP != 0 {
                    if self.page < 1 {
                        return;
                    }
                    env.nav_sound();
                    self.page - 1
                } else if b & btn::DOWN != 0 {
                    if 4 < self.page {
                        return;
                    }
                    env.nav_sound();
                    self.page + 1
                } else if b & btn::ACCEPT != 0 {
                    env.nav_sound();
                    self.menu_level = 1;
                    self.refresh_menu();
                    return;
                } else {
                    return;
                };
                self.page = np;
                let first = env.page_first_trick(np);
                self.win_pos = 0;
                self.win_top = first;
                self.trick = first;
            }
            2 => {
                if b & btn::BACK != 0 {
                    env.back_sound();
                    self.menu_level = 1;
                    self.refresh_menu();
                    return;
                }
                if b & btn::ACCEPT == 0 {
                    return;
                }
                match self.choice {
                    0 => {
                        env.select_sound();
                        self.set_state(env, State::Preview);
                    }
                    1 => {
                        env.select_sound();
                        self.set_state(env, State::Perform);
                    }
                    2 => {
                        env.select_sound();
                        self.set_state(env, State::Practice);
                    }
                    3 => {
                        env.select_sound();
                        env.dialog_set(text::EXIT_DIALOG);
                    }
                    _ => {
                        self.last_action = self.choice;
                        return;
                    }
                }
                self.last_action = self.choice;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct E {
        log: Vec<String>,
        presses: Vec<u32>,
        dialog: bool,
        poll: i64,
        blocked: bool,
        rand: u32,
        text: bool,
    }
    impl Env for E {
        fn cam_running(&self) -> bool { false }
        fn dialog_active(&self) -> bool { self.dialog }
        fn blocked(&self) -> bool { self.blocked }
        fn dialog_poll(&mut self) -> i64 { self.poll }
        fn dialog_set(&mut self, _s: [u32; 3]) { self.dialog = true; self.log.push("dialog".into()) }
        fn pressed(&mut self) -> u32 { if self.presses.is_empty() { 0 } else { self.presses.remove(0) } }
        fn nav_sound(&mut self) { self.log.push("nav".into()) }
        fn back_sound(&mut self) { self.log.push("back".into()) }
        fn select_sound(&mut self) { self.log.push("sel".into()) }
        fn page_first_trick(&self, page: i32) -> i32 { page * 5 }
        fn hud_show(&mut self, i: u32) { self.log.push(format!("show{i}")) }
        fn hud_hide(&mut self, i: u32) { self.log.push(format!("hide{i}")) }
        fn speech_clear(&mut self) { self.log.push("sclr".into()) }
        fn speech_line(&mut self, n: u32) { self.log.push(format!("line{n}")) }
        fn cam_script(&mut self, n: &str) { self.log.push(format!("cam {n}")) }
        fn cam_cut_fade(&mut self) { self.log.push("cutfade".into()) }
        fn reset_scene(&mut self) { self.log.push("reset".into()) }
        fn set_mode(&mut self, m: u32) { self.log.push(format!("mode{m}")) }
        fn restart_rider(&mut self) { self.log.push("restart".into()) }
        fn rider_finish(&mut self) { self.log.push("finish".into()) }
        fn set_rider_speed(&mut self, a: f32, b: f32) { self.log.push(format!("speed{a},{b}")) }
        fn pad_map(&mut self, l: bool) { self.log.push(format!("pad{l}")) }
        fn load_recording(&mut self, _r: usize, t: i32) -> u32 { self.log.push(format!("inp{t}")); 3 }
        fn replay_clear(&mut self) { self.log.push("rclr".into()) }
        fn rand_fx(&mut self) -> u32 { self.rand }
        fn trick_text_nonempty(&self) -> bool { self.text }
        fn set_done(&mut self) { self.log.push("done".into()) }
        fn load_step(&mut self) -> bool { true }
        fn clear_pad_key(&mut self) { self.log.push("clearkey".into()) }
    }

    fn run_to_wait(l: &mut Lesson, e: &mut E) {
        l.tick(e); // Init (set by the constructor)
        assert_eq!(l.state, State::Init);
        l.on_load_done(e); // the race's LoadLesson handler
        assert!(e.log.contains(&"restart".to_string()));
        l.tick(e); // Intro: Enter, Update requests Start
        assert!(e.log.contains(&"cam RaceInBlocks2".to_string()));
        l.tick(e); // Start Enter, camera script over: Update requests Wait
        assert_eq!(l.state, State::Start);
        l.tick(e);
        assert_eq!(l.state, State::Wait);
        assert_eq!(l.panel, WAIT_PANEL);
    }

    #[test]
    fn starts_in_init_and_runs_the_intro() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        run_to_wait(&mut l, &mut e);
        assert_eq!(l.items[0], text::PAGE_TITLE);
    }

    #[test]
    fn pages_and_tricks_window() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        run_to_wait(&mut l, &mut e);
        e.presses = vec![btn::UP]; // page 0: up does nothing
        l.tick(&mut e);
        assert_eq!(l.page, 0);
        e.presses = vec![btn::DOWN];
        l.tick(&mut e);
        assert_eq!((l.page, l.trick, l.win_pos, l.win_top), (1, 5, 0, 5));
        e.presses = vec![btn::ACCEPT];
        l.tick(&mut e);
        assert_eq!(l.menu_level, 1);
        for _ in 0..4 {
            e.presses = vec![btn::DOWN];
            l.tick(&mut e);
        }
        assert_eq!((l.trick, l.win_pos), (9, 4));
        e.presses = vec![btn::DOWN]; // the last trick of the page: stays
        l.tick(&mut e);
        assert_eq!(l.trick, 9);
        e.presses = vec![btn::BACK];
        l.tick(&mut e);
        assert_eq!(l.menu_level, 0);
    }

    #[test]
    fn preview_plays_the_recording_then_asks() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        run_to_wait(&mut l, &mut e);
        e.presses = vec![btn::ACCEPT, btn::ACCEPT];
        l.tick(&mut e);
        l.tick(&mut e);
        assert_eq!(l.menu_level, 2);
        e.presses = vec![btn::ACCEPT]; // choice 0: preview
        l.tick(&mut e);
        l.tick(&mut e); // Preview Enter
        assert_eq!(l.state, State::Preview);
        assert!(e.log.contains(&"inp0".to_string()) && e.log.contains(&"padtrue".to_string()));
        l.rec_pos = 3;
        l.tick(&mut e);
        l.tick(&mut e);
        assert_eq!(l.state, State::PreviewEnd);
        assert!(e.log.contains(&"padfalse".to_string()));
        assert_eq!(l.panel, END_PANEL);
        assert_eq!(l.count, 4);
    }

    #[test]
    fn practice_picks_a_camera_and_ends_when_done() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        e.rand = 4; // 4 % 3 = 1
        run_to_wait(&mut l, &mut e);
        l.set_state(&mut e, State::Practice);
        l.tick(&mut e);
        assert_eq!(l.cam_pick, PRACTICE_CAMS[1]);
        e.text = true;
        l.tick(&mut e);
        assert!(l.trick_seen);
        l.attempt_done = true;
        l.tick(&mut e);
        l.tick(&mut e);
        assert_eq!(l.state, State::PracticeEnd);
    }

    #[test]
    fn exit_cuts_and_sets_done() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        run_to_wait(&mut l, &mut e);
        l.set_state(&mut e, State::Exit);
        assert!(e.log.contains(&"cutfade".to_string()));
        l.tick(&mut e);
        assert!(e.log.contains(&"done".to_string()));
    }

    #[test]
    fn perform_ends_into_the_result_menu() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        run_to_wait(&mut l, &mut e);
        l.menu_level = 2; // as after choosing an action in the Wait menu
        l.set_state(&mut e, State::Perform);
        l.tick(&mut e);
        l.attempt_done = true;
        l.trick_seen = true;
        l.tick(&mut e);
        l.tick(&mut e);
        assert_eq!(l.state, State::End);
        assert!(e.log.contains(&"line4".to_string()));
        assert_eq!((l.count, l.panel), (3, RESULT_PANEL));
        e.presses = vec![btn::MENU_DOWN, btn::ACCEPT];
        l.tick(&mut e);
        l.tick(&mut e); // choice 1: end attempt, back to Perform
        l.tick(&mut e);
        assert_eq!(l.state, State::Perform);
    }

    #[test]
    fn retry_goes_back_through_the_intro() {
        let mut l = Lesson::new(0);
        let mut e = E::default();
        run_to_wait(&mut l, &mut e);
        l.set_state(&mut e, State::Perform);
        l.menu_level = 2;
        l.tick(&mut e);
        l.attempt_done = true;
        l.tick(&mut e);
        l.tick(&mut e);
        assert_eq!(l.state, State::End);
        e.presses = vec![btn::ACCEPT]; // choice 0: retry
        l.tick(&mut e);
        assert_eq!(l.menu_level, 1);
        assert!(e.log.contains(&"mode8".to_string()));
        l.tick(&mut e);
        assert_eq!(l.state, State::Intro);
        l.tick(&mut e);
        assert_eq!(l.state, State::Start);
        l.tick(&mut e);
        assert_eq!(l.state, State::Wait);
        assert_eq!(l.menu_level, 1);
    }

    #[test]
    fn blocked_manager_does_nothing() {
        let mut l = Lesson::new(0);
        let mut e = E { blocked: true, ..Default::default() };
        l.tick(&mut e);
        assert_eq!(l.state, State::None);
    }
}
