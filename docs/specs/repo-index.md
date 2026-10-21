# Repository index and trust model (draft v0)

Status: draft, not implemented yet (H2). Modeled on F-Droid index v2 and a reduced TUF.

## Files served by a repository

| File | Signed by | Content |
|------|-----------|---------|
| `repo.json` + `.minisig` | offline root key | Repository id, `version`, `expires` (~1 year), root keys (primary + offline backup), online index key, author keys bound to author ids |
| `entry.json` + `.minisig` | online key | Monotonic `version`, `timestamp`, short `expires` (14 days), hash of `repo.json`, BLAKE3 and size of every shard, diff and `revoked.json` |
| `index/*.json` | via `entry.json` hashes | Package records: id, version, type, title, tags, rating, size, hash, URL, license, preview |
| `revoked.json` | via `entry.json` hash | Revoked package versions and keys |
| `packages/<id>/<version>.wpak` | via index hashes | Packages |

Root rotation: the old root signs the new `repo.json` (a chain of roots). Tools:
`wprs-pack repo keygen | rotate-root | refresh`; `refresh` must run before `entry.json`
expires.

## Client rules

- The trust anchor is the root key fingerprint carried in the repository URL or QR code
  (`?fingerprint=`). Trust on first use is only an explicit opt-in.
- Reject an `entry.json` or `repo.json` with a lower version than the last one seen.
- Mark a repository as stale when `entry.json` has expired; never silently use it as fresh.
- Fail hard on a key change not covered by the root chain.
- Apply `revoked.json` to installed packages.
- Never cache metadata that failed verification.
- A package's identity is (root key fingerprint of its origin repository, id). Updates are
  accepted only from that origin or from mirrors it declares.

## Dynamic servers

A `wprs-server` holds only the online key and publishes the same `entry.json` and shards as a
static repository. Search and filter endpoints are hints: before installing, the client
resolves (id, version, hash, size) against the signed shards.
