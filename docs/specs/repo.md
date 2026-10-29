# Signed repositories

A repository is a static tree of files that any web server, file share or `file://` path can
serve. Every file the client trusts is either signed with minisign (Ed25519, prehashed `ED`
signatures only) or pinned by SHA-256 from a signed file. Mirrors and servers are never
trusted. The reference implementation lives in `wprs-format` (verification always, key
generation and signing behind the `sign` feature).

## Keys and roles

| Role | Signs | Threshold | Default lifetime |
|------|-------|-----------|------------------|
| `root` | `root/<N>.json` | t of n, 1 to 8 keys | 365 days (30 to 1096) |
| `online` | `entry/<N>.json`, `details/<N>.json` | 1 | 7 days, at most `policy.entry_max_days` (1 to 90, default 14) |
| author | `manifest.json` inside a package | 1 | no expiry; revoked through `revoked` |

- A key is identified by its fingerprint: lowercase hex SHA-256 of the 32 Ed25519 public key
  bytes (bytes 10..42 of the decoded minisign public key). The minisign key id is only a hint.
- A key belongs to at most one role or author. Root keys stay offline.

## Signature files

`<file>.minisig` holds 1 to 16 standard four-line minisign blocks, concatenated. A file with
one block verifies with stock minisign tools.

- Every block must parse and carry the same trusted comment, or the whole file is rejected.
- Each block is tried against every key of the role; the threshold counts distinct key
  fingerprints, never blocks.
- Trusted comments bind role, version and repository: `wpak-repo root <N> <repo_id>`,
  `wpak-repo entry <N> <repo_id>`, `wpak-repo details <N> <repo_id>`, and
  `wpak manifest <id> <version>` for package authors.

## Layout

```
root/<N>.json, root/<N>.json.minisig    immutable, never deleted
entry/<N>.json, entry/<N>.json.minisig  immutable
entry/latest                            "<N>\n", unsigned hint, the only mutable file
details/<N>.json, details/<N>.json.minisig  immutable, only when a second-version release exists
index/<sha256>.json                     shards, immutable
revoked/<sha256>.json                   revocations, immutable
previews/<sha256>.<ext>                 preview images copied from packages
packages/<id>/<version>.wpak            packages, immutable once published
catalog.json                            operator state, not used by clients
```

Serve `entry/latest` with `Cache-Control: no-cache` and everything else as immutable.
Mirrors copy immutable files first and `entry/latest` last.

## Identity and URL

- `repo_id` is the SHA-256 of the exact bytes of `root/1.json`. It never changes.
- Share URL: `https://example.org/wallpapers/?fingerprint=<root key fingerprint>&repo_id=<repo_id>`.
  At least one of the two parameters is required. `fingerprint` must belong to a key of the
  `root` role of `root/1.json` that signed it; `repo_id` must equal its hash.
- The query is stripped before any request. A package is identified by `(repo_id, id)`.

## Formats

All metadata is compact JSON with fixed field order; unknown fields are rejected, except in
`details/<N>.json` and in the releases of `wpak-repo-shard/2.0` and `3.0`, whose readers ignore fields and
values they do not know (also inside `content` and `requires`; an unknown rating counts as
`mature`). Times are RFC 3339 UTC (`2027-01-15T08:00:00Z`).

`root/<N>.json` (at most 512 KiB):

| Field | Rule |
|-------|------|
| `format` | `wpak-repo-root/1.0` |
| `genesis` | `null` in version 1, the `repo_id` after |
| `version` | equals `N` |
| `expires`, `name` | expiry and display name |
| `keys` | map of fingerprint to `{"minisign": "<base64 public key>"}`; fingerprints must match |
| `roles` | `{"root": {keys, threshold}, "online": {keys, threshold}}` |
| `authors` | `[{"id": "[a-z][a-z0-9_-]{0,31}", "keys": [fingerprint…]}]`, unique ids |
| `policy` | `{require_author_signature, max_package_size, entry_max_days}` |
| `mirrors` | at most 16 `https://` or `file://` base URLs |

`entry/<N>.json` (at most 128 KiB): `format` (`wpak-repo-entry/1.0`), `repo_id`, `version`
(= `N`), `timestamp`, `expires` (`0 < expires - timestamp ≤ entry_max_days`), `root`
(`{version, sha256}` of the current root), `shard_bits` (0 to 8), `shards` (exactly
`2^shard_bits` `{sha256, size}` in shard order) and `revoked` (`{sha256, size}`).

