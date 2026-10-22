# Self-hosted server

`wprs-server` is one binary that serves a signed package repository, a web catalog, accounts,
uploads with moderation, and device sync for one community or household. It keeps every byte
of state in one data directory, makes no outgoing connection except to an OIDC issuer the
operator configures, and sends no telemetry.

## Running

```sh
docker compose up -d                         # builds the image from this checkout
docker compose logs wprs | grep setup        # one-time token for the first administrator
docker compose exec wprs wprs-server repo init --name "Example wallpapers"
```

| Command | Effect |
|---------|--------|
| `serve` (default) | Run the server; migrations run first |
| `migrate` | Apply database migrations and exit |
| `healthcheck` | Exit 0 when `GET /healthz` on the local port answers 200 (the image `HEALTHCHECK`) |
| `setup-token` | Print a one-time token (24 h) that creates an administrator at `/setup` |
| `repo init --name N [--require-author-signature]` | Create the repository: a root key and an online key. The root secret key is printed once and never stored |
| `repo import DIR [--online-key FILE]` | Import `root/<N>.json` files signed offline (and `catalog.json` the first time) plus the online key |
| `repo status`, `repo publish` | Show the repository and its share URL; sign and publish now |
| `backup [DIR]` | Snapshot the database with `VACUUM INTO`, copy the blobs, thumbnails and complete private files that the snapshot names (a file deleted meanwhile takes a new snapshot, up to three times; files missing from two snapshots in a row are listed as `missing` in the manifest with a warning), the withdrawn packages, the keys and the repository; default `DIR` is `<data>/backups/<UTC time>`, removed again when the backup fails |
| `restore DIR [--force] [--advance N]` | Check every checksum and `PRAGMA integrity_check`, restore with the server stopped, then publish `N` (default 1) new index versions. Signed root and index files newer than the snapshot stay, so index numbers are never reused; after losing the data directory, pass an `N` that goes past the last version clients saw. Sync heads jump to the restore time in milliseconds, so every device downloads its records again (`410 resync`), and unfinished private files start again from the first byte |

The image builds from base images pinned by digest and runs as user 65532 on a read-only root
file system without a shell; `/data` is the only writable path. `deploy/compose.caddy.yaml` adds Caddy with automatic TLS:
`WPRS_DOMAIN=wallpapers.example.org docker compose -f compose.yaml -f deploy/compose.caddy.yaml up -d`.
Caddy is pinned by the digest of its multi-platform index too; to update it, take the
`Docker-Content-Digest` of `docker buildx imagetools inspect docker.io/library/caddy:<tag>` (the
index, not one platform) and replace the tag and digest in that file.

### Configuration

Every setting is an environment variable (`.env` next to `compose.yaml` is read). `NAME_FILE`
reads the value of `NAME` from a file, for secrets.

