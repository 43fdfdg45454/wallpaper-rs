use std::path::Path;

const REMOTE: [u32; 9] =
    [0x6969, 0x517b, 0xff53_4d42, 0xfe53_4d42, 0x0102_1997, 0x00c3_6400, 0x5346_414f, 0x7375_7245, 0x564c];
const FUSE: u32 = 0x6573_5546;
const NETWORK: [&str; 18] = [
    "sshfs",
    "rclone",
    "s3fs",
    "gcsfuse",
    "goofys",
    "curlftpfs",
    "davfs",
    "smbnetfs",
    "gvfsd-fuse",
    "kio-fuse",
    "onedriver",
    "google-drive-ocamlfuse",
    "blobfuse2",
    "juicefs",
    "glusterfs",
    "ceph-fuse",
    "httpdirfs",
    "cvmfs2",
];

pub(super) fn remote(dir: &Path) -> bool {
    let Ok(found) = rustix::fs::statfs(dir) else { return false };
    let kind = (found.f_type as u64 & 0xffff_ffff) as u32;
    REMOTE.contains(&kind)
        || (kind == FUSE && fuse(dir, &std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default()))
}

fn unescape(field: &str) -> String {
    let (bytes, mut out, mut i) = (field.as_bytes(), Vec::new(), 0);
    while i < bytes.len() {
        let code = (bytes[i] == b'\\').then(|| field.get(i + 1..i + 4)).flatten();
        let (byte, step) = code.and_then(|c| u8::from_str_radix(c, 8).ok()).map_or((bytes[i], 1), |b| (b, 4));
        out.push(byte);
        i += step;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(super) fn fuse(dir: &Path, mountinfo: &str) -> bool {
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_owned());
    let mounts = mountinfo.lines().filter_map(|line| {
        let (head, tail) = line.split_once(" - ")?;
        let point = unescape(head.split(' ').nth(4)?);
        Some((point, tail.split(' ').next()?.to_owned()))
    });
    let under = mounts.filter(|(point, _)| dir.starts_with(point));
    let Some((_, kind)) = under.max_by_key(|(point, _)| point.len()) else { return true };
    match kind.strip_prefix("fuse.") {
        Some(sub) => NETWORK.contains(&sub),
        None => kind != "fuseblk",
    }
}
