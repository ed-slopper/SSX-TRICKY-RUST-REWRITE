//! The front end: main menu, the event structure (race heats, show-off, free ride), the results
//! screen and the on-screen display while riding.

use crate::rider::RaceState;
use crate::{clock, standings, CharLib, LevelList, Mode, Opponents, RaceRes, RiderRes, SmoothDt};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Screen { Menu, Playing, Paused, Results }
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event { Race, ShowOff, FreeRide }
/// A circuit race is two heats and a final (`Circuit_AfterRaceAdvanceHeat`): the top three go through.
pub const ROUNDS: [&str; 3] = ["HEAT 1", "HEAT 2", "FINAL"];
pub const MEDALS: [&str; 3] = ["GOLD", "SILVER", "BRONZE"];
/// Show-off medal scores (gold, silver, bronze) and time limit (s) per track, from the game's
/// track table (0x332f2c) and medal table (0x335370).
pub fn showoff(track: &str) -> ([u32; 3], f32) {
    match track {
        "gari" => ([55_000, 40_000, 25_000], 120.0),
        "snowdream" => ([95_000, 65_000, 35_000], 90.0),
        "elysium" => ([225_000, 150_000, 75_000], 90.0),
        "mesablanca" => ([225_000, 150_000, 75_000], 90.0),
        "merqury" => ([275_000, 175_000, 125_000], 90.0),
        "aloha" => ([175_000, 115_000, 75_000], 90.0),
        "megaplex" => ([350_000, 225_000, 100_000], 90.0),
        "alaska" => ([500_000, 300_000, 150_000], 135.0),
        "pipedream" => ([800_000, 500_000, 250_000], 90.0),
        _ => ([55_000, 40_000, 25_000], 120.0),
    }
}
pub fn track_key(list: &LevelList) -> String { list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default() }
/// riders in a race heat, the player included; the first three go through
pub const HEAT: usize = 6;

/// Best results per track, kept in `tricky-save.json` next to the program.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct Record { pub time: Option<f32>, pub score: u32, pub race: Option<usize>, pub showoff: Option<usize> }
#[derive(Resource, Default)]
pub struct Records(pub std::collections::HashMap<String, Record>);
fn save_path() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default().join("tricky-save.json")
}
/// Board unlock costs in career points (0x332148 table): the original's twelve boards per rider.
pub const BOARD_POINTS: [u32; 12] = [0, 0, 5, 20, 35, 60, 90, 120, 160, 200, 240, 999];
/// The circuit's track order (0x3352f0): a medal on one opens the next. Garibaldi, Snowdream and
/// Elysium are open from the start (`Profile_ResetNewGame`: tracks 0x407); a race medal on Alaska
/// opens Untracked, a show-off medal there opens Pipedream.
pub const TRACK_ORDER: [&str; 8] = ["gari", "snowdream", "elysium", "mesablanca", "merqury", "megaplex", "aloha", "alaska"];
/// Riders open at the start (Mac, Moby, Elise, Eddie), then one more for every gold, in this order.
pub const CHAR_START: [&str; 4] = ["mac", "moby", "elise", "eddie"];
pub const CHAR_NEXT: [&str; 8] = ["brodi", "zoe", "jp", "kaori", "marisol", "psymon", "seeiah", "luther"];
impl Records {
    fn medal(&self, key: &str) -> (bool, bool) { self.0.get(key).map_or((false, false), |r| (r.race.is_some(), r.showoff.is_some())) }
    pub fn golds(&self) -> usize { self.0.values().map(|r| (r.race == Some(0)) as usize + (r.showoff == Some(0)) as usize).sum() }
    pub fn track_open(&self, key: &str) -> bool {
        match TRACK_ORDER.iter().position(|k| *k == key) {
            Some(i) if i < 3 => true,
            Some(i) => { let (r, s) = self.medal(TRACK_ORDER[i - 1]); r || s }
            None => match key { "untracked" => self.medal("alaska").0, "pipedream" => self.medal("alaska").1, _ => true },
        }
    }
    pub fn char_open(&self, name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        CHAR_START.contains(&n.as_str()) || CHAR_NEXT.iter().take(self.golds()).any(|c| *c == n)
    }
}

