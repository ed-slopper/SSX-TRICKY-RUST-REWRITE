//! The front end: main menu, the event structure (race heats, show-off, free ride), the results
//! screen and the on-screen display while riding.
//!
//! STANDIN: our menus, event flow and results screen, for the game's front end (its menus, circuit and results screens)

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
/// Show-off medal scores (gold, silver, bronze) and time limit (s) per track: the game's course table (0x332f28),
/// medal table (0x335370) and Pipedream's own thresholds (`Circuit_GetMedal`), via `tricky_game::courses` (row C0).
/// An unknown folder is taken as Garibaldi.
pub fn showoff(track: &str) -> ([u32; 3], f32) {
    use tricky_game::courses::{by_file, time_limit, MEDALS};
    let course = tricky_data::aip::big_name(track).and_then(by_file).or_else(|| by_file("gari")).expect("garibaldi");
    let medals = if course.number == 6 {
        [800_000, 500_000, 250_000]
    } else {
        MEDALS.iter().find(|(c, _)| *c == course.number).map(|(_, t)| *t).unwrap_or(MEDALS[7].1)
    };
    (medals.map(|v| v as u32), time_limit(course.number, 3))
}
pub fn track_key(list: &LevelList) -> String { list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default() }
/// riders in a race heat, the player included; the first three go through
pub const HEAT: usize = 6;