| Variable | Default | Meaning |
|----------|---------|---------|
| `WPRS_PUBLIC_URL` | `http://localhost:8080` | External URL; `https://` enables HSTS and the `__Host-wprs` cookie |
| `WPRS_LISTEN` | `0.0.0.0:8080` | Listen address |
| `WPRS_DATA` | `/data` | Data directory |
| `WPRS_TRUSTED_PROXIES` | empty | CIDRs whose `X-Forwarded-For` is believed, read right to left |
| `WPRS_NAME` | `wallpaper-rs` | Instance name |
| `WPRS_REGISTRATION` | `invite` | `closed`, `invite` or `open` |
| `WPRS_AUTO_PUBLISH` | `trusted` | Publish uploads without review: `never`, `trusted` (role trusted and above) or `all` |
| `WPRS_LICENSES` | `CC0-1.0,CC-BY-4.0,CC-BY-SA-4.0` | SPDX ids accepted in uploads |
| `WPRS_PACKAGE_MAX_MIB`, `WPRS_USER_QUOTA_MIB` | `256`, `2048` | Package size and storage per account |
| `WPRS_UPLOADS_PER_DAY` | `20` | Upload sessions per account and day |
| `WPRS_SYNC_QUOTA_MIB` | `64` | Sync records per account |
| `WPRS_FILES_QUOTA_MIB` | `1024` | Private files per account |
| `WPRS_ENTRY_DAYS` | `7` | Lifetime of each signed index (1 to 14, capped by the root) |
| `WPRS_RESIGN_HOURS` | `24` | Re-sign the index at least this often (at most half the index lifetime) |
| `WPRS_PUBLISH_DELAY_SECS` | `30` | Wait after a moderation decision before signing, to batch changes |
| `WPRS_LEGAL_OPERATOR`, `WPRS_LEGAL_CONTACT`, `WPRS_LEGAL_JURISDICTION`, `WPRS_LEGAL_DMCA_AGENT` | empty | Shown on `/legal` and in the terms |
| `WPRS_OIDC_ISSUER`, `WPRS_OIDC_CLIENT_ID`, `WPRS_OIDC_CLIENT_SECRET`, `WPRS_OIDC_AUTO_PROVISION` | unset, `false` | Optional single sign-on (authorization code with PKCE) |
| `WPRS_LOG_CLIENTS` | `off` | Abuse log of client addresses: `off`, `prefix` (IPv4 /24, IPv6 /48) or `full` (address and user agent) |
| `WPRS_LOG_CLIENTS_DAYS` | `7` | Days the abuse log keeps (1 to 30) |
| `WPRS_NOTICE_DAYS` | `180` | Days a legal notice keeps the sender's name, email and signature (1 to 730) |
| `WPRS_TAKEDOWN_DAYS` | `90` | Days a removed or revoked package stays in `held/` as evidence and for a counter-notice (1 to 3650) |
| `WPRS_RATE_LIMITS` | `true` | In-memory rate limits |
| `WPRS_LOG` | `info` | Log filter |

Logs go to standard output and never contain addresses, user agents, tokens, bodies or emails.
With `WPRS_LOG_CLIENTS` the server writes `logs/clients-<date>.jsonl` (`at`, `ip`, `agent`,
`method`, `route`, `status`) and deletes files older than the retention.

### Data directory

```
db/wprs.sqlite      accounts, packages, moderation, sync records (WAL)
blobs/<sha256>.wpak  uploads waiting for review
thumbs/<sha256>.jpg  640 px previews re-encoded by the validator
held/<sha256>.wpak   removed or revoked packages (and their previews) for WPRS_TAKEDOWN_DAYS, not served
private/<user>/<id>  end-to-end encrypted private files of each account
repo/                the static repository (docs/specs/repo.md) built with wprs-format
keys/online.key      online signing key (0600); the root key never lives here
tmp/ logs/ backups/
```

## Conventions

- JSON in UTF-8. Times are Unix milliseconds unless they belong to repository files.
- Errors are `application/problem+json`: `{type: "about:blank", title, status, code, detail}`.
  `code` is stable (`invalid`, `malformed`, `unauthorized`, `forbidden`, `not_found`,
  `rate_limited`, `handle_taken`, `version`, `epoch_mismatch`, `resync`, …).
- `429` responses carry `Retry-After`.
- Authentication is `Authorization: Bearer wprs_<kind>_<64 hex>_<8 hex CRC-32>` (kinds `d`
  device, `p` personal) or the session cookie of the web site. The server stores SHA-256 of
  tokens and sessions only; revocation is immediate.
- Scopes: `catalog:read`, `upload`, `sync`, `devices`, `mod`, `admin`. Device tokens get
  `upload sync devices`. Personal tokens are created on the web site; `mod` and
  `admin` need the role and expire within 90 days. Web sessions have every scope of the role.
- Roles: `user`, `trusted`, `moderator`, `admin`.
- Requests with a cookie and an unsafe method must be same-origin (`Sec-Fetch-Site` and
  `Origin` are checked); bearer requests are exempt.
- Request bodies are limited to 1 MiB, 6 MiB for sync changes and 8 MiB for a piece of a private
  file; uploads stream without limit up to the declared size. Answers stay small: a page of sync changes holds at most 4 MiB of
  payloads (always one change), the mailbox at most 1 MiB of bodies. Requests time out after 30 s except uploads, private file pieces, moderation decisions and event streams.
