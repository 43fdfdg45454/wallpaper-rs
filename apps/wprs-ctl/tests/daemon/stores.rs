use super::*;

pub(super) struct Playlists(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Playlists1")]
impl Playlists {
    fn get(&self, playlist: String, offset: u32, limit: u32) -> String {
        self.0.push(format!("Get {playlist} {offset} {limit}"));
        let rules = json!({"order": "sequential", "interval": "30m", "at": [], "days": [], "synced": false,
            "resume": true, "advance_while_paused": false, "skip_videos_on_battery": false, "transition_ms": null});
        let info = json!({"uid": UIDS[4], "name": playlist, "query": null, "rules": rules, "count": 600,
            "outputs": []});
        let item = |i: u32| {
            json!({"uid": UIDS[i as usize % 5], "kind": "image", "title": "Sea", "width": null, "height": null,
            "duration_ms": null, "color": null, "tags": [], "rating": 0, "favorite": false, "missing": false,
            "error": null, "revoked": null, "removable": true, "imported": false, "thumb": null})
        };
        let items: Vec<_> = (offset..600.min(offset.saturating_add(limit))).map(item).collect();
        json!({"model": 1, "serial": 9, "playlist": info, "total": 600, "offset": offset, "items": items})
            .to_string()
    }

    fn remove_entries(&self, playlist: String, indexes: Vec<u32>, expected: Vec<String>) {
        self.0.push(format!("RemoveEntries {playlist} {indexes:?} {expected:?}"));
    }
}

pub(super) struct Profiles(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Profiles1")]
impl Profiles {
    fn list_presets(&self, item: String) -> String {
        self.0.push(format!("ListPresets {item}"));
        let preset = |uid: &str, name: &str| json!({"uid": uid, "item": item, "name": name, "value": {"fit": "fit", "background": null}});
        let presets = [preset(UIDS[3], "Dim: late"), preset(UIDS[4], "late")];
        json!({"model": 1, "serial": 9, "item": item, "presets": presets}).to_string()
    }

    fn delete_preset(&self, preset: String) {
        self.0.push(format!("DeletePreset {preset}"));
    }
}

pub(super) struct Inputs(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Inputs1")]
impl Inputs {
    fn get_inputs(&self) -> String {
        let shown = |key: &str, granted: &[&str]| {
            json!({"scene": "org.example.sea", "audio": true, "pointer": false, "clicks": true, "title": "Sea", "key": key,
                "granted": granted})
        };
        let shown = json!({"DP-1": shown("key:signer", &["audio"]), "DP-2": shown("sha256:other", &[])});
        json!({"model": 1, "serial": 9, "capturing": true, "tracking": false,
            "grants": {"org.example.sea": ["audio"]}, "shown": shown, "parallax": [], "declined": {},
            "clicks": ["DP-1"], "clicking": []})
        .to_string()
    }

    fn set_clicks(&self, output: String, on: bool) {
        self.0.push(format!("SetClicks [{output}] {on}"));
    }

    fn grant_package(&self, scene: String, key: String, input: String) -> Result<(), Failure> {
        self.0.push(format!("GrantPackage {scene} {key} {input}"));
        match key.as_str() {
            "sha256:other" => Err(Failure::Unconfirmed("the user is asked in a notification".into())),
            _ => Ok(()),
        }
    }
}

pub(super) struct Providers(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Providers1")]
impl Providers {
    fn add(&self, id: String, kind: String, options: HashMap<String, String>, secret: String) -> String {
        let options: BTreeMap<_, _> = options.into_iter().collect();
        self.0.push(format!("Add {id} {kind} {options:?} [{secret}]"));
        UIDS[0].into()
    }
}

pub(super) struct Repos(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Repos1")]
impl Repos {
    fn install(&self, spec: String, allow_stale: bool, allow_downgrade: bool) -> Result<u64, Failure> {
        self.0.push(format!("Install {spec} {allow_stale} {allow_downgrade}"));
        match allow_stale {
            true => Ok(5),
            false => Err(Failure::Stale("repository 01234567 is stale: its signed index has expired".into())),
        }
    }

    fn list_installed(&self) -> String {
        let package = |repo: &str, id: &str| {
            json!({"repo_id": repo, "id": id, "version": "1.0.0", "sha256": "ab", "size": 1, "signer": null,
            "revoked": null, "item": null, "update": "1.1.0"})
        };
        let packages = [
            package("0123456789abcdef", "org.example.dawn"),
            package("fedcba9876543210", "org.example.dusk"),
        ];
        json!({"model": 1, "serial": 9, "packages": packages}).to_string()
    }

    fn browse(&self, query: String, offset: u32, limit: u32) -> String {
        self.0.push(format!("Browse {query} {offset} {limit}"));
        let package = json!({"repo_id": "0123456789abcdef", "repo_name": "Example wallpapers", "id": query,
            "author": null, "version": "1.0.0", "kind": "image", "title": "Dawn", "description": "", "tags": [],
            "license": "CC0-1.0", "authors": [], "size": 1, "published": 0, "preview": false,
            "versions": ["1.0.0"], "installed": [], "update": false});
        json!({"model": 1, "serial": 9, "query": query, "total": 1, "offset": 0, "packages": [package]})
            .to_string()
    }
}
