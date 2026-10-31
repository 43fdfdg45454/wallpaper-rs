use crate::Error;
use wprs_core::message::{Message, said};

impl Error {
    pub fn message(&self) -> Message {
        match self {
            Error::Io(e) => said("format.io", "I/O error: %1", &[e]),
            Error::Zip(e) => said("format.zip", "invalid zip archive: %1", &[e]),
            Error::Json(e) => said("format.json", "malformed JSON: %1", &[e]),
            Error::Invalid { field, reason } => {
                said("format.invalid", "invalid %1: %2", &[field, reason]).texts(&[2])
            }
            Error::UnsafePath(p) => said("format.unsafe_path", "unsafe path \"%1\"", &[p]),
            Error::Duplicate(p) => said("format.duplicate", "duplicate entry \"%1\"", &[p]),
            Error::Symlink(p) => said("format.symlink", "\"%1\" is a symbolic link or special file", &[p]),
            Error::Missing(p) => said("format.missing", "missing file \"%1\"", &[p]),
            Error::Extension { path, kind } => {
                said("format.extension", "\"%1\" has an extension not allowed in %2 packages", &[path, kind])
            }
            Error::Limit { what, actual, limit } => {
                said("format.limit", "%1 %2 exceeds the limit of %3", &[what, actual, limit]).texts(&[1])
            }
            Error::Unsupported(p, what) => {
                said("format.unsupported", "entry \"%1\" uses an unsupported feature: %2", &[p, what])
                    .texts(&[2])
            }
            Error::SizeMismatch(p) => {
                said("format.size_mismatch", "entry \"%1\" does not match its declared size", &[p])
            }
            Error::Blocked(p) => said("format.blocked", "destination \"%1\" is in the way", &[&p.display()]),
            Error::Signature(r) => said("format.signature", "signature rejected: %1", &[r]).texts(&[1]),
            Error::Key(r) => said("format.key", "invalid key: %1", &[r]),
            Error::Expired(r) => said("format.expired", "%1 has expired", &[r]),
            Error::Stale(r) => {
                said("format.stale", "repository %1 is stale: its signed index has expired", &[r])
            }
            Error::Rollback(r) => said("format.rollback", "rollback rejected: %1", &[r]),
            Error::Equivocation(n) => {
                said("format.equivocation", "entry %1 was published twice with different content", &[n])
            }
            Error::Mismatch(r) => said("format.mismatch", "%1 does not match its signed hash or size", &[r]),
            Error::Revoked(r) => said("format.revoked", "%1 is revoked", &[r]),
            Error::NotFound(r) => said("format.not_found", "%1 not found", &[r]),
            Error::Exists(r) => said("format.exists", "%1 already exists", &[r]),
            Error::Newer(r) => {
                said("format.newer", "this package needs a newer version of wallpaper-rs: it uses %1", &[r])
            }
            Error::Elsewhere(r) => said("format.elsewhere", "this package is made for %1 only", &[r]),
        }
    }
}
