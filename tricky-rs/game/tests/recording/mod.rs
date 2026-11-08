//! A race recorded over PINE (`tools/pine.py record`), as `tests/replay.rs` and `tests/ground_motion.rs` read it.
#![allow(dead_code)]

pub struct Tick {
    pub tick: u32,
    pub riders: Vec<Vec<u8>>,
    /// the windows `pine.py record ... <off>:<len>` adds (header `extra <off> <len>`): (offset, per rider)
    pub extra: Vec<(i64, Vec<Vec<u8>>)>,
}

pub fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

pub fn vec4(b: &[u8], off: usize) -> [f32; 4] {
    [f32_at(b, off), f32_at(b, off + 4), f32_at(b, off + 8), f32_at(b, off + 12)]
}

pub fn load(path: &str) -> (usize, Vec<Tick>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("header").split_whitespace().collect();
    // race X riders a b c ... bytes N [pad P]
    let at = head.iter().position(|w| *w == "bytes").unwrap();
    let (n, size) = (at - 3, usize::from_str_radix(head[at + 1], 16).unwrap());
    // the pad's block follows the riders when the recorder found it, then the extra windows, rider by rider
    let pad = head.iter().position(|w| *w == "pad").is_some_and(|k| head[k + 1] != "0") as usize;
    // (an offset may be negative: `-170` is the spin object, before the rider)
    let windows: Vec<i64> = head.iter().enumerate().filter(|(_, w)| **w == "extra").map(|(k, _)| i64::from_str_radix(head[k + 1], 16).unwrap()).collect();
    let hex = |h: &str| (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
    let ticks = lines
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let riders = w[2..2 + n].iter().map(|h| hex(h)).collect();
            let extra = windows.iter().enumerate().map(|(k, off)| {
                let from = 2 + n + pad + k * n;
                (*off, w[from..from + n].iter().map(|h| hex(h)).collect())
            }).collect();
            Tick { tick: w[1].parse().unwrap(), riders, extra }
        })
        .collect::<Vec<_>>();
    assert!(ticks.iter().all(|t| t.riders.iter().all(|r| r.len() == size)));
    (n, ticks)
}

pub fn u32_le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
