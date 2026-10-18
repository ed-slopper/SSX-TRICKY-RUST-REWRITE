//! The application object (`cApplication`, the game's `gApp` at 0x337c58) and its small helpers (board rows E1c..)
//! on the game's bytes: its constructor, the course table it keeps at +0x64 (the count, then 0x84-byte records: a
//! path at +4, the course's file name at +0x44, its number at +0x84), the course archives (+0x698), the strings and
//! the effects loader; then its start, next module, shutdown and destructor, its run and main loop and the vsync
//! mode the loop runs pending modules on. Every call out goes through [`FeEnv`]. Checked in the function runner
//! (`tests/app.rs`, `tests/app_loop.rs`).

use crate::fe3dstate::{FeEnv, GAPP};
use crate::pf::Mem;
use crate::ps2f::F;

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

// ---- the application's life and its loop (board row E1e)

/// The calls out of the application's life: a message printed (0x23d5d0), the world's set-up (0x2e5838, then
/// 0x2575b8 on 0x346480), the renderer made (0x1dba58), the fades made (0x1fecc8), the GS's set-up (0x1db9f0), the
/// display's wait (0x2ced00), the strings for the region (`Loc_LoadForRegion` 0x17c150), the career's and sound's
/// starts (0x1617a0, 0x20da50, 0x22be18, 0x161928, 0x161a98), the start screen made (`StartScreenSingle_Create`
/// 0x184318), the archives opened (`App_OpenBigFiles` 0x17bdf8), the front end made (`cFE3D::cFE3D` 0x263280), a film
/// module made (`cVideoModule::cVideoModule` 0x1864a0), the career's course (0x163468) and race (0x163980), a font
/// destroyed (0x19c808), a heap block freed (0x2c8fc0), the lesson object's reset (0x159070, 0x158cc8), the
/// records' destructor (0x17c280), the archives' (0x23bb98), the input's constructor (0x17a420), destructor
/// (0x17a5c0), frame (0x17a658), pending frames (0x17a740) and step (0x17a668)
pub const PRINT: u32 = 0x23d5d0;
pub const WORLD_NEW: u32 = 0x2e5838;
pub const WORLD_INIT: u32 = 0x2575b8;
pub const WORLD_SLOT: u32 = 0x346480;
pub const RENDERER_NEW: u32 = 0x1dba58;
pub const FADES_CTOR: u32 = 0x1fecc8;
pub const GS_INIT: u32 = 0x1db9f0;
pub const VSYNC_WAIT: u32 = 0x2ced00;
pub const LOC_FOR_REGION: u32 = 0x17c150;
pub const CAREER_STARTS: [u32; 5] = [0x1617a0, 0x20da50, 0x22be18, 0x161928, 0x161a98];
pub const START_SCREEN: u32 = 0x184318;
pub const OPEN_BIG_FILES: u32 = 0x17bdf8;
pub const FRONT_END_CTOR: u32 = 0x263280;
pub const VIDEO_MODULE_CTOR: u32 = 0x1864a0;
pub const CAREER_COURSE: u32 = 0x163468;
pub const CAREER_PREPARE_RACE: u32 = 0x163980;
pub const FONT_DTOR: u32 = 0x19c808;
pub const BLOCK_FREE: u32 = 0x2c8fc0;
pub const LESSON_RESET: [u32; 2] = [0x159070, 0x158cc8];
pub const LESSON: u32 = 0x3211a8;
pub const RECORDS_DTOR: u32 = 0x17c280;
pub const BIG_DTOR: u32 = 0x23bb98;
pub const INPUT_CTOR: u32 = 0x17a420;
pub const INPUT_DTOR: u32 = 0x17a5c0;
pub const INPUT_FRAME: u32 = 0x17a658;
pub const INPUT_PENDING: u32 = 0x17a740;
pub const INPUT_STEP: u32 = 0x17a668;
pub const CAREER: u32 = 0x32b390;
/// The allocations' names, the messages, the input's, the front end's, the film module's and the game's; the
/// object kept at 0x33efb8 that the shutdown destroys
pub const FADES_NAME: u32 = 0x3859c8;
pub const RECORDS_NAME: u32 = 0x3859d8;
pub const INIT_MESSAGE: u32 = 0x3859e0;
pub const QUIT_MESSAGE: u32 = 0x385af0;
pub const INPUT_NAME: u32 = 0x386398;
pub const FRONT_END_NAME: u32 = 0x3859f8;
pub const VIDEO_MODULE_NAME: u32 = 0x385a08;
pub const GAME_NAME: u32 = 0x385a20;
pub const KEPT_OBJECT: u32 = 0x33efb8;
/// The paused flag the loop's draw checks
pub const PAUSE_DRAW: u32 = 0x337c5c;

