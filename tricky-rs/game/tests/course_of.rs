//! Which course a recording was made on, for recordings made before the recorder wrote the course id (`course N`,
//! F4h5k): for each course on the player's disc, a line from 1 m above each rider on the snow (motion 2) down along
//! its ground normal (+0x2a0) to 2 m below; the course where the most hit within 60 cm of the rider (its ride height is under that)
//! is the one. TRICKY_RECORDING (`;`-separated) and TRICKY_DISC; skipped without them.

mod recording;

use recording::{load, u32_le, vec4};

const COURSES: [&str; 11] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "trick", "alaska"];

#[test]
fn course_of_each_recording() {
    let (Ok(paths), Ok(disc)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_DISC")) else { return };
    let courses: Vec<_> = COURSES
        .iter()
        .map(|name| {
            let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).unwrap());
            files.mount(&format!("data/models/{name}.big")).unwrap();
            let mut get = |ext: &str| files.load(&format!("|data/models/{name}.{ext}")).unwrap().unwrap();
            let (p, l, s) = (get("pbd"), get("ltg"), get("ssf"));
            tricky_game::course_ground::CourseGround::new(&p, &l, &s).unwrap()
        })
        .collect();
    for path in paths.split(';') {
        let (n, ticks) = load(path);
        let mut score = vec![(0usize, 0usize); COURSES.len()];
        for t in ticks.iter().step_by(97) {
            for i in 0..n {
                let a = &t.riders[i];
                if u32_le(a, 0x424) != 2 {
                    continue;
                }
                let (p, g) = (vec4(a, 0x140), vec4(a, 0x2a0));
                let at = |k: f32| [p[0] + g[0] * k, p[1] + g[1] * k, p[2] + g[2] * k, 1.0];
                for (c, course) in courses.iter().enumerate() {
                    let mut q = tricky_game::worldline::WorldLine::new(1.0, at(100.0), at(-200.0), 2);
                    let (t, h) = course.ray_cast(&mut q, false, None, &|_| false);
                    score[c].1 += 1;
                    if let Some(h) = h.filter(|_| 0.0 < t) {
                        let d = [h.pos[0] - p[0], h.pos[1] - p[1], h.pos[2] - p[2]];
                        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < 60.0 {
                            score[c].0 += 1;
                        }
                    }
                }
            }
        }
        let best = (0..COURSES.len()).max_by_key(|&c| score[c].0).unwrap();
        eprintln!("{path}: {} ({:?})", COURSES[best], COURSES.iter().zip(&score).map(|(c, s)| format!("{c} {}/{}", s.0, s.1)).collect::<Vec<_>>());
    }
}
