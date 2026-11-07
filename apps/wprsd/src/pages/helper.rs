use std::path::PathBuf;
use std::sync::OnceLock;

const NAME: &str = "wprs-web";

static PROBED: OnceLock<Result<String, String>> = OnceLock::new();

pub fn helper() -> Option<PathBuf> {
    let given = std::env::var_os("WPRS_WEB").map(PathBuf::from);
    let beside = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join(NAME)));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let searched = std::env::split_paths(&path).map(|dir| dir.join(NAME));
    given.into_iter().chain(beside).chain(searched).find(|p| p.is_file())
}

fn probe() -> Result<String, String> {
    let Some(path) = helper() else { return Err(crate::words::web_missing()) };
    let output = std::process::Command::new(&path).arg("--check").stdin(std::process::Stdio::null()).output();
    let output = output.map_err(|e| crate::words::web_broken(&e.to_string()))?;
    let said = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    match output.status.success() {
        true => Ok(said),
        false => Err(refusal(&said, crate::flatpak::app_id().is_some())),
    }
}

fn refusal(said: &str, flatpak: bool) -> String {
    match said {
        s if s.contains("libWPEWebKit") => crate::words::web_engine(flatpak),
        s if s.contains("older than") => crate::words::web_old(s),
        s => crate::words::web_broken(s),
    }
}

pub fn available() -> Result<String, String> {
    PROBED.get_or_init(probe).clone()
}

pub fn unavailable() -> Option<String> {
    available().err()
}

#[cfg(test)]
mod tests {
    use super::refusal;

    #[test]
    fn a_missing_engine_is_explained_for_the_flatpak_and_for_the_rest() {
        let missing = "libWPEWebKit-2.0.so.1: cannot open shared object file";
        let (flatpak, other) = (refusal(missing, true), refusal(missing, false));
        assert!(flatpak.contains("Flatpak") && flatpak.contains("WPE WebKit"), "{flatpak}");
        assert!(other.contains("Ubuntu 24.04") && other.contains("WPE WebKit"), "{other}");
        assert!(refusal("WPE WebKit 2.52 is older than 2.54", false).contains("2.52"));
        assert!(refusal("boom", true).contains("boom"));
    }
}
