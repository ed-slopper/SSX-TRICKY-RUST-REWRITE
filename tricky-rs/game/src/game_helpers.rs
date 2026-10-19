//! The game's and the app's small helpers on the game's bytes (board row E1i): the pause tests, the HUD option,
//! viewport mode and brightness setters, the race restart and camera reset, the views, `Race_GetRider`, and the app's
//! module switch, string region, archives and the load module's destructor; and the interrupt wrapper 0x182170.
//! Every call out goes through [`FeEnv`]. Checked in the function runner (`tests/game_helpers.rs`), all but the
//! wrapper.

use crate::fe3dstate::{FeEnv, GAPP};
use crate::game_main as gm;
use crate::pf::Mem;
use crate::ps2f::F;

/// The calls out: the pause menu's open test (0x1895c8), the lesson's pause test (0x18f020 on the riders, +0x40),
/// the speech's (0x1a77f8 on +0x38), the views laid out (0x180350), the race's start of a restart (0x180518), a
/// camera's reset (0x173a98), the camera script's stop (0x175eb8) and reset (0x174c48), the riders' restart
/// (0x18ebd0), the lesson helper's (0x14b5a8), the replay's (0x116648), the speech's (0x22e338), the sound's
/// (0x211b58), the pads' latch (0x17a658), the career's course record (0x163468), the display's brightness and colour
/// (0x1e0eb8, 0x1e0db8), the archives' set up (0x23be28), a file's test (0x2c7e38) and an archive's open (0x2c8f50),
/// the strings' load (0x2c82f0), `sprintf` (0x2fca28)
pub const PAUSE_OPEN: u32 = 0x1895c8;
pub const LESSON_PAUSED: u32 = 0x18f020;
pub const SPEECH_BUSY: u32 = 0x1a77f8;
pub const LAYOUT_VIEWS: u32 = 0x180350;
pub const RESTART_BEGIN: u32 = 0x180518;
pub const CAMERA_RESET: u32 = 0x173a98;
pub const SCRIPT_STOP: u32 = 0x175eb8;
pub const SCRIPT_RESET: u32 = 0x174c48;
pub const RIDERS_RESTART: u32 = 0x18ebd0;
pub const HELPER_RESTART: u32 = 0x14b5a8;
pub const REPLAY_RESTART: u32 = 0x116648;
pub const SPEECH_RESET: u32 = 0x22e338;
pub const SOUND_RESTART: u32 = 0x211b58;
pub const PAD_LATCH: u32 = 0x17a658;
pub const COURSE_RECORD: u32 = 0x163468;
pub const BRIGHTNESS: u32 = 0x1e0eb8;
pub const REND_COLOUR: u32 = 0x1e0db8;
pub const BIGS_INIT: u32 = 0x23be28;
pub const FILE_EXISTS: u32 = 0x2c7e38;
pub const BIG_OPEN: u32 = 0x2c8f50;
pub const STRINGS_LOAD: u32 = 0x2c82f0;
pub const SPRINTF: u32 = 0x2fca28;
pub const SPEECH: u32 = 0x34370c;
/// The settings changed flag and the settings the setters keep in the career (+0x3258; +0x16b0 the HUD option,
/// +0x1688 the viewport mode, +0x16b4 the brightness); the brightness the game reads (0x32ca44)
pub const SETTINGS_CHANGED: u32 = 0x3258;
pub const BRIGHTNESS_NOW: u32 = 0x32ca44;
/// The archives' quiet flag (0x3bb978), cleared while they open
pub const QUIET: u32 = 0x3bb978;
/// The archives: (name, kept at, opened only when the file is there)
pub const ARCHIVES: [(u32, u32, bool); 7] = [
    (0x385a28, 0x4c, true),
    (0x385a40, 0x44, true),
    (0x385a58, 0x38, false),
    (0x385a70, 0x3c, false),
    (0x321190, 0x34, false),
    (0x385a88, 0x40, false),
    (0x385aa0, 0x48, true),
];
/// The build string's format and its two parts (0x385ab8, 0x385ad0, 0x385ae0), and the string files by region
/// (0x386380 for 0, 0x386368 for 1); the region kept at 0x32ca50
pub const BUILD: [u32; 3] = [0x385ab8, 0x385ad0, 0x385ae0];
pub const STRING_FILES: [u32; 2] = [0x386380, 0x386368];
pub const REGION: u32 = 0x32ca50;
pub const LOAD_MODULE_VTABLE: u32 = 0x386f40;

/// A virtual call through a table at `at` of the object
fn vcall_at(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(at));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

