//! The animation bank's loads (board row G5c2): which of the 0x1be `.afl` files (`data/char/<name>.afl`, the names at
//! 0x3258b0) the bank loads for a mode, loading them now or through the BIG file `data/char/anm.big`, binding a
//! loaded file, and indexing the clips by name into the key table. On the bytes at their addresses ([`Mem`]); the
//! file and BIG reads are calls out ([`LoadEnv`]). Checked in the function runner (`tests/animload.rs`).
//!
//! **A loaded `.afl`:** +2 the clip count (signed 16), +4 the offset of the curve offsets table, +8 the offset of the
//! curve data, then the clip headers from +0xc (0x24 bytes each; +0 the clip name's hash).

use crate::animdata::{self as d, FILES};
use crate::animseq::KEY_TABLE;
use crate::pf::Mem;

/// The file names (0x3258b0, 0x1be pointers) and the path format (0x379000, `|data/char/%s.afl`)
pub const NAMES: u32 = 0x3258b0;
pub const PATH_FORMAT: u32 = 0x379000;
/// The BIG file's path (0x321190)
pub const BIG_PATH: u32 = 0x321190;
/// The career block (its race roster from +0x27dc, 0x84 bytes a rider)
pub const CAREER: u32 = crate::career::CAREER_BASE;

/// The calls out
pub trait LoadEnv {
    /// `File_Open` 0x2c8018(path, 0x10): a handle
    fn open(&mut self, path: &[u8]) -> u32;
    /// 0x2c3190(handle): its size
    fn size(&mut self, h: u32) -> u32;
    /// 0x2c2f90(handle, dst): read it all into memory
    fn read(&mut self, m: &mut Mem, h: u32, dst: u32);
    /// 0x2ccfc0(handle)
    fn free(&mut self, h: u32);
    /// The BIG file object's virtual at +0x80(path, 0x10, 0, 0): an async request's handle
    fn big_request(&mut self, m: &mut Mem, big: u32, path: &[u8]) -> u32;
    /// Its virtual at +0x60(handle): whether the request is in
    fn big_ready(&mut self, m: &mut Mem, big: u32, h: u32) -> bool;
    /// 0x23c7c8(big, handle): the size; 0x23c818(big, handle, dst): the bytes into memory
    fn big_size(&mut self, m: &mut Mem, big: u32, h: u32) -> u32;
    fn big_read(&mut self, m: &mut Mem, big: u32, h: u32, dst: u32);
    /// Its virtual at +0x58(handle): the request done
    fn big_close(&mut self, m: &mut Mem, big: u32, h: u32);
    /// 0x23be98(big, the BIG's path, 0x10): opened
    fn big_open(&mut self, m: &mut Mem, big: u32);
    /// 0x23bea0(big)
    fn big_reset(&mut self, m: &mut Mem, big: u32);
}

/// A C string in memory
fn cstr(m: &Mem, mut p: u32) -> Vec<u8> {
    let mut v = Vec::new();
    loop {
        let c = m.r8(p);
        if c == 0 {
            return v;
        }
        v.push(c);
        p = p.wrapping_add(1);
    }
}

/// The file's path as `sprintf` makes it from the format (its one `%s` the file's name)
pub fn file_path(m: &Mem, idx: u32) -> Vec<u8> {
    let fmt = cstr(m, PATH_FORMAT);
    let name = cstr(m, m.r32(NAMES.wrapping_add(idx.wrapping_mul(4))));
    let mut out = Vec::new();
    let mut k = 0;
    while k < fmt.len() {
        if fmt[k] == b'%' && fmt.get(k + 1) == Some(&b's') {
            out.extend_from_slice(&name);
            k += 2;
        } else {
            out.push(fmt[k]);
            k += 1;
        }
    }
    out
}

/// `AnimFile_Bind` 0x158c10(file, data): the file's headers (data + 0xc), its curve offsets and data (data + its +4
/// and +8) and the data itself
pub fn bind(m: &mut Mem, f: u32, data: u32) {
    m.w32(f + 8, data + 0xc);
    m.w32(f + 0xc, data.wrapping_add(m.r32(data + 4)));
    m.w32(f + 0x10, data.wrapping_add(m.r32(data + 8)));
    m.w32(f + 4, data);
}

