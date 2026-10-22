use crate::args::{Out, fail, fixture, invalid, next};
use crate::bus::Daemon;
use serde_json::Value;
use std::collections::HashMap;
use wprs_client::Error;

macro_rules! calls {
    ($($iface:ident $proxy:ident { $($member:ident $method:ident($($arg:ident: $sig:tt),*) $($json:ident)?;)* })*) => {
        #[cfg(test)]
        pub const CALLS: &[(&str, &str, &[&str])] =
            &[$($((stringify!($iface), stringify!($member), &[$(stringify!($arg)),*]),)*)*];

        pub async fn dispatch(daemon: &Daemon, iface: &str, member: &str, args: Vec<Value>) -> Result<String, Error> {
            let mut args = args.into_iter();
            match (iface, member) {
                $($((stringify!($iface), stringify!($member)) => {
                    $(let $arg: arg!($sig) = next(&mut args, stringify!($arg))?;)*
                    if args.len() > 0 {
                        return Err(invalid(format!("{member} takes fewer arguments")));
                    }
                    match daemon {
                        Daemon::Bus(c) => reply!($($json)? c.$proxy.$method($(pass!($sig, $arg)),*).await),
                        Daemon::Fixtures => Ok(fixture(iface, member)),
                    }
                })*)*
                _ => Err(invalid(format!("unknown method {iface}.{member}"))),
            }
        }
    };
}

calls! {
    Daemon1 daemon {
        SetWallpaper set_wallpaper(path: str, output: str, fit: str);
        SetBackground set_background(color: str, output: str);
        SetOption set_option(key: str, value: str);
        SetPaused set_paused(paused: bool);
        SetVolume set_volume(volume: f64);
        SetMuted set_muted(muted: bool);
        PreviewEdit preview_edit(path: str, edit_json: str);
        Reload reload();
        GetState get_state() json;
        GetSchema get_schema() json;
        ListJobs list_jobs() json;
        GetJob get_job(job: u64) json;
        CancelJob cancel_job(job: u64);
    }
    Library1 library {
        AddSource add_source(path: str, recursive: bool);
        RemoveSource remove_source(source: str);
        ListSources list_sources() json;
        Rescan rescan(source: str);
        Import import(paths: strs, copy: bool);
        Remove remove(uids: strs, delete_files: bool);
        Search search(query: str, offset: u32, limit: u32) json;
        GetItem get_item(uid: str) json;
        SetTitle set_title(uid: str, title: str);
        SetRating set_rating(uids: strs, rating: u32);
        SetFavorite set_favorite(uids: strs, favorite: bool);
        SetTags set_tags(uids: strs, add: strs, remove: strs);
        ListTags list_tags() json;
        RenameTag rename_tag(from: str, to: str);
        Thumbnail thumbnail(uid: str, size: u32);
        RequestThumbnails request_thumbnails(uids: strs, size: u32);
        Apply apply(uid: str, output: str, fit: str, preset: str);
    }
    Playlists1 playlists {
        List list() json;
        Get get(playlist: str, offset: u32, limit: u32) json;
        Create create(name: str, query: str);
        Rename rename(playlist: str, name: str);
        SetQuery set_query(playlist: str, query: str);
        Delete delete(playlist: str);
        AddItems add_items(playlist: str, uids: strs);
        RemoveItems remove_items(playlist: str, uids: strs);
        RemoveEntries remove_entries(playlist: str, indexes: u32s, expected: strs);
        MoveItem move_item(playlist: str, from: u32, to: u32, expected: str);
        SetRule set_rule(playlist: str, key: str, value: str);
        Start start(playlist: str, output: str);
        Stop stop(output: str);
        Next next(output: str);
        Previous previous(output: str);
        History history(output: str, limit: u32) json;
    }
    Profiles1 profiles {
        List list() json;
        Get get(profile: str) json;
        Save save(name: str, with_options: bool);
        Activate activate(profile: str);
        Rename rename(profile: str, name: str);
        Delete delete(profile: str);
        SetAuto set_auto(profile: str, key: str, value: str);
        ListPresets list_presets(item: str) json;
        SavePreset save_preset(item: str, name: str, values: map);
        DeletePreset delete_preset(preset: str);
    }
    Providers1 providers {
        List list() json;
        Add add(id: str, kind: str, options: map, secret: str);
        Set set(id: str, key: str, value: str);
        Remove remove(id: str, purge: bool);
        Sync sync(id: str);
        SetSecret set_secret(id: str, secret: str);
        ForgetSecret forget_secret(id: str);
    }
    Repos1 repos {
        List list() json;
        Add add(url: str);
        Refresh refresh(repo: str);
        Remove remove(repo: str, uninstall: bool);
        Browse browse(query: str, offset: u32, limit: u32) json;
        RequestPreviews request_previews(specs: strs);
        Install install(spec: str, allow_stale: bool, allow_downgrade: bool);
        Uninstall uninstall(id: str, version: str);
        ListInstalled list_installed() json;
    }
}
