//! A first track editor, used from the free camera (Tab): pick an object, move, turn, scale,
//! copy or remove it, then save. It edits the level project's `Instances.json` in place (the
//! first save keeps the original as `Instances.json.bak`) and reloads the track, so the change
//! can be ridden straight away. `ssxlevel build` turns the same project back into a .BIG.

use crate::{FlyCam, LevelRes, Mode, World};
use bevy::prelude::*;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Resource, Default)]
pub struct Editor { pub on: bool, dir: PathBuf, doc: Option<Value>, picked: Option<usize>, changes: u32 }
/// Set to load the current level again from disk.
#[derive(Resource, Default)]
pub struct Reload(pub bool);
#[derive(Component)]
pub struct EditorText;

pub fn setup_editor(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        TextShadow { offset: Vec2::splat(1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.9) },
        Node { position_type: PositionType::Absolute, bottom: Val::Px(30.0), left: Val::Px(10.0), ..default() },
        EditorText,
    ));
}

fn g2b(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(2)?.as_f64()? as f32, -(a.get(1)?.as_f64()? as f32)) * 0.01)
}
fn b2g(p: Vec3) -> Value { json!([p.x * 100.0, -p.z * 100.0, p.y * 100.0]) }

pub fn editor(
    time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mode: Res<Mode>, level: Res<LevelRes>, world: Res<World>,
    cam: Single<&Transform, With<FlyCam>>, mut ed: ResMut<Editor>, mut reload: ResMut<Reload>, mut gizmos: Gizmos,
    mut text: Single<&mut Text, With<EditorText>>, mut frame: Local<u32>,
) {
    // TRICKY_EDITTEST plays a short script of key presses (for checking the editor without a keyboard)
    *frame += 1;
    let script = if std::env::var("TRICKY_EDITTEST").is_ok() { match *frame { 3 => Some(KeyCode::F4), 5 => Some(KeyCode::KeyF), 8 => Some(KeyCode::KeyC), 9 => Some(KeyCode::KeyX), 10 => Some(KeyCode::Period), 12 => Some(KeyCode::Enter), _ => None } } else { None };
    let press = |k: KeyCode| keys.just_pressed(k) || script == Some(k);
    if *mode != Mode::Fly { if !text.0.is_empty() { text.0.clear(); } return; }
    if press(KeyCode::F4) { ed.on = !ed.on; }
    if !ed.on {
        let hint = "F4: track editor";
        if text.0 != hint { text.0 = hint.into(); }
        return;
    }
    // (re)open the project's instance list when the editor starts or the track changes
    if ed.doc.is_none() || ed.dir != level.0.dir {
        ed.dir = level.0.dir.clone();
        ed.doc = std::fs::read_to_string(ed.dir.join("Instances.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
        ed.picked = None;
        ed.changes = 0;
    }
    let ed = &mut *ed;
    let Some(list) = ed.doc.as_mut().and_then(|d| d.get_mut("Instances")).and_then(|i| i.as_array_mut()) else {
        text.0 = "editor: could not read Instances.json".into();
        return;
    };

    // where the centre of the view meets the world
    let from = cam.translation;
    let dir = *cam.forward();
    let aim = world.0.raycast(from, from + dir * 600.0).map(|d| from + dir * d);
    if let Some(p) = aim {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] { gizmos.line(p - axis * 0.4, p + axis * 0.4, Color::WHITE); }
    }

    // F: pick the object nearest the centre of the view
    if press(KeyCode::KeyF) {
        let mut best: Option<(usize, f32)> = None;
        for (i, inst) in list.iter().enumerate() {
            if inst.get("Visable").and_then(|v| v.as_bool()) == Some(false) { continue; }
            let Some(p) = inst.get("Location").and_then(g2b) else { continue };
            let t = (p - from).dot(dir);
            if t < 1.0 || t > 400.0 { continue; }
            let off = (p - (from + dir * t)).length();
            if off > 1.5 + 0.03 * t { continue; }
            let score = off / (1.0 + 0.03 * t) + t * 0.002;
            if best.is_none_or(|b| score < b.1) { best = Some((i, score)); }
        }
        ed.picked = best.map(|b| b.0);
        if script.is_some() { println!("editor test: picked {:?} of {} objects, aim {:?}", ed.picked, list.len(), aim); }
    }

    let mut edited = false;
    if let Some(i) = ed.picked {
        let Some(mut pos) = list[i].get("Location").and_then(g2b) else { return };
        let before = pos;
        // T: bring it to the aim point. Arrows slide it, Y / H raise and lower it.
        if press(KeyCode::KeyT) { if let Some(p) = aim { pos = p; } }
        let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::NEG_Z);
        let right = Vec3::new(-flat.z, 0.0, flat.x);
        let step = time.delta_secs() * if keys.pressed(KeyCode::ShiftLeft) { 12.0 } else { 2.5 };
        let held = |k: KeyCode| keys.pressed(k) as i32 as f32;
        pos += flat * (held(KeyCode::ArrowUp) - held(KeyCode::ArrowDown)) * step;
        pos += right * (held(KeyCode::ArrowRight) - held(KeyCode::ArrowLeft)) * step;
        pos.y += (held(KeyCode::KeyY) - held(KeyCode::KeyH)) * step;
        if pos != before { list[i]["Location"] = b2g(pos); edited = true; }
        // Z / X: turn a twelfth of a turn about the vertical (game space is Z-up)
        let turn = press(KeyCode::KeyX) as i32 - press(KeyCode::KeyZ) as i32;
        if turn != 0 {
            let q = list[i].get("Rotation").and_then(|r| r.as_array()).map(|a| a.iter().filter_map(|v| v.as_f64()).map(|v| v as f32).collect::<Vec<_>>()).filter(|a| a.len() == 4);
            if let Some(a) = q {
                let q = Quat::from_rotation_z(turn as f32 * std::f32::consts::TAU / 12.0) * Quat::from_xyzw(a[0], a[1], a[2], a[3]);
                list[i]["Rotation"] = json!([q.x, q.y, q.z, q.w]);
                edited = true;
            }
        }
        // , and . : smaller and bigger
        let grow = press(KeyCode::Period) as i32 - press(KeyCode::Comma) as i32;
        if grow != 0 {
            if let Some(s) = list[i].get_mut("Scale").and_then(|s| s.as_array_mut()) {
                for v in s.iter_mut() { if let Some(f) = v.as_f64() { *v = json!(f * if grow > 0 { 1.1 } else { 1.0 / 1.1 }); } }
                edited = true;
            }
        }
        // Delete: take it out of the track (hidden and not solid; the list keeps its order,
        // because the game's effects refer to objects by their place in it)
        if press(KeyCode::Delete) {
            list[i]["Visable"] = json!(false);
            list[i]["PlayerCollision"] = json!(false);
            ed.picked = None;
            edited = true;
        }
        // C: a copy at the aim point
        if press(KeyCode::KeyC) {
            let mut copy = list[i].clone();
            if let Some(p) = aim { copy["Location"] = b2g(p); }
            let name = copy.get("InstanceName").and_then(|n| n.as_str()).unwrap_or("Object").to_string();
            copy["InstanceName"] = json!(format!("{name}_copy{}", list.len()));
            list.push(copy);
            ed.picked = Some(list.len() - 1);
            edited = true;
        }
        // show what is picked
        gizmos.line(pos, pos + Vec3::Y * 8.0, Color::srgb(1.0, 0.9, 0.1));
        for axis in [Vec3::X, Vec3::Z] { gizmos.line(pos - axis * 1.5, pos + axis * 1.5, Color::srgb(1.0, 0.9, 0.1)); }
    }
    if edited { ed.changes += 1; }

    // Enter: write the project and load the track again so the change is real
    let mut saved = "";
    if press(KeyCode::Enter) && ed.changes > 0 {
        let path = ed.dir.join("Instances.json");
        let bak = ed.dir.join("Instances.json.bak");
        if !bak.exists() { let _ = std::fs::copy(&path, &bak); }
        match ed.doc.as_ref().and_then(|d| serde_json::to_string_pretty(d).ok()).map(|s| std::fs::write(&path, s)) {
            Some(Ok(())) => { ed.changes = 0; reload.0 = true; saved = "   saved, reloading..."; }
            _ => saved = "   COULD NOT SAVE",
        }
    }
    let name = ed.picked.and_then(|i| ed.doc.as_ref()?.get("Instances")?.get(i)?.get("InstanceName")?.as_str().map(|s| format!("#{i} {s}"))).unwrap_or_else(|| "nothing picked".into());
    let new = format!("TRACK EDITOR   {name}   {} unsaved{saved}\nF pick at the cross   T move to the cross   arrows slide   Y / H up / down   Z / X turn   , . size   C copy   Delete remove   Enter save and reload   F4 off",
        if ed.changes > 0 { "changes" } else { "nothing" });
    if text.0 != new { text.0 = new; }
}
