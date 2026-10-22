use std::collections::HashMap;
use zbus::fdo;
use zbus::object_server::SignalEmitter;

type Paths = HashMap<String, String>;

macro_rules! stub {
    ($ty:ident $iface:tt {
        $($method:ident($($arg:ident: $t:ty),*) -> $ret:tt $([$out:tt])?;)*
        $(@signal $signal:ident($($sarg:ident: $st:ty),*);)*
    }) => {
        pub struct $ty;

        #[allow(dead_code)]
        #[zbus::interface(name = $iface)]
        impl $ty {
            $(
                $(#[zbus(out_args($out))])?
                async fn $method(&self, $($arg: $t),*) -> fdo::Result<$ret> {
                    let _ = ($($arg,)*);
                    Err(fdo::Error::NotSupported(format!("{} is not supported by this version of wprsd", $iface)))
                }
            )*
            $(
                #[zbus(signal)]
                pub async fn $signal(emitter: &SignalEmitter<'_>, $($sarg: $st),*) -> zbus::Result<()>;
            )*
        }
    };
}

stub!(Providers "io.github.wallpaper_rs.Providers1" {
    list() -> String ["json"];
    add(id: String, kind: String, options: HashMap<String, String>, secret: String) -> String ["source"];
    set(id: String, key: String, value: String) -> ();
    remove(id: String, purge: bool) -> ();
    sync(id: String) -> u64 ["job"];
    set_secret(id: String, secret: String) -> ();
    forget_secret(id: String) -> ();
});

stub!(Repos "io.github.wallpaper_rs.Repos1" {
    list() -> String ["json"];
    add(url: String) -> u64 ["job"];
    refresh(repo: String) -> u64 ["job"];
    remove(repo: String, uninstall: bool) -> ();
    browse(query: String, offset: u32, limit: u32) -> String ["json"];
    request_previews(specs: Vec<String>) -> Paths ["paths"];
    install(spec: String, allow_stale: bool, allow_downgrade: bool) -> u64 ["job"];
    uninstall(id: String, version: String) -> ();
    list_installed() -> String ["json"];
    @signal preview_ready(spec: &str, path: &str);
});
