//! Each course's own music for the pre-race intro (DATA/AUDIO/<COURSE>.BIG per INTROMUS.INF,
//! `Music_StartCourseSong` 0x214e60, `Audio_InGameUpdate` 0x20ee58, `Music_FinishCourseSong`
//! 0x214e08, `Audio_OnRaceGo` 0x215288). The BIG holds 17 segments: A1-A4, B1-B4, C1-C8, end.
//!
//! At the start: A1, a random one of A2-A4, two random B, then random C for as long as the intro
//! runs (Mesablanca, Untracked, Megaplex and Alaska play theirs in order and loop C1-C8). When
//! the intro ends (or is skipped) the `end` segment plays; GO cuts it off and the race song,
//! held back until then, starts.
//!
//! Files: `audio/course/<track>/seg_00.wav` .. `seg_16.wav` (tools/bnk/levelaudio.py).

use crate::rider::RaceState;
use crate::{ui, LevelList, Mode, RaceRes};
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct CourseMusic {
    /// the current course has its music and it is the intro or countdown
    pub holding: bool,
    dir: std::path::PathBuf,
    seed: u32,
}

#[derive(Component, PartialEq)]
pub enum CoursePart { Intro, End }

/// Join 16-bit PCM wavs of one format into one wav.
fn join(parts: &[Vec<u8>]) -> Option<Vec<u8>> {
    let chunk = |w: &[u8], id: &[u8; 4]| -> Option<(usize, usize)> {
        let mut p = 12;
        while p + 8 <= w.len() {
            let n = u32::from_le_bytes(w[p + 4..p + 8].try_into().ok()?) as usize;
            if &w[p..p + 4] == id { return Some((p + 8, n.min(w.len() - p - 8))); }
            p += 8 + n + (n & 1);
        }
        None
    };
    let first = parts.first()?;
    let (fp, fl) = chunk(first, b"fmt ")?;
    let fmt = first[fp..fp + fl].to_vec();
    let mut data = Vec::new();
    for w in parts {
        let (dp, dl) = chunk(w, b"data")?;
        data.extend_from_slice(&w[dp..dp + dl]);
    }
    let mut out = Vec::with_capacity(data.len() + 64);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((4 + 8 + fmt.len() + 8 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    out.extend_from_slice(&fmt);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    Some(out)
}

/// The intro's course music (`Music_StartCourseSong` 0x214e60, `Audio_InGameUpdate` 0x20ee58, `Audio_OnRaceGo` 0x215288).
#[allow(clippy::too_many_arguments)]
pub fn course_music(
    mut cm: ResMut<CourseMusic>, race: Res<RaceRes>, list: Res<LevelList>, game: Res<ui::Game>, mode: Res<Mode>, sounds: Res<crate::sound::Sounds>,
    mut assets: ResMut<Assets<AudioSource>>, mut commands: Commands, parts: Query<(Entity, &CoursePart)>, mut sinks: Query<&mut AudioSink, With<CoursePart>>,
    mut last: Local<Option<(usize, bool, RaceState, ui::Screen)>>,
) {
    if cm.dir.as_os_str().is_empty() { cm.dir = sounds.dir.join("course"); cm.seed = 0x9e3779b9; }
    let track = list.dirs.get(list.current).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = cm.dir.join(&track);
    let have = dir.join("seg_16.wav").is_file();
    let r = &race.0;
    let playing = *mode == Mode::Ride && matches!(game.screen, ui::Screen::Playing | ui::Screen::Paused);
    let now = (list.current, r.intro, r.state, game.screen);
    let was = last.replace(now);
    cm.holding = have && playing && r.state == RaceState::Countdown;
    let restart = was.is_none_or(|w| w.0 != now.0 || (w.2 != RaceState::Countdown && now.2 == RaceState::Countdown) || (w.3 == ui::Screen::Menu && now.3 != ui::Screen::Menu));
    let intro_over = was.is_some_and(|w| w.1 && !now.1);
    let go = was.is_some_and(|w| w.2 == RaceState::Countdown && now.2 != RaceState::Countdown);
    let quit = !playing || r.state != RaceState::Countdown;
    let clear = |commands: &mut Commands, which: Option<&CoursePart>| for (e, p) in &parts { if which.is_none_or(|w| w == p) { commands.entity(e).try_despawn(); } };
    if go || (quit && !parts.is_empty() && was.is_some_and(|w| w.2 == RaceState::Countdown || w.3 != now.3)) { clear(&mut commands, None); }
    if !have || !playing || r.state != RaceState::Countdown { return; }
    let vol = if sounds.music { 0.4 } else { 0.0 };
    for mut s in &mut sinks { s.set_volume(Volume::Linear(vol)); }
    let load = |i: usize| std::fs::read(dir.join(format!("seg_{i:02}.wav"))).ok();
    let mut rand = |n: u32| { cm.seed ^= cm.seed << 13; cm.seed ^= cm.seed >> 17; cm.seed ^= cm.seed << 5; (cm.seed % n) as usize };
    if restart {
        clear(&mut commands, None);
        if r.intro {
            // the queue the game builds, long enough for any intro (the C part loops)
            let order: Vec<usize> = if matches!(track.as_str(), "mesablanca" | "untracked" | "megaplex" | "alaska") {
                (0..16).chain(8..16).chain(8..16).collect()
            } else {
                let mut o = vec![0, 1 + rand(3), 4 + rand(4), 4 + rand(4)];
                for _ in 0..24 { o.push(8 + rand(8)); }
                o
            };
            let parts: Option<Vec<Vec<u8>>> = order.iter().map(|i| load(*i)).collect();
            if let Some(bytes) = parts.and_then(|p| join(&p)) {
                let h = assets.add(AudioSource { bytes: bytes.into() });
                commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(vol), ..default() }, CoursePart::Intro));
                println!("course music: intro, {} segments", order.len());
            }
            return;
        }
    }
    // the intro over (or never shown): the closing segment until GO
    if intro_over || (restart && !r.intro) {
        clear(&mut commands, Some(&CoursePart::Intro));
        if let Some(bytes) = load(16) {
            let h = assets.add(AudioSource { bytes: bytes.into() });
            commands.spawn((AudioPlayer::new(h), PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol), ..default() }, CoursePart::End));
            println!("course music: end");
        }
    }
}