- Every form endpoint also accepts `application/x-www-form-urlencoded`; with `?back=/path` the
  answer is an HTML page. `POST …?_method=DELETE|PATCH` stands for those methods in forms.
- Public reads (`/repo/`, `/media/`, `/api/v1/catalog`, `/api/v1/packages/…`) send
  `Access-Control-Allow-Origin: *`; nothing else allows cross-origin use.

Rate limits per client address (IPv6 per /64): 20 requests/s with bursts of 50 overall; 5 per
minute for login, registration, setup, single sign-on, device code start and every password check
of a signed-in account; 10 failed logins or password checks per account in 15 minutes; 5 reports or
notices per hour. Sync writes: 60 per minute per device.

## Discovery and health

| Method | Path | Answer |
|--------|------|--------|
| GET | `/healthz` | `ok` |
| GET | `/readyz` | `200` or `503` with `{database, repository, index_fresh}` |
| GET | `/.well-known/wprs-server` | `{name, version, api: ["v1"], registration, oidc, sync: {end_to_end_default, plaintext_requires_confirmation, record_max_bytes, quota_bytes}, uploads: {max_bytes, per_day, licenses}, repository: {url, repo_id, entry_expires} \| null, legal: {notice, terms, privacy, contact}}` |

## Repository

`/repo/<path>` serves the tree of `docs/specs/repo.md`: `root/<N>.json[.minisig]`,
`entry/latest`, `entry/<N>.json[.minisig]`, `index/<sha256>.json`, `revoked/<sha256>.json`,
`previews/<sha256>.<ext>` and `packages/<id>/<version>.wpak`. `entry/latest` is `no-cache`;
everything else is immutable. A package that moderators removed or revoked answers `410`, and its
preview is deleted unless a release still listed uses it.
The share URL is `repository.url` of the discovery document
(`https://example.org/repo/?fingerprint=…&repo_id=…`).

Only the online key signs on the server, and only entries: an approval adds the package with
`wprs_format::add`, a removal or revocation calls `wprs_format::revoke`, and a background job
calls `wprs_format::publish` with a lifetime of `WPRS_ENTRY_DAYS` after changes and at least
every `WPRS_RESIGN_HOURS`. Packages are either unsigned (when the root does not require author
signatures) or signed by an author key that the root lists and the uploader registered.

Clients must treat the catalog API as hints and resolve `(id, version, sha256, size)` against
the verified shards before installing.

## Catalog

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/catalog?q=&kind=&cursor=&limit=` | — | Newest published packages; `q` matches title, description, tags and id; `kind` is `image`, `video` or `scene`; `limit` 1 to 100 (24) |
| GET | `/api/v1/packages/{id}` | — | Latest version plus `versions: [string]` |
| GET | `/api/v1/packages/{id}/versions` | — | `{items: [item]}` |
| POST | `/api/v1/packages/{id}/reports` | — | `{category: copyright\|illegal\|malware\|mature\|spam\|other, details}` → `201 {case}` |
| GET | `/media/{sha256}.jpg` | — | Thumbnail of a pending or published version |

Search uses an SQLite FTS5 index with the trigram tokenizer (without diacritics) over the id,
title, description and tags of published versions, kept up to date by triggers: every word of `q`
with at least 3 characters must appear as a substring, in any case and with or without accents;
a `q` without such a word matches tags that start with it.

Item: `{id, version, kind, title, description, license, tags, authors, size, sha256,
published_at, owner, thumbnail, download}` (`thumbnail` and `download` are paths). The list
answer is `{items, next_cursor}` with `Link: <…>; rel="next"` when there is more.

## Accounts

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/v1/auth/register` | — | `{handle, password, invite}` → `201 {id, handle}` |
| POST | `/api/v1/auth/login` | — | `{handle, password, device: {platform, enc_key?, sig_key?}}` → device token (below) |
| POST | `/api/v1/auth/logout` | token or session | Revokes the token or ends the session → `204` |
| GET | `/api/v1/me` | any | `{id, handle, role, created_at, quota_bytes, used_bytes}` |
| DELETE | `/api/v1/me` | session | `{password}` → deletes the account and everything it owns → `204` |
| POST | `/api/v1/me/password` | session | `{current, new, devices?}`; see below |
| GET | `/api/v1/me/export` | session | Profile, devices, tokens, keys, versions, uploads, sync head and records |
| GET, POST | `/api/v1/me/tokens` | any, session | List; create `{name, scopes: "upload sync", days?}` → `201 {id, token, scopes, expires_at}` (shown once) |
| DELETE | `/api/v1/me/tokens/{id}` | session | Revoke a personal token |
| GET | `/api/v1/me/devices` | any | `{items: [{id, name, platform, enc_key, sig_key, created_at, seen_at}]}` |
| DELETE | `/api/v1/me/devices/{id}` | `devices` | Revoke a device, its tokens, its mailbox and its event stream; asks for a key rotation |
| GET, POST | `/api/v1/me/author-keys` | any, `upload` | List; add `{public_key}` (minisign public key file or line) → `201 {fingerprint}` |
| DELETE | `/api/v1/me/author-keys/{fingerprint}` | `upload` | Remove |
| GET | `/api/v1/me/packages` | `upload` | `{items: [{package, version, title, state, created_at, published_at, decision, statement}], uploads: [{id, state, errors, created_at}]}` |

