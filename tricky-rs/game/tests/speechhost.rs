//! `tricky_game::speechhost` on the disc (board row E1b2g7d3b2; TRICKY_DISC, skipped without it): the speech system
//! set up for a race loads the announcer's and the riders' banks from SPEECH.BIG, and the announcer's place-change
//! event (0x1002049, `Speech_OnPlaceChange` 0x233ba0) posted over its words is started on the announcer's stream as
//! parts of the MC banks.

use tricky_game::speechhost::SpeechHost;

fn host() -> Option<SpeechHost> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount_game_archives().ok()?;
    let inf = files.load("data/config/speech.inf").ok().flatten().expect("SPEECH.INF");
    Some(SpeechHost::new(&inf, Box::new(move |p: &str| files.load(p).ok().flatten())))
}

#[test]
fn an_announcer_line_from_the_disc() {
    let Some(mut h) = host() else { return };
    assert_eq!(h.bank_count(), 115);
    let mut plays = Vec::new();
    for who in 0..3 {
        for new in 0..6i64 {
            for grade in 0..4 {
                let mut ev = [0u32; 20];
                ev[0] = 0x1002049;
                ev[1] = who;
                ev[2] = tricky_game::speech::place_mask(new as i32 + 1, new);
                ev[3] = grade;
                assert_eq!(h.post(&ev), 1);
                h.update();
                plays.extend(h.take_plays());
                h.frame(10.0);
            }
        }
    }
    assert!(!plays.is_empty());
    assert!(plays.iter().all(|p| p.stream == 0 && p.file.starts_with("|data\\speech\\mc\\") && p.file.ends_with(".dat")));
}
