# `.wpak` package format (draft v0)

Status: draft, not implemented yet (H2). Normative once implemented in `wprs-format`; one
validator is shared by the client, the server and the packaging tool.

## Container

A `.wpak` is a ZIP archive with these entries at the root:

| Entry | Required | Content |
|-------|----------|---------|
| `manifest.json` | yes | Metadata, see below |
| `SIGNATURE` | no | Author signature: minisign (`ED` format) over the exact bytes of `manifest.json` |
| `scene.json` | for scenes | Scene description |
| `preview.webp` / `preview.gif` | recommended | Static and animated previews |
| `assets/…` | no | Textures (KTX2, PNG, JPEG, WebP, AVIF), audio, fonts, glTF, video |
| `scripts/…`, `effects/…`, `locale/…` | no | Scripts, custom effects (JSON + WGSL), translations |

## Strict container profile

A package is rejected if any rule fails:

1. `manifest.json` lists every other entry except `SIGNATURE` in `files`, each with size and
   BLAKE3 hash. Unlisted, missing or mismatching entries invalidate the package. Readers open
   files only through that list and verify size and hash while reading.
2. Names are UTF-8, use `/`, are relative, have no empty, `.` or `..` segments, no `\`, `:`,
   NUL or control characters, are at most 255 bytes, and are unique after NFC normalization
   and case folding.
3. Only regular files. No symlinks, no directories with data, no encryption.
4. Compression is STORE or DEFLATE only. Media should use STORE so it can be read in place.
5. Exactly one end-of-central-directory record, no data before the first local header or
   after the central directory, no duplicate names, local headers consistent with the central
   directory, no overlapping entries.
6. Limits: entry count, per-entry size, total size, compression ratio, and decoded image
   dimensions, all configurable with safe defaults.
7. Packages are read in place. Nothing is ever extracted with a generic "extract all"; if a
   file must be written to disk it goes through `openat`-style APIs with no symlink following.

## Manifest

```json
{
  "schema": "wallpaper-rs.wpak/0",
  "id": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB",
  "version": "1.0.0",
  "type": "image | video | scene | web",
  "title": { "en": "…" },
  "license": "CC-BY-4.0",
  "content_rating": "everyone | mature",
  "photosensitive": false,
  "entry": "scene.json",
  "permissions": { "network": { "allow": [] }, "user_directories": false },
  "properties": { "general": {}, "user": [], "presets": [] },
  "files": [ { "path": "assets/bg.ktx2", "size": 123, "blake3": "…" } ],
  "origin": {
    "kind": "original | imported | provider",
    "derived_from": null,
    "provider": null, "source_url": null, "credit": null, "terms_ref": null
  }
}
```

`origin` is mandatory and inherited by derived packages; editing adds `derived_from` and never
removes it. Packages from image providers can only be published with a license on the
allow-list (public domain, CC0, CC-BY).
