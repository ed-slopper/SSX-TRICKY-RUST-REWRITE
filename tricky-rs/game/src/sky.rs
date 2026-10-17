//! The sky (board row G7c3): the course's sky models (`Sky_*` 0x1859c0..0x186078, the sky object at 0x337ce8) and the
//! world's sky box node `cSkyBoxObjNode` 0x1c7900..0x1c7cfc, which draws the course's sky around the camera. On the
//! bytes at their addresses ([`Mem`]); the models' own functions and virtuals, the files, the course info, the
//! world's texture table, the lights' build and the renderer are calls out ([`SkyEnv`]). Checked in the function
//! runner (`tests/sky.rs`).
//!
//! **The sky object** (0x337ce8): +0 the course sky table's row count; +4 ten layers (models of 0x14 bytes, their
//! vtable at +0x10, their data at +4); +0xcc the layers loaded; +0xd0 twelve layer numbers by sky id (−1 none).
//!
//! **The course sky table** (0x3880c0, 0x30 bytes a row, −1 ends it): the course, how many skies, their ids.
//! The sky names by id are at 0x337cb8. (`courses.rs` has the same loads as the files a course names.)

use crate::pf::Mem;

pub const SKY: u32 = 0x337ce8;
pub const TABLE: u32 = 0x3880c0;
pub const NAMES: u32 = 0x337cb8;
pub const LAYER_VTABLE: u32 = 0x388350;
pub const NODE_VTABLE: u32 = 0x390bf8;
pub const GAPP: u32 = 0x337c58;
pub const WORLD: u32 = 0x346488;
/// "data/models/%s%s", ".pbd", ".ssh", "%s_sky"
pub const PATH_FORMAT: u32 = 0x388330;
pub const EXT_MODEL: u32 = 0x3a7b58;
pub const EXT_TEXTURES: u32 = 0x388348;
pub const SKY_FORMAT: u32 = 0x390bf0;