/// `cGame::IsPaused` 0x17f078(g) -> whether the game is paused: by its own flag (+0x48), the pause menu, the
/// lesson (when there are riders, +0x40) or the speech (+0x38)
pub fn is_paused(m: &mut Mem, e: &mut dyn FeEnv, g: u32) -> u32 {
    if m.r32(g + 0x48) != 0 || e.call(m, PAUSE_OPEN, &[gm::PAUSE_MENU]) != 0 {
        return 1;
    }
    let r = m.r32(g + 0x40);
    if r != 0 && e.call(m, LESSON_PAUSED, &[r]) != 0 {
        return 1;
    }
    let sp = m.r32(g + 0x38);
    (e.call(m, SPEECH_BUSY, &[sp]) != 0) as u32
}

/// `Game_IsMenuOrOverlayActive` 0x17f0f8(g): as the pause test without the lesson's
pub fn menu_or_overlay(m: &mut Mem, e: &mut dyn FeEnv, g: u32) -> u32 {
    if m.r32(g + 0x48) != 0 || e.call(m, PAUSE_OPEN, &[gm::PAUSE_MENU]) != 0 {
        return 1;
    }
    let sp = m.r32(g + 0x38);
    (e.call(m, SPEECH_BUSY, &[sp]) != 0) as u32
}

/// A career setting at `at` set to `v`, the settings marked changed when it differs
fn setting(m: &mut Mem, at: u32, v: u32) {
    if m.r32(gm::CAREER + at) != v {
        m.w32(gm::CAREER + at, v);
        m.w32(gm::CAREER + SETTINGS_CHANGED, 1);
    }
}

/// `Game_SetHudOption` 0x17f168(g, v): the career's HUD option
pub fn set_hud_option(m: &mut Mem, v: u32) {
    setting(m, 0x16b0, v);
}

/// `Game_SetViewportMode` 0x17f1a0(g, v): +0x29c the other mode, the views laid out, the career's mode
pub fn set_viewport_mode(m: &mut Mem, e: &mut dyn FeEnv, g: u32, v: u32) {
    m.w32(g + 0x29c, v ^ 1);
    e.call(m, LAYOUT_VIEWS, &[g]);
    setting(m, 0x1688, v);
}

/// `Render_GetBrightness` 0x17b970(app) -> the brightness (0x32ca44)
pub fn get_brightness(m: &Mem) -> F {
    F(f32::from_bits(m.r32(BRIGHTNESS_NOW)))
}

/// `Render_SetBrightness` 0x17b980(app, b; `buf` 0x20 bytes): the renderer's (+0x724) colour read, its +0x67c `b` +
/// 0.5 kept from 0 to 2, the colour set again; the career's brightness (+0x16b4) when it differs
pub fn set_brightness(m: &mut Mem, e: &mut dyn FeEnv, app: u32, b: F, buf: u32) {
    let r = m.r32(app + 0x724);
    e.call(m, BRIGHTNESS, &[buf, r]);
    let mut f = b + F(0.5);
    if f.ee() < 0.0 {
        f = F(0.0);
    } else if 2.0 < f.ee() {
        f = F(2.0);
    }
    m.w32(r + 0x67c, f.0.to_bits());
    for k in 0..3u32 {
        let v = m.r32(buf + 4 * k);
        m.w32(buf + 0x10 + 4 * k, v);
    }
    e.call(m, REND_COLOUR, &[r, buf + 0x10]);
    let kept = F(f32::from_bits(m.r32(gm::CAREER + 0x16b4)));
    if b.ee() != kept.ee() {
        m.w32(gm::CAREER + 0x16b4, b.0.to_bits());
        m.w32(gm::CAREER + SETTINGS_CHANGED, 1);
    }
}

/// The start the restart and the camera reset share: the race's start of a restart, each camera (+0xa0, 0x80 apart,
/// +0x294 of them) reset, the camera script stopped and reset, the riders restarted when there are
fn restart_cameras(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    e.call(m, RESTART_BEGIN, &[g]);
    let mut k = 0u32;
    while (k as i32) < m.r32(g + 0x294) as i32 {
        let c = m.r32(g + 0xa0 + 0x80 * k);
        k += 1;
        e.call(m, CAMERA_RESET, &[c]);
    }
    let c = m.r32(g + 0xa0);
    e.call(m, SCRIPT_STOP, &[c, 1]);
    e.call(m, SCRIPT_RESET, &[]);
    let r = m.r32(g + 0x40);
    if r != 0 {
        e.call(m, RIDERS_RESTART, &[r]);
    }
}

/// The end they share: the replay, the speech and the sound restarted, the pads latched
fn restart_end(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    let rp = m.r32(g + 0x14);
    e.call(m, REPLAY_RESTART, &[rp]);
    let sp = m.r32(SPEECH);
    e.call(m, SPEECH_RESET, &[sp]);
    let so = m.r32(gm::SOUND);
    e.call(m, SOUND_RESTART, &[so]);
    let input = m.r32(m.r32(GAPP) + 0x24);
    e.call(m, PAD_LATCH, &[input]);
}

