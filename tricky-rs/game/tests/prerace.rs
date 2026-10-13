//! The pre-race sequence (board row G11c) against the game in the function runner (TRICKY_ELF; skipped without
//! it): `cPreRaceHandler::Enter`, several frames of `Update`, and `Exit` on random careers, races and scene objects,
//! with the engine's calls (cameras, HUD, sound, riders, pads) scripted the same on both sides: the handler's
//! state, the career block, the scene object, the pads and every call in order compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::career::{Career, CAREER_BASE, CAREER_SIZE};
use tricky_game::cheats::Cheats;
use tricky_game::prerace::{EndRace, EndRaceEnv, PreRace, PreRaceEnv, RaceView};
use tricky_game::rand::Streams;
use tricky_game::scene::{RaceScene, SceneEnv, Slot, NO_RIDER, SCENES, SLOTS};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Ev {
    Streaming,
    CamStop,
    CamStart(String),
    CamRunning,
    SpeechMode(u32, u32),
    InitSamples,
    Sample(i32),
    StopScript,
    FreeSamples,
    FadeIn,
    FadeOut,
    RiderReset(usize),
    Place(usize, bool),
    HudShow(u32),
    HudHide(u32),
    StopIntro,
    RaceStart,
    RaceEntered,
    Overlays,
    SingleView,
    RestoreViews,
    WorldScript(String),
    Pressed(i32),
    Device(i32),
    Rider,
    Marker(u32, i32),
    ClearAi,
    MoveLeader,
    CheckRecords,
    Freeze(usize),
    OpenMenu,
    ReplaySounds,
    Restart,
}

#[derive(Clone, Default)]
struct World {
    loading: bool,
    cam_running: bool,
    app_mode: u32,
    devices: i32,
    pressed: Vec<bool>,
    /// latched, held, +0x2c
    pads: Vec<(u32, u32, u32)>,
    players: Vec<i8>,
    rider0: u32,
    frames: Vec<(i32, i32)>,
    counts: [[i32; 6]; 6],
    place: [i32; 6],
    time: [i32; 6],
    score: [i32; 6],
    results_shown: bool,
    results_busy: bool,
    log: Vec<Ev>,
}

impl World {
    fn frame(&self, clip: i32) -> i32 {
        self.frames.iter().find(|(c, _)| *c == clip).map_or(-1, |(_, f)| *f)
    }
}

impl SceneEnv for World {
    fn race_rider0(&mut self) -> u32 {
        self.log.push(Ev::Rider);
        self.rider0
    }
    fn first_marker_frame(&mut self, rider: u32, clip: i32) -> i32 {
        self.log.push(Ev::Marker(rider, clip));
        self.frame(clip)
    }
}

