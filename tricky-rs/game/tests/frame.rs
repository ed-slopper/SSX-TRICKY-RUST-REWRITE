//! The board's frame (rider+0x4aa0..+0x4ae0) against a recorded race (board row F4b3t): `Boarder_UpdatePose`
//! 0x129628 as `pose::board_frame` and `pose::frame_where` make it from each sample, against the frame the game left
//! in the same sample (the recorder reads at the frame's end, after `cPostAI::vf1` 0x114730 posed the riders).
//! Needs TRICKY_RECORDING with the windows `4aa0:40` (the frame), `8c0:20` (the pivot bone, +0x8c8, for +0x44d8 = 0),
//! `44d8:4` and `46c0:10` (the layer-0 clip), TRICKY_STATS, and TRICKY_ELF (the clip table).

mod recording;
use recording::{f32_at, load, u32_le, vec4};
use tricky_game::animctl;
use tricky_game::pose::{board_frame, frame_where, PoseIn};

type V4 = [f32; 4];

fn pose_in(r: &[u8], st: &[u8], pivot_z: f32) -> PoseIn {
    PoseIn {
        quat: vec4(r, 0x170),
        steer: f32_at(r, 0x214),
        motion: u32_le(r, 0x424),
        class: u32_le(r, 0x420),
        pos: vec4(r, 0x140),
        up: vec4(r, 0x2c0),
        height: f32_at(r, 0x308),
        side: vec4(r, 0x330),
        lean: f32_at(r, 0x244),
        lean_scale: f32_at(st, 0x60),
        spin: f32_at(r, 0x1d0),
        flip: f32_at(r, 0x1d4),
        pivot_z,
    }
}

#[test]
fn board_frame_against_the_recording() {
    let (Ok(path), Ok(stats), Ok(elf)) = (std::env::var("TRICKY_RECORDING"), std::env::var("TRICKY_STATS"), std::env::var("TRICKY_ELF")) else { return };
    let stats: Vec<Vec<u8>> = std::fs::read_to_string(&stats).unwrap().lines()
        .map(|l| { let h = l.split_whitespace().nth(2).unwrap(); (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect() })
        .collect();
    let elf = std::fs::read(elf).unwrap();
    // the clip table's category word (0x325fa8 + 0x1c × clip), from the ELF (file offset = address − 0xff000)
    let category = |clip: u32| u32_le(&elf, (animctl::CLIP_TABLE - 0xff000) as usize + 0x1c * clip as usize);
    let (n, ticks) = load(&path);
    let win = |o: i64| ticks[0].extra.iter().position(|(x, _)| *x == o);
    let (Some(kf), Some(kp), Some(ki), Some(ka)) = (win(0x4aa0), win(0x8c0), win(0x44d8), win(0x46c0)) else {
        eprintln!("{path}: no 4aa0, 8c0, 44d8 and 46c0 windows: skipping");
        return;
    };
    // per pivot source (this sample's, the sample before's): [rows right, where right, all right]; and the riders
    // left out (pivot index not 0, stats+0x78 set)
    let mut count = [[0usize; 3]; 2];
    let (mut total, mut other) = (0, 0);
    let mut misses = Vec::new();
    // the pivot bone by motion: per axis (sum, least, most), and the samples
    let mut piv = [[(0.0f64, f64::MAX, f64::MIN); 3]; 6];
    let mut piv_n = [0usize; 6];
    for (ti, t) in ticks.iter().enumerate() {
        let prev = (ti > 0 && ticks[ti - 1].tick + 1 == t.tick).then(|| &ticks[ti - 1]);
        for i in 0..n {
            let r = &t.riders[i];
            let st = &stats[i];
            if u32_le(&t.extra[ki].1[i], 0) != 0 || u32_le(st, 0x78) != 0 {
                other += 1;
                continue;
            }
            let game: Vec<u32> = (0..16).map(|k| u32_le(&t.extra[kf].1[i], 4 * k)).collect();
            total += 1;
            let clip = u32_le(&t.extra[ka].1[i], 8);
            {
                let pb = &t.extra[kp].1[i];
                let m = u32_le(r, 0x424) as usize;
                if m < 6 {
                    for c in 0..3 {
                        let v = f32_at(pb, 8 + 4 * c) as f64;
                        piv[m][c].0 += v;
                        piv[m][c].1 = piv[m][c].1.min(v);
                        piv[m][c].2 = piv[m][c].2.max(v);
                    }
                    piv_n[m] += 1;
                }
            }
            let skin_child = (u32_le(r, 0x428) == 1 && clip != 0x1fc) || clip == 0x291 || clip == 0x2ab || (clip != animctl::NONE && category(clip) == 0x10);
            for (s, src) in [Some(t), prev].into_iter().enumerate() {
                let Some(src) = src else { continue };
                let pb = &src.extra[kp].1[i];
                let pivot = [f32_at(pb, 8), f32_at(pb, 0xc), f32_at(pb, 0x10)];
                let mut f: [V4; 4] = board_frame(&pose_in(r, st, pivot[2]));
                let rows_ok = (0..12).all(|k| f[k / 4][k % 4].to_bits() == game[k]);
                f[3] = frame_where(f, pivot, skin_child, u32_le(r, 0x424), vec4(r, 0x2c0), f32_at(r, 0x1c0), f32_at(st, 0x60));
                let where_ok = (0..4).all(|k| f[3][k].to_bits() == game[12 + k]);
                count[s][0] += rows_ok as usize;
                count[s][1] += where_ok as usize;
                count[s][2] += (rows_ok && where_ok) as usize;
                if s == 0 && !where_ok && misses.len() < 12 {
                    let g: Vec<f32> = game[12..].iter().map(|&b| f32::from_bits(b)).collect();
                    let bad: Vec<String> = (0..12).filter(|&k| f[k / 4][k % 4].to_bits() != game[k]).map(|k| format!("{k}:{:08x}/{:08x}", f[k / 4][k % 4].to_bits(), game[k])).collect();
                    misses.push(format!("tick {} rider {i}: rows {rows_ok} {bad:?} where ours {:?} game {g:?} (motion {} state {:x} clip {clip:x} spin {} flip {})", t.tick, f[3], u32_le(r, 0x424), u32_le(r, 0x428), f32_at(r, 0x1d0), f32_at(r, 0x1d4)));
                }
            }
        }
    }
    eprintln!("{path}: board frames: {total} compared ({other} left out: pivot bone not 0 or stats+0x78 set)");
    for (s, name) in ["this sample's pivot", "the sample before's pivot"].iter().enumerate() {
        eprintln!("  {name}: rows {}, where {}, whole {}", count[s][0], count[s][1], count[s][2]);
    }
    for (m, (p, k)) in piv.iter().zip(piv_n).enumerate().filter(|(_, (_, k))| *k > 0) {
        let line: Vec<String> = p.iter().map(|(sum, lo, hi)| format!("{:.2} ({lo:.2}..{hi:.2})", sum / k as f64)).collect();
        eprintln!("  the pivot bone in motion {m} ({k} samples): {}", line.join(", "));
    }
    for m in misses {
        eprintln!("  miss {m}");
    }
}
