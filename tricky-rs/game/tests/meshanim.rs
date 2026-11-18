//! A keyframed part's frame (`tricky_game::meshanim`, row F4h5l) against the game's code in the function runner
//! (TRICKY_ELF, TRICKY_DISC: every course's `.pbd`, `.ltg`, `.ssf` from the player's disc; skipped without them):
//! the course loaded by the game's loaders (`runner_course`), then for every animated part of every object's model a
//! `cMeshAnimFrame` set up as `cAnimObjectNode::cAnimObjectNode` 0x198810 does (`AnimFrame_Clear` 0x1cb150 and
//! `AnimFrame_SetKey` 0x1cb0d8 at the clip's start) and sampled by `cMeshAnimFrame::vf1` 0x1cb498 over a run of
//! times (a frame's steps, then jumps, so the segment it keeps from the last call is tried both ways): the matrix
//! (+0x90..+0xcc) and the channels (+0x4c..+0x88) bit for bit.

mod runner_course;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::meshanim::{key_channels, mesh_frame};

const COURSES: [&str; 11] = ["alaska", "aloha", "elysium", "gari", "megaple", "merquer", "mesa", "pipe", "snow", "trick", "untrack"];

fn files(disc: &str, name: &str) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(disc).unwrap());
    files.mount(&format!("data/models/{name}.big")).unwrap();
    let mut get = |ext: &str| files.load(&format!("|data/models/{name}.{ext}")).unwrap().unwrap();
    (get("pbd"), get("ltg"), get("ssf"))
}

#[test]
fn frames_against_the_original() {
    let Ok(disc) = std::env::var("TRICKY_DISC") else { return };
    if std::env::var("TRICKY_ELF").is_err() {
        return;
    }
    let (mut parts, mut samples) = (0, 0);
    for course in COURSES {
        let (pb, lb, sb) = files(&disc, course);
        let mut r = Runner::from_env().unwrap();
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        runner_course::load(&mut r, &pb, &lb, &sb);
        // EAMem_Set 0x2cc808 (a byte fill)
        r.stub(0x2cc808, |c, m| {
            let (d, v, n) = (c.gpr(4) as u32, c.gpr(5) as u8, c.gpr(6) as u32);
            for k in 0..n {
                m.write_u8(d + k, v);
            }
        });
        for a in [0x1cb150, 0x1cb0d8, 0x1cb498, 0x1cb300, 0x1cb770, 0x250c10] {
            r.allow(a);
        }
        let pbd = tricky_data::pbd::Pbd::parse(&pb).unwrap();
        let frame = r.mem.alloc(0xd0);
        for m in &pbd.models {
            for n in &m.nodes {
                let Some(an) = &n.animation else { continue };
                let anim = tricky_data::level::node_animation(&pb, an).unwrap();
                // the part's animation as the loader resolved it (the hierarchy entry's +0x10)
                let ptr = r.mem.read_u32(0x80_0000 + n.offset + 0x10);
                for o in (0..0xd0).step_by(4) {
                    r.mem.write_u32(frame + o, 0xdead_c0ed);
                }
                r.mem.write_u32(frame + 0x8c, 0x391af8);
                r.cpu.set_gpr(4, frame as u64);
                r.call(0x1cb150).unwrap();
                let start = anim.entries.iter().filter_map(|e| e.segs.first()).map(|s| s.t0).fold(0.0f32, f32::min);
                let end = anim.entries.iter().filter_map(|e| e.segs.last()).map(|s| s.t1).fold(0.0f32, f32::max);
                r.cpu.set_f(12, start);
                r.cpu.set_gpr(4, frame as u64);
                r.cpu.set_gpr(5, ptr as u64);
                r.call(0x1cb0d8).unwrap();
                let mut ch = key_channels(&anim);
                let _ = mesh_frame(&anim, start, &mut ch);
                // a frame's steps over the clip and past both ends, then jumps
                let mut times: Vec<f32> = (-10..((end - start) * 60.0) as i32 + 10).map(|k| start + k as f32 / 60.0).collect();
                times.extend((0..40).map(|k| start + (end - start) * ((k * 37 % 41) as f32 / 40.0)));
                for t in times {
                    r.cpu.set_f(12, t);
                    r.cpu.set_gpr(4, frame as u64);
                    r.call(0x1cb498).unwrap();
                    let ours = mesh_frame(&anim, t, &mut ch);
                    let game: Vec<u32> = (0..16).map(|k| r.mem.read_u32(frame + 0x90 + 4 * k)).collect();
                    let mine: Vec<u32> = ours.iter().flatten().map(|v| v.to_bits()).collect();
                    assert_eq!(mine, game, "{course} part at {:#x}, t {t}", n.offset);
                    let game_ch: Vec<u32> = (0..16).map(|k| r.mem.read_u32(frame + 0x4c + 4 * k)).collect();
                    let mine_ch: Vec<u32> = ch.iter().map(|v| v.to_bits()).collect();
                    assert_eq!(mine_ch, game_ch, "{course} part at {:#x}, t {t}: channels", n.offset);
                    samples += 1;
                }
                parts += 1;
            }
        }
    }
    eprintln!("{parts} animated parts, {samples} frames the same bit for bit");
    assert!(parts > 0);
}