/// Career rank by points (`Career_BoardTierFromPoints`): 240 is Master.
pub const RANKS: [(u32, &str); 10] = [(240, "MASTER"), (200, "rank 1"), (160, "rank 2"), (120, "rank 3"), (90, "rank 4"), (60, "rank 5"), (35, "rank 6"), (20, "rank 7"), (5, "rank 8"), (0, "rank 9")];
pub fn rank(points: u32) -> &'static str { RANKS.iter().find(|r| points >= r.0).map_or("rank 9", |r| r.1) }
impl Game {
    /// how far the riders are trained: fully when "master" is picked, else by career points
    pub fn training(&self) -> f32 { if self.master { 1.0 } else { (self.points as f32 / 240.0).min(1.0) } }
}
impl Records {
    /// World Circuit points (`Circuit_AwardMedalPoints`): gold 15, silver 10, bronze 5, per track
    /// and event, counting the best medal only.
    pub fn points(&self) -> u32 {
        let p = |m: Option<usize>| m.map_or(0, |m| [15, 10, 5].get(m).copied().unwrap_or(0));
        self.0.values().map(|r| p(r.race) + p(r.showoff)).sum()
    }
    pub fn load() -> Self { Self(std::fs::read_to_string(save_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()) }
    fn save(&self) { if let Ok(s) = serde_json::to_string_pretty(&self.0) { let _ = std::fs::write(save_path(), s); } }
}

/// When a run ends, keep what was better than before.
pub fn records(mut game: ResMut<Game>, race: Res<RaceRes>, rider: Res<RiderRes>, list: Res<LevelList>, mut recs: ResMut<Records>, mut was: Local<bool>) {
    game.points = recs.points();
    let done = race.0.state == RaceState::Finished && game.screen != Screen::Menu;
    if done && !*was {
        let track = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let rec = recs.0.entry(track).or_default();
        let better = |old: Option<usize>, new: usize| Some(old.map_or(new, |o| o.min(new)));
        match game.event {
            Event::Race => {
                rec.time = Some(rec.time.map_or(race.0.time, |t| t.min(race.0.time)));
                if game.round + 1 >= ROUNDS.len() && game.place < 3 { rec.race = better(rec.race, game.place); }
            }
            Event::ShowOff => {
                rec.score = rec.score.max(rider.0.score);
                if let Some(m) = showoff(&track_key(&list)).0.iter().position(|s| rider.0.score >= *s) { rec.showoff = better(rec.showoff, m); }
            }
            Event::FreeRide => { rec.time = Some(rec.time.map_or(race.0.time, |t| t.min(race.0.time))); rec.score = rec.score.max(rider.0.score); }
        }
        recs.save();
    }
    *was = done;
}

#[derive(Resource)]
pub struct Game {
    pub screen: Screen,
    pub event: Event,
    pub round: usize,
    /// set to line everyone up again (new run, new rider)
    pub lineup: bool,
    /// seconds on the current screen
    pub since: f32,
    /// the finishing order of the last run: (name, time if finished, is the player)
    pub table: Vec<(String, Option<f32>, bool)>,
    pub place: usize,
    /// riders fully trained on the best boards, instead of as they start the game
    pub master: bool,
    /// career points earned (World Circuit medals), for training and the rank
    pub points: u32,
    /// how each computer rider (by character) feels about the player after the last heat
    pub attitude: std::collections::HashMap<usize, f32>,
    pub cursor: usize,
    pub pause_cursor: usize,
    /// which menu row is being changed, the player's board (0 to 11), and a pending track change
    pub row: usize,
    pub board: usize,
    pub track_step: i32,
}
impl Default for Game {
    fn default() -> Self { Self { screen: Screen::Menu, event: Event::Race, round: 0, points: 0, attitude: Default::default(), lineup: true, since: 0.0, table: Vec::new(), place: 0, master: false, cursor: 0, pause_cursor: 0, row: 0, board: std::env::var("TRICKY_BOARD").ok().and_then(|b| b.parse().ok()).unwrap_or(0), track_step: 0 } }
}
impl Game {
    pub fn opponents(&self) -> usize { if self.screen != Screen::Menu && self.event != Event::Race { 0 } else { HEAT - 1 } }
    pub fn finish(&mut self, table: Vec<(String, Option<f32>, bool)>) {
        self.place = table.iter().position(|t| t.2).unwrap_or(0);
        self.table = table;
        self.screen = Screen::Results;
        self.since = 0.0;
    }
}

#[derive(Component, Clone, Copy, PartialEq)]
pub enum HudItem { Place, Time, Score, Speed, Letters, Trick, Big, Fps, Panel }
/// One of the boost meter's fourteen segments (`HUD_DrawPlayerSprites`), bottom first.
#[derive(Component)]
pub struct BoostSeg(usize);
/// The segments' heights in the original's 640x480 HUD: three bands with a gap between them.
const SEG_Y: [f32; 14] = [288.0, 270.0, 252.0, 234.0, 216.0, 188.0, 170.0, 152.0, 134.0, 106.0, 88.0, 70.0, 52.0, 34.0];
/// The bands' colours: red, orange, yellow
const SEG_RGB: [(f32, f32, f32); 3] = [(0.965, 0.22, 0.141), (0.953, 0.659, 0.063), (0.984, 0.984, 0.337)];
fn seg_band(i: usize) -> usize { if i < 5 { 0 } else if i < 9 { 1 } else { 2 } }
#[derive(Component)]
pub struct PanelBox;
#[derive(Component)]
pub struct HudRoot;

pub fn setup_ui(mut commands: Commands) {
    let text = |size: f32, item: HudItem| (Text::new(""), TextFont { font_size: size, ..default() }, TextShadow { offset: Vec2::splat(2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.8) }, item);
    let abs = |n: Node| Node { position_type: PositionType::Absolute, ..n };
    commands.spawn((abs(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }), HudRoot)).with_children(|p| {
        p.spawn((text(54.0, HudItem::Place), abs(Node { top: Val::Px(124.0), left: Val::Px(24.0), ..default() })));
        p.spawn((text(34.0, HudItem::Time), abs(Node { top: Val::Px(14.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }), TextLayout::new_with_justify(JustifyText::Center)));
        p.spawn((text(34.0, HudItem::Score), abs(Node { top: Val::Px(14.0), right: Val::Px(24.0), ..default() }), TextLayout::new_with_justify(JustifyText::Right)));
        // speed bottom left, as the original's (90,430)
        p.spawn((text(40.0, HudItem::Speed), abs(Node { bottom: Val::Px(28.0), left: Val::Px(24.0), ..default() })));
        p.spawn((text(30.0, HudItem::Letters), abs(Node { bottom: Val::Px(84.0), left: Val::Px(24.0), ..default() })));
        p.spawn((text(30.0, HudItem::Trick), abs(Node { bottom: Val::Px(110.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }), TextLayout::new_with_justify(JustifyText::Center)));
        p.spawn((text(120.0, HudItem::Big), abs(Node { top: Val::Percent(28.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }), TextLayout::new_with_justify(JustifyText::Center)));
        p.spawn((text(13.0, HudItem::Fps), abs(Node { bottom: Val::Px(4.0), left: Val::Px(10.0), ..default() })));
        // the boost meter: a column of fourteen segments on the right, as the original's
        for (i, y) in SEG_Y.iter().enumerate() {
            p.spawn((abs(Node { left: Val::Percent(570.0 / 6.4), top: Val::Percent((140.0 + y) / 4.8), width: Val::Px(26.0), height: Val::Px(12.0), border: UiRect::all(Val::Px(1.0)), ..default() }),
                     BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)), BorderColor(Color::srgba(1.0, 1.0, 1.0, 0.5)), BorderRadius::all(Val::Px(3.0)), BoostSeg(i)));
        }
    });
    // menu and results panel
    commands.spawn((abs(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::FlexStart, align_items: AlignItems::Center, padding: UiRect::left(Val::Px(50.0)), ..default() }), PanelBox))
        .with_children(|p| {
            p.spawn((Node { padding: UiRect::axes(Val::Px(48.0), Val::Px(30.0)), ..default() }, BackgroundColor(Color::srgba(0.02, 0.05, 0.15, 0.78))))
                .with_children(|b| { b.spawn((text(24.0, HudItem::Panel), TextLayout::new_with_justify(JustifyText::Center))); });
        });
}