/// The results the app kept before the career block held them (`tricky-save.json` next to the program): read once,
/// carried into the career, then renamed (row E12d2).
#[derive(serde::Deserialize, Default, Clone)]
struct OldRecord { race: Option<usize>, showoff: Option<usize> }
fn save_path() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default().join("tricky-save.json")
}
/// The game's own career block (row E12d), kept as the memory card's image in `tricky-career.sav` next to the
/// program: loaded when its CRC is right, else a new game. The medals, best times and scores and what is unlocked
/// live in it (row E12d2): the player is slot 0 on the chosen character, whose character record holds its World
/// Circuit medals, points and boards.
#[derive(Resource)]
pub struct CareerRes(pub tricky_game::career::Career);
fn career_path() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default().join("tricky-career.sav")
}
impl CareerRes {
    pub fn load() -> Self {
        let file = std::fs::read(career_path()).ok();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u32);
        let (mut career, _) = tricky_game::career::CareerFile::open(file.as_deref(), 0, seed, None);
        // the player slots are not in the card's part: no one in them until the menus put the player in slot 0
        for k in 0..4 { career.clear_player_slot(k); }
        Self(career)
    }
    pub fn save(&mut self) {
        let image = self.0.save_image(None);
        let _ = std::fs::write(career_path(), image);
    }
    /// The game's number of a character (`rivals::NAMES`: Eddie 0 .. Marisol 11)
    pub fn character(name: &str) -> u32 {
        tricky_game::rivals::NAMES.iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or(0) as u32
    }
    /// The course number of a level folder (its archive name in the course table)
    pub fn course(key: &str) -> Option<u32> {
        tricky_data::aip::big_name(key).and_then(tricky_game::courses::by_file).map(|c| c.number)
    }
    /// The game mode of an event: a World Circuit race (4) or show-off (5), or freeride (7)
    pub fn mode(event: Event) -> u32 {
        match event { Event::Race => 4, Event::ShowOff => 5, Event::FreeRide => 7 }
    }
    /// The character in slot 0, if a player is in it
    fn slot_character(&self) -> Option<u32> {
        (0 <= self.0.bytes[0x30fc] as i8 && self.0.r32(0x3114) != 0).then(|| ((self.0.ptr(0x3114) - 0x100) / 0x1c4) as u32)
    }
    /// Player 0 in slot 0 on character `c` (`Career_AssignPlayerRider`), unless it is there already
    pub fn ensure_player(&mut self, c: u32) {
        if self.slot_character() != Some(c) {
            self.0.assign_player_rider(0, 0, c as i32, &tricky_game::cheats::Cheats::default());
        }
    }
    /// `Career_IsCharacterUnlocked`
    pub fn char_open(&self, name: &str) -> bool {
        self.0.is_character_unlocked(Self::character(name), &tricky_game::cheats::Cheats::default())
    }
    /// `Career_IsCourseUnlocked` for the event (a folder not in the course table is open)
    pub fn course_open(&self, event: Event, key: &str) -> bool {
        Self::course(key).is_none_or(|c| tricky_game::courses::course_unlocked(&self.0.unlocks(), Self::mode(event), c, false))
    }
    /// `Career_IsUnlockBitSet`: the slot's board bits (record +0x58)
    pub fn board_open(&self, board: usize) -> bool {
        self.slot_character().is_some() && self.0.is_board_unlocked(0, board as u32, &tricky_game::cheats::Cheats::default())
    }
    /// The slot's medal points (record +4 race, +8 show-off), what the boards and the levels go by
    pub fn points(&self) -> u32 {
        if self.slot_character().is_none() { return 0; }
        let rec = self.0.ptr(0x3114);
        self.0.r32(rec + 4).wrapping_add(self.0.r32(rec + 8))
    }
    /// The slot's character's four group levels (its record's +0xc..+0xf, as `Career_SetupPlayerRider` reads them)
    pub fn levels(&self) -> Option<[u8; 4]> {
        let rec = self.0.ptr(0x3114);
        self.slot_character().map(|_| [self.0.bytes[rec + 0xc], self.0.bytes[rec + 0xd], self.0.bytes[rec + 0xe], self.0.bytes[rec + 0xf]])
    }
    /// The slot's character record's +0x164 + 8 · character: the value `Career_BuildRaceRiders` passes
    /// `Career_SetupAIRider` for each computer rider (`*param_3`, from the last player's record)
    pub fn rivals(&self) -> Option<[i32; 12]> {
        let rec = self.0.ptr(0x3114);
        self.slot_character().map(|_| std::array::from_fn(|c| self.0.r32(rec + 0x164 + 8 * c) as i32))
    }
    /// `Career_GetCourseMedal`: 0 gold, 1 silver, 2 bronze
    pub fn medal(&self, event: Event, key: &str) -> Option<usize> {
        let c = Self::course(key)?;
        let m = self.0.course_medal(Self::mode(event) as i32, c as i32);
        (m < 3).then_some(m as usize)
    }
    /// The character's best time (s) and score on the course (`Career_RecordBestTime` / `_RecordBestScore`'s words)
    pub fn best(&self, key: &str) -> (Option<f32>, u32) {
        let (Some(c), Some(ch)) = (Self::course(key), self.slot_character()) else { return (None, 0) };
        let t = self.0.r32(0x1fc + 4 * c as usize + 0x1c4 * ch as usize) as i32;
        let s = self.0.r32(0x230 + 4 * c as usize + 0x1c4 * ch as usize) as i32;
        ((t > 0).then(|| t as f32 / 100.0), s.max(0) as u32)
    }
    /// The old `tricky-save.json`'s medals into the slot's character record (not the game's: a one-time move of the
    /// app's own earlier save): each better medal with its points (as `Circuit_ProcessRaceResult` adds them) and the
    /// next course of its event opened, then the file renamed
    fn carry_old(&mut self) {
        let path = save_path();
        let Some(old) = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<std::collections::HashMap<String, OldRecord>>(&s).ok()) else { return };
        if self.slot_character().is_none() { return; }
        let rec = self.0.ptr(0x3114);
        let showoff_order = tricky_game::courses::MEDALS.map(|x| x.0);
        for (key, r) in old {
            let Some(c) = Self::course(&key) else { continue };
            for (m, byte, pts, open, order) in [(r.race, 0x25, 4, 0x26, &tricky_game::courses::CIRCUIT[..]), (r.showoff, 0x24, 8, 0x27, &showoff_order[..])] {
                let Some(m) = m.filter(|m| *m < 3) else { continue };
                let at = rec + 4 * c as usize + byte;
                let was = (self.0.bytes[at] as usize).min(3);
                if m < was {
                    let d = tricky_game::circuit::MEDAL_POINTS[m] - tricky_game::circuit::MEDAL_POINTS[was];
                    self.0.bytes[at] = m as u8;
                    self.0.w32(rec + pts, self.0.r32(rec + pts).wrapping_add(d as u32));
                    self.0.w32(rec, self.0.r32(rec).wrapping_add(d as u32));
                }
                if let Some(next) = order.iter().position(|k| *k == c).and_then(|i| order.get(i + 1)) {
                    self.0.bytes[rec + 4 * *next as usize + open] = 1;
                    self.0.w32(8, self.0.r32(8) | 1 << next);
                }
            }
        }
        self.0.update_attributes(&tricky_game::cheats::Cheats::default());
        let _ = std::fs::rename(&path, path.with_extension("json.carried"));
        self.save();
    }
}