/// A virtual call through a table at `at` of the object
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(at));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// An object's destructor through its table at `at` (+8 with 3), when there is one
fn destroy(m: &mut Mem, e: &mut dyn FeEnv, o: u32, at: u32) {
    if o != 0 {
        vcall_at(m, e, o, at, 8, &[3]);
    }
}

/// The adjust of an object's virtual at `off` (its table at +0), which the loop leaves in a1
fn slot_adjust(m: &Mem, o: u32, off: u32) -> u32 {
    m.r16(m.r32(o).wrapping_add(off)) as i16 as i32 as u32
}

/// `cPS2Device` vf6 0x17b668(d, out) and vf7 0x17b6b0(d, src): the device's eight words from +0xc out, or in
pub fn device_get(m: &mut Mem, d: u32, out: u32) {
    for k in 0..8u32 {
        let v = m.r32(d + 0xc + 4 * k);
        m.w32(out + 4 * k, v);
    }
}

pub fn device_set(m: &mut Mem, d: u32, src: u32) {
    for k in 0..8u32 {
        let v = m.r32(src + 4 * k);
        m.w32(d + 0xc + 4 * k, v);
    }
}

/// `cApplication` vf1 0x17ba50(app), the start: the world (0x346480), the renderer (+0x724, its +0x10), the fades
/// (0x7c bytes; +0x72c), the GS; 200 records (0x1614 bytes; +0x728: each -1, 1, 1, 0, 0, 1, 1); the message, the
/// display's wait, the strings (+0x738 0, the region's 0), the career's and sound's starts, +0x73c 0
pub fn app_start(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    nothing();
    let w = e.call(m, WORLD_NEW, &[0]);
    m.w32(WORLD_SLOT, w);
    e.call(m, WORLD_INIT, &[WORLD_SLOT]);
    let r = e.call(m, RENDERER_NEW, &[]);
    m.w32(app + 0x724, r);
    let vt = m.r32(r + 0x10);
    let leftover = m.r32(vt + 0x14);
    vcall_at(m, e, r, 0x10, 0x10, &[leftover]);
    let a = alloc(m, e, FADES_NAME, 0x7c);
    let f = e.call(m, FADES_CTOR, &[a]);
    m.w32(app + 0x72c, f);
    e.call(m, GS_INIT, &[]);
    let s = alloc(m, e, RECORDS_NAME, 0x1614);
    m.w32(s + 4, 0);
    for i in 0..200u32 {
        let r = s + 8 + 0x1c * i;
        m.w32(r, u32::MAX);
        m.w32(r + 4, 0x3f80_0000);
        m.w32(r + 8, 0x3f80_0000);
        m.w32(r + 0xc, 0);
        m.w32(r + 0x10, 0);
        m.w32(r + 0x14, 0x3f80_0000);
        m.w32(r + 0x18, 0x3f80_0000);
    }
    m.w32(app + 0x728, s);
    m.w32(s + 0x1610, 0);
    e.call(m, PRINT, &[INIT_MESSAGE]);
    e.call(m, VSYNC_WAIT, &[0]);
    loc_load(m, e, app);
    m.w32(app + 0x738, 0);
    e.call(m, LOC_FOR_REGION, &[app, 0]);
    e.call(m, CAREER_STARTS[0], &[CAREER]);
    let v = m.r32(CAREER + 0x1670);
    e.call(m, CAREER_STARTS[1], &[v]);
    e.call(m, CAREER_STARTS[2], &[]);
    e.call(m, CAREER_STARTS[3], &[CAREER]);
    e.call(m, CAREER_STARTS[4], &[CAREER]);
    m.w32(app + 0x73c, 0);
}

/// A block (zeroed) for a module
fn alloc_zero(m: &mut Mem, e: &mut dyn FeEnv, name: u32, size: u32) -> u32 {
    let a = e.call(m, ALLOC, &[name, size, 0]);
    if a != 0 {
        e.call(m, FILL, &[a, 0, size]);
    }
    a
}

