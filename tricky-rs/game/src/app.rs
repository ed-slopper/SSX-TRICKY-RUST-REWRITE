//! The application object (`cApplication`, the game's `gApp` at 0x337c58) and its small helpers (board rows E1c..)
//! on the game's bytes: its constructor, the course table it keeps at +0x64 (the count, then 0x84-byte records: a
//! path at +4, the course's file name at +0x44, its number at +0x84), the course archives (+0x698), the strings and
//! the effects loader. Every call out goes through [`FeEnv`]. Checked in the function runner (`tests/app.rs`).

use crate::fe3dstate::{FeEnv, GAPP};
use crate::pf::Mem;

/// The calls out: the application manager's constructor (0x17c530), the archives' set-up (`BigArchive_Init`
/// 0x23bb70), open (0x23bc28) and close (0x23bcc0), the allocation and its fill, the heap's free, a font made
/// (0x19c788), the effects loader made (0x1caef8), an object freed (0x114f80), a file's size (0x2c7ec8), `strlen`
/// (0x2fd260), `strcmp` (0x2fd008), `strcpy` (0x2fd14c), `sprintf` (0x2fca28), a localised string (0x2c5160) and a
/// wide string's length (0x17cc48)
pub const APPMAN_CTOR: u32 = 0x17c530;
pub const BIG_INIT: u32 = 0x23bb70;
pub const BIG_OPEN: u32 = 0x23bc28;
pub const BIG_CLOSE: u32 = 0x23bcc0;
pub const ALLOC: u32 = 0x2ccf70;
pub const FILL: u32 = 0x2cc808;
pub const HEAP_FREE: u32 = 0x2ccfc0;
pub const FONT_CTOR: u32 = 0x19c788;
pub const EFFECTS_CTOR: u32 = 0x1caef8;
pub const FREE: u32 = 0x114f80;
pub const FILE_SIZE: u32 = 0x2c7ec8;
pub const STRLEN: u32 = 0x2fd260;
pub const STRCMP: u32 = 0x2fd008;
pub const STRCPY: u32 = 0x2fd14c;
pub const SPRINTF: u32 = 0x2fca28;
pub const LOC_STRING: u32 = 0x2c5160;
pub const WSTR_LEN: u32 = 0x17cc48;
/// The application's table, the two fonts' and the effects loader's allocation names, the strings file's name and
/// its allocation's, the courses (0x1c bytes each: the number, the file name at +0x10, the path's part at +0x14) and
/// the path's format
pub const APP_VTABLE: u32 = 0x385b10;
pub const FONT_NAMES: [u32; 2] = [0x385998, 0x3859a8];
pub const EFFECTS_NAME: u32 = 0x3859b8;
pub const STRINGS_FILE: u32 = 0x386380;
pub const STRINGS_NAME: u32 = 0x385b08;
pub const COURSES: u32 = 0x332f28;
pub const MODELS_FORMAT: u32 = 0x3884f0;
/// The front end's course (left out of the table)
const FRONT_END: u32 = 12;

fn alloc(m: &mut Mem, e: &mut dyn FeEnv, name: u32, size: u32) -> u32 {
    let a = e.call(m, ALLOC, &[name, size, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, 0xdead_c0de, size]);
    }
    a
}

/// `cAppMan`'s constructor 0x17c530(app, exec): its table (+0x2c), the app made the game's (`gApp`), the execution
/// manager (+0x28), +0x14 10, +0x18 1, +0x10, +0x1c and +0x24 cleared
pub const APPMAN_VTABLE: u32 = 0x386168;

pub fn appman_ctor(m: &mut Mem, app: u32, exec: u32) -> u32 {
    m.w32(app + 0x2c, APPMAN_VTABLE);
    m.w32(GAPP, app);
    m.w32(app + 0x28, exec);
    m.w32(app + 0x14, 10);
    m.w32(app + 0x18, 0x3f80_0000);
    m.w32(app + 0x24, 0);
    m.w32(app + 0x10, 0);
    m.w32(app + 0x1c, 0);
    app
}