impl PreRaceEnv for World {
    fn update_streaming(&mut self) {
        self.log.push(Ev::Streaming);
    }
    fn loading(&mut self) -> bool {
        self.loading
    }
    fn cam_stop(&mut self) {
        self.log.push(Ev::CamStop);
    }
    fn cam_start(&mut self, script: &str) {
        self.log.push(Ev::CamStart(script.to_string()));
    }
    fn cam_running(&mut self) -> bool {
        self.log.push(Ev::CamRunning);
        self.cam_running
    }
    fn speech_mode(&mut self, a: u32, b: u32) {
        self.log.push(Ev::SpeechMode(a, b));
    }
    fn init_char_samples(&mut self) {
        self.log.push(Ev::InitSamples);
    }
    fn cast_sample(&mut self, v: i32) {
        self.log.push(Ev::Sample(v));
    }
    fn stop_char_script(&mut self) {
        self.log.push(Ev::StopScript);
    }
    fn free_char_samples(&mut self) {
        self.log.push(Ev::FreeSamples);
    }
    fn fade_in(&mut self) {
        self.log.push(Ev::FadeIn);
    }
    fn fade_out(&mut self) {
        self.log.push(Ev::FadeOut);
    }
    fn rider_reset(&mut self, i: usize) {
        self.log.push(Ev::RiderReset(i));
    }
    fn place_at_start(&mut self, i: usize, gate: bool) {
        self.log.push(Ev::Place(i, gate));
    }
    fn hud_show(&mut self, item: u32) {
        self.log.push(Ev::HudShow(item));
    }
    fn hud_hide(&mut self, item: u32) {
        self.log.push(Ev::HudHide(item));
    }
    fn stop_intro_stream(&mut self) {
        self.log.push(Ev::StopIntro);
    }
    fn race_start(&mut self) {
        self.log.push(Ev::RaceStart);
    }
    fn race_entered(&mut self) {
        self.log.push(Ev::RaceEntered);
    }
    fn player_of(&mut self, k: usize) -> i8 {
        self.players[k]
    }
    fn overlays_hidden(&mut self) {
        self.log.push(Ev::Overlays);
    }
    fn single_view(&mut self) {
        self.log.push(Ev::SingleView);
    }
    fn restore_views(&mut self) {
        self.log.push(Ev::RestoreViews);
    }
    fn run_world_script(&mut self, name: &str) {
        self.log.push(Ev::WorldScript(name.to_string()));
    }
    fn device_count(&mut self) -> i32 {
        self.devices
    }
    fn device_pressed(&mut self, i: i32) -> bool {
        self.log.push(Ev::Pressed(i));
        self.pressed[i as usize]
    }
    fn take_device(&mut self, i: i32) -> (u32, u32) {
        self.log.push(Ev::Device(i));
        let p = &mut self.pads[i as usize];
        p.2 = 0;
        (p.0, p.1)
    }
    fn latch_device(&mut self, i: i32, latched: u32) {
        self.pads[i as usize].0 = latched;
    }
    fn app_mode(&mut self) -> u32 {
        self.app_mode
    }
}

impl EndRaceEnv for World {
    fn clear_ai_targets(&mut self) {
        self.log.push(Ev::ClearAi);
    }
    fn move_leader_to_slot0(&mut self) {
        self.log.push(Ev::MoveLeader);
    }
    fn check_new_records(&mut self, _c: &mut Career) {
        self.log.push(Ev::CheckRecords);
    }
    fn freeze(&mut self, i: usize) {
        self.log.push(Ev::Freeze(i));
    }
    fn rider_count(&mut self, i: usize, j: usize) -> i32 {
        self.counts[i][j]
    }
    fn rider_place(&mut self, i: usize) -> i32 {
        self.place[i]
    }
    fn rider_time(&mut self, i: usize) -> i32 {
        self.time[i]
    }
    fn rider_score(&mut self, i: usize) -> i32 {
        self.score[i]
    }
    fn results_shown(&mut self) -> bool {
        self.results_shown
    }
    fn results_busy(&mut self) -> bool {
        self.results_busy
    }
    fn open_results_menu(&mut self) {
        self.log.push(Ev::OpenMenu);
    }
    fn replay_rider_sounds(&mut self) {
        self.log.push(Ev::ReplaySounds);
    }
    fn restart_race(&mut self) {
        self.log.push(Ev::Restart);
    }
}

const SCENE_OBJ: u32 = 0x32edc0;

fn write_scene(r: &mut Runner, o: &RaceScene) {
    let at = SCENE_OBJ;
    r.mem.write_u32(at, o.current as u32);
    for i in 0..SCENES {
        for j in 0..SLOTS {
            let s = o.slots[i][j];
            let p = at + 4 + 0x48 * i as u32 + 12 * j as u32;
            r.mem.write_u32(p, s.a as u32);
            r.mem.write_u32(p + 4, s.b as u32);
            r.mem.write_u32(p + 8, s.pick as u32);
        }
    }
    // (the selection the scene names its script from: the race's +0x33c, career +0x2b18)
    r.mem.write_u32(at + 0x43c, CAREER_BASE + 0x2b18);
    for (k, c) in o.cast.iter().enumerate() {
        r.mem.write_u32(at + 0x440 + 4 * k as u32, *c as u32);
    }
    for (k, c) in o.riders.iter().enumerate() {
        r.mem.write_u32(at + 0x458 + 4 * k as u32, *c as u32);
    }
}