/// `cApplication` vf3 0x17bbd8(app) -> the next module, by the stage (+0x73c): 0 the start screen (stage 1); 1 the
/// archives opened and the front end (with 1; stage 3); 2 a film when the career asks (its +0x3260 bit 5, then
/// cleared), else the front end (with 0; stage 3) on bits 0, 1, 3..5, else the race set up from the career's chosen
/// course (+0x3814 the course, +0x3818 the mode, +0x381c the player; mode 7 its course's +0x10 at +0x2b08) and the
/// game; 3 the game (stage 2); others none. The game's module is its +8.
pub fn next_module(m: &mut Mem, e: &mut dyn FeEnv, app: u32) -> u32 {
    let c = CAREER;
    let game = |m: &mut Mem, e: &mut dyn FeEnv| -> u32 {
        let a = alloc_zero(m, e, GAME_NAME, 0x2b0);
        let g = crate::game_main::game_ctor(m, a);
        if g == 0 {
            0
        } else {
            g + 8
        }
    };
    match m.r32(app + 0x73c) {
        0 => {
            m.w32(app + 0x73c, 1);
            e.call(m, START_SCREEN, &[1, app, 2])
        }
        1 => {
            m.w32(app + 0x73c, 3);
            e.call(m, OPEN_BIG_FILES, &[app]);
            let a = alloc_zero(m, e, FRONT_END_NAME, 0x130);
            e.call(m, FRONT_END_CTOR, &[a, 1])
        }
        2 => {
            let flags = m.r32(c + 0x3260);
            if flags & 0x20 != 0 {
                m.w32(c + 0x3260, flags & !0x20);
                let a = alloc_zero(m, e, VIDEO_MODULE_NAME, 0x114);
                return e.call(m, VIDEO_MODULE_CTOR, &[a, c + 0x3714, 1]);
            }
            if flags & 0x3b != 0 {
                m.w32(app + 0x73c, 3);
                let a = alloc_zero(m, e, FRONT_END_NAME, 0x130);
                return e.call(m, FRONT_END_CTOR, &[a, 0]);
            }
            let course = m.r32(c + 0x3814);
            let mode = m.r32(c + 0x3818);
            let player = m.r32(c + 0x381c);
            m.w32(c + 0x2af8, course);
            m.w32(c + 0x27dc, player);
            m.w32(c + 0x2afc, mode);
            if mode == 7 {
                let r = e.call(m, CAREER_COURSE, &[c, 2, course]);
                let v = m.r32(r + 0x10);
                m.w32(c + 0x2b08, v);
            } else {
                m.w32(c + 0x2b08, 0);
            }
            e.call(m, CAREER_PREPARE_RACE, &[c]);
            game(m, e)
        }
        3 => {
            m.w32(app + 0x73c, 2);
            game(m, e)
        }
        _ => 0,
    }
}

/// `cApplication` vf2 0x17bf48(app), the shutdown: the message; the fonts destroyed (+0x60, +0x5c); the heap blocks
/// +0x34..+0x40 freed, and +0x4c, +0x48, +0x44 when there; the strings; the lesson object reset; the records
/// (+0x728) destroyed, the fades freed; the renderer's +0x18 and destruction; the object at 0x33efb8 destroyed
pub fn app_shutdown(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    e.call(m, PRINT, &[QUIT_MESSAGE]);
    for at in [0x60u32, 0x5c] {
        let f = m.r32(app + at);
        if f != 0 {
            e.call(m, FONT_DTOR, &[f, 3]);
        }
    }
    for at in [0x34u32, 0x38, 0x3c, 0x40] {
        let b = m.r32(app + at);
        e.call(m, BLOCK_FREE, &[b, 0x64]);
    }
    for at in [0x4cu32, 0x48, 0x44] {
        let b = m.r32(app + at);
        if b != 0 {
            e.call(m, BLOCK_FREE, &[b, 0x64]);
        }
    }
    loc_unload(m, e, app);
    e.call(m, LESSON_RESET[0], &[LESSON]);
    e.call(m, LESSON_RESET[1], &[LESSON]);
    let rec = m.r32(app + 0x728);
    if rec != 0 {
        e.call(m, RECORDS_DTOR, &[rec, 3]);
    }
    let f = m.r32(app + 0x72c);
    e.call(m, FREE, &[f]);
    let r = m.r32(app + 0x724);
    vcall_at(m, e, r, 0x10, 0x18, &[r]);
    let r = m.r32(app + 0x724);
    destroy(m, e, r, 0x10);
    let k = m.r32(KEPT_OBJECT);
    destroy(m, e, k, 0);
}

/// `cApplication::~cApplication` 0x17c418(app, flags): the archives (with 2) and the course table (with 2), then
/// the manager's part
pub fn app_dtor(m: &mut Mem, e: &mut dyn FeEnv, app: u32, flags: u32) {
    e.call(m, BIG_DTOR, &[app + 0x698, 2]);
    table_delete(m, e, app + 0x64, 2);
    appman_dtor(m, e, app, flags);
}

