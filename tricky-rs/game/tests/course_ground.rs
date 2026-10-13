//! The course's collision with its keyframed objects posed (row E7c3b; needs the course files in TRICKY_PBD_DIR,
//! TRICKY_LTG_DIR, TRICKY_SSF_DIR; skipped without them): a cast down through an animated object hits it as the
//! course loaded it (one whose every mesh moves with an animated part), the same once [`CourseGround::pose`] gives its animated parts their rest matrices, and no
//! longer once they are lifted 1 km out of the way.

use tricky_game::course_ground::CourseGround;
use tricky_game::worldline::WorldLine;

type V4 = [f32; 4];

#[test]
fn posed_objects_move_in_the_casts() {
    let (Ok(pdir), Ok(ldir), Ok(sdir)) = (std::env::var("TRICKY_PBD_DIR"), std::env::var("TRICKY_LTG_DIR"), std::env::var("TRICKY_SSF_DIR")) else { return };
    let mut checked = 0;
    for c in ["alaska", "aloha", "elysium", "gari", "megaple", "merquer", "mesa", "pipe", "snow", "trick", "untrack"] {
        let read = |d: &str, e: &str| std::fs::read(format!("{d}/{c}.{e}")).unwrap();
        let cg = CourseGround::new(&read(&pdir, "pbd"), &read(&ldir, "ltg"), &read(&sdir, "ssf")).expect(c);
        // objects with an animated part: (index, centre, top, bottom, the animated parts' rest matrices)
        let objs: Vec<(usize, [f32; 2], f32, f32, Vec<[V4; 4]>)> = cg.with_world(|w| {
            w.objects
                .iter()
                .enumerate()
                .filter(|(_, o)| o.flags & 0x20 != 0 && o.surface != -1 && (1..=3).contains(&o.mode))
                .filter_map(|(i, o)| {
                    let rest: Vec<[V4; 4]> = w.models[o.model].iter().filter(|p| p.animated).map(|p| p.matrix.unwrap_or([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])).collect();
                    // only objects all of whose meshes move with an animated part
                    let parts = &w.models[o.model];
                    let mut moves = vec![false; parts.len()];
                    for (k, p) in parts.iter().enumerate() {
                        moves[k] = p.animated || usize::try_from(p.parent).ok().filter(|&j| j < k).is_some_and(|j| moves[j]);
                    }
                    let all = parts.iter().zip(&moves).all(|(p, m)| p.mesh.is_none() || *m);
                    (all && !rest.is_empty()).then(|| (i, [(o.min[0] + o.max[0]) * 0.5, (o.min[1] + o.max[1]) * 0.5], o.max[2] + 100.0, o.min[2] - 100.0, rest))
                })
                .collect()
        });
        for (i, [x, y], top, bottom, rest) in objs {
            let cast = || {
                let mut q = WorldLine::new(0.0, [x, y, top, 1.0], [x, y, bottom, 1.0], 2);
                let (t, h) = cg.ray_cast(&mut q, false, None, &|_| false);
                (t, h.and_then(|h| h.object))
            };
            let before = cast();
            if before.1 != Some(i) {
                continue;
            }
            cg.pose(i as u32, rest.clone());
            assert_eq!(cast(), before, "{c} object {i} in its rest pose");
            let lifted = rest.iter().map(|m| [m[0], m[1], m[2], [m[3][0], m[3][1], m[3][2] + 100_000.0, 1.0]]).collect();
            cg.pose(i as u32, lifted);
            assert_ne!(cast().1, Some(i), "{c} object {i} lifted");
            cg.pose(i as u32, rest);
            checked += 1;
        }
    }
    println!("{checked} keyframed objects moved");
    assert!(checked > 0 || pdir.is_empty());
}