Handles are 3 to 32 of `a-z 0-9 _ -`; passwords 10 to 256 characters without the handle,
hashed with Argon2id (19 MiB, 2 passes). A new password revokes every session (the answer sets a
new session cookie with `204`), every personal token and every approved device code of the
account; with `devices: true` it also revokes every device and asks for a key rotation, so devices
link again and get the sync key from a device that holds it or from the escrow. The password of an
account without one (single sign-on) can only be set from a session signed in within the last 10
minutes, and so can such an account be deleted. The web site adds `/login`, `/register`, `/setup`,
`/link` and `/account`.

Devices are named on the server `Device 1`, `Device 2`… and never by the client: the real name
travels inside the encrypted `device` sync record. `platform` is `linux`, `android` or
`other`; `enc_key` (X25519) and `sig_key` (Ed25519) are base64 of 32 bytes.

Device token answer: `{token, device_id, device_name, scopes}`.

### Device enrollment

For devices without a keyboard or accounts without a password (RFC 8628 style):

1. `POST /api/v1/auth/device/start {platform, enc_key?, sig_key?, client?}` (`client`: at most 64
   characters naming the client software) →
   `{device_code, user_code: "BCDF-GHJK", verification_uri, verification_uri_complete, interval: 5, expires_in: 600}`.
2. The device shows `user_code` (and a QR code of `verification_uri_complete`).
3. The user approves at `/link`, or another device calls
   `POST /api/v1/auth/device/approve {user_code, approve: true|false}` (scope `devices`). Before
   that, `POST /api/v1/auth/device/preview {user_code}` (scope `devices`) answers what the pending
   request sent, `{platform, client, requested_at, expires_at}`, and never an address; `/link`
   shows the same with the warning to approve only a code shown on the user's own screen.
4. The device polls `POST /api/v1/auth/device/token {device_code}` no faster than `interval`.
   Answers `400` with `code` `authorization_pending`, `slow_down` (add 5 s), `expired_token`,
   `access_denied` or `invalid_grant`, or `200` with the device token.

### Single sign-on

With OIDC configured, `GET /api/v1/auth/oidc/start?next=/account` redirects to the issuer
(PKCE S256, `state`, `nonce`) and `/api/v1/auth/oidc/callback` signs in the account bound to
`(issuer, subject)`. The discovery document and key set are kept for one hour; when an ID token
fails verification they are fetched again, at most once a minute, and the same token is checked
once more. `?link=true` binds the identity to the signed-in account. Unknown
identities are refused unless `WPRS_OIDC_AUTO_PROVISION` creates an account.

