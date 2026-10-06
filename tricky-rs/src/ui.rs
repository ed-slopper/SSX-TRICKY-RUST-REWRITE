//! The front end: main menu, the event structure (race heats, show-off, free ride), the results
//! screen and the on-screen display while riding.

use crate::rider::RaceState;
use crate::{clock, standings, CharLib, LevelList, Mode, Opponents, RaceRes, RiderRes, SmoothDt};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Screen { Menu, Playing, Results }
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event { Race, ShowOff, FreeRide }
pub const ROUNDS: [&str; 3] = ["QUARTER-FINAL", "SEMI-FINAL", "FINAL"];
pub const MEDALS: [&str; 3] = ["GOLD", "SILVER", "BRONZE"];
/// show-off scores for gold, silver and bronze (my numbers, not the game's)
pub const SHOWOFF: [u32; 3] = [60_000, 35_000, 15_000];
/// riders in a race heat, the player included; the first three go through
pub const HEAT: usize = 6;

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
    pub cursor: usize,
}
impl Default for Game {
    fn default() -> Self { Self { screen: Screen::Menu, event: Event::Race, round: 0, lineup: true, since: 0.0, table: Vec::new(), place: 0, cursor: 0 } }
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
#[derive(Component)]
pub struct BoostFill;
#[derive(Component)]
pub struct PanelBox;
#[derive(Component)]
pub struct HudRoot;

pub fn setup_ui(mut commands: Commands) {
    let text = |size: f32, item: HudItem| (Text::new(""), TextFont { font_size: size, ..default() }, TextShadow { offset: Vec2::splat(2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.8) }, item);
    let abs = |n: Node| Node { position_type: PositionType::Absolute, ..n };
    commands.spawn((abs(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }), HudRoot)).with_children(|p| {
        p.spawn((text(54.0, HudItem::Place), abs(Node { top: Val::Px(62.0), left: Val::Px(24.0), ..default() })));
        p.spawn((text(34.0, HudItem::Time), abs(Node { top: Val::Px(14.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }), TextLayout::new_with_justify(JustifyText::Center)));
        p.spawn((text(34.0, HudItem::Score), abs(Node { top: Val::Px(14.0), right: Val::Px(24.0), ..default() }), TextLayout::new_with_justify(JustifyText::Right)));
        p.spawn((text(40.0, HudItem::Speed), abs(Node { bottom: Val::Px(52.0), right: Val::Px(24.0), ..default() }), TextLayout::new_with_justify(JustifyText::Right)));
        p.spawn((text(30.0, HudItem::Letters), abs(Node { bottom: Val::Px(84.0), left: Val::Px(24.0), ..default() })));
        p.spawn((text(30.0, HudItem::Trick), abs(Node { bottom: Val::Px(110.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }), TextLayout::new_with_justify(JustifyText::Center)));
        p.spawn((text(120.0, HudItem::Big), abs(Node { top: Val::Percent(28.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() }), TextLayout::new_with_justify(JustifyText::Center)));
        p.spawn((text(13.0, HudItem::Fps), abs(Node { bottom: Val::Px(4.0), left: Val::Px(10.0), ..default() })));
        // the boost meter
        p.spawn((abs(Node { bottom: Val::Px(22.0), right: Val::Px(24.0), width: Val::Px(300.0), height: Val::Px(22.0), border: UiRect::all(Val::Px(2.0)), ..default() }),
                 BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)), BorderColor(Color::WHITE)))
            .with_children(|b| { b.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(1.0, 0.75, 0.1)), BoostFill)); });
    });
    // menu and results panel
    commands.spawn((abs(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::FlexStart, align_items: AlignItems::Center, padding: UiRect::left(Val::Px(50.0)), ..default() }), PanelBox))
        .with_children(|p| {
            p.spawn((Node { padding: UiRect::axes(Val::Px(48.0), Val::Px(30.0)), ..default() }, BackgroundColor(Color::srgba(0.02, 0.05, 0.15, 0.78))))
                .with_children(|b| { b.spawn((text(30.0, HudItem::Panel), TextLayout::new_with_justify(JustifyText::Center))); });
        });
}

