use crate::{Cache, Item, Media, Provider, Result, invalid};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalFolder {
    pub root: PathBuf,
}

impl LocalFolder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait::async_trait]
impl Provider for LocalFolder {
    async fn list(&self) -> Result<Vec<Item>> {
        let root = self.root.clone();
        let scan = move || {
            let mut items = Vec::new();
            scan(&root, &mut items)?;
            items.sort_by(|a, b| a.url.cmp(&b.url));
            Ok(items)
        };
        tokio::task::spawn_blocking(scan).await.map_err(invalid)?
    }

    async fn fetch(&self, item: &Item, _cache: &Cache) -> Result<PathBuf> {
        let path = PathBuf::from(&item.url);
        tokio::fs::metadata(&path).await?;
        Ok(path)
    }
}

fn scan(dir: &Path, items: &mut Vec<Item>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = path.to_str().filter(|_| !file.starts_with('.')) else { continue };
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = scan(&path, items);
        } else if let Some((media, _)) = Media::detect(name)
            && path.is_file()
        {
            let (id, url) = (format!("local-{name}"), name.to_string());
            items.push(Item { id, title: file, url, attribution: None, license: None, media });
        }
    }
    Ok(())
}
