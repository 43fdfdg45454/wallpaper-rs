use image::{GrayImage, Luma, RgbImage};
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use wprs_core::edit::Depth;
use wprs_depth::{Map, Places, beside, read, write};

const SHA: &str = "9f2c4e8eeda3a27f34701831b6a2ec7753d7b38779b215acb5633424703deed8";

fn places(dir: &Path) -> Places {
    Places { data: dir.join("data"), cache: dir.join("cache") }
}

fn age(path: &Path, seconds: u64) {
    let file = File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(seconds)).unwrap();
}

#[test]
fn the_newest_map_wins_and_imports_must_fit_the_image() {
    let dir = tempfile::tempdir().unwrap();
    let (places, depth) = (places(dir.path()), Depth::default());
    let image = dir.path().join("hills.png");
    RgbImage::new(12, 8).save(&image).unwrap();
    let generated = places.generated(SHA, &depth).unwrap();
    assert!(generated.ends_with(format!("depth/{SHA}-depth-anything-v2-small-518.png")));
    assert!(places.generated("../../etc", &depth).is_none() && places.own("").is_none());
    assert_eq!(places.find(&image, SHA, &depth), None);
    let map = Map::from_fn(6, 4, |x, y| Luma([(x * 9000 + y * 77) as u16]));
    write(&map, &generated).unwrap();
    assert_eq!(std::fs::metadata(&generated).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!((read(&generated).unwrap(), places.find(&image, SHA, &depth)), (map, Some(generated.clone())));
    age(&generated, 60);
    let own = dir.path().join("own.png");
    GrayImage::from_fn(6, 4, |x, _| Luma([if x < 3 { 0 } else { 255 }])).save(&own).unwrap();
    let imported = places.import(&own, &image, SHA).unwrap();
    assert_eq!((imported.clone(), places.find(&image, SHA, &depth)), (beside(&image), Some(beside(&image))));
    assert_eq!(read(&imported).unwrap().get_pixel(5, 0)[0], u16::MAX);
    let sea = dir.path().join("sea.png");
    RgbImage::new(300, 200).save(&sea).unwrap();
    let older = dir.path().join("older.png");
    write(&Map::new(150, 100), &older).unwrap();
    age(&older, 60);
    std::os::unix::fs::symlink(&older, beside(&sea)).unwrap();
    assert_eq!(places.find(&sea, SHA, &depth), Some(beside(&sea)));
    let fits = dir.path().join("fits.png");
    GrayImage::new(151, 100).save(&fits).unwrap();
    let kept = places.import(&fits, &sea, SHA).unwrap();
    assert_eq!((Some(kept.clone()), places.find(&sea, SHA, &depth)), (places.own(SHA), places.own(SHA)));
    let square = dir.path().join("square.png");
    GrayImage::new(100, 100).save(&square).unwrap();
    let refused = places.import(&square, &sea, SHA).unwrap_err().to_string();
    assert!(refused.contains("100x100") && refused.contains("300x200"), "{refused}");
    assert!(read(&dir.path().join("missing.png")).is_err());
}

#[test]
fn the_cache_keeps_its_size_and_loses_stale_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let (places, cache) = (places(dir.path()), dir.path().join("cache").join("depth"));
    let map = Map::from_fn(64, 64, |x, y| Luma([(x * 977 + y * 131) as u16]));
    let names: Vec<PathBuf> = (0..4).map(|i| cache.join(format!("{i}.png"))).collect();
    for (i, path) in names.iter().enumerate() {
        write(&map, path).unwrap();
        age(path, 100 - i as u64 * 10);
    }
    let size = std::fs::metadata(&names[0]).unwrap().len();
    assert_eq!(places.trim(&names[0], size * 2).unwrap(), size * 2);
    let left: Vec<bool> = names.iter().map(|p| p.exists()).collect();
    assert_eq!(left, [true, false, false, true]);
    assert_eq!(places.trim(&names[0], size * 2).unwrap(), 0);
    let [stale, fresh, other] = [".depth-abc123", ".depth-xyz789", ".depth-notes.txt"].map(|n| cache.join(n));
    for path in [&stale, &fresh, &other] {
        std::fs::write(path, b"x").unwrap();
    }
    age(&stale, 7200);
    age(&other, 7200);
    write(&map, &names[1]).unwrap();
    assert!(!stale.exists() && fresh.exists() && other.exists());
}