/// Menu and results input. Riding input is read in `ride`.
pub fn menus(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, time: Res<Time>, mut game: ResMut<Game>, mut lib: ResMut<CharLib>, mode: Res<Mode>) {
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
            if up { game.cursor = (game.cursor + 2) % 3; }
            if down { game.cursor = (game.cursor + 1) % 3; }
            game.event = [Event::Race, Event::ShowOff, Event::FreeRide][game.cursor];
            let n = lib.chars.len();
            if n > 0 && (left || right) {
                lib.player = (lib.player + if right { 1 } else { n - 1 }) % n;
                game.lineup = true;
            }
            if ok && game.since > 0.2 { game.round = 0; game.screen = Screen::Playing; game.since = 0.0; game.lineup = true; }
        }
        Screen::Playing => {
            if back { game.screen = Screen::Menu; game.since = 0.0; game.lineup = true; }
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
    opponents: Res<Opponents>, lib: Res<CharLib>, mode: Res<Mode>,
    mut texts: Query<(&mut Text, &HudItem, &mut TextColor)>, mut fill: Single<(&mut Node, &mut BackgroundColor), With<BoostFill>>,
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
    let track = if track == "gari" { "Garibaldi".to_string() } else { track };
    let me = lib.chars.get(lib.player).map(|c| crate::proper(&c.name)).unwrap_or_else(|| "Rider".into());
    **root = if playing { Visibility::Inherited } else { Visibility::Hidden };
    **panel = if riding && game.screen != Screen::Playing { Visibility::Inherited } else { Visibility::Hidden };

    fill.0.width = Val::Percent(if r.tricky() { 100.0 } else { r.boost.clamp(0.0, 1.0) * 100.0 });
    fill.1.0 = if r.tricky() { Color::srgb(1.0, 0.25, 0.8) } else if r.boost >= 0.99 { Color::srgb(1.0, 0.3, 0.15) } else if r.boosting { Color::srgb(1.0, 1.0, 0.5) } else { Color::srgb(1.0, 0.75, 0.1) };

    for (mut text, item, mut color) in &mut texts {
        let mut tint = Color::WHITE;
        let new = match item {
            HudItem::Place => {
                if game.event == Event::Race && !opponents.0.is_empty() && race.0.state != RaceState::Countdown {
                    let place = standings(&race.0, &opponents.0, &lib, &me).iter().position(|t| t.2).unwrap_or(0);
                    format!("{}", ordinal(place))
                } else { String::new() }
            }
            HudItem::Time => match game.event {
                Event::Race => format!("{}\n", clock(race.0.time)),
                Event::ShowOff => format!("{}", clock(race.0.time)),
                Event::FreeRide => format!("{}{}", clock(race.0.time), race.0.best.map(|b| format!("   best {}", clock(b))).unwrap_or_default()),
            },
            HudItem::Score => {
                let goal = if game.event == Event::ShowOff {
                    match SHOWOFF.iter().rposition(|s| r.score < *s) { Some(i) => format!("\n{} at {}", MEDALS[i].to_lowercase(), SHOWOFF[i]), None => "\nGOLD".into() }
                } else { String::new() };
                format!("{}{}{goal}", r.score, if r.multiplier > 1 { format!("  x{}", r.multiplier) } else { String::new() })
            }
            HudItem::Speed => format!("{:.0} km/h", r.vel.length() * 3.6),
            HudItem::Letters => {
                let word: String = "TRICKY".chars().enumerate().map(|(i, c)| if (i as u8) < r.letters { c } else { '.' }).collect();
                if r.tricky() { tint = Color::srgb(1.0, 0.4, 0.85); }
                let note = if r.tricky() { "  unlimited boost" } else if r.boost >= 0.99 { "  UBER READY - U in the air" } else { "" };
                format!("{word}{note}")
            }
            HudItem::Trick => {
                if r.last_trick == "CRASH" { tint = Color::srgb(1.0, 0.3, 0.25); } else { tint = Color::srgb(1.0, 0.92, 0.4); }
                tint = tint.with_alpha(r.trick_timer.min(1.0));
                let state = if r.rail.is_some() { "GRIND" } else if r.charge > 0.0 && r.grounded { "" } else { "" };
                if r.trick_timer > 0.0 { r.last_trick.clone() } else { state.to_string() }
            }
            HudItem::Big => {
                if !playing { String::new() } else {
                    match race.0.state {
                        RaceState::Countdown => if race.0.countdown > 0.4 { format!("{}", race.0.countdown.ceil() as i32) } else { "GO!".into() },
                        RaceState::Running if race.0.time < 0.8 => "GO!".into(),
                        RaceState::Finished if game.event == Event::FreeRide => "FINISH".into(),
                        _ => String::new(),
                    }
                }
            }
            HudItem::Fps => format!("{:.0} fps   slowest frame {:.0} ms{}", fps.0, smooth.worst_ms, if riding { "" } else { "   free camera" }),
            HudItem::Panel => match game.screen {
                Screen::Menu => {
                    let row = |i: usize, s: &str| if game.cursor == i { format!(">  {s}  <") } else { s.to_string() };
                    format!("S S X   T R I C K Y\ntricky-rs\n\n{}\n{}\n{}\n\nRider   <  {me}  >\nTrack   {track}\n\nUp / Down: event    Left / Right: rider    Enter: start",
                        row(0, "World Circuit - Race"), row(1, "World Circuit - Show-off"), row(2, "Free Ride"))
                }
                Screen::Playing => String::new(),
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
                        let medal = SHOWOFF.iter().position(|s| r.score >= *s);
                        format!("{track}  -  SHOW-OFF\n\n{me}\nscore {}\ntime {}\n\n{}\n\ngold {}   silver {}   bronze {}\n\nEnter: try again    Esc: menu", r.score, clock(race.0.time),
                            medal.map(|m| format!("{} MEDAL", MEDALS[m])).unwrap_or_else(|| "No medal".into()), SHOWOFF[0], SHOWOFF[1], SHOWOFF[2])
                    }
                    Event::FreeRide => String::new(),
                },
            },
        };
        if text.0 != new { text.0 = new; }
        if color.0 != tint { color.0 = tint; }
    }
}