/// The trick book: each trick the player lands is checked against the rider's current chapter.
pub fn trick_book(mut rider: ResMut<RiderRes>, lib: Res<CharLib>, mut book: ResMut<crate::book::Book>) {
    if rider.0.landed.is_empty() { return; }
    let landed: Vec<String> = std::mem::take(&mut rider.0.landed);
    let me = lib.chars.get(lib.player).map(|c| c.name.clone()).unwrap_or_default();
    for t in landed {
        if let Some((name, chapter)) = book.check(&me, &t) {
            let r = &mut rider.0;
            r.last_trick = match chapter {
                Some(6) => format!("{}\nTRICK BOOK: {name}\nBOOK COMPLETE - UBERBOARD", r.last_trick),
                Some(c) => format!("{}\nTRICK BOOK: {name}\nCHAPTER {c} COMPLETE", r.last_trick),
                None => format!("{}\nTRICK BOOK: {name}", r.last_trick),
            };
            r.trick_timer = r.trick_timer.max(3.0);
        }
    }
}

/// Menu and results input. Riding input is read in `ride`.
pub fn menus(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, time: Res<Time>, mut game: ResMut<Game>, mut lib: ResMut<CharLib>, mode: Res<Mode>, recs: Res<Records>, list: Res<LevelList>, book: Res<crate::book::Book>) {
    game.since += time.delta_secs();
    if *mode != Mode::Ride { return; }
    let pad = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    let ok = keys.just_pressed(KeyCode::Enter) || pad(GamepadButton::South) || pad(GamepadButton::Start);
    let back = keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::East);
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || pad(GamepadButton::DPadUp);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || pad(GamepadButton::DPadDown);
    let left = keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) || pad(GamepadButton::DPadLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) || pad(GamepadButton::DPadRight);
    match game.screen {
        Screen::Menu => {
            // five rows: Up / Down picks the row, Left / Right changes what is on it
            if up { game.row = (game.row + 4) % 5; }
            if down { game.row = (game.row + 1) % 5; }
            let step = right as i32 - left as i32;
            let turn = |v: usize, n: usize| ((v as i32 + step).rem_euclid(n.max(1) as i32)) as usize;
            if step != 0 {
                match game.row {
                    0 => game.cursor = turn(game.cursor, 3),
                    1 => { lib.player = turn(lib.player, lib.chars.len()); game.lineup = true; }
                    2 => game.board = turn(game.board, 12),
                    3 => game.track_step = step,
                    _ => { game.master = !game.master; game.lineup = true; }
                }
            }
            game.event = [Event::Race, Event::ShowOff, Event::FreeRide][game.cursor];
            // locked riders, boards and tracks can't be picked (master unlocks everything, as the cheat)
            let me = lib.chars.get(lib.player).map(|c| c.name.clone()).unwrap_or_default();
            let open = game.master || (recs.char_open(&me) && recs.track_open(&track_key(&list)) && (BOARD_POINTS[game.board.min(11)] <= recs.points() || (game.board == 11 && book.complete(&me))));
            if ok && game.since > 0.2 && open { game.round = 0; game.screen = Screen::Playing; game.since = 0.0; game.lineup = true; }
        }
        Screen::Playing => {
            if back { game.screen = Screen::Paused; game.since = 0.0; game.pause_cursor = 0; }
        }
        Screen::Paused => {
            if up { game.pause_cursor = (game.pause_cursor + 2) % 3; }
            if down { game.pause_cursor = (game.pause_cursor + 1) % 3; }
            if back { game.screen = Screen::Playing; game.since = 0.0; }
            else if ok && game.since > 0.15 {
                match game.pause_cursor {
                    0 => game.screen = Screen::Playing,
                    1 => { game.screen = Screen::Playing; game.lineup = true; }
                    _ => { game.screen = Screen::Menu; game.lineup = true; }
                }
                game.since = 0.0;
            }
        }
        Screen::Results => {
            if back { game.screen = Screen::Menu; game.since = 0.0; game.lineup = true; }
            else if ok && game.since > 0.8 {
                let through = game.event == Event::Race && game.place < 3;
                if through && game.round + 1 >= ROUNDS.len() { game.screen = Screen::Menu; }
                else { if through { game.round += 1; } game.screen = Screen::Playing; }
                game.since = 0.0;
                game.lineup = true;
            }
        }
    }
}