## Uploads

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/v1/uploads` | `upload` | `{size, sha256}` → `201 {id, put_url, expires_at}`; checks blocklist, strikes, daily limit, two open sessions and quota first |
| PUT | `/api/v1/uploads/{id}` | `upload` | Body is the `.wpak`; `Content-Length` must equal `size`; size and SHA-256 are checked while streaming; one request at a time per upload (`429`) |
| POST | `/api/v1/uploads/{id}/finalize` | `upload` | Validates and returns the status |
| GET | `/api/v1/uploads/{id}` | `upload` | Status |
| DELETE | `/api/v1/uploads/{id}` | `upload` | Cancel an unfinished upload → `204` |
| POST | `/upload` | session | `multipart/form-data` field `package`, for the web site; strikes, daily limit, open uploads and quota are checked and an open upload session is reserved before the package is read, the blocklist right after; at most 8 fields, others up to 4 KiB, 30 s each |

Status: `{id, state, errors: [string], package, version}` with `state` one of `open`,
`received`, `failed`, `pending_review`, `published`, `rejected`, `removed`, `revoked`.

Validation runs `wprs-server validate` in a child process limited to 1 GiB of address space
and 60 s of CPU, two at a time. It opens the package with `wprs_format::open` (strict ZIP
profile and manifest rules), verifies `manifest.json.minisig` against the uploader's author
keys that the root also lists (required when the root demands signatures), and decodes the
preview image (PNG, JPEG, WebP, GIF or BMP, at most 16384 px) into a JPEG thumbnail. The
server then checks the licence list, that the id belongs to the uploader (the first uploader
owns it) and that the version is newer than every earlier one. Accepted versions wait for
review unless `WPRS_AUTO_PUBLISH` publishes them.

## Moderation and administration

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/mod/queue` | moderator | Open cases, highest priority first: `{items: [{id, reason, detail, priority, created_at, package, version, title, state, sha256, uploader}]}` |
| POST | `/api/v1/mod/cases/{id}/decision` | moderator | `{decision, statement, reason?, strike?}` |
| GET, POST | `/api/v1/mod/blocklist` | moderator | List; add `{sha256, reason}` |
| DELETE | `/api/v1/mod/blocklist/{sha256}` | moderator | Remove |
| GET | `/api/v1/mod/audit` | admin | Last 500 actions |
| GET, POST | `/api/v1/admin/invites` | admin | List; create `{role, uses?, days?}` → `201 {id, code, url, …}` |
| DELETE | `/api/v1/admin/invites/{id}` | admin | Revoke |
| GET | `/api/v1/admin/users` | admin | Accounts |
| PATCH | `/api/v1/admin/users/{id}` | admin | `{role?, status?: active\|suspended, quota_mib?}` |
| GET | `/api/v1/admin/repo` | admin | Repository state |
| POST | `/api/v1/admin/repo/publish` | admin | Sign now → `{entry_version, expires, packages}` |
| GET | `/api/v1/admin/transparency` | admin | CSV counts of notices, cases and decisions, without personal data |

Decisions: `approve` (pending → published), `reject` (pending → rejected, the file and its
thumbnail are deleted), `remove` (published → removed: revoked with reason `takedown`, downloads
answer `410`), `revoke` (published → revoked with `reason` `malware`, `license`, `takedown`,
`broken` or `other`), `restore` (removed or revoked → published again, for a counter-notice:
the revocation is withdrawn with `wprs_format::unrevoke`) and `dismiss`. Removing or revoking
moves the package file, and the thumbnail unless another pending or published version uses it,
out of the public tree into `held/`, kept for `WPRS_TAKEDOWN_DAYS` (also in backups); `restore`
answers `409 deleted` after that. Nobody approves, restores or dismisses a case of their own
upload (`403`); restrictive decisions on one's own upload are allowed. Restrictive decisions need a `statement` of at least 10
characters, which the uploader sees in `/api/v1/me/packages`. `strike: true` adds a strike;
three strikes within a year suspend uploads. Every decision is audited. Cases come from
uploads (priority 0), reports (1), copyright notices and counter-notices (2) and notices of
illegal content (3).

## Legal pages

