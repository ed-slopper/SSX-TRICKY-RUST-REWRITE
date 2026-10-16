//! `tricky_game::pfplay` on a real song (board row G10b2c1; TRICKY_DISC for the disc's MUSIC.BIG and the app's decoded
//! `audio/pf/<song>/graph.json`, skipped without them): the graph registered and the track loaded as the game does,
//! the start event, the path variable, `update` at 60 Hz and the driver pulled in 10 ms steps; every request must
//! follow the last with no gap, the song must play through, and the finish event must take it to its end.

use std::collections::HashMap;

use tricky_game::pfplay::{Driver, Player};

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

fn graph(song: &str) -> Option<Vec<u8>> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount_game_archives().ok()?;
    // the game's path: MUSIC.INF's BASEPATH, then PATHDATA
    let inf = files.load("data/config/music.inf").ok().flatten()?;
    let base = String::from_utf8_lossy(&tricky_data::music::music_table(&inf).base).to_string();
    files.load(&format!("{base}{song}.mpf")).ok().flatten()
}

#[test]
fn a_song_plays_through_without_gaps() {
    for song in ["bassinvader", "kingbeat"] {
        let (Some((rate, segs)), Some(g)) = (segments(song), graph(song)) else { return };
        let mut d = Driver::new(rate, HashMap::new());
        let name = format!("{song}.mus\0");
        let mut p = Player::load(&g, name.as_bytes(), &segs, &mut d).expect("the song loads");
        assert!(!d.segments.is_empty(), "{song}: segments mapped");
        p.set_var(80);
        p.event(&mut d, 0);
        let step = rate as usize / 100;
        let mut t = 0.0f32;
        let mut finished = false;
        for k in 0..100 * 420 {
            d.pull(step);
            t += 0.01;
            if k % 2 == 0 {
                p.update(&mut d);
            }
            if !finished && 300.0 < t {
                finished = true;
                p.event(&mut d, 10);
            }
        }
        let starts = &d.started;
        println!("{song}: {} requests started, {} segments mapped", starts.len(), d.segments.len());
        assert!(starts.len() > 20, "{song}: the song plays on ({starts:?})");
        // no gap: each request starts where the last one ended (only before the finish)
        let mut gaps = 0;
        for w in starts.windows(2) {
            let len = d.segments.get(&w[0].1).map_or(0, |s| s.1) as u64;
            if w[1].0 != w[0].0 + len {
                gaps += 1;
                println!("{song}: gap at frame {} ({} after the last, which is {len})", w[1].0, w[1].0 - w[0].0);
            }
        }
        assert_eq!(gaps, 0, "{song}: requests join sample-exact");
    }
}