fn ordinal(i: usize) -> &'static str { ["1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th"][i.min(7)] }

pub fn hud(
    game: Res<Game>, race: Res<RaceRes>, list: Res<LevelList>, time: Res<Time>, smooth: Res<SmoothDt>, rider: Res<RiderRes>,
    opponents: Res<Opponents>, lib: Res<CharLib>, mode: Res<Mode>, (recs, book): (Res<Records>, Res<crate::book::Book>),
    mut texts: Query<(&mut Text, &HudItem, &mut TextColor)>, mut segs: Query<(&mut Node, &mut BackgroundColor, &BoostSeg)>, mut shown: Local<f32>,
    mut panel: Single<&mut Visibility, (With<PanelBox>, Without<HudRoot>)>, mut root: Single<&mut Visibility, (With<HudRoot>, Without<PanelBox>)>,
    mut fps: Local<(f32, f32, u32)>,
) {
    fps.1 += time.delta_secs();
    fps.2 += 1;
    if fps.1 >= 0.5 { fps.0 = fps.2 as f32 / fps.1; fps.1 = 0.0; fps.2 = 0; }
    let r = &rider.0;
    let riding = *mode == Mode::Ride;
    let playing = riding && game.screen != Screen::Menu;
    let track = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let track = match track.as_str() {
        "gari" => "Garibaldi", "snowdream" => "Snowdream", "elysium" => "Elysium Alps", "mesablanca" => "Mesablanca", "merqury" => "Merqury City",
        "megaplex" => "Tokyo Megaplex", "aloha" => "Aloha Ice Jam", "alaska" => "Alaska", "pipedream" => "Pipedream", "untracked" => "Untracked", "trick" => "Trick Tutorial", o => o,
    }.to_string();
    let me = lib.chars.get(lib.player).map(|c| crate::proper(&c.name)).unwrap_or_else(|| "Rider".into());
    **root = if playing { Visibility::Inherited } else { Visibility::Hidden };
    **panel = if riding && game.screen != Screen::Playing { Visibility::Inherited } else { Visibility::Hidden };

    // the shown level slews 0.005 a frame (0.3 a second) towards the meter; each segment is a
    // fourteenth, grows from 0.2 to full size as it fills, and takes its band's colour
    let want = if r.tricky() { 1.0 } else { r.boost.clamp(0.0, 1.0) };
    let step = 0.3 * time.delta_secs();
    *shown += (want - *shown).clamp(-step, step);
    let pulse = if r.uber_timer > 0.0 || r.tricky() { 0.75 + 0.25 * (time.elapsed_secs() * 8.0).sin() } else { 1.0 };
    for (mut node, mut bg, seg) in segs.iter_mut() {
        let f = (*shown * 14.0 - seg.0 as f32).clamp(0.0, 1.0);
        let k = 0.2 + 0.8 * f;
        node.width = Val::Px(26.0 * if f > 0.0 { k } else { 1.0 });
        node.height = Val::Px(12.0 * if f > 0.0 { k } else { 1.0 });
        let (cr, cg, cb) = SEG_RGB[seg_band(seg.0)];
        bg.0 = if f > 0.0 { Color::srgba(cr * pulse, cg * pulse, cb * pulse, 1.0) } else { Color::srgba(0.0, 0.0, 0.0, if *shown <= 0.0 { 0.2 } else { 0.4 }) };
    }

    for (mut text, item, mut color) in &mut texts {
        let mut tint = Color::WHITE;
        let new = match item {
            HudItem::Place => {
                if game.event == Event::Race && !opponents.0.is_empty() && race.0.state != RaceState::Countdown {
                    let place = standings(&race.0, &opponents.0, &lib, &me).iter().position(|t| t.2).unwrap_or(0);
                    let lap = if race.0.lines.has_laps() { format!("\nlap {}/4", if race.0.prog.started { (5 - race.0.prog.laps.min(4)).min(4) } else { 1 }) } else { String::new() };
                    // gold in the lead (HUD_DrawPlayerText)
                    if place == 0 { tint = Color::srgb(0.953, 0.729, 0.106); }
                    format!("{}{lap}", ordinal(place))
                } else { String::new() }
            }
            HudItem::Time => match game.event {
                Event::Race => format!("{}\n", clock(race.0.time)),
                // show-off runs against the clock
                Event::ShowOff => clock((showoff(&track_key(&list)).1 + race.0.bonus - race.0.time).max(0.0)).to_string(),
                Event::FreeRide => format!("{}{}", clock(race.0.time), race.0.best.map(|b| format!("   best {}", clock(b))).unwrap_or_default()),
            },
            HudItem::Score => {
                let goal = if game.event == Event::ShowOff {
                    let sh = showoff(&track_key(&list)).0;
                    match sh.iter().rposition(|s| r.score < *s) { Some(i) => format!("\n{} at {}", MEDALS[i].to_lowercase(), sh[i]), None => "\nGOLD".into() }
                } else { String::new() };
                format!("{}{}{goal}", r.score, if r.multiplier > 1 { format!("  x{}", r.multiplier) } else { String::new() })
            }
            HudItem::Speed => format!("{:.0} km/h", r.vel.length() * 3.6),
            HudItem::Letters => {
                let word: String = "TRICKY".chars().enumerate().map(|(i, c)| if (i as u8) < r.letters { c } else { '.' }).collect();
                if r.tricky() { tint = Color::srgb(1.0, 0.4, 0.85); }
                let note = if r.tricky() { "  unlimited boost" } else if r.uber_timer > 0.0 { "  UBER READY - grab + K" } else { "" };
                format!("{word}{note}")
            }
            HudItem::Trick => {
                // the original shows each trick name in gold at full strength until it is replaced or
                // its time is up (Score_PostTrickName: 3 s); a crash puts up nothing
                tint = Color::srgb(1.0, 0.867, 0.0);
                if r.trick_timer > 0.0 && r.last_trick == "CRASH" { String::new() }
                else if r.trick_timer > 0.0 { r.last_trick.clone() }
                else if r.rail.is_some() { if r.rail_twist.sin() > 0.7 { "BS RAIL".into() } else if r.rail_twist.sin() < -0.7 { "FS RAIL".into() } else if r.rail_twist.cos() < 0.0 { "SWITCH 50/50".into() } else { "50/50".into() } }
                else if r.charge > 0.0 && (r.wind.abs() > 0.15 || r.wind_flip.abs() > 0.15) {
                    // what the jump is being wound up for
                    tint = Color::WHITE;
                    let bar = |v: f32| "|".repeat((v.abs() * 8.0).round() as usize);
                    let spin = if r.wind.abs() > 0.15 { format!("spin {} {}", if r.wind > 0.0 { "right" } else { "left" }, bar(r.wind)) } else { String::new() };
                    let flip = if r.wind_flip.abs() > 0.15 { format!("{} flip {}", if r.wind_flip > 0.0 { "front" } else { "back" }, bar(r.wind_flip)) } else { String::new() };
                    format!("wind-up   {spin}   {flip}")
                } else if r.switch && r.grounded { tint = Color::srgba(1.0, 1.0, 1.0, 0.6); "switch".into() }
                else { String::new() }
            }
            HudItem::Big => {
                if !playing { String::new() } else {
                    match race.0.state {
                        RaceState::Countdown => if race.0.intro { String::new() } else if race.0.countdown > 0.4 { format!("{}", race.0.countdown.ceil() as i32) } else { "GO!".into() },
                        RaceState::Running if race.0.time < 0.8 => "GO!".into(),
                        RaceState::Finished if game.event == Event::ShowOff => "TIME UP".into(),
                        RaceState::Finished => if race.0.post == 2 { String::new() } else { "FINISH".into() },
                        _ => String::new(),
                    }
                }
            }
            HudItem::Fps => format!("{:.0} fps   slowest frame {:.0} ms{}", fps.0, smooth.worst_ms, if riding { "" } else { "   free camera" }),
            HudItem::Panel => match game.screen {
                Screen::Menu => {
                    let key = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let best = match recs.0.get(&key) {
                        Some(rec) => {
                            let mut parts = Vec::new();
                            if let Some(t) = rec.time { parts.push(format!("best time {}", clock(t))); }
                            if rec.score > 0 { parts.push(format!("best score {}", rec.score)); }
                            if let Some(m) = rec.race { parts.push(format!("race {}", MEDALS[m.min(2)].to_lowercase())); }
                            if let Some(m) = rec.showoff { parts.push(format!("show-off {}", MEDALS[m.min(2)].to_lowercase())); }
                            if parts.is_empty() { "no results yet".to_string() } else { parts.join("   ") }
                        }
                        None => "no results yet".to_string(),
                    };
                    let data = crate::trickdata::RIDERS.iter().position(|d| d.name.eq_ignore_ascii_case(&me)).unwrap_or(0);
                    let board = &crate::trickdata::BOARDS[data][game.board.min(11)];
                    let kind = ["BX", "freestyle", "alpine"][(board.kind as usize).min(2)];
                    let st = r.stats;
                    let line = |i: usize, label: &str, value: String| if game.row == i { format!(">  {label}   <  {value}  >") } else { format!("{label}   {value}") };
                    format!("S S X   T R I C K Y\ntricky-rs\n\n{}\n{}\n{}\n{}\n{}\n\nedging {:.0}  speed {:.0}  stability {:.0}  tricks {:.0}\n{best}\ncareer points {}\n{}\n\nUp / Down: row    Left / Right: change    Enter: start",
                        line(0, "Event", ["World Circuit - Race", "World Circuit - Show-off", "Free Ride"][game.cursor.min(2)].to_string()),
                        line(1, "Rider", format!("{me}{}", if game.master || recs.char_open(&me) { String::new() } else { format!("  - locked ({} golds to go)", CHAR_NEXT.iter().position(|c| c.eq_ignore_ascii_case(&me)).map_or(0, |i| i + 1 - recs.golds().min(i + 1))) })),
                        line(2, "Board", format!("{} of 12  {}  ({kind}){}", game.board + 1, board.name,
                            { let need = BOARD_POINTS[game.board.min(11)]; if need > recs.points() { format!("  - unlocks at {need} pts") } else { String::new() } })),
                        line(3, "Track", format!("{track}{}", if game.master || recs.track_open(&track_key(&list)) { "" } else { "  - locked (medal on the track before)" })),
                        line(4, "Training", if game.master { "master (fully trained)".to_string() } else { format!("career - {} ({} of 240 pts)", rank(game.points), game.points) }),
                        st.edging * 100.0, st.speed * 100.0, st.stability * 100.0, st.tricks * 100.0, recs.points(), book.summary(&me))
                }
                Screen::Playing => String::new(),
                Screen::Paused => {
                    let row = |i: usize, s: &str| if game.pause_cursor == i { format!(">  {s}  <") } else { s.to_string() };
                    format!("PAUSED\n\n{}\n{}\n{}\n\nUp / Down, Enter    Esc: back to the run", row(0, "Resume"), row(1, "Restart"), row(2, "Quit to menu"))
                }
                Screen::Results => match game.event {
                    Event::Race => {
                        let mut s = format!("{track}  -  {}\n\n", ROUNDS[game.round.min(2)]);
                        for (i, (name, t, you)) in game.table.iter().enumerate() {
                            s += &format!("{}   {}{}   {}\n", ordinal(i), name, if *you { " (you)" } else { "" }, t.map(clock).unwrap_or_else(|| "--".into()));
                        }
                        let last = game.round + 1 >= ROUNDS.len();
                        s += &match (game.place < 3, last) {
                            (true, true) => format!("\n{} MEDAL\n\nEnter: back to the menu", MEDALS[game.place]),
                            (true, false) => format!("\nYou go through to the {}\n\nEnter: continue    Esc: menu", ROUNDS[game.round + 1].to_lowercase()),
                            (false, _) => "\nKnocked out - the first three go through\n\nEnter: try again    Esc: menu".to_string(),
                        };
                        s
                    }
                    Event::ShowOff => {
                        let sh = showoff(&track_key(&list)).0;
                        let medal = sh.iter().position(|s| r.score >= *s);
                        format!("{track}  -  SHOW-OFF\n\n{me}\nscore {}\ntime {}\n\n{}\n\ngold {}   silver {}   bronze {}\n\nEnter: try again    Esc: menu", r.score, clock(race.0.time),
                            medal.map(|m| format!("{} MEDAL", MEDALS[m])).unwrap_or_else(|| "No medal".into()), sh[0], sh[1], sh[2])
                    }
                    Event::FreeRide => String::new(),
                },
            },
        };
        if text.0 != new { text.0 = new; }
        if color.0 != tint { color.0 = tint; }
    }
}