/// The calls out
pub trait SkyEnv {
    /// A model's or the renderer's virtual (its address read from the vtable) with these words (floats as bits)
    fn vcall(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32;
    /// A function of the game's with these words
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// A function given a string the game builds on its stack (file checks and loads, the texture table)
    fn call_path(&mut self, m: &mut Mem, func: u32, path: &[u8], args: &[u32]) -> u32;
    /// The world's texture table for a texture file (0x2609d0 with the world, the path and a word it fills): its answer
    /// and the word
    fn texture_table(&mut self, m: &mut Mem, path: &[u8]) -> (u32, u32);
    /// `Lights_Build` 0x1ce140 on the sky box's fixed lights: the light block's words
    fn lights_build(&mut self, m: &mut Mem, ambient: [u32; 4], lights: [[u32; 4]; 6]) -> Vec<u32>;
    /// The renderer's model draw (+0x2a8): the model, its +0xc, the matrix's and the lights' words
    fn draw_model(&mut self, m: &mut Mem, func: u32, this: u32, model: u32, part: u32, matrix: [u32; 16], lights: &[u32]);
}

/// The functions called out
pub const MEM_FREE: u32 = 0x14a040;
pub const DELETE: u32 = 0x114f80;
pub const FILE_EXISTS: u32 = 0x2c7e38;
pub const FILE_LOAD: u32 = 0x2c8a70;
pub const MODEL_RELEASE: u32 = 0x255df0;
pub const MODEL_CONSTRUCT: u32 = 0x2555a8;
pub const MODEL_FIRST: u32 = 0x2522e0;
pub const MODEL_PART: u32 = 0x254af0;
pub const MODEL_MATERIAL: u32 = 0x2549f8;
pub const TEXTURE_TABLE: u32 = 0x2609d0;
pub const COURSE_INFO: u32 = 0x186350;
pub const BASE_CONSTRUCT: u32 = 0x17cf50;
pub const BASE_DESTRUCT: u32 = 0x17cfe0;

fn vfn(m: &Mem, obj: u32, off: u32) -> (u32, u32) {
    let vt = m.r32(obj + 0x10);
    (obj.wrapping_add(m.r16(vt + off) as i16 as i32 as u32), m.r32(vt + off + 4))
}
fn lcall(m: &mut Mem, env: &mut dyn SkyEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let (this, f) = vfn(m, obj, off);
    env.vcall(m, f, this, args)
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

/// `sprintf` with the format's `%s`s taken in turn
fn format(fmt: &[u8], args: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut k, mut a) = (0, 0);
    while k < fmt.len() {
        if fmt[k] == b'%' && fmt.get(k + 1) == Some(&b's') {
            out.extend_from_slice(&args[a]);
            a += 1;
            k += 2;
        } else {
            out.push(fmt[k]);
            k += 1;
        }
    }
    out
}

/// `Sky_GetLayer` 0x185d50(sky, i): the layer (+4 + 0x14 i)
pub fn layer(sky: u32, i: u32) -> u32 {
    sky.wrapping_add(i.wrapping_mul(0x14)).wrapping_add(4)
}

/// `Sky_GetLayerCount` 0x185dc0 / `Sky_SetLayerCount` 0x185dc8 (+0xcc)
pub fn layer_count(m: &Mem, sky: u32) -> u32 {
    m.r32(sky + 0xcc)
}
pub fn set_layer_count(m: &mut Mem, sky: u32, n: u32) {
    m.w32(sky + 0xcc, n);
}

/// `Sky_InitLayers` 0x1859c0: each layer reset (vtable slot +0x10), none loaded
pub fn init_layers(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32) {
    for i in 0..10 {
        lcall(m, env, layer(sky, i), 0x10, &[]);
    }
    set_layer_count(m, sky, 0);
}

/// `Sky_FreeLayer` 0x185a78(i): the layer's data freed and the layer reset; the last loaded one no longer counted
pub fn free_layer(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32, i: u32) {
    let l = layer(sky, i);
    let data = m.r32(l + 4);
    if data != 0 {
        env.call(m, MEM_FREE, &[data]);
    }
    lcall(m, env, l, 0x10, &[]);
    if i == layer_count(m, sky).wrapping_sub(1) {
        let n = layer_count(m, sky).wrapping_sub(1);
        set_layer_count(m, sky, n);
    }
}

/// `Sky_FreeLayers` 0x185a28: every layer freed, from the last; none loaded
pub fn free_layers(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32) {
    for i in (0..10).rev() {
        free_layer(m, env, sky, i);
    }
    set_layer_count(m, sky, 0);
}

/// `Sky_GetModelByIndex` 0x185d68(sky, id): the sky id's layer's first model (0x2522e0(layer, 0)), 0 with none
pub fn model_by_index(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32, id: u32) -> u32 {
    let n = m.r32(sky.wrapping_add(id.wrapping_mul(4)) + 0xd0) as i32;
    if n < 0 {
        return 0;
    }
    env.call(m, MODEL_FIRST, &[layer(sky, n as u32), 0])
}

/// `Course_RemapSkyTextures` 0x185dd0(sky, layer, textures' path): the world's texture table for the file
/// (0x2609d0; nothing when it answers 0); then each of the model's parts (its data's +0x14 of them, 0x254af0) has its
/// four texture shorts (when not negative) and its +0x44 list of words looked up in the table (the part's first short
/// then the list's first word's low half), and each material (+8 of them, 0x2549f8) its four shorts at +0x1b0 unless
/// the flags at +0x1ac say 5 for it; the table freed (when there is one)
pub fn remap_textures(m: &mut Mem, env: &mut dyn SkyEnv, _sky: u32, l: u32, path: &[u8]) {
    let (ok, table) = env.texture_table(m, path);
    if ok == 0 {
        return;
    }
    let lookup16 = |m: &Mem, v: i32| m.r16(table.wrapping_add((v as u32).wrapping_mul(4)));
    let parts = m.r32(m.r32(l + 4) + 0x14) as i32;
    let mut i = 0;
    while i < parts {
        let p = env.call(m, MODEL_PART, &[l, i as u32]);
        for k in 0..4 {
            let v = m.r16(p + 2 * k) as i16 as i32;
            if v >= 0 {
                let t = lookup16(m, v);
                m.w16(p + 2 * k, t);
            }
        }
        let list = m.r32(p + 0x44);
        if list != 0 {
            let n = m.r32(list) as i32;
            let mut k = 0;
            while k < n {
                let at = list + 4 + 4 * k as u32;
                let w = m.r32(table.wrapping_add(m.r32(at).wrapping_mul(4)));
                m.w32(at, w);
                k += 1;
            }
            m.w16(p, m.r16(list + 4));
        }
        i += 1;
    }
    let mats = m.r32(m.r32(l + 4) + 8) as i32;
    for i in 0..mats.max(0) as u32 {
        let q = env.call(m, MODEL_MATERIAL, &[l, i]);
        for k in 0..4 {
            let at = q + 0x1b0 + 2 * k;
            let v = m.r16(at) as i16 as i32;
            if v < 0 {
                continue;
            }
            if (m.r32(q + 0x1ac) >> ((3 * k) & 31)) & 7 == 5 {
                continue;
            }
            let t = lookup16(m, v);
            m.w16(at, t);
        }
    }
    if table != 0 {
        env.call(m, MEM_FREE, &[table]);
    }
}

/// `Course_LoadSky` 0x185c28(sky, name): `data/models/<name>.pbd` (and its `.ssh`) into the next layer: none with ten
/// loaded or no such file (−1); the file loaded and handed to the layer (slot +0x30); when it takes it, its textures
/// remapped, the layer counted and the loaded file released (0x255df0). The layer's number
pub fn load_sky(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32, name: u32) -> i32 {
    let fmt = cstr(m, PATH_FORMAT);
    let nm = cstr(m, name);
    let model = format(&fmt, &[nm.clone(), cstr(m, EXT_MODEL)]);
    let textures = format(&fmt, &[nm, cstr(m, EXT_TEXTURES)]);
    let n = layer_count(m, sky);
    if (n as i32) >= 10 {
        return -1;
    }
    let l = layer(sky, n);
    if env.call_path(m, FILE_EXISTS, &model, &[]) == 0 {
        return -1;
    }
    let data = env.call_path(m, FILE_LOAD, &model, &[0]);
    let r = lcall(m, env, l, 0x30, &[data]);
    if r != 0 {
        return n as i32;
    }
    remap_textures(m, env, sky, l, &textures);
    set_layer_count(m, sky, n + 1);
    env.call(m, MODEL_RELEASE, &[l, data]);
    n as i32
}

/// `Course_LoadSkies` 0x185b18(sky, course): the course's row of the sky table (none: nothing); each of its skies
/// loaded by name and its layer noted by its id (+0xd0)
pub fn load_skies(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32, course: u32) {
    let mut row = 0u32;
    let mut k = 0u32;
    loop {
        let r = TABLE + 0x30 * k;
        let c = m.r32(r) as i32;
        if c < 0 {
            break;
        }
        if c as u32 == course {
            row = r;
            break;
        }
        k += 1;
    }
    if row == 0 {
        return;
    }
    let mut i = 0;
    while i < m.r32(row + 4) as i32 {
        let id = m.r32(row + 8 + 4 * i as u32);
        let name = m.r32(NAMES.wrapping_add(id.wrapping_mul(4)));
        let v = load_sky(m, env, sky, name);
        let id = m.r32(row + 8 + 4 * i as u32);
        m.w32(sky.wrapping_add(id.wrapping_mul(4)) + 0xd0, v as u32);
        i += 1;
    }
}

/// `Sky_Delete` 0x186078(sky, flags): each layer destroyed from the last (slot +8 with 0); deleted with flag 1
pub fn delete(m: &mut Mem, env: &mut dyn SkyEnv, sky: u32, flags: u32) {
    for i in (0..10).rev() {
        lcall(m, env, layer(sky, i), 8, &[0]);
    }
    if flags & 1 != 0 {
        env.call(m, DELETE, &[sky]);
    }
}

/// The sky object's static constructor 0x185f90: ten layers made (0x2555a8, then the layer vtable), the sky table's
/// rows counted, no layers by id
pub fn static_construct(m: &mut Mem, env: &mut dyn SkyEnv) {
    for i in 0..10 {
        let l = layer(SKY, i);
        env.call(m, MODEL_CONSTRUCT, &[l]);
        m.w32(l + 0x10, LAYER_VTABLE);
    }
    m.w32(SKY, 0);
    if m.r32(TABLE + 4) != u32::MAX {
        let mut n = 0u32;
        loop {
            n += 1;
            if m.r32(TABLE + 0x30 * n + 4) == u32::MAX {
                break;
            }
        }
        m.w32(SKY, n);
    }
    for k in (0..12).rev() {
        m.w32(SKY + 0xd0 + 4 * k, u32::MAX);
    }
}

/// `cSkyBoxObjNode`'s constructor 0x1c7900(this, a1): the base object, the vtable; the course's sky (`<course
/// name>_sky`, the course info of the game's course) found among the sky names (`stricmp`, case ignored; −1 none) and its model kept (+0xc)
pub fn node_construct(m: &mut Mem, env: &mut dyn SkyEnv, this: u32, a1: u32) -> u32 {
    env.call(m, BASE_CONSTRUCT, &[this, a1]);
    m.w32(this + 8, NODE_VTABLE);
    let app = m.r32(GAPP);
    let game = m.r32(app + 0x730);
    let info = env.call(m, COURSE_INFO, &[app + 0x64, m.r32(game + 0x10)]);
    let want = format(&cstr(m, SKY_FORMAT), &[cstr(m, info)]);
    let id = (0..12u32).find(|&i| cstr(m, m.r32(NAMES + 4 * i)).eq_ignore_ascii_case(&want)).map_or(u32::MAX, |i| i);
    let model = model_by_index(m, env, SKY, id);
    m.w32(this + 0xc, model);
    this
}

/// `cSkyBoxObjNode`'s destructor 0x1c79d0: the vtable, then the base's
pub fn node_destruct(m: &mut Mem, env: &mut dyn SkyEnv, this: u32, a1: u32) {
    m.w32(this + 8, NODE_VTABLE);
    env.call(m, BASE_DESTRUCT, &[this, a1]);
}

/// `cSkyBoxObjNode::Draw` 0x1c79e8: with a model, drawn at the camera: the matrix at 0x346440 with its last row the
/// camera's position; the lights (`Lights_Build`) an ambient (1, 1.99, 1.99, 1.99) and three dark lights; the renderer
/// state saved (+0x1d8) and set (+0x1d0 0, +0x1f8 (1, 0), +0x1c0 1, +0x1c8 (2, 0, 0.08)); the model drawn (+0x2a8);
/// the state put back (+0x1c8 (0, 2, 0.08), +0x1c0 0, +0x1f8 (0, 0), +0x1d0 the saved one)
pub fn node_draw(m: &mut Mem, env: &mut dyn SkyEnv, this: u32) {
    let model = m.r32(this + 0xc);
    if model == 0 {
        return;
    }
    let app = m.r32(GAPP);
    let r = m.r32(app + 0x724);
    let game = m.r32(app + 0x730);
    let mut mat = [0u32; 16];
    for k in 0..12 {
        mat[k] = m.r32(0x346440 + 4 * k as u32);
    }
    let cam = game.wrapping_add(m.r32(game + 0x290) << 7) + 0xb0;
    for k in 0..4 {
        mat[12 + k] = m.r32(cam + 4 * k as u32);
    }
    let big = f32::from_bits(0x3ffe_f9db).to_bits();
    let ambient = [1f32.to_bits(), big, big, big];
    let lights = env.lights_build(m, ambient, [[0; 4]; 6]);
    let fog = f32::from_bits(0x3da3_d70a).to_bits();
    let rc = |m: &mut Mem, env: &mut dyn SkyEnv, off: u32, args: &[u32]| {
        let (t, f) = vfn(m, r, off);
        env.vcall(m, f, t, args)
    };
    let saved = rc(m, env, 0x1d8, &[]);
    rc(m, env, 0x1d0, &[0]);
    rc(m, env, 0x1f8, &[1, 0]);
    rc(m, env, 0x1c0, &[1]);
    rc(m, env, 0x1c8, &[2, 0, fog]);
    let (t, f) = vfn(m, r, 0x2a8);
    let part = m.r32(model + 0xc);
    env.draw_model(m, f, t, model, part, mat, &lights);
    rc(m, env, 0x1c8, &[0, 2, fog]);
    rc(m, env, 0x1c0, &[0]);
    rc(m, env, 0x1f8, &[0, 0]);
    rc(m, env, 0x1d0, &[saved]);
}

/// The sky box's static set-up 0x1c7c58: the light colours and directions at 0x3d0d20..0x3d0d6c
pub fn node_static_init(m: &mut Mem) {
    let one = 1f32.to_bits();
    m.w32(0x3d0d50 + 8, 0);
    let f5 = m.r32(0x3d0d50 + 8);
    m.w32(0x3d0d60 + 8, 0x3b03_126f);
    m.w32(0x3d0d20 + 0x18, one);
    m.w32(0x3d0d60, one);
    m.w32(0x3d0d60 + 4, 0x3d9b_3d08);
    m.w32(0x3d0d50, one);
    m.w32(0x3d0d50 + 4, 0x3ef2_fec5);
    m.w32(0x3d0d40, one);
    m.w32(0x3d0d40 + 4, 0x3f76_e979);
    m.w32(0x3d0d40 + 8, 0);
    m.w32(0x3d0d20, u32::MAX);
    m.w32(0x3d0d20 + 0x10, f5);
    m.w32(0x3d0d20 + 4, one);
    m.w32(0x3d0d20 + 8, one);
    m.w32(0x3d0d20 + 0xc, f5);
    m.w32(0x3d0d20 + 0x14, one);
}
