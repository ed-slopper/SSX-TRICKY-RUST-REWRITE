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

/// The air loop on the game's code (row G10b2c2c1): the song's LOOPDATA bank asked for at the start; a break (as big
/// air asks one) fires the break event, which schedules the air loop at the song's next beat; then one sound of the
/// bank a beat, the programs from that beat of the loop's phrase up through it (BeatsPerPhrase) and round, at the volume
/// AsyncLevel % gives; Tricky takes the bank's last phrase; the forced cancel stops it.
#[test]
fn the_air_loop_plays_a_beat_at_a_time() {
    let Some(mut files) = disc_files() else { return };
    let Some((rate, segs)) = segments("bassinvader") else { return };
    let inf = files.load("data/config/music.inf").ok().flatten().expect("MUSIC.INF");
    let mut host = MusicHost::new(&inf, Box::new(move |p: &str| files.load(p).ok().flatten()));
    let index = host.song_index(b"Bassinvaders").expect("the song's section");
    let mut d = Driver::new(rate, HashMap::new());
    host.start_race_music(&mut d, index);
    host.map_segments(&mut d, &segs);
    let s = host.song().unwrap();
    let m = &host.mem;
    let (beats, phrases, align, level) = (m.r32(s + 0x114), m.r32(s + 0x10c), m.r32(s + 0x118), m.r32(s + 0x130));
    let beat_ms = 60000.0 / f32::from_bits(m.r32(s + 0x100));
    println!("bank {:?}; {beats} beats a phrase, {phrases} phrases, align {align}, AsyncLevel {level}, a beat {beat_ms} ms", host.loop_bank());
    assert!(host.loop_bank().is_some_and(|p| p.to_ascii_lowercase().ends_with(".bnk")), "the song's LOOPDATA bank");
    // a few seconds of the song, then the break
    let step = rate as usize / 100;
    let mut t = 0u32;
    let mut run = |host: &mut MusicHost, d: &mut Driver, ms: u32, out: &mut Vec<(u32, tricky_game::musichost::LoopPlay)>| {
        for _ in 0..ms / 10 {
            d.pull(step);
            host.frame(d, 0.01);
            t += 10;
            out.extend(host.take_loops().into_iter().map(|l| (t, l)));
        }
    };
    let mut got = Vec::new();
    run(&mut host, &mut d, 3000, &mut got);
    assert!(got.is_empty(), "no air loop before a break");
    host.schedule_break(&mut d, false);
    run(&mut host, &mut d, 8000, &mut got);
    println!("{} loop sounds: {:?}", got.len(), &got[..got.len().min(12)]);
    let n = (8000.0 / beat_ms) as usize;
    assert!(got.len() + 2 >= n && got.len() <= n + 1, "one a beat: {} in 8 s", got.len());
    // the loop's first program a phrase's start (of the bank's first PhrasesPerbank − 1), the first sound on the
    // song's next beat into it
    let first = got[0].1.program - got[0].1.program % align;
    assert!(first / align < phrases - 1, "a phrase of the bank's first PhrasesPerbank − 1");
    for w in got.windows(2) {
        let dt = w[1].0 - w[0].0;
        assert!((dt as f32 - beat_ms).abs() <= 10.0, "a beat apart: {dt} ms");
    }
    for (k, (_, l)) in got.iter().enumerate() {
        assert_eq!(l.volume, (127.0 * level as f32 / 100.0) as u8);
        assert_eq!(l.bank, host.mem.r32(s + 8));
        // the program: the loop's first one plus the beat in the phrase, round again
        let k0 = (got[0].1.program - first) as usize;
        assert_eq!(l.program - first, ((k0 + k) % beats as usize) as u32, "sound {k}");
    }
    // the forced cancel stops it
    host.cancel_break(&mut d, true);
    let mut after = Vec::new();
    run(&mut host, &mut d, 2000, &mut after);
    assert!(after.is_empty(), "stopped");
    // Tricky through its trigger (row G10b2c2c2a): the bank's last phrase at the Tricky volume 0x7f × 90 %
    host.tricky_trigger(&mut d, false);
    let mut tricky = Vec::new();
    run(&mut host, &mut d, 3000, &mut tricky);
    println!("Tricky: {:?}", &tricky[..tricky.len().min(4)]);
    assert!(!tricky.is_empty());
    assert_eq!(tricky[0].1.program - tricky[0].1.program % align, (phrases - 1) * align, "the last phrase");
    assert!(tricky.iter().all(|(_, l)| l.volume == (127.0 * level as f32 / 100.0) as u8), "the Tricky volume");
    // its phrase out and once again (the break it asked), then `Music_CancelTricky`: no more
    let mut rest = Vec::new();
    run(&mut host, &mut d, 10000, &mut rest);
    println!("Tricky {} + {} sounds", tricky.len(), rest.len());
    // (from the song's next beat to the phrase's end, then once through again)
    let k0 = (tricky[0].1.program % align) as usize;
    assert_eq!(tricky.len() + rest.len(), beats as usize - k0 + beats as usize, "to the phrase's end, then once again");
}