/// `cAppMan::~cAppMan` 0x17c488(app, flags): its table put back, and freed when bit 0 is set
pub fn appman_dtor(m: &mut Mem, e: &mut dyn FeEnv, app: u32, flags: u32) {
    m.w32(app + 0x2c, APPMAN_VTABLE);
    if flags & 1 != 0 {
        e.call(m, FREE, &[app]);
    }
}

/// `App_StopVSyncMode` 0x17c908(app): with a module pending (+4), the vsync stage (+0xc) set to 3 and waited on
/// (the execution manager's +0x20 until the interrupt moves it on), then the pending module destroyed and cleared
pub fn stop_vsync_mode(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    if m.r32(app + 4) == 0 {
        return;
    }
    m.w32(app + 0xc, 3);
    while m.r32(app + 0xc) == 3 {
        let ex = m.r32(app + 0x28);
        vcall_at(m, e, ex, 0, 0x20, &[ex]);
    }
    let p = m.r32(app + 4);
    destroy(m, e, p, 0);
    m.w32(app + 4, 0);
}

/// `App_StartVSyncMode` 0x17c998(app): the pending module's +0x28, then the vsync stage 2
pub fn start_vsync_mode(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    let p = m.r32(app + 4);
    vcall_at(m, e, p, 0, 0x28, &[p]);
    m.w32(app + 0xc, 2);
}

/// `App_RunVSyncModeTicks` 0x17cb40(app): the pending module's +0x30 for every pending input frame, then its +0x38
pub fn run_vsync_mode_ticks(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    loop {
        let input = m.r32(app + 0x24);
        if e.call(m, INPUT_PENDING, &[input]) == 0 {
            break;
        }
        let p = m.r32(app + 4);
        vcall_at(m, e, p, 0, 0x30, &[p]);
    }
    let p = m.r32(app + 4);
    vcall_at(m, e, p, 0, 0x38, &[p]);
}

/// `App_VSyncTick` 0x17c9d8(app), the vsync interrupt: the tick count (+0x10) up, the fraction (+0x1c) moved on by
/// the rate (+0x18) and the whole frames it crosses stepped through the input; then by the vsync stage: 1 the
/// pending module's +0x28 and stage 2 and its ticks; 2 its ticks; 3 its +0x20 (done: stage 0, else its ticks); then
/// the execution manager's +0x28
pub fn vsync_tick(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    let t = F(f32::from_bits(m.r32(app + 0x1c)));
    let rate = F(f32::from_bits(m.r32(app + 0x18)));
    let before = t.ee() as i32;
    let input = m.r32(app + 0x24);
    let n = m.r32(app + 0x10);
    m.w32(app + 0x10, n.wrapping_add(1));
    let t2 = t + rate;
    let frames = (t2.ee() as i32).wrapping_sub(before);
    let left = t2 - F(frames as f32);
    m.w32(app + 0x1c, left.0.to_bits());
    if input != 0 {
        let mut k = frames.wrapping_sub(1);
        while k != -1 {
            let input = m.r32(app + 0x24);
            e.call(m, INPUT_STEP, &[input]);
            k = k.wrapping_sub(1);
        }
    }
    match m.r32(app + 0xc) as i32 {
        1 => {
            let p = m.r32(app + 4);
            vcall_at(m, e, p, 0, 0x28, &[p]);
            m.w32(app + 0xc, 2);
            run_vsync_mode_ticks(m, e, app);
        }
        2 => run_vsync_mode_ticks(m, e, app),
        3 => {
            let p = m.r32(app + 4);
            if vcall_at(m, e, p, 0, 0x20, &[p]) != 0 {
                m.w32(app + 0xc, 0);
            } else {
                run_vsync_mode_ticks(m, e, app);
            }
        }
        _ => {}
    }
    let ex = m.r32(app + 0x28);
    vcall_at(m, e, ex, 0, 0x28, &[ex]);
}