fn read_scene(r: &mut Runner) -> RaceScene {
    let at = SCENE_OBJ;
    let mut o = RaceScene { current: r.mem.read_u32(at) as i32, has_selection: true, ..Default::default() };
    for i in 0..SCENES {
        for j in 0..SLOTS {
            let p = at + 4 + 0x48 * i as u32 + 12 * j as u32;
            o.slots[i][j] = Slot { a: r.mem.read_u32(p) as i32, b: r.mem.read_u32(p + 4) as i32, pick: r.mem.read_u32(p + 8) as i32 };
        }
    }
    for k in 0..SLOTS {
        o.cast[k] = r.mem.read_u32(at + 0x440 + 4 * k as u32) as i32;
    }
    for k in 0..90 {
        o.riders[k] = r.mem.read_u32(at + 0x458 + 4 * k as u32) as i32;
    }
    o
}

fn read_string(r: &mut Runner, p: u32) -> String {
    let mut s = String::new();
    for k in 0.. {
        let b = (r.mem.read_u32((p + k) & !3) >> (8 * ((p + k) & 3))) as u8;
        if b == 0 {
            break;
        }
        s.push(b as char);
    }
    s
}

// fake code addresses for the virtual calls
const RACE_ENTERED: u32 = 0x01f0_0000;
const RACE_START: u32 = 0x01f0_0010;
const RIDER_RESET: u32 = 0x01f0_0020;

#[test]
fn prerace_is_the_original() {
    run(false);
}

#[test]
fn endrace_is_the_original() {
    run(true);
}

