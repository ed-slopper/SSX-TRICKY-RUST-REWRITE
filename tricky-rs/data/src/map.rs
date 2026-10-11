//! A course's `.map` file (`data/models/<course>.map`, row E3a2): the level tools' listing of every model, patch,
//! instance, spline, light, material and so on with its name, its number (UID), how often it is used and its name's
//! hash. The game never reads it (no `.map` name is in the ELF); the port uses it for the names. Text: sections
//! `### <NAME> BEGIN` … `### <NAME> END`, one entry a line (the name, then the numbers; instances also SCROLL).

/// One line of a section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub uid: i64,
    pub refs: i64,
    pub hash: u32,
    /// the instance sections' SCROLL column, when present
    pub scroll: Option<String>,
}

/// The sections, in the file's order (MODELS, PARTICLE MODELS, PATCHES, INTERNAL INSTANCES, PLAYER STARTS, PARTICLE
/// INSTANCES, SPLINES, LIGHTS, MATERIALS, CONTEXT BLOCKS, CAMERAS, TEXTURES, LIGHTMAPS).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Map(pub Vec<(String, Vec<Entry>)>);

impl Map {
    pub fn parse(text: &str) -> Map {
        let mut out: Vec<(String, Vec<Entry>)> = vec![];
        let mut cur: Option<(String, Vec<Entry>)> = None;
        for line in text.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("###") {
                let rest = rest.trim();
                if let Some(name) = rest.strip_suffix(" BEGIN") {
                    cur = Some((name.trim().to_string(), vec![]));
                } else if rest.ends_with(" END") {
                    if let Some(c) = cur.take() {
                        out.push(c);
                    }
                }
                continue;
            }
            let Some((_, entries)) = cur.as_mut() else { continue };
            let mut it = t.split_whitespace();
            let (Some(name), Some(uid), Some(refs), Some(hash)) = (it.next(), it.next(), it.next(), it.next()) else { continue };
            let (Ok(uid), Ok(refs), Ok(hash)) = (uid.parse(), refs.parse(), u32::from_str_radix(hash, 16)) else { continue };
            entries.push(Entry { name: name.to_string(), uid, refs, hash, scroll: it.next().map(str::to_string) });
        }
        Map(out)
    }

    /// A section's entries by its name (`"PATCHES"`, `"INTERNAL INSTANCES"`, …).
    pub fn section(&self, name: &str) -> &[Entry] {
        self.0.iter().find(|(n, _)| n == name).map(|(_, e)| e.as_slice()).unwrap_or(&[])
    }

    /// The name of entry `uid` of a section.
    pub fn name(&self, section: &str, uid: usize) -> Option<&str> {
        self.section(section).iter().find(|e| e.uid == uid as i64).map(|e| e.name.as_str())
    }
}
