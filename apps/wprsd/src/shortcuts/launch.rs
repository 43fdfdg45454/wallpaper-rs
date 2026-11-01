use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use zbus::Variant as Value;

const CODES: &str = "fFuUdDnNickvm";

pub(crate) fn valid_id(id: &str) -> bool {
    let allowed = |c: char| c.is_ascii_alphanumeric() || "._-".contains(c);
    (1..=255).contains(&id.len()) && !id.starts_with(['.', '-']) && id.chars().all(allowed)
}

fn dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"));
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or(home);
    let shared = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data.into_iter().chain(shared.split(':').filter(|d| d.starts_with('/')).map(PathBuf::from)).collect()
}

pub(crate) fn entry(text: &str) -> HashMap<&str, &str> {
    let mut group = "";
    let mut found = HashMap::new();
    for line in text.lines().map(str::trim) {
        match line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            Some(name) => group = name,
            None if group == "Desktop Entry" => {
                if let Some((key, value)) = line.split_once('=') {
                    found.entry(key.trim()).or_insert(value.trim());
                }
            }
            None => {}
        }
    }
    found
}

pub(crate) fn words(exec: &str) -> Vec<String> {
    let (mut out, mut word, mut quoted, mut chars) = (Vec::new(), None::<String>, false, exec.chars());
    while let Some(c) = chars.next() {
        match c {
            '"' => (quoted, word) = (!quoted, word.or_else(|| Some(String::new()))),
            '\\' if quoted => word.get_or_insert_default().extend(chars.next()),
            '%' => match chars.next() {
                Some('%') => word.get_or_insert_default().push('%'),
                Some(code) if CODES.contains(code) => {}
                _ => return Vec::new(),
            },
            c if c.is_whitespace() && !quoted => out.extend(word.take()),
            c => word.get_or_insert_default().push(c),
        }
    }
    out.extend(word);
    out
}

pub(super) async fn app(connection: &zbus::Connection, id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(crate::flatpak::app_id().is_none(), "applications open only outside the Flatpak");
    let name = format!("applications/{id}.desktop");
    let path = dirs().into_iter().map(|d| d.join(&name)).find(|p| p.is_file());
    let text =
        std::fs::read_to_string(path.ok_or_else(|| anyhow::anyhow!("no application {id} is installed"))?)?;
    let found = entry(&text);
    let yes = |key: &str| found.get(key) == Some(&"true");
    anyhow::ensure!(
        found.get("Type") == Some(&"Application") && !yes("Hidden"),
        "{id} is not an application"
    );
    anyhow::ensure!(!yes("Terminal"), "{id} runs in a terminal");
    if yes("DBusActivatable") {
        let object = format!("/{}", id.replace('.', "/").replace('-', "_"));
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        let iface = Some("org.freedesktop.Application");
        connection.call_method(Some(id), object.as_str(), iface, "Activate", &(options,)).await?;
        return Ok(());
    }
    let args = words(found.get("Exec").copied().unwrap_or_default());
    let (program, rest) = args.split_first().ok_or_else(|| anyhow::anyhow!("{id} names no program"))?;
    let mut child = Command::new(program);
    child.args(rest).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    std::os::unix::process::CommandExt::process_group(&mut child, 0);
    let mut child = child.spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}
