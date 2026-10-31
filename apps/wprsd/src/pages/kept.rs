use std::path::Path;

const SLACK: u64 = 1 << 20;
const MAX_ENTRIES: usize = 100_000;

pub fn within(dir: &Path, kib: u32) -> bool {
    let allowed = u64::from(kib) * 2048 + SLACK;
    let (mut used, mut seen, mut todo) = (0u64, 0usize, vec![dir.join("data")]);
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            seen += 1;
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => todo.push(entry.path()),
                Ok(meta) => used += meta.len(),
                Err(_) => {}
            }
            if used > allowed || seen > MAX_ENTRIES {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn kept_data_stays_near_what_the_page_declared() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data/origin");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("small"), vec![0; 100 << 10]).unwrap();
        assert!(super::within(dir.path(), 64));
        std::fs::write(data.join("large"), vec![0; 2 << 20]).unwrap();
        assert!(!super::within(dir.path(), 64));
        assert!(super::within(dir.path(), 2048));
    }
}