`/legal` (operator, contact, jurisdiction, copyright agent), `/legal/terms`,
`/legal/privacy`, `/legal/content`, `/legal/notice` (form covering copyright and other illegal
content; answers with a receipt number), `/legal/notice/{receipt}` (state of the decision) and
`/legal/counter-notice`. The pages use no JavaScript and no third-party resources. A notice keeps
the sender's name, email and signature for `WPRS_NOTICE_DAYS`, then only its content.

## Sync

Sync stores the latest state of each item per account. The server never interprets payloads.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/sync/state` | device | `{seq, min_seq, epoch, encrypted, key_id, rotate, used_bytes, quota_bytes, warning, escrow, files_bytes, files_quota_bytes}` (`escrow` tells whether one is stored); `404 sync_not_set_up` before setup |
| POST | `/api/v1/sync/setup` | device | `{encrypted: true, key_id?}` (default) or `{encrypted: false, confirm_plaintext: true, key_id?}` → `201` state |
| GET | `/api/v1/sync/changes?since=&limit=` | device | `{seq, min_seq, epoch, more, changes: [{collection, item, seq, epoch, device, deleted, payload}]}`; `410 resync` when `0 < since < min_seq` or `since > seq` |
| POST | `/api/v1/sync/changes` | device | `{epoch, changes: [{collection, item, base, deleted, payload}]}` (at most 500) → `{seq, results}` |
| POST | `/api/v1/sync/epoch` | device | Key epochs, below → state |
| POST | `/api/v1/sync/reset` | `sync` | `{confirm: "reset"}` deletes every record, private file and escrow → `204` |
| GET, PUT | `/api/v1/sync/escrow` | device | `{escrow: base64 \| null}`, at most 4 KiB, only with encryption |
| GET | `/api/v1/sync/blobs?after=` | device | Private files by id after `after`, 1000 per page: `{items: [{id, size, received, created_at}], more, used_bytes, quota_bytes}` |
| POST | `/api/v1/sync/blobs/{id}` | device | `{size}` opens or finds the file → `{id, size, received}`; `409 size` for another size, `413 quota` |
| PUT | `/api/v1/sync/blobs/{id}?offset=` | device | Body: the next bytes at `offset`, which must equal `received` (`409 offset`), with a `Content-Length` of 1 byte to 8 MiB (`411 length`), streamed to disk within 2 minutes; one piece per file and four per account at a time (`429`) → `{id, size, received}` |
| GET | `/api/v1/sync/blobs/{id}?offset=&length=` | device | Bytes of a complete file, at most 8 MiB per request (`409 incomplete` before) |
| DELETE | `/api/v1/sync/blobs/{id}` | device | Delete → `204` |
| GET | `/api/v1/sync/events` | device | Server-sent events, one stream per device |

Collections: `repos`, `library`, `playlists`, `settings`, `device` and `secrets` (only with
encryption).

Private files are opaque files of at most the account's files quota, named by 64 lowercase hex
digits, only in encrypted accounts. A file is uploaded in pieces and resumes at `received`;
incomplete files are deleted after 7 days. When the server holds fewer bytes of a file than
`received` (after a restore), it lowers `received` to what it holds. Clients encrypt them in 64 KiB segments with keys the
server never sees, and name them with a keyed hash so that one account stores identical files once
(see `Sync1` in `dbus.md`). Payloads are base64, at most 256 KiB each; a tombstone is `deleted: true` without
payload and is purged after 180 days, which raises `min_seq`.

Writes use optimistic concurrency per item. `base` is the `seq` of the item the client last saw
(0 for a new item). A matching base is stored with `seq = head + 1` and answers
`{item, status: "ok", seq}`; otherwise the answer is `{item, status: "conflict", current:
{seq, epoch, device, deleted, payload}}`, except that a retry of the same device's own last write
(same payload hash) answers `ok`. A different `epoch` refuses the batch with `409
epoch_mismatch`; going over the quota answers `413 quota` and stores nothing. A tombstone for an
item that already is one answers `ok` with its `seq` and writes nothing.

Key epochs: `key_id` (32 lowercase hex digits) names the key of the current epoch without
revealing it. `{epoch: current + 1, key_id}` starts a rotation and clears `rotate`; from then on
writes need the new epoch. `{epoch: current, done: true}` finishes it: records of older epochs are
deleted. `{epoch: current, key_id}` repeats the start (same `key_id`) or records the key id of an
account that has none. Anything else answers `409 epoch_mismatch`. Revoking a device sets
`rotate`, which asks the remaining devices to rotate.

Events: `event: seq` with `{"seq": n}` after any write, `event: mail` with `{"id": n}` for the
receiving device, `event: epoch` with `{"epoch": n}` after a rotation step or a revoked device
(`-1` after a reset). The stream of a revoked device ends.
Keep-alive comments arrive every 25 s.

### End-to-end encryption

Encryption is the default and plain-text sync exists only as an explicit opt-out that the
server labels with a warning. Clients keep a random 32-byte sync key `SK` that never reaches
the server:

- `K_rec = HKDF-SHA256(SK, info = "wprs-sync/records/v1" ‖ epoch)` encrypts records, and the
  first 16 bytes of `HKDF-SHA256(SK, info = "wprs-sync/key-id/v1")` in hex are its `key_id`.
- `K_id`, 32 random bytes that travel with `SK` and stay across rotations, names items (accounts
  set up before it existed keep `HKDF-SHA256(SK, info = "wprs-sync/ids/v1")`). `library` items are
  `hex(HMAC-SHA-256(K_id, repo_id ‖ 0x00 ‖ package id))` (or `blob:` and a hash for private
  files); the server accepts only 64 lowercase hex digits there, in both modes. Other items
  are random ids or keyed hashes of at most 64 characters of `A-Z a-z 0-9 _ -`.
- A record is `0x01 ‖ nonce (24) ‖ XChaCha20-Poly1305(K_rec, nonce, pad256(json), aad)` with
  `aad = "wprs-sync/1" ‖ user id ‖ epoch ‖ collection ‖ 0x00 ‖ item`. The plaintext is padded
  to a multiple of 256 bytes, so in an encrypted account the server refuses payloads unless
  `len = 41 + 256·k` with `k ≥ 1` and the first byte is 1.
- The JSON inside carries a hybrid logical clock; clients reject clocks older than the last
  one seen for an item.

What the server still sees: accounts, generic device names and platforms, public keys,
collection names, item ids, padded sizes, times and sequence numbers, and the sizes of private
files.

Recovery escrow (optional): `PUT /api/v1/sync/escrow` stores `SK` and `K_id` sealed with
XChaCha20-Poly1305 under a key that Argon2id (256 MiB, 3 passes) derives from a passphrase of at
least 20 characters; the escrow names its costs, salt and epoch, and the server cannot open it but
can try passphrases offline (format in `Sync1.SetEscrow` of `dbus.md`). A device without any other
device left links, fetches the escrow and opens it with the passphrase.

Key rotation: a device makes `SK'`, starts the next epoch with its `key_id`, re-encrypts and
uploads every record with the new epoch, finishes the epoch, and sends `SK'` through the mailbox
to the devices whose public keys it pinned in a code comparison, which pass it on to theirs; the
public keys the server lists are never trusted for this, and every other device joins again.

### Mailbox

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/v1/mailbox` | device | `{to_device, kind: key-share\|apply\|send\|ping, body: base64}` → `201 {id}` |
| GET | `/api/v1/mailbox?after=` | device | Messages for this device: `{items: [{id, from_device, kind, body, created_at}], more}` |
| DELETE | `/api/v1/mailbox/{id}` | device | Acknowledge → `204` |

Bodies hold at most 64 KiB; a device keeps at most 100 pending messages for 7 days. A new
device receives `SK` and `K_id` as a `key-share` sealed with HPKE (RFC 9180, auth mode, DHKEM
X25519, HKDF-SHA256, ChaCha20-Poly1305) to its `enc_key`, after a commit-then-reveal exchange of
nonces through the mailbox and a code derived from both public keys and both nonces that the
user compares on both screens (see `Sync1.Join` in `dbus.md`).
