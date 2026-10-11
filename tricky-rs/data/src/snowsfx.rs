//! The board's sound programs as the game compiles them from `DATA/CONFIG/SNOW.INF` (board row E5b2):
//! `Sfx_LoadSurfaceConfig` 0x210c38 and `Sfx_LoadBoardConfig` 0x21a7c8, on [`crate::ini::IniReader`]. Each surface
//! (PACK, POWDER, LOOSE, ICE, METAL, WOOD, RAIL, ROCK, GLASS, CHUTE) has a program for each of three modes (GLIDE,
//! AIGLIDE, CARVE): bytes of a small accumulator-and-stack language that turns the board's load, slip, dig, lean and
//! bend into a volume and a pitch bend. Checked against the game's own compiler in the function runner
//! (`tricky-game/tests/ini.rs`).

use crate::ini::IniReader;

/// The surfaces in the order of the game's table (`cAudioSys` +0x745c + i·0x50).
pub const SURFACES: [&str; 10] = ["PACK", "POWDER", "LOOSE", "ICE", "METAL", "WOOD", "RAIL", "ROCK", "GLASS", "CHUTE"];
/// The modes (+0x320 apart).
pub const MODES: [&str; 3] = ["GLIDE", "AIGLIDE", "CARVE"];

/// One instruction as the compiler writes it (the byte, then its operands, little-endian words).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// 0 then the input byte: 0 SLIP, 1 DIG, 2 LEAN, 3 BOARD, 4 BEND (after an unknown word the compiler writes the
    /// 0 alone, and the next instruction's byte is read as the input)
    Load(u8),
    /// 1 LOADCONST n
    LoadConst(i32),
    /// 2 MAP n
    Map(i32),
    /// 3 SQUARE, 4 SQRT, 5 LOG
    Square,
    Sqrt,
    Log,
    /// 6 ADD, 7 SUB, 8 MUL, 9 DIV with n
    Add(i32),
    Sub(i32),
    Mul(i32),
    Div(i32),
    /// 10 NEG, 11 ABS
    Neg,
    Abs,
    /// 12 BOUND lo hi
    Bound(i32, i32),
    /// 13 PUSH, 14 POP, 15 SADD, 16 SSUB, 17 SMUL, 18 SDIV
    Push,
    Pop,
    SAdd,
    SSub,
    SMul,
    SDiv,
    /// 19 ASSIGNVOL, 20 ASSIGNBEND
    AssignVol,
    AssignBend,
}

fn eq_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

/// `Sfx_LoadBoardConfig` 0x21a7c8: the program of `surface` × `mode`, compiled to bytes: from the top of the file,
/// on to the surface's section and then on to the mode's after it (so a run of empty surface headers shares the
/// modes that follow), the instructions up to the next section, then 0x7f. Unknown words are skipped; an operand
/// is `sscanf` "%d" of the next word (0 where it has none: the game's stack word, here 0).
pub fn compile(r: &mut IniReader, surface: &[u8], mode: &[u8]) -> Vec<u8> {
    r.rewind();
    r.find_section(surface);
    r.find_section(mode);
    let mut out = Vec::new();
    let int = |r: &mut IniReader, out: &mut Vec<u8>| {
        let v = r.parse_int().unwrap_or(0);
        out.extend_from_slice(&v.to_le_bytes());
    };
    while !r.at_section_end() {
        let tok = r.read_token();
        let is = |k: &str| eq_ignore_case(k.as_bytes(), &tok);
        if is("LOAD") {
            out.push(0);
            let t = r.read_token();
            let which = [("SLIP", 0u8), ("DIG", 1), ("LEAN", 2), ("BOARD", 3), ("BEND", 4)].iter().find(|(k, _)| eq_ignore_case(k.as_bytes(), &t)).map(|(_, v)| *v);
            if let Some(v) = which {
                out.push(v);
            }
        } else if is("LOADCONST") {
            out.push(1);
            int(r, &mut out);
        } else if is("MAP") {
            out.push(2);
            int(r, &mut out);
        } else if let Some(b) = [("SQUARE", 3u8), ("SQRT", 4), ("LOG", 5)].iter().find(|(k, _)| is(k)).map(|(_, b)| *b) {
            out.push(b);
        } else if let Some(b) = [("ADD", 6u8), ("SUB", 7), ("MUL", 8), ("DIV", 9)].iter().find(|(k, _)| is(k)).map(|(_, b)| *b) {
            out.push(b);
            int(r, &mut out);
        } else if is("NEG") {
            out.push(10);
        } else if is("ABS") {
            out.push(11);
        } else if is("BOUND") {
            out.push(12);
            int(r, &mut out);
            int(r, &mut out);
        } else if let Some(b) = [("PUSH", 13u8), ("POP", 14), ("SADD", 15), ("SSUB", 16), ("SMUL", 17), ("SDIV", 18), ("ASSIGNVOL", 19), ("ASSIGNBEND", 20)].iter().find(|(k, _)| is(k)).map(|(_, b)| *b) {
            out.push(b);
        }
    }
    out.push(0x7f);
    out
}

/// `Sfx_LoadSurfaceConfig` 0x210c38: every surface × mode program (`[mode][surface]`).
pub fn compile_all(bytes: &[u8]) -> Vec<Vec<Vec<u8>>> {
    let mut r = IniReader::new(bytes);
    MODES.iter().map(|m| SURFACES.iter().map(|s| compile(&mut r, s.as_bytes(), m.as_bytes())).collect()).collect()
}

/// A compiled program read back as instructions (to 0x7f or the end).
pub fn decode(p: &[u8]) -> Vec<Op> {
    let mut out = Vec::new();
    let mut i = 0;
    let word = |p: &[u8], i: usize| p.get(i..i + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0);
    while i < p.len() && p[i] != 0x7f {
        let b = p[i];
        i += 1;
        let op = match b {
            0 => {
                let v = p.get(i).copied().unwrap_or(0x7f);
                i += 1;
                Op::Load(v)
            }
            1 | 2 | 6 | 7 | 8 | 9 => {
                let n = word(p, i);
                i += 4;
                match b {
                    1 => Op::LoadConst(n),
                    2 => Op::Map(n),
                    6 => Op::Add(n),
                    7 => Op::Sub(n),
                    8 => Op::Mul(n),
                    _ => Op::Div(n),
                }
            }
            12 => {
                let (lo, hi) = (word(p, i), word(p, i + 4));
                i += 8;
                Op::Bound(lo, hi)
            }
            3 => Op::Square,
            4 => Op::Sqrt,
            5 => Op::Log,
            10 => Op::Neg,
            11 => Op::Abs,
            13 => Op::Push,
            14 => Op::Pop,
            15 => Op::SAdd,
            16 => Op::SSub,
            17 => Op::SMul,
            18 => Op::SDiv,
            19 => Op::AssignVol,
            20 => Op::AssignBend,
            _ => continue,
        };
        out.push(op);
    }
    out
}
