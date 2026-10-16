//! `tricky_game::musichost` on a real song (board row G10b2c2b; TRICKY_DISC for MUSIC.INF and MUSIC.BIG and the app's
//! decoded `audio/pf/<song>/graph.json`, skipped without them): `Audio_StartRaceMusic` on the music system's layers
//! reads the song's MUSIC.INF section into the song (BPM, the beats, PathLevel), loads and registers its graph, loads
//! its track; the song then plays through with no gap at the stream volume the layers give, and the finish takes it
//! to its end.

use std::collections::HashMap;

use tricky_game::musichost::MusicHost;
use tricky_game::pfplay::Driver;

fn disc_files() -> Option<tricky_data::big::Files> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount_game_archives().ok()?;
    Some(files)
}

fn segments(song: &str) -> Option<(u32, HashMap<i16, (usize, usize)>)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../audio/pf").join(song);
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("graph.json")).ok()?).ok()?;
    let rate = v["rate"].as_u64()? as u32;
    let mut m = HashMap::new();
    for (k, s) in v["segments"].as_object()? {
        let a = s.as_array()?;
        m.insert(k.parse().ok()?, (a[0].as_u64()? as usize, a[1].as_u64()? as usize));
    }
    Some((rate, m))
}

#[test]
fn a_race_song_on_the_music_system() {
    let Some(mut files) = disc_files() else { return };
    let Some((rate, segs)) = segments("bassinvader") else { return };
    let inf = files.load("data/config/music.inf").ok().flatten().expect("MUSIC.INF");
    let loader = Box::new(move |p: &str| files.load(p).ok().flatten());
    let mut host = MusicHost::new(&inf, loader);
    let index = host.song_index(b"Bassinvaders").expect("the song's section");
    let mut d = Driver::new(rate, HashMap::new());
    host.start_race_music(&mut d, index);
    let s = host.song().expect("a song");
    let m = &host.mem;
    // the section, as MUSIC.INF gives it: BPM 142, 4 beats a bar, 2 bars, PathLevel 90, AsyncLevel 90
    assert_eq!(f32::from_bits(m.r32(s + 0x100)), 142.0);
    assert_eq!((m.r32(s + 0x104), m.r32(s + 0x108), m.r32(s + 0x12c), m.r32(s + 0x130)), (4, 2, 90, 90));
    assert!(0 <= m.r32(s) as i32, "the track loaded");
    host.map_segments(&mut d, &segs);
    assert!(!d.segments.is_empty());
    let step = rate as usize / 100;
    let mut finished = false;
    for k in 0..100 * 360 {
        d.pull(step);
        if k % 2 == 0 {
            host.set_path_var(&mut d, 80);
            host.frame(&mut d, 0.02);
        }
        if !finished && k == 100 * 300 {
            finished = true;
            host.music_event(&mut d, 10);
        }
    }
    println!("{} requests started; stream volume {}", d.started.len(), d.volume);
    assert!(d.started.len() > 150, "the song plays on");
    for w in d.started.windows(2) {
        let len = d.segments.get(&w[0].1).map_or(0, |s| s.1) as u64;
        assert_eq!(w[1].0, w[0].0 + len, "no gap");
    }
    // `Song_UpdateVolume`: 0x7f × PathLevel 90 % × the music volume 1.0 × the master 1.0
    assert_eq!(d.volume, (127.0f32 * 0.9) as u8);
}