/// `cApplication`'s constructor 0x17b748(app, exec) (the index's `App_LoadFonts`): the manager's part, then its table,
/// +0x34..+0x4c cleared, an empty course table (+0x64), the archives (+0x698), no strings (+0x50, +0x54), +0x7c8 0,
/// +0x7c4 and +0x7c0 1, and two fonts (0x3c bytes each; +0x5c, +0x60)
pub fn app_ctor(m: &mut Mem, e: &mut dyn FeEnv, app: u32, exec: u32) -> u32 {
    appman_ctor(m, app, exec);
    let _ = e;
    for at in (0x34..0x50).step_by(4) {
        m.w32(app + at, 0);
    }
    m.w32(app + 0x2c, APP_VTABLE);
    table_new(m, app + 0x64);
    e.call(m, BIG_INIT, &[app + 0x698]);
    m.w32(app + 0x50, 0);
    m.w32(app + 0x54, 0);
    m.w32(app + 0x7c8, 0);
    m.w32(app + 0x7c4, 1);
    m.w32(app + 0x7c0, 1);
    for (k, name) in FONT_NAMES.iter().enumerate() {
        let a = alloc(m, e, *name, 0x3c);
        let f = e.call(m, FONT_CTOR, &[a, 0]);
        m.w32(app + 0x5c + 4 * k as u32, f);
    }
    app
}

/// `Course_OpenBig` 0x17b848(app, k): course `k`'s archive opened (its path, with 0x10) when the path is not empty
pub fn course_open_big(m: &mut Mem, e: &mut dyn FeEnv, app: u32, k: u32) {
    let path = table_entry(app + 0x64, k);
    if e.call(m, STRLEN, &[path]) != 0 {
        e.call(m, BIG_OPEN, &[app + 0x698, path, 0x10]);
    }
}

/// `Course_CloseBig` 0x17c2e0(app): the archives closed
pub fn course_close_big(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    e.call(m, BIG_CLOSE, &[app + 0x698]);
}

/// `App_CreateEffectsLoader` 0x17b8a8(app): the effects loader (0x98 bytes; +0x30), and the renderer's (+0x724)
/// fog record (+0x47c) given the loader's fog colour words (+0x78..+0x80; read through `gApp`'s +0x30) at
/// +0x44..+0x4c
pub fn create_effects_loader(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    let a = alloc(m, e, EFFECTS_NAME, 0x98);
    let l = e.call(m, EFFECTS_CTOR, &[a]);
    m.w32(app + 0x30, l);
    let loader = m.r32(m.r32(GAPP) + 0x30);
    let fog = m.r32(m.r32(app + 0x724) + 0x47c);
    for k in 0..3u32 {
        let v = m.r32(loader + 0x78 + 4 * k);
        m.w32(fog + 0x44 + 4 * k, v);
    }
}

/// `App_FreeEffectsLoader` 0x17b938(app): the effects loader, when there is one, deleted (with 3) and cleared
pub fn free_effects_loader(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    let l = m.r32(app + 0x30);
    if l != 0 {
        effects_loader_delete(m, e, l, 3);
    }
    m.w32(app + 0x30, 0);
}

/// `EffectsLoader_Delete` 0x17c468(l, flags): freed when bit 0 is set
pub fn effects_loader_delete(m: &mut Mem, e: &mut dyn FeEnv, l: u32, flags: u32) {
    if flags & 1 != 0 {
        e.call(m, FREE, &[l]);
    }
}

/// `Loc_Load` 0x17c0a8(app): the strings (+0x50) freed when loaded, and a buffer made at least the strings file's
/// size (the larger of it and the size kept at +0x54, which a free clears)
pub fn loc_load(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    let old = m.r32(app + 0x50);
    if old != 0 {
        e.call(m, HEAP_FREE, &[old]);
    }
    m.w32(app + 0x54, 0);
    let size = e.call(m, FILE_SIZE, &[STRINGS_FILE]);
    let kept = m.r32(app + 0x54);
    let size = if (kept as i32) < (size as i32) { size } else { kept };
    m.w32(app + 0x54, size);
    let a = e.call(m, ALLOC, &[STRINGS_NAME, size, 0]);
    m.w32(app + 0x50, a);
}

/// `Loc_Unload` 0x17c118(app): the strings freed and cleared
pub fn loc_unload(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    let s = m.r32(app + 0x50);
    if s != 0 {
        e.call(m, HEAP_FREE, &[s]);
        m.w32(app + 0x50, 0);
    }
}

/// `Loc_GetUnquotedString` 0x1842a8(-, id): the localised string (the app's strings), past an opening quote and
/// with a closing one cut off (both wide)
pub fn loc_unquoted(m: &mut Mem, e: &mut dyn FeEnv, id: u32) -> u32 {
    let strings = m.r32(m.r32(GAPP) + 0x50);
    let s = e.call(m, LOC_STRING, &[strings, id]);
    let s = if m.r16(s) == 0x22 { s + 2 } else { s };
    let n = e.call(m, WSTR_LEN, &[s]);
    if n != 0 {
        let last = s.wrapping_add(n.wrapping_mul(2)).wrapping_sub(2);
        if m.r16(last) == 0x22 {
            m.w16(last, 0);
        }
    }
    s
}

