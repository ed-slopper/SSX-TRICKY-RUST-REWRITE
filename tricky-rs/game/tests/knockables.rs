//! The course's knockables (row E8c3; needs the course files in TRICKY_PBD_DIR, TRICKY_LTG_DIR, TRICKY_SSF_DIR;
//! skipped without them): a rider's contact with an object whose event-1 script spawns a roller
//! (`TriggerScript_Op0_SpawnController` 0x13c5d0, sub-type 0) makes it a `cRollerNode` that has left the casts (flags
//! `& ~0x22 | 0x45`), pushed off the contact, and the world's frames then move it; a second contact on the moving
//! roller starts nothing new.

use tricky_game::course_ground::CourseGround;
use tricky_game::knockables::{record_context, Rider};
use tricky_game::walls::Contact;

#[test]
fn a_contact_knocks_the_object_away() {
    let (Ok(pdir), Ok(ldir), Ok(sdir)) = (std::env::var("TRICKY_PBD_DIR"), std::env::var("TRICKY_LTG_DIR"), std::env::var("TRICKY_SSF_DIR")) else { return };
    let mut knocked = 0;
    for c in ["alaska", "aloha", "elysium", "gari", "megaple", "merquer", "mesa", "pipe", "snow", "trick", "untrack"] {
        let read = |d: &str, e: &str| std::fs::read(format!("{d}/{c}.{e}")).unwrap();
        let cg = CourseGround::new(&read(&pdir, "pbd"), &read(&ldir, "ltg"), &read(&sdir, "ssf")).expect(c);
        // objects whose event-1 script starts with a roller's spawn (op 0, sub-type 0) and that have a sphere tree
        let objs: Vec<(usize, usize, [f32; 4])> = cg.with_world(|w| {
            w.objects
                .iter()
                .enumerate()
                .filter(|(i, o)| {
                    let s = cg.ssf.event_script(*i, 1);
                    o.flags & 0x20 != 0
                        && w.trees.get(o.group as usize).is_some_and(|t| !t.is_empty())
                        && s >= 0
                        && cg.ssf.effects.get(s as usize).is_some_and(|ops| ops.iter().any(|op| op.op == 0 && op.args.first() == Some(&0)))
                })
                .map(|(i, o)| (i, o.group as usize, o.rows[3]))
                .take(3)
                .collect()
        });
        for (i, group, at) in objs {
            let rider = Rider { id: 1, human: true, vel: [1500.0, 0.0, 0.0, 0.0] };
            let contact = Contact { pos: at, dir: [1.0, 0.0, 0.0, 0.0], normal: [-1.0, 0.0, 0.0, 0.0], speed_in: 1500.0 };
            let record = record_context(at, [-1.0, 0.0, 0.0, 0.0], Some((group, 0)));
            let mut k = cg.knock.lock().unwrap();
            k.contact(&cg, i as u32, rider, &contact, record);
            let Some((p0, _)) = k.frame(i as u32) else { continue };
            assert_eq!(cg.object_flags(i) & 0x22, 0, "{c} object {i} left the casts");
            assert_eq!(cg.object_flags(i) & 0x45, 0x45, "{c} object {i}");
            let n = k.rollers.len();
            k.contact(&cg, i as u32, rider, &contact, record);
            assert_eq!(k.rollers.len(), n, "{c} object {i}: a moving roller is left alone");
            k.tick(&cg, false, &|_| false, 0.5);
            let (p1, q1) = k.frame(i as u32).unwrap();
            assert!(p1.iter().chain(&q1).all(|v| v.is_finite()), "{c} object {i}");
            let moved = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
            assert!(moved > 10.0, "{c} object {i} moved {moved} cm");
            knocked += 1;
        }
    }
    println!("{knocked} objects knocked");
    assert!(knocked > 0);
}