`index/<sha256>.json`: `format` (`wpak-repo-shard/1.0`), `repo_id`, `shard` (its position) and
`packages`: `[{id, author, versions: [release…]}]`. A package belongs to the shard given by
the first `shard_bits` bits of SHA-256 of its id, so every id appears once in the repository.
Ids follow the manifest rules; versions are canonical SemVer, unique and ascending. A release
holds `version`, `kind`, `title`, `description`, `tags`, `license`, `authors`, `size`,
`sha256` (whole `.wpak`), `manifest_sha256`, `preview` (`{sha256, size, ext}` or `null`) and
`published`. These shards hold only the releases of first-version packages (`package.md`), byte
for byte as before, so versions of wallpaper-rs that do not know the second version keep reading
the repository and never see packages they cannot open; a package with no such release is left
out of them.

When a release of a `wallpaper-rs.wpak/2` package exists, `details/<N>.json` (at most 128 KiB,
signed by the `online` threshold like the entry) holds `format` (`wpak-repo-details/1.0`),
`repo_id`, `version` (= `N`), `entry` (SHA-256 of the bytes of `entry/<N>.json`) and `shards`:
exactly as many `{sha256, size}` as the entry, pointing at `index/<sha256>.json` shards of format
`wpak-repo-shard/2.0`. They have the same layout and every release of an `image`, `video` or `scene`
package, the kinds that every version of wallpaper-rs reads; a release of
a second-version package adds `schema`, its `default_locale`, `changes`, `content`,
`photosensitive`, `requires` (with the `packages` it needs), the `facets` the repository computed,
`animated` (the animated preview, like `preview`), `platforms` and, for a preset, the id of its
`base`, so that clients can filter before downloading; `title`, `description`
and `changes` are then text or `{language: text}` with every `@key` resolved. Clients that know
this format read these shards instead of the entry's; when no base serves the details, they read
the first-version shards. When a release of another kind (`web`, `preset`, `assets`) exists, the
details also hold `kinds`: as many `{sha256, size}` again, pointing at shards of format
`wpak-repo-shard/3.0` with every release of every package. Clients that know this field read these
shards instead; older ones ignore it and never meet a kind they cannot read. A client skips the
releases whose `kind` it does not know, and a package left with none, so that a kind added later
never stops it from reading the rest of the shard; the fields it does not know in a release are
ignored, while the signed hashes still cover them.

`revoked/<sha256>.json`: `format` (`wpak-repo-revoked/1.0`), `repo_id`, `packages`
(`[{id, versions, reason, message}]`, `"*"` revokes every version) and `keys`
(`[{fingerprint, reason}]`). Reasons: `malware`, `license`, `takedown`, `broken`, `other`.
A revoked key invalidates every signature it made even while a root still lists it.

## Package signatures

An author signs the exact bytes of `manifest.json`; the signature is stored in the package as
`manifest.json.minisig` with the trusted comment `wpak manifest <id> <version>`. It can be
checked by hand with `minisign -Vm manifest.json -p author.pub`.

## Operator workflow

| Step | Function |
|------|----------|
| Create keys (`root`, a backup root key, `online`, authors) | `keygen(password)` |
| Write `root/1.json` | `init(dir, settings, now)` |
| Add root signatures until the threshold is met | `sign_root(dir, signer)` |
| Sign a package source before building it | `sign_source(dir, signer)` |
| Add a package (validated, never `imported`, provider packages under their licenses only, signature checked against root authors, preview copied) | `add(dir, wpak, author, now)` |
| Revoke a package version or a key, or withdraw the revocation of a version | `revoke(dir, id, version, reason, message)`, `revoke_key(dir, fingerprint, reason)`, `unrevoke(dir, id, version)` |
| Write shards, revocations, the next entry and its details; repeat before expiry | `publish(dir, online, lifetime_days, now)` |
| Replace keys or policy: write `root/<N+1>.json`, then sign with the old and the new root threshold, then publish with the current online key | `rotate_root(dir, settings, now)`, `sign_root`, `publish` |
| Check a tree as a client would, without storing anything | `verify(fetch, url_or_dir, now)` |

`status(dir)` reports the signers of the newest root, whether it is complete and the share
URL query. `settings(dir)` returns the current settings for editing before a rotation.

## Client update

Each step that fails for a base (network, size, hash, signature or format) is retried on the
next base: the user URL first, then the root mirrors. If every base fails nothing is stored. A
cancelled transfer (`io::ErrorKind::Interrupted`) stops at once without trying further bases.

