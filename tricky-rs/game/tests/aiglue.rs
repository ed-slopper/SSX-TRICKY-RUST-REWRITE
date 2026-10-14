//! The computer rider's modes driving the port's rider down elysium's first AI path on the course's own collision
//! (board row G4e1; needs elysium's `.pbd`, `.ltg`, `.ssf` in TRICKY_PBD_DIR, TRICKY_LTG_DIR, TRICKY_SSF_DIR and its
//! `.aip` in TRICKY_AIP_DIR; skipped without them): a minute of 60 Hz ticks keeps it on the path and carries it on
//! along it.
use glam::Vec3;
use std::sync::Arc;
use tricky_game::aiglue::{AiRider, RaceView};
use tricky_game::aipath::Path;
use tricky_game::collide::CollisionWorld;
use tricky_game::course_ground::CourseGround;
use tricky_game::rails::Rails;
use tricky_game::rider::Rider;

#[test]
fn computer_rider_rides_its_path() {
    let (Ok(pdir), Ok(ldir), Ok(sdir), Ok(adir)) = (std::env::var("TRICKY_PBD_DIR"), std::env::var("TRICKY_LTG_DIR"), std::env::var("TRICKY_SSF_DIR"), std::env::var("TRICKY_AIP_DIR")) else { return };
    let Ok(aip) = std::fs::read(format!("{adir}/elysium.aip")) else { return };
    let read = |d: &str, e: &str| std::fs::read(format!("{d}/elysium.{e}")).unwrap();
    let cg = CourseGround::new(&read(&pdir, "pbd"), &read(&ldir, "ltg"), &read(&sdir, "ssf")).unwrap();
    let mut world = CollisionWorld::default();
    world.min_y = -1e9;
    world.course = Some(Arc::new(cg));
    let rails = Rails(Vec::new(), Default::default());
    let paths: Vec<Path> = tricky_data::aip::read(&aip).unwrap().ai_paths.iter().map(Path::from_aip).collect();

    // on the first path 50 m in, facing along it, on the snow
    let p = &paths[0];
    let at = |d: f32| {
        let v = p.position_at(d).0;
        Vec3::new(v[0], v[2], -v[1]) * 0.01
    };
    let (a, b) = (at(5000.0), at(5100.0));
    let dir = (b - a).normalize();
    let ground = world.ground(a, 50.0, 50.0).expect("snow under the path").y;
    let mut r = Rider::new(Vec3::new(a.x, ground + 0.05, a.z), f32::atan2(-dir.x, -dir.z));
    r.ai = true;
    r.vel = dir * 10.0;
    let mut ai = AiRider::new(&paths, r.pos, 1.0);
    let mut seed = 0x1234_5678u32;
    let mut rand = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as i32 & 0x7fff
    };
    let (mut worst, mut far, mut gone) = (0.0f32, 0, 0.0f32);
    for tick in 0..3600 {
        let view = RaceView { me: 0, others: &[], recs: &[], tick, practice: false };
        let path = ai.follow.path;
        let input = ai.tick(&r, &paths, &view, &mut rand);
        // the way made along the path (none on the tick it changes path)
        if ai.follow.path == path {
            gone += ai.follow.along - ai.follow.last;
        }
        r.step(&world, &rails, input, 1.0 / 60.0);
        if r.crashed <= 0.0 {
            worst = worst.max(ai.follow.off);
            far += (ai.follow.off > 1500.0) as i32;
        }
    }
    eprintln!("{gone:.0} cm along the paths, on path {} at the end, worst {worst:.0} cm off, {far} ticks over 15 m off", ai.follow.path);
    assert!(gone > 50_000.0, "it rode {gone} cm along its paths");
    assert!(far < 60, "{far} ticks over 15 m off the path");
}
