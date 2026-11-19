//! A race recorded over PINE (`tools/pine.py record`), as `tests/replay.rs` and `tests/ground_motion.rs` read it.
#![allow(dead_code)]

pub struct Tick {
    pub tick: u32,
    pub riders: Vec<Vec<u8>>,
    /// the windows `pine.py record ... <off>:<len>` adds (header `extra <off> <len>`): (offset, per rider)
    pub extra: Vec<(i64, Vec<Vec<u8>>)>,
    /// the windows through pointers (`<off>><off>...:<len>`, header `extra 5ae0>610 10`): (the offsets, per rider)
    pub chased: Vec<(String, Vec<Vec<u8>>)>,
    /// the first controller's libpad record (header `pad P`, 20 bytes: status, mode, the buttons (0 = pressed), the
    /// right stick x, y, the left stick x, y, the pressures), when the recorder found it
    pub pad: Option<Vec<u8>>,
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
    let windows: Vec<&str> = head.iter().enumerate().filter(|(_, w)| **w == "extra").map(|(k, _)| head[k + 1]).collect();
    let hex = |h: &str| (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
    let ticks = lines
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let riders = w[2..2 + n].iter().map(|h| hex(h)).collect();
            let blocks = |k: usize| -> Vec<Vec<u8>> {
                let from = 2 + n + pad + k * n;
                w[from..from + n].iter().map(|h| hex(h)).collect()
            };
            let mut extra = Vec::new();
            let mut chased = Vec::new();
            for (k, spec) in windows.iter().enumerate() {
                // (and the walked lists, `L700`: node address + 0x10 bytes each)
                // (and `hookpad`, the pad word hook's 8 bytes: the pad word and the speed-sample count, F4b3k)
                if spec.contains('>') || spec.starts_with('L') || i64::from_str_radix(spec, 16).is_err() {
                    chased.push((spec.to_string(), blocks(k)));
                } else {
                    extra.push((i64::from_str_radix(spec, 16).unwrap(), blocks(k)));
                }
            }
            let pad = (pad == 1).then(|| hex(w[2 + n]));
            Tick { tick: w[1].parse().unwrap(), riders, extra, chased, pad }
        })
        .collect::<Vec<_>>();
    assert!(ticks.iter().all(|t| t.riders.iter().all(|r| r.len() == size)));
    (n, ticks)
}

pub fn u32_le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

/// The recorder read again every sample whose rider body looked the last frame's (`pine.py record`, header `posed 1`,
/// F4h5d)
pub fn posed(path: &str) -> bool {
    use std::io::BufRead;
    let mut head = String::new();
    std::io::BufReader::new(std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"))).read_line(&mut head).unwrap();
    head.split_whitespace().collect::<Vec<_>>().windows(2).any(|w| w == ["posed", "1"])
}

/// The course id the header names (`course N`, recordings since F4h5k)
pub fn course(path: &str) -> Option<u32> {
    use std::io::BufRead;
    let mut head = String::new();
    std::io::BufReader::new(std::fs::File::open(path).ok()?).read_line(&mut head).ok()?;
    let w: Vec<&str> = head.split_whitespace().collect();
    w.windows(2).find(|w| w[0] == "course").and_then(|w| w[1].parse().ok())
}

/// The riders' addresses, from the header (`race X riders a b c ... bytes N`)
pub fn rider_addrs(path: &str) -> Vec<u32> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let head: Vec<&str> = text.lines().next().expect("header").split_whitespace().collect();
    let at = head.iter().position(|w| *w == "bytes").unwrap();
    head[3..at].iter().map(|h| u32::from_str_radix(h, 16).unwrap()).collect()
}

/// Rider `me`'s pair list as `Boarder_RiderCollisions` 0x123fc8 walks it (the `L700` window: each node's address and
/// its 0x10 bytes, the two list heads at +0 and +4): the other riders in the list's order (other objects left out).
/// None without the window.
pub fn pair_list(t: &Tick, addrs: &[u32], me: usize) -> Option<Vec<usize>> {
    let (_, w) = t.chased.iter().find(|(s, _)| s == "L700")?;
    let b = &w[me];
    let head = addrs[me] + 0x700;
    let mut list = Vec::new();
    for k in (0..b.len() / 0x14).map(|k| k * 0x14) {
        if b[k..k + 4] == [0; 4] {
            break;
        }
        let key = |o: usize| u32::from_le_bytes(b[k + 4 + o..k + 8 + o].try_into().unwrap());
        let other = if key(4) == head { key(0) } else { key(4) };
        if let Some(j) = addrs.iter().position(|&a| a + 0x700 == other) {
            list.push(j);
        }
    }
    Some(list)
}

