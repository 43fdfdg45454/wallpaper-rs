pub const SORTINGS: [&str; 7] = ["date_added", "relevance", "random", "views", "favorites", "toplist", "hot"];
pub const ORDERS: [&str; 2] = ["desc", "asc"];
pub const RANGES: [&str; 7] = ["1d", "3d", "1w", "1M", "3M", "6M", "1y"];
pub const CATEGORIES: [&str; 3] = ["general", "anime", "people"];
pub const PURITIES: [&str; 3] = ["sfw", "sketchy", "nsfw"];
pub const RATIOS: [&str; 14] = [
    "16x9",
    "16x10",
    "21x9",
    "32x9",
    "48x9",
    "9x16",
    "10x16",
    "9x18",
    "1x1",
    "3x2",
    "4x3",
    "5x4",
    "landscape",
    "portrait",
];
pub const RESOLUTIONS: [&str; 18] = [
    "1280x720",
    "1280x800",
    "1366x768",
    "1440x900",
    "1600x900",
    "1680x1050",
    "1920x1080",
    "1920x1200",
    "2560x1080",
    "2560x1440",
    "2560x1600",
    "3440x1440",
    "3840x1080",
    "3840x1600",
    "3840x2160",
    "5120x1440",
    "5120x2880",
    "7680x4320",
];
pub const PALETTE: [&str; 29] = [
    "660000", "990000", "cc0000", "cc3333", "ea4c88", "993399", "663399", "333399", "0066cc", "0099cc",
    "66cccc", "77cc33", "669900", "336600", "666600", "999900", "cccc33", "ffff00", "ffcc33", "ff9900",
    "ff6600", "cc6633", "996633", "663300", "000000", "999999", "cccccc", "ffffff", "424153",
];

pub fn items(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').map(str::trim).filter(|item| !item.is_empty())
}

fn size(value: &str) -> bool {
    let number = |text: &str| text.len() <= 5 && text.parse::<u32>().is_ok_and(|n| n > 0);
    value.split_once('x').is_some_and(|(w, h)| number(w) && number(h))
}

fn name(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn digits(value: &str) -> bool {
    (1..=9).contains(&value.len()) && value.chars().all(|c| c.is_ascii_digit())
}

pub fn hint(key: &str, value: &str) -> Option<&'static str> {
    let every =
        |count: usize, ok: &dyn Fn(&str) -> bool| items(value).count() <= count && items(value).all(ok);
    let (ok, hint) = match key {
        "atleast" => (value.is_empty() || size(value), "WIDTHxHEIGHT or nothing"),
        "resolutions" => (every(24, &size), "up to 24 sizes WIDTHxHEIGHT separated by commas"),
        "ratios" => (
            every(14, &|r| RATIOS.contains(&r) || size(r)),
            "up to 14 ratios like 16x9, or landscape and portrait, separated by commas",
        ),
        "colors" => (
            every(29, &|c| PALETTE.contains(&c)),
            "up to 29 palette colors as hex without #, separated by commas",
        ),
        "similar" => (
            value.is_empty()
                || (4..=10).contains(&value.len()) && value.chars().all(|c| c.is_ascii_alphanumeric()),
            "a wallpaper id or nothing",
        ),
        "tag" => (value.is_empty() || digits(value), "a number or nothing"),
        "collection" => (
            value.is_empty() || value.split_once('/').is_some_and(|(user, id)| name(user) && digits(id)),
            "USER/ID of a collection or nothing",
        ),
        "uploader" => (value.is_empty() || name(value), "a Wallhaven user name or nothing"),
        _ => return None,
    };
    (!ok).then_some(hint)
}
