//! The trick book (`Score_TrickBookCheck` 0x157a00, `TrickBook_*`): six chapters of five tricks per
//! rider, read from `chars/trickbook.json` (decoded from the disc's DATA/TUTORIAL/TRICKDEF.DAT). Only
//! the first unfinished chapter counts; a landed trick ticks off an entry when its spin, flips and
//! grabs are the same (the original ignores the spin's direction, a switch takeoff and how it lands).
//! Chapters 1-5 each earn an outfit; the whole book earns the rider's UBERBOARD.

use bevy::prelude::*;
use std::collections::HashMap;

/// Each rider's 30 trick names, in book order.
#[derive(Resource, Default)]
pub struct Book { pub tricks: HashMap<String, Vec<String>>, pub done: HashMap<String, u32> }

fn save_path() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default().join("tricky-book.json")
}

/// The part of a trick name the book compares: no FS / BS, no Switch / Late / Rail To prefix,
/// no "Air" / "To Fakie" / "To Rail" ending, no points.
pub fn key(name: &str) -> String {
    let name = name.split("  +").next().unwrap_or(name);
    let mut w: Vec<&str> = name.split_whitespace().filter(|t| *t != "FS" && *t != "BS").collect();
    while matches!(w.first(), Some(&"Late") | Some(&"Switch")) { w.remove(0); }
    if w.len() >= 2 && w[0] == "Rail" && w[1] == "To" { w.drain(0..2); }
    loop {
        let n = w.len();
        if n >= 1 && w[n - 1] == "Air" { w.pop(); continue; }
        if n >= 2 && w[n - 2] == "To" && (w[n - 1] == "Fakie" || w[n - 1] == "Rail") { w.truncate(n - 2); continue; }
        break;
    }
    w.join(" ").to_ascii_lowercase()
}

impl Book {
    pub fn load(chars_dir: &std::path::Path) -> Self {
        let mut b = Book::default();
        if let Ok(text) = std::fs::read_to_string(chars_dir.join("trickbook.json")) {
            if let Ok(v) = serde_json::from_str::<HashMap<String, Vec<serde_json::Value>>>(&text) {
                for (set, list) in v {
                    // "set0_Eddie" -> "eddie"
                    let who = set.split('_').nth(1).unwrap_or(&set).to_ascii_lowercase();
                    b.tricks.insert(who, list.iter().filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from)).collect());
                }
            }
        }
        b.done = std::fs::read_to_string(save_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        b
    }
    fn save(&self) { if let Ok(s) = serde_json::to_string_pretty(&self.done) { let _ = std::fs::write(save_path(), s); } }
    pub fn bits(&self, who: &str) -> u32 { self.done.get(&who.to_ascii_lowercase()).copied().unwrap_or(0) }
    /// The first chapter not yet finished (0..6), 6 when the book is done.
    pub fn chapter(&self, who: &str) -> usize { let b = self.bits(who); (0..6).find(|c| (b >> (c * 5)) & 31 != 31).unwrap_or(6) }
    pub fn complete(&self, who: &str) -> bool { self.chapter(who) == 6 }
    /// A landed trick: returns the book entry it ticked off and whether that finished the chapter.
    pub fn check(&mut self, who: &str, trick: &str) -> Option<(String, Option<usize>)> {
        let who = who.to_ascii_lowercase();
        let list = self.tricks.get(&who)?;
        let c = self.chapter(&who);
        if c >= 6 { return None; }
        let k = key(trick);
        let bits = self.bits(&who);
        let t = (c * 5..c * 5 + 5).find(|t| bits & (1 << t) == 0 && list.get(*t).is_some_and(|n| key(n) == k))?;
        let name = list[t].clone();
        let bits = bits | (1 << t);
        self.done.insert(who.clone(), bits);
        self.save();
        let finished = ((bits >> (c * 5)) & 31 == 31).then_some(c + 1);
        Some((name, finished))
    }
    /// The menu's line: the chapter, how far through it, and the next trick to do.
    pub fn summary(&self, who: &str) -> String {
        let who = who.to_ascii_lowercase();
        let Some(list) = self.tricks.get(&who) else { return String::new() };
        let c = self.chapter(&who);
        if c >= 6 { return "trick book complete - UBERBOARD".into(); }
        let bits = self.bits(&who);
        let n = (0..5).filter(|i| bits & (1 << (c * 5 + i)) != 0).count();
        let next = (c * 5..c * 5 + 5).find(|t| bits & (1 << t) == 0).and_then(|t| list.get(t)).cloned().unwrap_or_default();
        format!("trick book chapter {}: {n}/5   next: {next}", c + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::key;
    #[test]
    fn keys() {
        assert_eq!(key("BS 360 Mute"), key("FS 360 Mute  +470"));
        assert_eq!(key("Indy Air"), key("Indy"));
        assert_eq!(key("BS 900 Tail WAG To Fakie"), key("Switch FS 900 Tail WAG"));
    }
}
