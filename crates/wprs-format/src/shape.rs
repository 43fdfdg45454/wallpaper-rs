pub(crate) fn shape([w, h]: [u32; 2]) -> Vec<String> {
    const SIZES: [(&str, u32, u32); 6] = [
        ("8k", 7680, 4320),
        ("5k", 5120, 2880),
        ("4k", 3840, 2160),
        ("1440p", 2560, 1440),
        ("1080p", 1920, 1080),
        ("720p", 1280, 720),
    ];
    let (long, short) = (w.max(h), w.min(h));
    let orient = match w.cmp(&h) {
        std::cmp::Ordering::Greater => "landscape",
        std::cmp::Ordering::Less => "portrait",
        std::cmp::Ordering::Equal => "square",
    };
    let size = SIZES.iter().find(|(_, a, b)| long >= *a && short >= *b).map(|(n, ..)| *n);
    let wide = (long as f32 / short.max(1) as f32 >= 2.2).then_some("ultrawide");
    [Some(orient), size, wide].into_iter().flatten().map(String::from).collect()
}