fn run(post: bool) {
    let Some(mut r) = Runner::from_env() else { return };
    let w = Rc::new(RefCell::new(World::default()));
    // the riders and their place in the race (for the logs)
    let riders: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x5a00)).collect();
    let idx = {
        let riders = riders.clone();
        move |p: u32| riders.iter().position(|a| *a == p).unwrap_or(99)
    };
    macro_rules! log_stub {
        ($addr:expr, $ev:expr) => {{
            let x = w.clone();
            r.stub($addr, move |_, _| x.borrow_mut().log.push($ev));
        }};
    }
    log_stub!(0x112920, Ev::Streaming);
    log_stub!(0x175eb8, Ev::CamStop);
    log_stub!(0x2355c8, Ev::InitSamples);
    log_stub!(0x235b70, Ev::StopScript);
    log_stub!(0x2356a0, Ev::FreeSamples);
    log_stub!(0x1ff2b0, Ev::FadeIn);
    log_stub!(0x1ff4c8, Ev::FadeOut);
    log_stub!(0x214e08, Ev::StopIntro);
    log_stub!(0x1a7758, Ev::Overlays);
    log_stub!(0x181d60, Ev::SingleView);
    log_stub!(0x181d50, Ev::RestoreViews);
    log_stub!(RACE_ENTERED, Ev::RaceEntered);
    log_stub!(RACE_START, Ev::RaceStart);
    log_stub!(0x115a30, Ev::ClearAi);
    log_stub!(0x180848, Ev::MoveLeader);
    log_stub!(0x164fc0, Ev::CheckRecords);
    log_stub!(0x1ef990, Ev::OpenMenu);
    log_stub!(0x193c68, Ev::ReplaySounds);
    log_stub!(0x180a10, Ev::Restart);
    let x = w.clone();
    let id = idx.clone();
    r.stub(0x11afa8, move |c, _| x.borrow_mut().log.push(Ev::Freeze(id(c.gpr(4) as u32))));
    // the results overlay: +0x42c → +0x18 busy
    let overlay = r.mem.alloc(0x500);
    let overlay_in = r.mem.alloc(0x40);
    r.mem.write_u32(overlay + 0x42c, overlay_in);
    r.stub(0x1a7be0, move |c, _| c.set_gpr(2, overlay as u64));
    let x = w.clone();
    r.stub(0x1814a8, move |c, _| {
        x.borrow_mut().log.push(Ev::Rider);
        let v = x.borrow().rider0;
        c.set_gpr(2, v as u64);
    });
    let x = w.clone();
    r.stub(0x15fb08, move |c, _| {
        let (rider, clip) = ((c.gpr(4) as u32).wrapping_sub(0x46c0), c.gpr(5) as i32);
        x.borrow_mut().log.push(Ev::Marker(rider, clip));
        let f = x.borrow().frame(clip);
        c.set_gpr(2, f as i64 as u64);
    });
    let x = w.clone();
    r.stub(0x175de0, move |c, m| {
        let p = c.gpr(5) as u32;
        let mut s = String::new();
        for k in 0.. {
            let b = (m.read_u32((p + k) & !3) >> (8 * ((p + k) & 3))) as u8;
            if b == 0 {
                break;
            }
            s.push(b as char);
        }
        x.borrow_mut().log.push(Ev::CamStart(s));
    });
    let x = w.clone();
    r.stub(0x13bad8, move |c, m| {
        let p = c.gpr(4) as u32;
        let mut s = String::new();
        for k in 0.. {
            let b = (m.read_u32((p + k) & !3) >> (8 * ((p + k) & 3))) as u8;
            if b == 0 {
                break;
            }
            s.push(b as char);
        }
        x.borrow_mut().log.push(Ev::WorldScript(s));
    });
    let x = w.clone();
    r.stub(0x175e50, move |c, _| {
        x.borrow_mut().log.push(Ev::CamRunning);
        let v = x.borrow().cam_running;
        c.set_gpr(2, v as u64);
    });
    let x = w.clone();
    r.stub(0x22e400, move |c, _| x.borrow_mut().log.push(Ev::SpeechMode(c.gpr(5) as u32, c.gpr(6) as u32)));
    let x = w.clone();
    r.stub(0x2356f0, move |c, _| x.borrow_mut().log.push(Ev::Sample(c.gpr(5) as i32)));
    let x = w.clone();
    let id = idx.clone();
    r.stub(RIDER_RESET, move |c, _| x.borrow_mut().log.push(Ev::RiderReset(id(c.gpr(4) as u32))));
    let x = w.clone();
    let id = idx.clone();
    r.stub(0x11a840, move |c, _| x.borrow_mut().log.push(Ev::Place(id(c.gpr(4) as u32), c.gpr(5) != 0)));
    let x = w.clone();
    r.stub(0x1a7848, move |c, _| x.borrow_mut().log.push(Ev::HudShow(c.gpr(5) as u32)));
    let x = w.clone();
    r.stub(0x1a7860, move |c, _| x.borrow_mut().log.push(Ev::HudHide(c.gpr(5) as u32)));
    let x = w.clone();
    r.stub(0x17a898, move |c, _| {
        let i = c.gpr(5) as i32;
        x.borrow_mut().log.push(Ev::Pressed(i));
        let v = x.borrow().pressed[i as usize];
        c.set_gpr(2, v as u64);
    });
    let pads = r.mem.alloc(0x40 * 8);
    let x = w.clone();
    r.stub(0x17a830, move |c, _| {
        let i = c.gpr(5) as i32;
        x.borrow_mut().log.push(Ev::Device(i));
        c.set_gpr(2, (pads + 0x40 * i as u32) as u64);
    });
    for a in [0x112848u32, 0x110f28, 0x112af0, 0x110980, 0x1109b0, 0x110a40, 0x110ab8, 0x1699a0, 0x169b60, 0x169bb8, 0x164840, 0x164288, 0x164318, 0x167010, 0x166550, 0x16baa8, 0x16ba60, 0x166fa0, 0x166fe0, 0x166580, 0x164330] {
        r.allow(a);
    }
    // the game's objects
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x100);
    let input = r.mem.alloc(0x40);
    let racectl = r.mem.alloc(0x80);
    r.mem.write_u32(app + 0x730, game);
    r.mem.write_u32(app + 0x24, input);
    let (cam, hud) = (r.mem.alloc(0x10), r.mem.alloc(0x10));
    r.mem.write_u32(game + 0xa0, cam);
    r.mem.write_u32(game + 0x38, hud);
    let race = r.mem.alloc(0x600);
    let race_vt = r.mem.alloc(0x100);
    r.mem.write_u32(race + 8, race_vt);
    r.mem.write_u32(race_vt + 0x6c, RACE_ENTERED);
    r.mem.write_u32(race_vt + 0x9c, RACE_START);
    let rider_vt = r.mem.alloc(0x60);
    r.mem.write_u32(rider_vt + 0x4c, RIDER_RESET);
    let entries: Vec<u32> = (0..6).map(|_| r.mem.alloc(0x80)).collect();
    for (k, rd) in riders.iter().enumerate() {
        r.mem.write_u32(*rd, rider_vt);
        r.mem.write_u32(rd + 0x464, entries[k]);
        r.mem.write_u32(race + 0xc4 + 4 * k as u32, *rd);
    }
    let handler = r.mem.alloc(0x10);
    let mut ends = [0u32; 0x11];
    let mut g = Rng(0x9e7a_ce00_0000_0001);
    let mut states = [0u32; 0x16];
    for case in 0..4000 {
        // a career: a new game with players in slots, a race of riders, the scenes' scripts
        let mut c = Career::default();
        c.init_new_game(0, &mut Streams::default(), None);
        let cheats = Cheats::default();
        for k in 0..1 + g.below(3) as i32 {
            c.assign_player_rider(k, k, g.below(12) as i32, &cheats);
        }
        for k in 0..4 {
            let rec = c.ptr(0x3114 + 0x44 * k);
            if rec + 0x10 < CAREER_SIZE && c.is_player_active(k) {
                c.w32(rec + 4, g.below(300));
                c.w32(rec + 8, g.below(300));
            }
        }
        let n = 1 + g.below(6) as usize;
        c.w32(0x27dc, n as u32);
        for i in 0..6 {
            c.bytes[0x2845 + 0x84 * i] = if g.below(3) == 0 { g.below(4) as u8 } else { 0xff };
        }
        c.w32(0x2afc, g.below(10));
        c.w32(0x2af8, g.below(12));
        c.w32(0x325c, g.below(4));
        for i in 0..SCENES {
            c.w32(0x2b18 + 100 * i, if g.below(4) == 0 { 0x39 } else { g.below(58) });
        }
        c.w32(0x30f4, g.below(n as u32));
        c.w32(0x2b08, g.below(9000));
        c.w32(0x3910, if g.below(2) == 0 { u32::MAX } else { g.below(4) });
        // the circuit's unlock list
        let unlocks: Vec<tricky_game::circuit::Unlock> =
            (0..g.below(3)).map(|_| tricky_game::circuit::Unlock { who: 0, kind: if g.below(2) == 0 { 7 } else { g.below(10) }, value: 0 }).collect();
        r.mem.write_u32(0x3353f8, unlocks.len() as u32);
        for (i, u) in unlocks.iter().enumerate() {
            r.mem.write_u32(0x335404 + 12 * i as u32, u.kind);
        }
        r.mem.write(CAREER_BASE, &c.bytes).unwrap();
        // the scene object, as the selection left it
        let mut scene = RaceScene { current: 0x10, has_selection: true, ..Default::default() };
        for i in 0..SCENES {
            for j in 0..SLOTS {
                scene.slots[i][j] = Slot { a: g.next() as i32, b: g.below(12) as i32, pick: if g.below(3) == 0 { -1 } else { g.below(500) as i32 } };
            }
        }
        for v in scene.riders.iter_mut() {
            *v = NO_RIDER;
        }
        write_scene(&mut r, &scene);
        // the race
        let view = RaceView {
            riders: n as i32,
            players: 1 + g.below(2) as i32,
            one_pad: g.below(3) == 0,
            restarted: (g.below(5) == 0) as u32,
            replayed: (g.below(4) == 0) as u32,
        };
        r.mem.write_u32(race + 0x88, view.riders as u32);
        r.mem.write_u32(race + 0x7c, view.players as u32);
        r.mem.write_u32(race + 0xdc, view.restarted);
        r.mem.write_u32(race + 0xe0, view.replayed);
        r.mem.write_u32(game + 0x40, if view.one_pad || g.below(2) == 0 { racectl } else { 0 });
        r.mem.write_u32(racectl + 0x4c, view.one_pad as u32);
        // (players in slots: the game asks for their level only)
        let active: Vec<i8> = (0..4).filter(|&k| c.is_player_active(k)).map(|k| c.player_id(k)).collect();
        let players: Vec<i8> = (0..view.players).map(|_| active[g.below(active.len() as u32) as usize]).collect();
        for k in 0..view.players as usize {
            r.mem.write_u32(race + 0x8c + 4 * k as u32, race + 0x200 + 4 * k as u32);
            r.mem.write_u32(race + 0x200 + 4 * k as u32, riders[k]);
            r.mem.write_u8(entries[k] + 0x65, players[k] as u8);
        }
        let mut ours_handler = PreRace::default();
        let mut ours_end = EndRace::default();
        let mut clock = g.below(1000) as f32 / 7.0;
        r.mem.write_f32(race + 0x14, clock);
        {
            let mut x = w.borrow_mut();
            *x = World { players: players.clone(), rider0: riders[0], ..Default::default() };
            x.frames = (0..12).filter_map(|cl| if g.below(4) != 0 { Some((cl, g.below(5) as i32)) } else { None }).collect();
            for i in 0..6 {
                for j in 0..6 {
                    x.counts[i][j] = g.below(6) as i32;
                    r.mem.write_u32(riders[i] + 0x5c + 0x20 * j as u32, x.counts[i][j] as u32);
                }
                x.place[i] = g.below(6) as i32;
                x.time[i] = g.below(12000) as i32;
                x.score[i] = g.below(600_000) as i32;
                r.mem.write_u32(riders[i] + 0x110, x.place[i] as u32);
                r.mem.write_u32(riders[i] + 0x418, x.time[i] as u32);
                r.mem.write_u32(riders[i] + 0x5910, x.score[i] as u32);
            }
        }
        let mut ours = w.borrow().clone();
        let mut car = c.clone();
        // Enter, then frames of Update, then Exit
        let frames = 1 + g.below(14);
        for f in 0..frames + 2 {
            // the frame's world
            {
                let mut x = w.borrow_mut();
                x.loading = g.below(3) == 0;
                x.cam_running = g.below(3) != 0;
                x.app_mode = g.below(6);
                x.devices = g.below(5) as i32;
                x.pressed = (0..8).map(|_| g.below(3) == 0).collect();
                x.pads = (0..8).map(|_| (g.next() & 0xffff, g.next() & 0xffff, g.next())).collect();
                x.results_shown = g.below(4) == 0;
                x.results_busy = g.below(4) == 0;
                x.log.clear();
            }
            r.mem.write_u32(racectl + 0x54, w.borrow().results_shown as u32);
            r.mem.write_u32(overlay_in + 0x18, w.borrow().results_busy as u32);
            // (the race's control object for the results: always there after a race)
            if post {
                r.mem.write_u32(game + 0x40, racectl);
            }
            // the results menu sets 0xb / 0xc now and then
            if post && 0 < f && g.below(10) == 0 {
                ours_end.state = [0xbu32, 0xc][g.below(2) as usize];
            }
            for (i, p) in w.borrow().pads.iter().enumerate() {
                let at = pads + 0x40 * i as u32;
                r.mem.write_u32(at + 0xc, p.0);
                r.mem.write_u32(at + 0x10, p.1);
                r.mem.write_u32(at + 0x2c, p.2);
            }
            r.mem.write_u32(race + 0x4f8, w.borrow().loading as u32);
            r.mem.write_u32(app + 0x58, w.borrow().app_mode);
            r.mem.write_u32(input + 0x14, w.borrow().devices as u32);
            {
                let x = w.borrow();
                ours.loading = x.loading;
                ours.cam_running = x.cam_running;
                ours.app_mode = x.app_mode;
                ours.devices = x.devices;
                ours.pressed = x.pressed.clone();
                ours.pads = x.pads.clone();
                ours.results_shown = x.results_shown;
                ours.results_busy = x.results_busy;
                ours.log.clear();
            }
            if post {
                r.mem.write_u32(handler + 4, ours_end.state);
                r.mem.write_u32(handler + 8, ours_end.first);
                r.mem.write_u32(handler + 0xc, ours_end.second);
            } else {
                r.mem.write_u32(handler + 4, ours_handler.state);
            }
            r.cpu.set_gpr(4, handler as u64);
            r.cpu.set_gpr(5, race as u64);
            if post {
                if f == 0 {
                    r.call(0x113e10).unwrap();
                    ours_end.enter(&mut ours, &mut car);
                } else if f <= frames {
                    r.call(0x1141e0).unwrap();
                    ours_end.update(&mut ours, &car, &view, &mut scene, &mut clock, &unlocks);
                } else {
                    r.call(0x1146a8).unwrap();
                }
                let what = format!("case {case} frame {f} (end state {:#x})", ours_end.state);
                assert_eq!(w.borrow().log, ours.log, "{what}: calls");
                assert_eq!((r.mem.read_u32(handler + 4), r.mem.read_u32(handler + 8), r.mem.read_u32(handler + 0xc)), (ours_end.state, ours_end.first, ours_end.second), "{what}: state");
                assert_eq!(r.mem.read_u32(race + 0x14), clock.to_bits(), "{what}: clock");
                assert_eq!(read_scene(&mut r), scene, "{what}: scene");
                let mut block = vec![0u8; CAREER_SIZE];
                r.mem.read(CAREER_BASE, &mut block).unwrap();
                assert!(block == car.bytes, "{what}: career");
                ends[ours_end.state.min(0x10) as usize] += 1;
                if f == 0 {
                    ends[0x10] += (ours_end.first != 0x10) as u32;
                }
                continue;
            }
            if f == 0 {
                r.call(0x112b88).unwrap();
                ours_handler.enter(&mut ours, &mut car, &view);
            } else if f <= frames {
                r.call(0x112d10).unwrap();
                ours_handler.update(&mut ours, &car, &view, &mut scene);
            } else {
                r.call(0x113468).unwrap();
                ours_handler.exit(&mut ours);
            }
            let what = format!("case {case} frame {f} (state {:#x})", ours_handler.state);
            assert_eq!(w.borrow().log, ours.log, "{what}: calls");
            assert_eq!(r.mem.read_u32(handler + 4), ours_handler.state, "{what}: state");
            for (i, p) in ours.pads.iter().enumerate() {
                let at = pads + 0x40 * i as u32;
                assert_eq!((r.mem.read_u32(at + 0xc), r.mem.read_u32(at + 0x2c)), (p.0, p.2), "{what}: pad {i}");
            }
            assert_eq!(read_scene(&mut r), scene, "{what}: scene");
            let mut block = vec![0u8; CAREER_SIZE];
            r.mem.read(CAREER_BASE, &mut block).unwrap();
            assert!(block == car.bytes, "{what}: career");
            states[ours_handler.state as usize] += 1;
        }
    }
    eprintln!("states after a frame {states:?}; post-race {ends:?}");
    let _ = read_string;
}
