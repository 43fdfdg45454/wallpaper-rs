# Packages

A `.wpak` package is a ZIP file with `manifest.json` at its root, the entry it names, and the
files that entry uses. `wprs-format` reads it strictly: stored or deflated entries only, no
encryption, no symbolic links, safe relative paths (no `..`, `\`, `:` or control characters,
at most 255 bytes), names unique without regard to case, at most 4096 files, 1 GiB per file and
2 GiB in total, a compression ratio of at most 100 for files above 1 MiB, and a manifest of at
most 1 MiB. Repositories (`repo.md`) and the server (`server.md`) distribute packages; scenes are
described in `scene.md`. An author signs the exact bytes of `manifest.json` into
`manifest.json.minisig`.

| `kind` | `entry` | Other files |
|--------|---------|-------------|
| `image` | png, jpg, jpeg, webp, avif, gif, bmp or ktx2 | images, txt |
| `video` | webm, mp4 or mkv | images, videos, txt |
| `scene` | `scene.json` | images, videos, json, wgsl, js, gltf, glb, bin, ogg, opus, flac, mp3, wav, ttf, otf, cube, txt |

## Versions

`schema` is `wallpaper-rs.wpak/1` or `wallpaper-rs.wpak/2`, and every reader (daemon, library,
repositories, server, sync and the Android app) accepts both. A first-version package keeps
working unchanged: it has no rating, so clients show it as not rated and the `content:unrated`
filter finds it. Within `/2` only optional fields, enum values and feature names are added; the
meaning of what exists never changes, and a new major version is made only to remove or change
something. The first version keeps its rules: none of the fields below marked *2* may appear
in it, and its texts are plain.

Readers work in two steps. They first read a tolerant header (`schema` and `requires`): a newer
major version, a `requires.engine` that this version of wallpaper-rs does not meet, or a name in
`requires.features` that it does not know returns `Newer` (`this package needs a newer version of
wallpaper-rs: it uses …`) instead of an error about an unknown field. Only then is the manifest
read strictly, refusing unknown fields. `wprs-ctl pack upgrade DIR --rating general|mature
[--reason …] --photosensitive true|false [--derived-from AUTHOR]` turns a first-version folder into
the second version: it never guesses the rating, moves `preview` to `previews.thumbnail`, sets
`default_locale` when the texts are in several languages (`en` when all of them have English, or a
language they all share), renames the property ids with a `-` (`snow-speed` becomes `snow_speed`)
in the manifest and in the bindings and triggers of the scene, adds the inputs and features the
scene uses and removes a signature that no longer matches. `wprs-ctl scene upgrade` does the same in
the scene editor (`editor.md`).

In the desktop library a package that needs a newer version still shows its thumbnail
(`previews.thumbnail` or `preview`, read without the strict manifest) next to the reason. The
library remembers which version of wallpaper-rs read each package and reads every package again once
after an upgrade, so packages that needed a newer version start working and older ones get the
facets of the newer rules. Versions of wallpaper-rs released before the second version cannot open
its packages; repositories keep them readable for those versions by listing only first-version
releases where they look (`repo.md`).

## Manifest

| Field | Version | Rules |
|-------|---------|-------|
| `schema`, `id`, `version`, `kind`, `entry` | 1, 2 | `id` is a lowercase reverse-DNS name (`org.example.dusk`), `version` SemVer |
| `title` | 1, 2 | 1 to 128 characters on one line; in *2* also `{language: text}` or `"@key"` |
| `description` | 1, 2 | plain text in *1*; in *2* text, map or `"@key"` of at most 8000 characters, with the markup below |
| `changes` | 2 | notes of this version, at most 2000 characters, with the markup below |
| `default_locale`, `strings` | 2 | see "Languages" |
| `authors`, `license`, `tags` | 1, 2 | authors of 1 to 128 characters, an SPDX expression, at most 32 tags of 1 to 32 characters; in *2* authors and tags have no `<` or `>` |
| `content` | 2, required | `{rating: general \| mature, descriptors: [sexual, violence, gore, language]}`; `mature` names at least one reason, each once |
| `photosensitive` | 2, required | `true` when the package flashes or shows fast patterns that can trigger seizures |
| `origin` | 2, required | `{kind: original}` for a work made for wallpaper-rs, or `{kind: derived, derived_from: [{author, title?, url?, license?}]}` for one remade by hand from 1 to 64 other works, crediting their authors (authors and titles without `<` or `>`) |
| `credits` | 2 | at most 64 `{paths, license, attribution, url?}`: who made which files (a path ending in `/` names a folder) under which SPDX license; paths and attribution have no `<` or `>` |
| `links` | 2 | `{homepage, source, support, donate}`, each optional, `https://` and at most 512 characters |
| `preview` | 1 | an image of the package |
| `previews` | 2 | `{thumbnail, animated}`: png, jpg or webp of at most 1024 px a side and 1 MiB; gif or webp of at most 640 px, 15 s and 8 MiB; the file must be what its extension says |
| `requires` | 2 | `{engine: "<SemVer requirement>", features: [..]}`, see "Features" |
| `inputs` | 2 | `[audio, pointer, clicks, media, network]`, see "Inputs" |
| `properties`, `presets` | 1, 2 | user properties (`scene.md`): in *1* at most 64 with ids `[a-z][a-z0-9_-]{0,62}`; in *2* at most 256 with ids `[A-Za-z][A-Za-z0-9_]{0,63}` that differ in more than case, and combos of at most 64 options; at most 32 presets in *1*, 64 in *2* |

```json
{
  "schema": "wallpaper-rs.wpak/2", "id": "org.example.dusk", "version": "1.3.0", "kind": "scene",
  "entry": "scene.json", "default_locale": "en", "title": { "en": "Dusk", "es": "Atardecer" },
  "description": "@description", "changes": "@changes",
  "strings": { "en": { "description": "Hills at **dusk**.", "changes": "- Warmer light", "speed": "Speed" },
               "es": { "description": "Colinas al **atardecer**.", "changes": "- Luz más cálida", "speed": "Velocidad" } },
  "authors": ["Example"], "license": "CC-BY-4.0",
  "credits": [{ "paths": ["assets/hills.webp"], "license": "CC0-1.0", "attribution": "Example" }],
  "origin": { "kind": "original" }, "content": { "rating": "general" }, "photosensitive": false,
  "previews": { "thumbnail": "preview.webp", "animated": "preview.gif" },
  "links": { "source": "https://example.org/dusk" },
  "requires": { "features": ["inputs.pointer"] }, "inputs": ["pointer"],
  "properties": { "speed": { "type": "slider", "label": "@speed", "min": 0, "max": 2, "step": 0.1, "default": 1 } }
}
```

## Rating and photosensitivity

The rating and the photosensitivity flag are declared by the author and never inferred. Clients
blur the thumbnails of packages rated `mature` until the user chooses to show them, from the
details of the item or with a click or a key on the blurred thumbnail of a screen in the app's
Settings page or the tray, and `library.hide_mature` leaves them out of searches and repository
listings. A photosensitive package shows a clear warning before it is applied, in the app,
`wprs-ctl` and the Android app, and the app can allow it from its details; the app's Settings page
and the tray warn on the screen that shows one. Until the user allows it the daemon refuses to
apply it (`Unconfirmed`), a profile leaves it out (the screen reports why) and playlists skip it.
Both decisions are kept once per package id (`Library1.Confirm`,
`wprs-ctl lib confirm mature|photosensitive ITEM`). The self-hosted server refuses packages rated
`mature`.

## Inputs

`inputs` declares what the package may ask the user for: the sound playing on the computer
(`audio`), the pointer (`pointer`), clicks (`clicks`), what the media player plays (`media`) and the
network (`network`). Declaring is no grant: each input is still allowed by the user per package,
when it is used. For scenes the validator computes the inputs that the scene can use with any value
of its properties and presets (audio and pointer bindings, spectra, beat triggers, parallax,
particles that follow the pointer or burst on clicks, effect parameters such as the clicks of
`water_ripple`) and refuses a manifest that does not declare them, naming the missing one and the
first place of `scene.json` that uses it (`the scene uses audio at /layers/2/effects/0 of
scene.json; add "audio"`).

## Features

`requires.features` lists, from a closed registry, what the package needs from the engine, so that
an older version or a weaker device says that it needs a newer one before reading anything else.
`wprs-ctl pack build` computes the list from the manifest and the scene and writes the missing
names into `manifest.json` before building (a signed folder must be signed again: `pack sign`
fills the list before signing); readers refuse a package that uses a feature it does not list,
and `wprs-ctl pack check` explains each one and warns about listed features the package does not
use.

| Feature | The package |
|---------|-------------|
| `inputs.audio`, `inputs.pointer`, `inputs.clicks`, `inputs.media`, `inputs.network` | declares or uses that input |
| `scripts` | runs scene scripts |
| `sound` | plays sound layers |
| `effect.<id>` | uses a built-in effect that older versions do not have; `pack build` and `pack check` name the ones a package needs |

Each profile states what it runs. The desktop runs every feature above; the Android app runs every
one except `scripts` and `sound`: such packages play there without their scripts and sounds, their
page on the phone says so, and only packages that run fully on the phone get the facet `android`.

## Languages

Texts are plain text, a map from language to text, or `"@key"`, a key of `strings`. `strings`
holds at most 64 languages, each with at most 2048 keys of `[A-Za-z0-9_.-]{1,64}` and 512 KiB in
all; languages are BCP 47 tags such as `en`, `es` or `pt-BR`. With maps or `strings`,
`default_locale` is required and every localized text has a value in it. Labels, help texts,
combo options and preset labels of properties take the same forms. A text is picked by comparing
tags without case: the user's languages in order (each as given, then its base language, `pt-BR`
then `pt`), then `default_locale`, then `en`, then the first one; `zh-chs`, `zh-cn` and `zh-sg`
read as `zh-Hans`, and `zh-cht`, `zh-tw`, `zh-hk` and `zh-mo` as `zh-Hant`. Control characters
(except line breaks in long texts) and byte order marks are refused.