/// What `Career_CheckNewRecords` asks: which race rider is the player, and the circuit's result
struct RecordsRun<'a> { player: usize, circuit: &'a mut tricky_game::circuit::Circuit }
impl tricky_game::career::RecordsEnv for RecordsRun<'_> {
    fn rider_is_human(&self, i: usize) -> bool { i == self.player }
    fn circuit_race_result(&mut self, career: &mut tricky_game::career::Career) { self.circuit.process_race_result(career); }
}


/// Board unlock costs in career points (0x332148 table): the original's twelve boards per rider.
pub const BOARD_POINTS: [u32; 12] = [0, 0, 5, 20, 35, 60, 90, 120, 160, 200, 240, 999];
/// Career rank by points (`Career_BoardTierFromPoints`): 240 is Master.
pub const RANKS: [(u32, &str); 10] = [(240, "MASTER"), (200, "rank 1"), (160, "rank 2"), (120, "rank 3"), (90, "rank 4"), (60, "rank 5"), (35, "rank 6"), (20, "rank 7"), (5, "rank 8"), (0, "rank 9")];
pub fn rank(points: u32) -> &'static str { RANKS.iter().find(|r| points >= r.0).map_or("rank 9", |r| r.1) }
impl Game {
    /// how far the riders are trained: fully when "master" is picked, else by career points
    pub fn training(&self) -> f32 { if self.master { 1.0 } else { (self.points as f32 / 240.0).min(1.0) } }
}
/// When a run ends, into the career block (`CareerRes::record_run`), and saved.
pub fn records(mut game: ResMut<Game>, race: Res<RaceRes>, rider: Res<RiderRes>, list: Res<LevelList>, lib: Res<CharLib>, mut career: ResMut<CareerRes>, mut was: Local<bool>, mut carried: Local<bool>) {
    let me = lib.chars.get(lib.player).map(|c| c.name.clone()).unwrap_or_default();
    career.ensure_player(CareerRes::character(&me));
    if !*carried {
        *carried = true;
        career.carry_old();
    }
    game.points = career.points();
    game.levels = career.levels();
    game.rivals = career.rivals();
    let done = game.screen == Screen::Results || (game.event == Event::FreeRide && race.0.state == RaceState::Finished && game.screen != Screen::Menu);
    if done && !*was {
        // the riders as they finished: the standings, or the player alone
        let table = if game.table.is_empty() || game.event == Event::FreeRide { vec![(me.clone(), Some(race.0.time), true)] } else { game.table.clone() };
        if career.record_run(game.event, game.round, &track_key(&list), &me, &table, rider.0.score) {
            game.points = career.points();
            career.save();
        }
    }
    *was = done;
}