/// A knocked object as `pine.py record ... rollers` keeps it (F4b3k3): the `cRollerNode`'s address, its instance
/// (+0x28; the course's index is (instance − [`instance_base`]) / 0x100, `Course_GetInstance` 0x254a28), its
/// bytes from +0x30 (`body[k]` is node + 0x30 + k: the rigid body at +0x30, mass +0xcc, timers +0xe4..+0xe8) and its
/// box (+0x2a0 min, +0x2b0 max).
#[derive(Clone, Debug)]
pub struct RecRoller {
    pub node: u32,
    pub instance: u32,
    pub body: Vec<u8>,
    pub min: [f32; 4],
    pub max: [f32; 4],
}

/// Rider `i`'s rollers among its pairs at sample `t` (window `rollers`: a count byte, then 0x128 bytes each). None
/// without the window.
pub fn rollers(t: &Tick, i: usize) -> Option<Vec<RecRoller>> {
    let (_, w) = t.chased.iter().find(|(s, _)| s == "rollers")?;
    Some(parse_rollers(&w[i]))
}

pub fn parse_rollers(b: &[u8]) -> Vec<RecRoller> {
    let n = b.first().copied().unwrap_or(0) as usize;
    let v4 = |x: &[u8]| std::array::from_fn(|k| f32::from_le_bytes(x[4 * k..4 * k + 4].try_into().unwrap()));
    (0..n)
        .map(|k| {
            let e = &b[1 + k * 0x128..1 + (k + 1) * 0x128];
            RecRoller { node: u32_le(e, 0), instance: u32_le(e, 4), body: e[8..0x108].to_vec(), min: v4(&e[0x108..0x118]), max: v4(&e[0x118..0x128]) }
        })
        .collect()
}

/// The course's instance base the header keeps with `rollers` (`instances <base>`)
pub fn instance_base(path: &str) -> Option<u32> {
    use std::io::BufRead;
    let mut head = String::new();
    std::io::BufReader::new(std::fs::File::open(path).ok()?).read_line(&mut head).ok()?;
    let w: Vec<&str> = head.split_whitespace().collect();
    w.windows(2).find(|w| w[0] == "instances").and_then(|w| u32::from_str_radix(w[1], 16).ok())
}

/// The player's pad word for control state `state` (board row F4b3d): `cPlayer::GetControls` 0x151250
/// (`controls::get_controls`) on the pad device (`PadDevice::read`, `cPS2Device` vf10 0x17b278) fed the recorded
/// libpad records `prev` then `now` (buttons 0 = pressed; the sticks when the mode is analog, 0x7_), through the rider's
/// button map (`BTNMAP<stats+0x6c>`, the grab combos on the trick slots of the character stats+0x5c and the board kind
/// stats+0x44). None for a computer rider (stats+0x6c = −1: no button map). The trick slot the random grab keeps
/// (player+0x868) is not recorded: taken as none (0x1a), and the easy trick it draws as 0.
pub fn player_pad(stat: &[u8], prev: Option<&[u8]>, now: &[u8], state: u32) -> Option<u32> {
    use tricky_game::controls::{get_controls, ButtonMap, ControlsIn, Mapper, PadDevice};
    let n = u32_le(stat, 0x6c);
    if n > 1 {
        return None;
    }
    let slots = tricky_data::trick_slots::TRICK_SLOTS[u32_le(stat, 0x5c) as usize][(u32_le(stat, 0x44) as usize).min(2)];
    let map = ButtonMap::btnmap(n, std::array::from_fn(|k| slots[k][0]));
    let mut pad = PadDevice::default();
    for r in prev.into_iter().chain([now]) {
        let analog = (r[1] >> 4 == 7).then(|| [r[6], r[7], r[4], r[5]]);
        pad.read(!u16::from_le_bytes([r[2], r[3]]), analog);
    }
    let (mut trick, mut slope) = (0x1a, false);
    Some(get_controls(ControlsIn { state, paused: false }, &mut Mapper { map: &map, pad: &pad }, &mut trick, &mut || 0, &mut slope))
}