1. `T = max(now, max_timestamp)`.
2. First contact: fetch `root/1.json`, check the anchor, `genesis`, `version` and its own root
   threshold.
3. Walk `root/N+1.json` while it exists (at most 64 per run). Each must meet the threshold of
   the previous root and of itself. If the `online` keys changed, reset the entry state.
4. Abort if the current root expired (`Expired`, possible freeze attack). Installed packages
   keep working.
5. Read `entry/latest` (at most 32 bytes), start at `max(latest, last seen)` and probe up to 16
   newer entries.
6. The entry must meet the `online` threshold, pin the current root by version and hash, have
   a valid lifetime, a timestamp not older than the last one and not more than 24 hours after
   `T`. The same version with different bytes is an equivocation. `details/<N>.json` of the same
   version, when a base has it, must meet the `online` threshold and pin that entry by version and
   hash.
7. If `expires ≤ T` the repository is stale: it is shown as such, installs need confirmation
   and revocations still apply. `repos(data, now)` recomputes staleness and expiry warnings at
   every call with the same `T`, so listings and installs agree.
8. Fetch every shard (those of the details when present) and the revocation list not already
   cached, cut at the declared size, check size and SHA-256, then parse them.
9. Store root, entry, details, shards and revocations, then `state.json`; drop unreferenced shards.
10. Apply revocations to installed packages and list available updates (newer SemVer, not
    revoked).

## Package installation

1. Resolve `id[@version]` (optionally `<repo_id>/id[@version]`) against verified shards only.
2. Refuse ids installed from another repository, expired roots, stale
   repositories without confirmation and revoked versions. Without an explicit version nothing
   is downgraded; an explicit version older than the newest installed one needs confirmation.
3. Download `packages/<id>/<version>.wpak` from the bases in order into a temporary file in
   `<data>/packages/<id>/`, cut at the declared size, and check size and SHA-256.
4. Validate the package, check `id`, `version`, `kind`, `license` and `manifest_sha256` against
   the release, and check the author signature when the root requires it or the listing names
   an author (key bound to that author in the root and not revoked).
5. Under the exclusive lock, re-read `installed.json`, rename into place and record it.

`preview(data, fetch, repo_id, id, version, dest)` downloads `previews/<sha256>.<ext>` of a
release from the bases in order, cut at the signed size, checks SHA-256 and writes `dest`
(`0600`) atomically. It returns `false` when the release has no preview.

## Client storage

```
<data>/repos/<repo_id>/state.json             versions, hashes, max_timestamp, online keys, stale
<data>/repos/<repo_id>/root.json[.minisig]    current verified root
<data>/repos/<repo_id>/entry.json[.minisig]   current verified entry
<data>/repos/<repo_id>/details.json[.minisig] its verified details, when the repository has them
<data>/repos/<repo_id>/blobs/<sha256>.json    verified shards and revocations
<data>/packages/<id>/<version>.wpak           installed packages
<data>/installed.json                         origin, hash, signer and revocation of each package
<data>/.repos.lock                            flock serializing writers
```

Files are written to a temporary file in the same directory, synced and renamed. Files are
`0600` and directories `0700`. Nothing is stored before it is verified. Committing a snapshot,
removing a repository and saving `installed.json` hold an exclusive `flock` on
`<data>/.repos.lock`; reading snapshots or `installed.json` holds a shared one. Downloads finish
before the lock is taken, so readers never wait for the network.

## Network interface

The library does no networking. Callers implement `Fetch::get(base, path, max)`, returning
at most `max + 1` bytes or `None` when the file does not exist, and may override
`Fetch::copy` to stream packages. `FileFetch` serves `file://` URLs and plain paths. Requests
carry no cookies or identifiers.

`wprs_providers::HttpFetch` is the blocking HTTP implementation: a generic `User-Agent`, no
cookies or `Referer`, `https://` or `http://` to a loopback host only (redirects included, never
from HTTPS to HTTP), redirects only to the origins of the bases it was asked to read or given with
`within(bases)` (a repository's URL and the mirrors of its signed root), `file://` URLs and plain
paths served by `FileFetch`, a 10 s connect timeout, and an abort (`io::ErrorKind::TimedOut`)
when less than 8 KiB arrive in 30 s. A shared cancel flag, checked for every chunk, fails the
transfer with `io::ErrorKind::Interrupted`.