impl CareerRes {
    /// A finished run into the block as the game's end of race does it: the race's riders, places, times (hundredths)
    /// and scores in the career's race area (+0x27dc: per rider +0x3c the character, +0x69 the player; the places
    /// +0x3264, the times +0x327c, the score blocks +0x3294), the mode, course and heat; then `Career_CheckNewRecords`
    /// (the course's high-score tables and, in the World Circuit, `Circuit_ProcessRaceResult`: medals, points,
    /// unlocks) and the character's best time or score. False for a folder not in the course table.
    /// STANDIN: the race area filled from our standings (names, times) for the game's own race objects (its riders'
    /// +0x418, `cAI::vf17_111820`'s tables, row G4j).
    pub fn record_run(&mut self, event: Event, round: usize, key: &str, me: &str, table: &[(String, Option<f32>, bool)], score: u32) -> bool {
        let Some(course) = Self::course(key) else { return false };
        let ch = Self::character(me);
        self.ensure_player(ch);
        let c = &mut self.0;
        let player = table.iter().position(|t| t.2).unwrap_or(0);
        c.w32(0x2afc, Self::mode(event));
        c.w32(0x2af8, course);
        c.w32(0x27dc, table.len() as u32);
        for (i, (name, time, mine)) in table.iter().enumerate() {
            let r = 0x27dc + 0x84 * i;
            c.bytes[r + 0x69] = if *mine { 0 } else { 0xff };
            c.w32(r + 0x3c, if *mine { ch } else { Self::character(name) });
            c.w32(0x3264 + 4 * i, i as u32);
            c.w32(0x327c + 4 * i, time.map_or(i32::MAX, |t| (t * 100.0) as i32) as u32);
            c.w32(0x3294 + 0xbc * i + 0x7c, if *mine { score } else { 0 });
        }
        // the heat (`RiderSetup` +0x14): the circuit's race medal comes at the final
        c.w32(0x320c + 0x14, round as u32);
        let mut circuit = tricky_game::circuit::Circuit::default();
        c.check_new_records(&mut RecordsRun { player, circuit: &mut circuit }, &tricky_game::cheats::Cheats::default());
        match event {
            Event::ShowOff => c.record_best_score(ch as usize, course as usize, score as i32),
            _ => {
                if let Some(t) = table[player].1 { c.record_best_time(ch as usize, course as usize, (t * 100.0) as i32); }
                if event == Event::FreeRide { c.record_best_score(ch as usize, course as usize, score as i32); }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> CareerRes {
        let (career, _) = tricky_game::career::CareerFile::open(None, 0, 1, None);
        CareerRes(career)
    }

    #[test]
    fn a_final_won_is_gold_and_opens_the_next() {
        let mut c = fresh();
        c.ensure_player(CareerRes::character("mac"));
        assert!(c.char_open("mac") && !c.char_open("brodi"));
        assert!(c.course_open(Event::Race, "gari") && !c.course_open(Event::Race, "snowdream"));
        assert_eq!(c.medal(Event::Race, "gari"), None);
        let table = |me: usize| (0..6).map(|i| (if i == me { "Mac".to_string() } else { tricky_game::rivals::NAMES[i + 4].to_string() }, Some(100.0 + i as f32), i == me)).collect::<Vec<_>>();
        // a heat won is no medal yet
        assert!(c.record_run(Event::Race, 0, "gari", "mac", &table(0), 0));
        assert_eq!(c.medal(Event::Race, "gari"), None);
        assert_eq!(c.best("gari").0, Some(100.0));
        // the final won: gold, its 15 points, the next course of the circuit and the next rider open
        assert!(c.record_run(Event::Race, 2, "gari", "mac", &table(0), 0));
        assert_eq!(c.medal(Event::Race, "gari"), Some(0));
        assert_eq!(c.points(), 15);
        assert!(c.course_open(Event::Race, "snowdream"));
        let next = tricky_game::rivals::NAMES[tricky_game::circuit::CHARACTER_ORDER.iter().map(|&k| k as usize).find(|&k| !["Mac", "Moby", "Elise", "Eddie"].contains(&tricky_game::rivals::NAMES[k])).unwrap()];
        assert!(c.char_open(next), "{next} opens with the gold");
        assert!(c.board_open(2) && !c.board_open(3), "boards up to 5 points");
        // a worse final keeps the gold
        assert!(c.record_run(Event::Race, 2, "gari", "mac", &table(2), 0));
        assert_eq!(c.medal(Event::Race, "gari"), Some(0));
        assert_eq!(c.points(), 15);
        // medals are the character's: another rider starts with none
        c.ensure_player(CareerRes::character("moby"));
        assert_eq!(c.medal(Event::Race, "gari"), None);
        assert_eq!(c.points(), 0);
    }

    #[test]
    fn a_showoff_scores_its_medal() {
        let mut c = fresh();
        let gold = showoff("gari").0[0];
        assert!(c.record_run(Event::ShowOff, 0, "gari", "elise", &[("Elise".into(), None, true)], gold));
        assert_eq!(c.medal(Event::ShowOff, "gari"), Some(0));
        assert_eq!(c.best("gari").1, gold);
        assert!(!c.record_run(Event::Race, 2, "nowhere", "elise", &[("Elise".into(), Some(1.0), true)], 0));
    }
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
    /// the player's character's four group levels in the career (its record's +0xc..+0xf), which its stat bytes are
    /// made from (`Career_SetupPlayerRider`)
    pub levels: Option<[u8; 4]>,
    /// how much each character dislikes the player's (its record's +0x164 + 8 · character), the value
    /// `Career_SetupAIRider` makes a computer rider's stat bytes with
    pub rivals: Option<[i32; 12]>,
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
    fn default() -> Self { Self { screen: Screen::Menu, event: Event::Race, round: 0, points: 0, levels: None, rivals: None, attitude: Default::default(), lineup: true, since: 0.0, table: Vec::new(), place: 0, master: false, cursor: 0, pause_cursor: 0, row: 0, board: std::env::var("TRICKY_BOARD").ok().and_then(|b| b.parse().ok()).unwrap_or(0), track_step: 0 } }
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
pub fn menus(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, time: Res<Time>, mut game: ResMut<Game>, mut lib: ResMut<CharLib>, mode: Res<Mode>, career: Res<CareerRes>, list: Res<LevelList>, book: Res<crate::book::Book>) {
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
            // (the uber board also by our trick book, which is not on the career block yet: row C15b)
            let open = game.master || (career.char_open(&me) && career.course_open(game.event, &track_key(&list)) && (career.board_open(game.board.min(11)) || (game.board == 11 && book.complete(&me))));
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
    opponents: Res<Opponents>, lib: Res<CharLib>, mode: Res<Mode>, (career, book): (Res<CareerRes>, Res<crate::book::Book>),
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
                    let best = {
                        let mut parts = Vec::new();
                        let (time, score) = career.best(&key);
                        if let Some(t) = time { parts.push(format!("best time {}", clock(t))); }
                        if score > 0 { parts.push(format!("best score {score}")); }
                        if let Some(m) = career.medal(Event::Race, &key) { parts.push(format!("race {}", MEDALS[m].to_lowercase())); }
                        if let Some(m) = career.medal(Event::ShowOff, &key) { parts.push(format!("show-off {}", MEDALS[m].to_lowercase())); }
                        if parts.is_empty() { "no results yet".to_string() } else { parts.join("   ") }
                    };
                    let data = crate::trickdata::RIDERS.iter().position(|d| d.name.eq_ignore_ascii_case(&me)).unwrap_or(0);
                    let board = &crate::trickdata::BOARDS[data][game.board.min(11)];
                    let kind = ["BX", "freestyle", "alpine"][(board.kind as usize).min(2)];
                    let st = r.stats;
                    let line = |i: usize, label: &str, value: String| if game.row == i { format!(">  {label}   <  {value}  >") } else { format!("{label}   {value}") };
                    format!("S S X   T R I C K Y\ntricky-rs\n\n{}\n{}\n{}\n{}\n{}\n\nedging {:.0}  speed {:.0}  stability {:.0}  tricks {:.0}\n{best}\ncareer points {}\n{}\n\nUp / Down: row    Left / Right: change    Enter: start",
                        line(0, "Event", ["World Circuit - Race", "World Circuit - Show-off", "Free Ride"][game.cursor.min(2)].to_string()),
                        line(1, "Rider", format!("{me}{}", if game.master || career.char_open(&me) { String::new() } else { "  - locked (each gold opens the next rider)".to_string() })),
                        line(2, "Board", format!("{} of 12  {}  ({kind}){}", game.board + 1, board.name,
                            { let need = BOARD_POINTS[game.board.min(11)]; if !career.board_open(game.board.min(11)) { format!("  - unlocks at {need} pts") } else { String::new() } })),
                        line(3, "Track", format!("{track}{}", if game.master || career.course_open(game.event, &track_key(&list)) { "" } else { "  - locked (medal on the track before)" })),
                        line(4, "Training", if game.master { "master (fully trained)".to_string() } else { format!("career - {} ({} of 240 pts)", rank(game.points), game.points) }),
                        st.edging * 100.0, st.speed * 100.0, st.stability * 100.0, st.tricks * 100.0, career.points(), book.summary(&me))
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