/// `App_MainLoop` 0x17c6e8(app): until quit (+0x20), each module (+0) entered (+0x10), the vsync mode stopped, its
/// +0x28 and the input's frame; then frame by frame: the display's wait; when quitting, or a pending module (+4) is
/// ready (+0x18), and the module agrees (+0x18): it is destroyed and the next one (the app's +0x20) taken, the vsync
/// mode started; otherwise the input's pending frames each update the module (+0x30) and, unless paused
/// (0x337c5c), draw it (+0x38), at most the app's +0x14 at a time (then the input's frame); when paused, the
/// module draws once after any updates, else the execution manager's +0x20
pub fn main_loop(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    if m.r32(app + 0x20) != 0 {
        return;
    }
    'modules: loop {
        let mut frames = 0u32;
        let mut leaving = false;
        let md = m.r32(app);
        vcall_at(m, e, md, 0, 0x10, &[md]);
        stop_vsync_mode(m, e, app);
        let md = m.r32(app);
        vcall_at(m, e, md, 0, 0x28, &[md]);
        let input = m.r32(app + 0x24);
        e.call(m, INPUT_FRAME, &[input]);
        loop {
            e.call(m, VSYNC_WAIT, &[0]);
            if m.r32(app + 0x20) != 0 {
                leaving = true;
            }
            if !leaving {
                let p = m.r32(app + 4);
                if p != 0 && vcall_at(m, e, p, 0, 0x18, &[p]) != 0 {
                    leaving = true;
                }
            }
            if leaving {
                let md = m.r32(app);
                let adj = slot_adjust(m, md, 0x18);
                if vcall_at(m, e, md, 0, 0x18, &[adj]) != 0 {
                    let md = m.r32(app);
                    destroy(m, e, md, 0);
                    m.w32(app, 0);
                    if m.r32(app + 0x20) != 0 {
                        return;
                    }
                    let next = vcall_at(m, e, app, 0x2c, 0x20, &[]);
                    m.w32(app, next);
                    start_vsync_mode(m, e, app);
                    if m.r32(app + 0x20) == 0 {
                        continue 'modules;
                    }
                    return;
                }
            }
            loop {
                let input = m.r32(app + 0x24);
                if e.call(m, INPUT_PENDING, &[input]) == 0 {
                    break;
                }
                frames += 1;
                let md = m.r32(app);
                vcall_at(m, e, md, 0, 0x30, &[md]);
                if m.r32(PAUSE_DRAW) == 0 {
                    let md = m.r32(app);
                    vcall_at(m, e, md, 0, 0x38, &[md]);
                }
                if (frames as i32) >= m.r32(app + 0x14) as i32 {
                    let input = m.r32(app + 0x24);
                    e.call(m, INPUT_FRAME, &[input]);
                    break;
                }
            }
            if m.r32(PAUSE_DRAW) == 0 {
                continue;
            }
            if frames != 0 {
                let md = m.r32(app);
                let adj = slot_adjust(m, md, 0x38);
                if vcall_at(m, e, md, 0, 0x38, &[adj]) != 0 {
                    frames = 0;
                    continue;
                }
            }
            let ex = m.r32(app + 0x28);
            let vt = m.r32(ex);
            let leftover = m.r32(vt + 0x24);
            vcall_at(m, e, ex, 0, 0x20, &[leftover]);
        }
    }
}

/// `App_Run` 0x17c570(app, argc, argv) -> 0: the app's pending module (+4), vsync stage (+0xc) and quit (+0x20)
/// cleared; the execution manager's +0x10; the app's start (+0x10 with the arguments); the input (0x28 bytes;
/// +0x24); the first module (+0x20); the loop; the app's shutdown (+0x18); the input destroyed and freed; the
/// execution manager's +0x18 and destruction; `gApp` cleared
pub fn app_run(m: &mut Mem, e: &mut dyn FeEnv, app: u32, argc: u32, argv: u32) -> u32 {
    m.w32(app + 4, 0);
    m.w32(app + 0xc, 0);
    m.w32(app + 0x20, 0);
    let ex = m.r32(app + 0x28);
    let vt = m.r32(ex);
    let leftover = m.r32(vt + 0x14);
    vcall_at(m, e, ex, 0, 0x10, &[leftover]);
    vcall_at(m, e, app, 0x2c, 0x10, &[argc, argv]);
    let a = alloc(m, e, INPUT_NAME, 0x28);
    e.call(m, INPUT_CTOR, &[a]);
    m.w32(app + 0x24, a);
    let first = vcall_at(m, e, app, 0x2c, 0x20, &[]);
    m.w32(app, first);
    main_loop(m, e, app);
    vcall_at(m, e, app, 0x2c, 0x18, &[]);
    let input = m.r32(app + 0x24);
    m.w32(app + 0x24, 0);
    e.call(m, INPUT_DTOR, &[input]);
    e.call(m, FREE, &[input]);
    let ex = m.r32(app + 0x28);
    vcall_at(m, e, ex, 0, 0x18, &[ex]);
    let ex = m.r32(app + 0x28);
    destroy(m, e, ex, 0);
    m.w32(GAPP, 0);
    0
}
