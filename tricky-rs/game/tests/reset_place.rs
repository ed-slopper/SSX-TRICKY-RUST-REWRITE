//! The game's reset place (board row G20c3b5; TRICKY_DISC for the courses' `.aip`, skipped without it): on every
//! course's own paths, `aiglue::reset_place` is `ResetState_PlaceRider` 0x119220 with no spread (`aipath::place_rider`,
//! checked against the original in `tests/reset.rs`): the respawn point on the rider's path (`Path::respawn_at`) and
//! the inline atan2 of the path's direction there; and a rider sent back to a safe spot kept there respawns at that
//! heading, as `respawn_heading` places it.

use tricky_game::aipath::{self, Cache, Follow, Path, RaceLines, RacePlace};
use tricky_game::rider::Rider;

fn courses() -> Vec<(&'static str, Vec<Path>)> {
    let Ok(disc) = std::env::var("TRICKY_DISC") else { return Vec::new() };
    let Ok(d) = tricky_data::disc::Disc::open(&disc) else { return Vec::new() };
    let mut files = tricky_data::big::Files::new(d);
    ["gari", "snowdream", "elysium", "mesablanca", "merqury", "megaplex", "aloha", "alaska", "pipedream", "untracked"]
        .into_iter()
        .filter_map(|c| Some((c, tricky_data::aip::from_disc(&mut files, c, "aip")?.ai_paths.iter().map(Path::from_aip).collect())))
        .collect()
}

fn follow(path: usize, along: f32) -> Follow {
    Follow { path, last: along, along, off: 0.0, near: [0.0; 4], ahead: [0.0; 4], heading: 0.0, cache: Cache::default() }
}

#[test]
fn the_reset_place_is_the_games() {
    let cs = courses();
    let mut n = 0;
    for (name, paths) in &cs {
        assert!(!paths.is_empty(), "{name}");
        for (k, p) in paths.iter().enumerate() {
            let len = p.length();
            for i in 0..40 {
                let along = len * i as f32 / 39.0;
                let f = follow(k, along);
                let (p_at, h) = tricky_game::aiglue::reset_place(paths, &f).expect("a place");
                // `ResetState_PlaceRider` itself, no spread
                let mut g = f;
                let mut r = RacePlace { line: 0, to_finish: 0.0, best: 0.0, cache: Cache::default() };
                let mut out = Vec::new();
                assert!(aipath::place_rider(paths, &RaceLines::default(), &mut g, &mut r, 0, false, 0.0, &[], &mut |_, _| -1.0, &mut out));
                assert_eq!(out.len(), 1);
                assert_eq!(h.to_bits(), out[0].heading.to_bits(), "{name} path {k} at {along}");
                // the respawn point and the path's direction there
                let (at, pt) = p.respawn_at(along);
                let dir = p.direction_at(at);
                assert_eq!(h.to_bits(), tricky_game::jump::atan2_inline(dir[1], dir[0]).to_bits());
                let back = [p_at.x * 100.0, -p_at.z * 100.0, p_at.y * 100.0];
                for c in 0..3 {
                    assert!((back[c] - pt[c]).abs() <= pt[c].abs() * 1e-6 + 1e-3, "{name} path {k}: the point");
                }
                n += 1;
            }
        }
    }
    if !cs.is_empty() {
        assert!(n > 1000, "{n}");
        println!("{} courses, {n} places", cs.len());
    }
}

