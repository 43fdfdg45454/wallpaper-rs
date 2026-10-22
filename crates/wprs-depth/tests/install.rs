use sha2::{Digest, Sha256};
use wprs_depth::{Asset, MODEL, Places};

const SIZE: usize = 300_000;

fn body() -> Vec<u8> {
    (0..SIZE).map(|i| (i * 7 % 251) as u8).collect()
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn assets_install_from_files_only_with_their_hash() {
    let dir = tempfile::tempdir().unwrap();
    let places = Places { data: dir.path().join("data"), cache: dir.path().join("cache") };
    let file = dir.path().join("weights.onnx");
    std::fs::write(&file, body()).unwrap();
    let sha: &'static str = hex(&body()).leak();
    let fake = Asset { sha256: sha, file_sha256: sha, bytes: SIZE as u64, ..MODEL };
    let refused = places.install(&MODEL, &file).unwrap_err();
    assert!(refused.to_string().contains("SHA-256 differs"));
    let installed = places.install(&fake, &file).unwrap();
    assert_eq!(installed, places.file(&fake));
    assert!(places.verify(&fake).unwrap() && !places.verify(&MODEL).unwrap_or(true));
    let notice = std::fs::read_to_string(installed.with_file_name("NOTICE.txt")).unwrap();
    assert!(notice.contains("Apache-2.0") && notice.contains(sha) && notice.contains("LICENSE.txt"));
    let license = std::fs::read_to_string(installed.with_file_name("LICENSE.txt")).unwrap();
    assert_eq!(license, MODEL.license_text);
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), Default::default()));
    for (path, data) in
        [("pkg/ThirdPartyNotices.txt", &b"zlib notice"[..]), ("pkg/lib/libdemo.so", &body()[..])]
    {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_cksum();
        archive.append_data(&mut header, path, data).unwrap();
    }
    let tgz = dir.path().join("runtime.tgz");
    std::fs::write(&tgz, archive.into_inner().unwrap().finish().unwrap()).unwrap();
    let packed: &'static str = hex(&std::fs::read(&tgz).unwrap()).leak();
    let runtime = Asset {
        id: "demo",
        kind: wprs_depth::Part::Runtime,
        sha256: packed,
        member: "pkg/lib/libdemo.so",
        notices: "pkg/ThirdPartyNotices.txt",
        file: "libdemo.so",
        ..fake
    };
    let stale = places.file(&runtime).with_file_name(".unpack-ab12cd");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, b"left over").unwrap();
    let file = std::fs::File::options().write(true).open(&stale).unwrap();
    file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(7200)).unwrap();
    let library = places.install(&runtime, &tgz).unwrap();
    assert!(!stale.exists());
    assert_eq!(std::fs::read(&library).unwrap(), body());
    let notices = std::fs::read_to_string(library.with_file_name("THIRD-PARTY-NOTICES.txt")).unwrap();
    let notice = std::fs::read_to_string(library.with_file_name("NOTICE.txt")).unwrap();
    assert!(notices == "zlib notice" && notice.contains("THIRD-PARTY-NOTICES.txt"), "{notice}");
    assert!(places.library(Some(&library)).first() == Some(&library));
    assert!(places.remove(&runtime).unwrap() && !library.exists() && !places.remove(&runtime).unwrap());
}