/// `Game_RestartRace` 0x180a10(g): the cameras and riders restarted, the race's +0x80, the replay, speech, sound and
/// pads; in mode 7 the career's course record (2, the course) gives +0x2b08 its +0x10
pub fn restart_race(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    restart_cameras(m, e, g);
    let race = m.r32(g + 0x1c);
    vcall_at(m, e, race, 8, 0x80, &[]);
    restart_end(m, e, g);
    let c = gm::CAREER;
    if m.r32(c + 0x2afc) == 7 {
        let course = m.r32(c + 0x2af8);
        let rec = e.call(m, COURSE_RECORD, &[c, 2, course]);
        let v = m.r32(rec + 0x10);
        m.w32(c + 0x2b08, v);
    }
}

/// `Game_ResetCameras` 0x180b58(g): as the restart, with the lesson helper's (+0x34) and the race's +0x88
pub fn reset_cameras(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    restart_cameras(m, e, g);
    let h = m.r32(g + 0x34);
    if h != 0 {
        e.call(m, HELPER_RESTART, &[h]);
    }
    let race = m.r32(g + 0x1c);
    vcall_at(m, e, race, 8, 0x88, &[]);
    restart_end(m, e, g);
}

/// `Game_RestoreViews` 0x181d50(g): the views (+0x298) as many as the cameras (+0x294), laid out
pub fn restore_views(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    let n = m.r32(g + 0x294);
    m.w32(g + 0x298, n);
    e.call(m, LAYOUT_VIEWS, &[g]);
}

/// `Game_UseSingleView` 0x181d60(g): one view, laid out
pub fn use_single_view(m: &mut Mem, e: &mut dyn FeEnv, g: u32) {
    m.w32(g + 0x298, 1);
    e.call(m, LAYOUT_VIEWS, &[g]);
}

/// `Race_GetRider` 0x1814a8(g, k) -> the race's (+0x1c) rider `k` (+0xc4)
pub fn race_rider(m: &Mem, g: u32, k: u32) -> u32 {
    m.r32(m.r32(g + 0x1c).wrapping_add(k << 2).wrapping_add(0xc4))
}

/// `cLoadModule::~cLoadModule` 0x181e48(o, flags): its table, freed when bit 0 is set
pub fn load_module_dtor(m: &mut Mem, e: &mut dyn FeEnv, o: u32, flags: u32) {
    m.w32(o, LOAD_MODULE_VTABLE);
    if flags & 1 != 0 {
        e.call(m, gm::FREE, &[o]);
    }
}

/// `App_SetMode` 0x17c6b8(app, md): the module kept pending (+4) and entered (+0x10)
pub fn app_set_mode(m: &mut Mem, e: &mut dyn FeEnv, app: u32, md: u32) {
    m.w32(app + 4, md);
    vcall_at(m, e, md, 0, 0x10, &[]);
}

/// `Loc_LoadForRegion` 0x17c150(app, region): the strings' buffer made, the region kept (+0x58), region 0's or
/// 1's strings file loaded into it, the strings' 0x17c1d0 (nothing), the region kept at 0x32ca50
pub fn loc_load_for_region(m: &mut Mem, e: &mut dyn FeEnv, app: u32, region: u32) {
    crate::app::loc_load(m, e, app);
    m.w32(app + 0x58, region);
    if region < 2 {
        let file = STRING_FILES[region as usize];
        let buf = m.r32(app + 0x50);
        let size = m.r32(app + 0x54);
        e.call(m, STRINGS_LOAD, &[file, buf, size]);
    }
    crate::app::nothing();
    m.w32(REGION, region);
}

/// `App_OpenBigFiles` 0x17bdf8(app): with the archives' quiet flag cleared, the seven archives opened (three only
/// when their file is there) into the app; the app's game cleared (+0x730), the flag put back, the course table
/// filled, and the build string (+0x740)
pub fn open_big_files(m: &mut Mem, e: &mut dyn FeEnv, app: u32) {
    e.call(m, BIGS_INIT, &[]);
    let quiet = m.r32(QUIET);
    m.w32(QUIET, 0);
    for (name, at, optional) in ARCHIVES {
        if optional && e.call(m, FILE_EXISTS, &[name]) == 0 {
            continue;
        }
        e.call(m, BIG_OPEN, &[name, 0, 0x64, app + at]);
    }
    m.w32(app + 0x730, 0);
    m.w32(QUIET, quiet);
    crate::app::table_fill(m, e, app + 0x64);
    e.call(m, SPRINTF, &[app + 0x740, BUILD[0], BUILD[1], BUILD[2]]);
}

/// 0x182170(cause, f), the interrupt wrapper: every float register kept over the handler `f`'s call (with the
/// cause), interrupts on again (`ei`) -> 1. Not checked in the runner (it has no `ei`); the registers are the
/// runner's
pub fn intc_wrapper(m: &mut Mem, e: &mut dyn FeEnv, cause: u32, f: u32) -> u32 {
    e.call(m, f, &[cause]);
    1
}