/// `AnimFileList_Load` 0x158908(file, index): unless loaded, the file opened by its path, a block of its size from
/// the bank's memory, read into it and bound; the handle freed
pub fn load_now(m: &mut Mem, env: &mut dyn LoadEnv, f: u32, idx: u32) {
    if m.r32(f + 4) != 0 {
        return;
    }
    let path = file_path(m, idx);
    let h = env.open(&path);
    let size = env.size(h);
    let p = d::mem_bank_alloc(m, m.r32(f), size);
    env.read(m, h, p);
    bind(m, f, p);
    env.free(h);
}

/// `AnimFileList_Load2` 0x1589b8(file, big, index): the file asked of the BIG file; the request's handle at +0x14
pub fn load_async(m: &mut Mem, env: &mut dyn LoadEnv, f: u32, big: u32, idx: u32) {
    let path = file_path(m, idx);
    let h = env.big_request(m, big, &path);
    m.w32(f + 0x14, h);
}

/// `AnimFile_PollLoad` 0x158a30(file, big): loaded (1); or, its request in, a block of its size from the bank's
/// memory, read, bound, the request closed and forgotten (1); else 0
pub fn poll_file(m: &mut Mem, env: &mut dyn LoadEnv, f: u32, big: u32) -> bool {
    if m.r32(f + 4) != 0 {
        return true;
    }
    if !env.big_ready(m, big, m.r32(f + 0x14)) {
        return false;
    }
    let size = env.big_size(m, big, m.r32(f + 0x14));
    let p = d::mem_bank_alloc(m, m.r32(f), size);
    env.big_read(m, big, m.r32(f + 0x14), p);
    bind(m, f, p);
    env.big_close(m, big, m.r32(f + 0x14));
    m.w32(f + 0x14, 0xffff_ffff);
    true
}

/// `AnimFile_IndexClipNames` 0x158b10(file, key): for each clip of a loaded file, its number written into the key's
/// second short, and its name hash (header +0) found among the clip names' hashes (the first match) and the key
/// table's word there set to the key (the file, the clip)
pub fn index_clip_names(m: &mut Mem, f: u32, key: u32) {
    let data = m.r32(f + 4);
    if data == 0 {
        return;
    }
    let mut c: i32 = 0;
    while c < m.r16(data + 2) as i16 as i32 {
        m.w16(key + 2, c as u16);
        let hash = m.r32(m.r32(f + 8).wrapping_add((c as u32).wrapping_mul(0x24)));
        if hash != 0 {
            if let Some(i) = (0..d::NCLIP).find(|&i| m.r32(d::HASHES + 4 * i) == hash) {
                let k = m.r16(key) as u32 | (m.r16(key + 2) as u32) << 16;
                m.w32(KEY_TABLE + 4 * i, k);
            }
        }
        c = (c + 1) as i16 as i32;
    }
}

/// [`index_clip_names`] with the key held here (the game's is on its stack)
fn index_file(m: &mut Mem, f: u32, file_no: u16) {
    let data = m.r32(f + 4);
    if data == 0 {
        return;
    }
    let mut c: i32 = 0;
    while c < m.r16(data + 2) as i16 as i32 {
        let hash = m.r32(m.r32(f + 8).wrapping_add((c as u32).wrapping_mul(0x24)));
        if hash != 0 {
            if let Some(i) = (0..d::NCLIP).find(|&i| m.r32(d::HASHES + 4 * i) == hash) {
                m.w32(KEY_TABLE + 4 * i, file_no as u32 | (c as u16 as u32) << 16);
            }
        }
        c = (c + 1) as i16 as i32;
    }
}

/// `AnimData_IndexAllClips` 0x1597d0: every file's clips indexed
pub fn index_all(m: &mut Mem, bank: u32) {
    for i in 0..d::NFILE {
        index_file(m, bank + FILES + 0x18 * i, i as u16);
    }
}

/// `AnimData_LoadFileList` 0x159738(index, now): the file loaded now; or, when not yet asked for (+0x18 + 4i) nor
/// loaded, asked of the BIG file and noted as asked
pub fn load_file(m: &mut Mem, env: &mut dyn LoadEnv, bank: u32, idx: u32, now: bool) {
    let f = bank.wrapping_add(FILES).wrapping_add(idx.wrapping_mul(0x18));
    if now {
        load_now(m, env, f, idx);
        return;
    }
    let asked = bank.wrapping_add(0x18).wrapping_add(idx.wrapping_mul(4));
    if m.r32(asked) != 0 || m.r32(f + 4) != 0 {
        return;
    }
    load_async(m, env, f, bank + 0x710, idx);
    m.w32(asked, 1);
}