#[test]
fn a_rider_sent_back_comes_back_at_the_games_heading() {
    let cs = courses();
    let Some((_, paths)) = cs.first() else { return };
    let k = (0..paths.len()).find(|&k| paths[k].respawnable != 0).unwrap_or(0);
    let (p, h) = tricky_game::aiglue::reset_place(paths, &follow(k, paths[k].length() * 0.5)).unwrap();
    let mut a = Rider::new(glam::Vec3::ZERO, 0.0);
    (a.safe, a.safe_heading, a.safe_external) = ((p, 0.7), Some(h), true);
    send_back(&mut a);
    let mut b = Rider::new(glam::Vec3::ZERO, 0.0);
    b.respawn_heading(p, h);
    assert_eq!(a.place_ray, Some((p, h)));
    assert_eq!((a.pos, a.yaw, a.vel), (b.pos, b.yaw, b.vel));
    assert_eq!(format!("{:?}", a.att), format!("{:?}", b.att));
    // without the game's heading, the port's yaw as before
    let mut c = Rider::new(glam::Vec3::ZERO, 0.0);
    c.safe = (p, 0.7);
    send_back(&mut c);
    assert_eq!(c.yaw, 0.7);
}

/// A reset through the reset state (row E1b2g7d3b4h): the rider stands still for 48 ticks, faded as
/// `ResetState_Update` 0x106a90 fades it, and is placed on the 49th (past 0.8 s), the fades back
fn send_back(r: &mut Rider) {
    r.request_reset(Some(1));
    assert!(r.reset.is_some());
    let at = r.pos;
    for k in 1..=49 {
        r.reset_tick(1.0 / 60.0);
        if k < 49 {
            assert_eq!(r.pos, at, "tick {k}");
            assert!(r.reset.is_some());
        }
        if k == 30 {
            // 0.5 s: the rider faded out, the screen faded in
            assert!(r.fade.0 < 1e-5 && (r.fade.1 - 1.0).abs() < 1e-5, "{:?}", r.fade);
        }
    }
    assert!(r.reset.is_none());
    assert_eq!(r.fade, (1.0, 0.0));
    assert_eq!(r.respawns, 1);
}

#[test]
fn no_reset_where_the_path_forbids_it_or_once_finished() {
    let mut r = Rider::new(glam::Vec3::ZERO, 0.0);
    r.reset_ctx.no_reset = true;
    r.request_reset(Some(1));
    assert!(r.reset.is_none() && r.speech.is_empty());
    let mut r = Rider::new(glam::Vec3::ZERO, 0.0);
    r.fin = Some((0, 0.0, true));
    r.request_reset(Some(1));
    assert!(r.reset.is_none());
    // a player's reset is heard, a computer rider's is not (+0x41c)
    let mut r = Rider::new(glam::Vec3::ZERO, 0.0);
    r.request_reset(Some(1));
    assert_eq!(r.speech.len(), 1);
    let mut r = Rider::new(glam::Vec3::ZERO, 0.0);
    r.ai = true;
    r.request_reset(Some(1));
    assert!(r.reset.is_some() && r.speech.is_empty());
}

#[test]
fn a_race_reset_moves_on_by_the_next_places_speed() {
    use tricky_game::reset::RaceRider;
    // mode 4 (World Circuit race), 200 m from the finish, second of two: moved on by 0.8003195 × the leader's speed
    let mut r = Rider::new(glam::Vec3::ZERO, 0.0);
    r.reset_ctx = tricky_game::rider::ResetCtx { no_reset: false, mode: 4, course: 1, me: 1, riders: vec![
        RaceRider { to_finish: 15000.0, place: 0, vel: [3000.0, 4000.0, 0.0, 0.0] },
        RaceRider { to_finish: 20000.0, place: 1, vel: [0.0; 4] },
    ] };
    r.request_reset(Some(1));
    assert_eq!(r.reset.unwrap().advance, 5000.0 * 0.8003195);
    // a computer rider (the stats' +0x65 negative) carries no speed
    r = Rider::new(glam::Vec3::ZERO, 0.0);
    r.ai = true;
    r.reset_ctx = tricky_game::rider::ResetCtx { no_reset: false, mode: 4, course: 1, me: 1, riders: vec![
        RaceRider { to_finish: 15000.0, place: 0, vel: [3000.0, 4000.0, 0.0, 0.0] },
        RaceRider { to_finish: 20000.0, place: 1, vel: [0.0; 4] },
    ] };
    r.request_reset(Some(1));
    assert_eq!(r.reset.unwrap().advance, 0.0);
}
