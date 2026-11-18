//! A keyframed object's clock over a race against the running game's (row F4h5l): `tools/pine.py clock` reads one
//! object's controller and its `cAnimObjectNode` time every tick with the riders' positions and tick counts at the
//! frame's end (TRICKY_CLOCK, the file; TRICKY_DISC, the player's disc; the course from the file's header or
//! TRICKY_COURSE; skipped without them). The riders' positions go through `course_clock::clocks` (the live cells, the
//! persistent scripts, the clock with PCSX2's adder): the frames the object has a node, and its time on each, bit for
//! bit. Samples not of a frame's end (a rider's tick count − the race tick not the recording's usual) are left out.

mod course_clock;

use std::collections::HashMap;

use r5900::ps2float::{add_by, Round};

const COURSES: [&str; 12] = ["gari", "snow", "elysium", "mesa", "merquer", "aloha", "pipe", "untrack", "megaple", "", "trick", "alaska"];

fn word(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

#[test]
fn clock_against_the_running_game() {
    let (Ok(path), Ok(disc)) = (std::env::var("TRICKY_CLOCK"), std::env::var("TRICKY_DISC")) else { return };
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(' ').collect();
    let object: usize = head[2].parse().unwrap();
    let course = match std::env::var("TRICKY_COURSE") {
        Ok(c) => c,
        Err(_) => COURSES[head.iter().position(|w| *w == "course").map(|k| head[k + 1].parse::<usize>().unwrap()).expect("course in the header, or TRICKY_COURSE")].to_string(),
    };
    let n: usize = head[head.iter().position(|w| *w == "riders").unwrap() + 1].parse().unwrap();
    // tick, controller, node time, riders' positions, whether all the riders' tick counts are the frame's end
    let parsed: Vec<(u32, u32, u32, Vec<[f32; 4]>, Option<Vec<i64>>)> = lines
        .map(|l| {
            let w: Vec<&str> = l.split(' ').collect();
            let pos = w[7..7 + n].iter().map(|p| {
                let v: Vec<f32> = p.split(',').map(|x| f32::from_bits(word(x))).collect();
                [v[0], v[1], v[2], 1.0]
            });
            let tick: u32 = w[0].parse().unwrap();
            let counts = w.get(7 + n).map(|c| c.split(',').map(|x| x.parse::<i64>().unwrap() - tick as i64).collect());
            (tick, word(w[1]), word(w[4]), pos.collect(), counts)
        })
        .collect();
    // the usual count − tick (a frame's end), when recorded
    let usual = {
        let mut seen: HashMap<i64, usize> = HashMap::new();
        for c in parsed.iter().filter_map(|r| r.4.as_ref()) {
            *seen.entry(c[0]).or_default() += 1;
        }
        seen.into_iter().max_by_key(|x| x.1).map(|x| x.0)
    };
    let rows: Vec<(u32, u32, u32, Vec<[f32; 4]>)> = parsed
        .into_iter()
        .filter(|r| match (&r.4, usual) {
            (Some(c), Some(u)) => c.iter().all(|x| *x == u),
            _ => true,
        })
        .map(|r| (r.0, r.1, r.2, r.3))
        .collect();
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).unwrap());
    files.mount(&format!("data/models/{course}.big")).unwrap();
    let mut get = |ext: &str| files.load(&format!("|data/models/{course}.{ext}")).unwrap().unwrap();
    let (pb, lb, sb) = (get("pbd"), get("ltg"), get("ssf"));
    let cg = tricky_game::course_ground::CourseGround::new(&pb, &lb, &sb).unwrap();
    let pbd = tricky_data::pbd::Pbd::parse(&pb).unwrap();
    let ssf = tricky_data::ssf::Ssf::parse(&sb, pbd.instances.len() as u32).unwrap();
    let samples: Vec<(u32, Vec<[f32; 4]>)> = rows.iter().map(|r| (r.0, r.3.clone())).collect();
    let clocks = cg.with_world(|w| course_clock::clocks(&pbd, &ssf, w, &samples, |a, b| add_by(a, b, Round::Guard)));
    let (mut with, mut same, mut off) = (0, 0, Vec::new());
    for (k, (tick, ctrl, time, _)) in rows.iter().enumerate().skip(1) {
        let ours = clocks.get(&k).and_then(|l| l.iter().find(|x| x.0 == object)).map(|x| x.1);
        let game = (*ctrl != 0).then_some(f32::from_bits(*time));
        with += game.is_some() as usize;
        if ours.map(f32::to_bits) == game.map(f32::to_bits) {
            same += 1;
        } else if off.len() < 10 {
            off.push(format!("tick {tick}: ours {ours:?}, the game's {game:?}"));
        }
    }
    for o in &off {
        eprintln!("  {o}");
    }
    eprintln!("{path}: object {object} on {course}, {} ticks at a frame's end ({with} with its node): the same {same}", rows.len() - 1);
    assert!(with > 0);
    assert_eq!(same, rows.len() - 1);
}