/// `AnimData_PollLoad` 0x158fc0: nothing waiting (+0x14 0): done. Else each asked file polled (done ones no longer
/// asked); when all are in, the BIG file reset, nothing waiting, every clip indexed
pub fn poll(m: &mut Mem, env: &mut dyn LoadEnv, bank: u32) -> bool {
    if m.r32(bank + 0x14) == 0 {
        return true;
    }
    let mut all = true;
    for i in 0..d::NFILE {
        let asked = bank + 0x18 + 4 * i;
        if m.r32(asked) != 0 {
            if poll_file(m, env, bank + FILES + 0x18 * i, bank + 0x710) {
                m.w32(asked, 0);
            } else {
                all = false;
            }
        }
    }
    if all {
        env.big_reset(m, bank + 0x710);
        m.w32(bank + 0x14, 0);
        index_all(m, bank);
    }
    all
}

/// The characters' order in each board class's uber files (from the class's first: Eddie, Elise, Mac, JP, Zoe, Moby,
/// Luther, Kaori, Psymon, Seeiah, Brodi, Marisol)
pub const UBER_ORDER: [u32; 12] = [0, 7, 3, 6, 5, 4, 2, 1, 8, 9, 10, 11];

/// The class's first uber file (2: ex 0x19a, 0: bx 0x1a6, 1: fr 0x1b2)
pub fn uber_base(class: u32) -> Option<u32> {
    match class {
        2 => Some(0x19a),
        0 => Some(0x1a6),
        1 => Some(0x1b2),
        _ => None,
    }
}

/// `AnimData_LoadClipTable` 0x159150(now): the shared files (cmanim, kr2anim, ed2anim, lu2anim, mr2anim, ps2anim,
/// br2anim, jp2anim, bxanim, exanim, franim), then for each human rider of the career's race roster (+0x27dc; the
/// player byte +0x69 not negative) whose character (+0x3c) and board class (+0x48) no earlier human rider had, the
/// character's uber file for the class
pub fn load_clip_table(m: &mut Mem, env: &mut dyn LoadEnv, bank: u32, now: bool) {
    for f in [0u32, 7, 5, 6, 8, 9, 0xa, 0xb, 1, 2, 3] {
        load_file(m, env, bank, f, now);
    }
    let roster = CAREER + 0x27dc;
    let row = |i: u32| roster + 0x84 * i;
    let mut i = 0u32;
    while (i as i32) < m.r32(roster) as i32 {
        let r = row(i);
        if (m.r8(r + 0x69) as i8) >= 0 {
            let (ch, class) = (m.r32(r + 0x3c), m.r32(r + 0x48));
            let dup = (0..i).any(|t| {
                let o = row(t);
                (m.r8(o + 0x69) as i8) >= 0 && m.r32(o + 0x3c) == ch && m.r32(o + 0x48) == class
            });
            if !dup {
                if let (Some(base), Some(k)) = (uber_base(class), UBER_ORDER.iter().position(|&c| c == ch)) {
                    load_file(m, env, bank, base + k as u32, now);
                }
            }
        }
        i += 1;
    }
}

/// `AnimData_OpenBig` 0x158ea0(mode, now, list, n): nothing for the mode already open. Else the bank reset and the
/// mode noted; not loading now, the BIG file opened and loads waiting. Mode 0 loads feanim, modes 1 and 2 the clip
/// table; then the list's n files; loading now, every clip indexed
pub fn open_big<E: LoadEnv + crate::animseq::SeqEnv>(m: &mut Mem, env: &mut E, bank: u32, mode: u32, now: bool, list: u32, n: i32) {
    if m.r32(bank + 0x10) == mode {
        return;
    }
    d::reset(m, env, bank);
    m.w32(bank + 0x10, mode);
    if !now {
        env.big_open(m, bank + 0x710);
        m.w32(bank + 0x14, 1);
    }
    match mode {
        0 => load_file(m, env, bank, 4, now),
        1 | 2 => load_clip_table(m, env, bank, now),
        _ => {}
    }
    for k in 0..n.max(0) as u32 {
        let f = m.r32(list + 4 * k);
        load_file(m, env, bank, f, now);
    }
    if now {
        index_all(m, bank);
    }
}