/// The course table's record `k`'s path (`CourseTable_GetEntry` 0x186368: +4 + 0x84 k)
pub fn table_entry(t: u32, k: u32) -> u32 {
    t.wrapping_add(k.wrapping_mul(0x84)).wrapping_add(4)
}

/// `Course_GetInfo` 0x186350(t, k): record `k`'s file name (+0x44)
pub fn table_info(t: u32, k: u32) -> u32 {
    t.wrapping_add(k.wrapping_mul(0x84)).wrapping_add(0x44)
}

/// `CourseTable_Destruct` 0x186348(t): emptied
pub fn table_clear(m: &mut Mem, t: u32) {
    m.w32(t, 0);
}

/// 0x1861f0(t): an empty table
pub fn table_new(m: &mut Mem, t: u32) -> u32 {
    table_clear(m, t);
    t
}

/// `CourseTable_Delete` 0x186218(t, flags): emptied, and freed when bit 0 is set
pub fn table_delete(m: &mut Mem, e: &mut dyn FeEnv, t: u32, flags: u32) {
    table_clear(m, t);
    if flags & 1 != 0 {
        e.call(m, FREE, &[t]);
    }
}

/// `Course_OpenModelsBig` 0x186268(t) -> 1: the table filled from the courses (0x332f28) but the front end: each
/// record's file name copied, its path made from the format and the course's part, its number kept
pub fn table_fill(m: &mut Mem, e: &mut dyn FeEnv, t: u32) -> u32 {
    m.w32(t, 0);
    for k in 0..13u32 {
        if k == FRONT_END {
            continue;
        }
        let c = COURSES + 0x1c * k;
        let n = m.r32(t);
        let name = m.r32(c + 0x10);
        e.call(m, STRCPY, &[t.wrapping_add(n.wrapping_mul(0x84)).wrapping_add(0x44), name]);
        let n = m.r32(t);
        let part = m.r32(c + 0x14);
        e.call(m, SPRINTF, &[t.wrapping_add(n.wrapping_mul(0x84)).wrapping_add(4), MODELS_FORMAT, part]);
        let n = m.r32(t);
        let number = m.r32(c);
        m.w32(t.wrapping_add(n.wrapping_mul(0x84)).wrapping_add(0x84), number);
        let n = m.r32(t);
        m.w32(t, n.wrapping_add(1));
    }
    1
}

/// `CourseTable_FindByName` 0x186380(t, name): the first record whose file name is `name`, or the count
pub fn table_find_by_name(m: &mut Mem, e: &mut dyn FeEnv, t: u32, name: u32) -> u32 {
    let mut i = 0u32;
    while (i as i32) < m.r32(t) as i32 {
        if e.call(m, STRCMP, &[name, t + 0x44 + 0x84 * i]) == 0 {
            break;
        }
        i += 1;
    }
    i
}

/// `CourseTable_FindById` 0x1863f8(t, id): the first record whose number is `id`, or -1
pub fn table_find_by_id(m: &Mem, t: u32, id: u32) -> u32 {
    let n = m.r32(t) as i32;
    for i in 0..n.max(0) as u32 {
        if m.r32(t + 0x84 + 0x84 * i) == id {
            return i;
        }
    }
    u32::MAX
}

/// `CourseTable_GetId` 0x186440(t, k): record `k`'s number, 13 for one past the courses (14 or more)
pub fn table_id(m: &Mem, t: u32, k: u32) -> u32 {
    let v = m.r32(t.wrapping_add(k.wrapping_mul(0x84)).wrapping_add(0x84));
    if (v as i32) < 14 {
        v
    } else {
        13
    }
}

/// `App_GetCourseId` 0x17c360, `App_FindCourseById` 0x17c380, `App_GetCourseInfo` 0x17c3a8: the app's table's
pub fn app_course_id(m: &Mem, app: u32, k: u32) -> u32 {
    table_id(m, app + 0x64, k)
}

pub fn app_find_course(m: &Mem, app: u32, id: u32) -> u32 {
    table_find_by_id(m, app + 0x64, id)
}

pub fn app_course_info(app: u32, k: u32) -> u32 {
    table_info(app + 0x64, k)
}

/// `cCourseResolve::~cCourseResolve` 0x186168(r, flags): the PS2 one's destructor (0x2555e8)
pub const COURSE_RESOLVE_PS2_DTOR: u32 = 0x2555e8;

pub fn course_resolve_dtor(m: &mut Mem, e: &mut dyn FeEnv, r: u32, flags: u32) {
    e.call(m, COURSE_RESOLVE_PS2_DTOR, &[r, flags]);
}

/// 0x17bdf0 and 0x17c1d0: nothing
pub fn nothing() {}