## Markup

`description` and `changes` use a small, safe markup: paragraphs separated by an empty line,
lists of lines starting with `- ` or `* `, `**bold**`, `*italic*` and links `[text](https://…)`.
A `*` or `**` opens before a word and closes after one on the same line; one without a partner is
refused, and `\*` writes an asterisk. HTML, images and links other than `https://` are refused.
Clients render it without loading anything remote and open a link only after a click and a
confirmation (through the OpenURI portal in the Flatpak); `wprs-ctl` prints it as plain text. Every
other text, and the description of a first-version package, is plain: clients show it as written,
and in the second version plain texts (titles, labels, help, options, preset labels) have no `<`
or `>`.

## Facets and tags

Repositories, the library and the server compute facets from the package itself, so that authors
cannot lie: the inputs the scene uses (`audio`, `pointer`, `clicks`), `sound`, `scripts`, `clock` (a
text shows the time), `customizable` (it has properties to change), `photosensitive`, `localized`
(more than one language), `animated` (scenes and videos), `android` (it runs fully on the phone),
the orientation (`landscape`, `portrait`, `square`), `ultrawide` from a 2.2 ratio and the size
(`720p`, `1080p`, `1440p`, `4k`, `5k`, `8k`) of scenes and images. Only `media` and `network` come
from `inputs` as the author declares them, since nothing in a package shows that use; an `audio`,
`pointer` or `clicks` input the scene cannot use gets a warning and no facet. Searches filter by
them (`facet:audio`, `facet:4k`) and by rating (`content:general`, `content:mature`,
`content:unrated`). Tags stay free; these lowercase slugs are recommended for genres: `abstract`,
`anime`, `animals`, `cars`, `cityscape`, `fantasy`, `games`, `minimal`, `mountains`, `music`,
`nature`, `pixel-art`, `retro`, `sci-fi`, `space`, `water`, `winter`.
