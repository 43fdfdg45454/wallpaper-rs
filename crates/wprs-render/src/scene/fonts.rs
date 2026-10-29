use ab_glyph::{FontArc, FontVec};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use wprs_scene::Scene;

pub const MAX_FONT: usize = 32 << 20;

pub(crate) struct Fonts {
    packaged: HashMap<String, FontArc>,
    system: HashMap<String, Option<FontArc>>,
}

fn database() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let has =
            |db: &fontdb::Database, name: &str| db.faces().any(|f| f.families.iter().any(|(n, _)| n == name));
        let pick =
            |db: &fontdb::Database, names: &[&str]| names.iter().find(|n| has(db, n)).map(|n| n.to_string());
        if let Some(name) = pick(&db, &["DejaVu Sans", "Noto Sans", "Roboto", "Liberation Sans", "Cantarell"])
        {
            db.set_sans_serif_family(name);
        }
        if let Some(name) = pick(&db, &["DejaVu Serif", "Noto Serif", "Liberation Serif", "Droid Serif"]) {
            db.set_serif_family(name);
        }
        if let Some(name) =
            pick(&db, &["DejaVu Sans Mono", "Noto Sans Mono", "Liberation Mono", "Droid Sans Mono"])
        {
            db.set_monospace_family(name);
        }
        db
    })
}

fn system(name: &str) -> Option<FontArc> {
    let db = database();
    let family = match name {
        "" | "sans-serif" => fontdb::Family::SansSerif,
        "serif" => fontdb::Family::Serif,
        "monospace" => fontdb::Family::Monospace,
        name => fontdb::Family::Name(name),
    };
    let query = fontdb::Query { families: &[family, fontdb::Family::SansSerif], ..Default::default() };
    let id = db.query(&query).or_else(|| db.faces().next().map(|f| f.id))?;
    db.with_face_data(id, |data, index| FontVec::try_from_vec_and_index(data.to_vec(), index).ok())?
        .map(FontArc::new)
}

pub fn font(data: &[u8], face: u32) -> Option<FontArc> {
    let packed = data.starts_with(b"wOF2");
    let declared = data.get(16..20).map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    if data.len() > MAX_FONT || (packed && (declared > MAX_FONT || !super::woff2::fits(data))) {
        return None;
    }
    let data = match packed {
        true => {
            std::panic::catch_unwind(|| woff2_patched::convert_woff2_to_ttf(&mut &data[..])).ok()?.ok()?
        }
        false => data.to_vec(),
    };
    FontVec::try_from_vec_and_index(data, face).ok().map(FontArc::new)
}

impl Fonts {
    pub(crate) fn new(scene: &Scene, files: &HashMap<String, Arc<Vec<u8>>>) -> Fonts {
        let load = |(id, data): (&String, &Arc<Vec<u8>>)| {
            let face = scene.assets.get(id).and_then(|a| a.face).unwrap_or(0);
            let found = font(data, face);
            if found.is_none() {
                tracing::warn!(font = id, "the font cannot be read and the text uses another one");
            }
            Some((id.clone(), found?))
        };
        Fonts { packaged: files.iter().filter_map(load).collect(), system: HashMap::new() }
    }

    fn one(&mut self, name: &str) -> Option<FontArc> {
        if let Some(font) = self.packaged.get(name) {
            return Some(font.clone());
        }
        self.system.entry(name.to_owned()).or_insert_with(|| system(name)).clone()
    }

    pub(crate) fn chain(&mut self, font: Option<&str>, fallback: &[String]) -> Vec<FontArc> {
        let names = std::iter::once(font.unwrap_or_default()).chain(fallback.iter().map(String::as_str));
        let mut found: Vec<FontArc> = names.filter_map(|n| self.one(n)).collect();
        found.extend(self.one(""));
        found
    }
}
