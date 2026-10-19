<p align="center"><img src="assets/icon/samadhi.svg" width="96" alt=""></p>

# Samādhi

*Samādhi* (Pali): concentration, one-pointed attention.

A calm, distraction-free, full-screen writing app for Linux, heavily inspired
by [OmmWriter](https://ommwriter.com). You get one page, a living landscape
behind it, ambient music and soft typing sounds. The interface fades away the moment you start typing and
comes back when you move the mouse.

New writing starts as a draft. Save it once to give it a name, and from then
on it saves itself a moment after you stop typing. Unsaved drafts survive
crashes and power cuts, and Samādhi reopens where you left off. Documents
are plain text, with optional live markdown styling. OmmWriter `.omm` files
open and save natively.

## Features

- **Always full screen.** There is no window chrome. Esc saves and quits.
- **Saving:**
  - A new document is an unsaved draft. Press Ctrl+S, or use the save
    button, to name it; the name is pre-filled from your first line.
  - After that first save the file autosaves under a second after you pause.
  - Opened files autosave the same way.
  - Until a draft is saved it is continuously mirrored to a recovery cache
    (`~/.local/share/samadhi/recovery/`). After a crash or forced shutdown it
    is restored on the next launch.
  - Quitting, starting a new document or opening a file with an unsaved
    draft asks whether to save it first.
  - The last saved file opens again at start-up.
- **Live markdown (optional).** As you type, `# headings`, `**bold**`,
  `*italic*`, `> quotes`, lists, `` `code` ``, links and `~~strikethrough~~`
  render styled, much like Notion. The syntax marks fade away except on the
  line you're editing. Toggle it in the font menu.
- **Sixteen themes:**
  - Eight public-domain landscape photographs: Hazy Hills, White Forest,
    Desert Haze, Sea Mist, Fog Forest, Dusk Ridges, Milky Way, Island in Fog.
    Where a photo is bright in one part and dark in another, a soft veil
    keeps the text readable.
  - Eight landscapes painted in code: Dawn Mist, Snowfield, Dunes, Sea Glass,
    Paper, Dusk, Forest Night, Night Sky.

  Each theme sets the text colour, and the interface switches to light text
  on dark themes. The controls are bare icons with no panels.
- **Immersion:**
  - Layered scenery moves with gentle mouse parallax.
  - Distant layers are blurred for depth of field.
  - A vignette deepens while you write.
  - Ambient motion: drifting fog, snowfall, twinkling stars, fireflies,
    dust motes.

  *Reduce motion* turns all of it off.
- **Sound.** Endless generated soundscapes (an ambient pad, gentle rain,
  ocean swell, and a coffee shop with distant chatter, cups and the odd
  espresso machine), levelled to the same loudness, with mute and volume
  controls. There are nine key sounds,
  from barely-there (Felt, Whisper, Pencil, Raindrop, Wood tap) to classic
  (Soft tick, Typewriter, Bamboo, Droplet). Each keystroke varies slightly,
  so fast typing sounds natural.
- **Typography.** A few curated, bundled faces:
  - Literata (default), Newsreader and Inter (Sans), set Light, with true
    bold.
  - iA Writer Quattro and Mono.

  Lines are generously spaced, and a centred word count sits under the page.
- **The page.** A dashed frame you can move and resize by dragging its
  edge. Handles appear when you approach it. An optional sheet behind the
  text makes busy backgrounds easier to read.
- **Safe saving:**
  - Saves are atomic: a crash can never leave a half-written file.
  - The original encoding (UTF-8 with or without BOM, UTF-16, legacy
    Windows-1252) and line endings are preserved, so files round-trip back
    to OmmWriter unchanged.

## Install

You need a Rust toolchain (1.95 or newer) and the ALSA development headers:

```sh
sudo apt install libasound2-dev        # Debian, Ubuntu, Pop!_OS
./packaging/install.sh
```

The script builds a release binary and installs everything for your user
only, with no root needed:

- the binary at `~/.local/bin/samadhi`
- the icons and the launcher
- an `.omm` file type, so you can double-click OmmWriter documents

You can also run it straight from the source tree with
`cargo run --release -- [file.omm]`.

## Use

| Keys | Action |
| --- | --- |
| Esc | Quit (saved files are already saved; asks about unsaved drafts) |
| Ctrl+N / Ctrl+O | New document / Open |
| Ctrl+S | Save (names a new draft; later just saves) |
| Ctrl+Shift+S | Save as (a new name; autosave continues there) |
| Ctrl+= / Ctrl+- | Larger / smaller text |
| Ctrl+M | Minimise |

Move the mouse to bring the dock back, then hover an icon to see its options.

### Your own backgrounds, music and clicks

Drop files into these folders and they appear after the built-in ones:

| Folder | Formats |
| --- | --- |
| `~/.local/share/samadhi/backgrounds/` | jpg, png, webp, gif, bmp, tiff |
| `~/.local/share/samadhi/music/` | mp3, ogg, flac, wav (loops seamlessly) |
| `~/.local/share/samadhi/clicks/` | mp3, ogg, flac, wav (first 2 s used) |

Your own images become themes, and Samādhi picks light or dark text from
their brightness.

Settings live in `~/.config/samadhi/settings.toml`, which you can edit by
hand:

- `documents_dir` changes the folder the save dialog starts in.
- `default_extension` chooses between `.omm` and `.txt` for new documents.

## Development

```sh
cargo test                                   # unit and headless UI tests
cargo clippy --all-targets -- -D warnings    # pedantic lints are enabled
cargo test -- --ignored render_previews      # writes UI and theme renders to target/previews/
cargo run --example render_icons             # re-renders the PNG icons from the SVG
cargo run --release --example prepare_photos -- <dir>  # resizes source photos into assets/photos/
cargo run --release --example time_scenes    # how long each theme takes to paint
```

Layout of `src/`:

| Module | What it does |
| --- | --- |
| `app` | UI and input |
| `stage` | Parallax, particles and vignette |
| `scene` | Procedural landscapes |
| `document` | Text and `.omm` I/O |
| `markdown` | Live markdown styling |
| `audio` | Playback and sound synthesis |
| `settings`, `page`, `fonts`, `theme`, `icons`, `widgets` | Supporting pieces |

## Credits and licence

Samādhi is free software, released under the GNU General Public License,
version 3 (see [`LICENSE`](LICENSE)). Copyright © 2026 Anirudh Iyer and the
Samādhi contributors.

Samādhi is heavily inspired by [OmmWriter](https://ommwriter.com), whose
calm, immersive approach to writing (soft backgrounds, ambient sound and
gentle typing sounds) it follows closely. It is an independent open-source
project, not affiliated with or endorsed by OmmWriter or its makers.
"OmmWriter" is used here only to describe that inspiration and the `.omm`
file format Samādhi can open.

It began as a Rust port of KoalaWriter (© 2010 Alessio Mazzarelli, GPLv3), a
Qt4 OmmWriter clone, and has since been rewritten and extended.

Bundled material keeps its own licence. [`NOTICE.md`](NOTICE.md) has the
full list:

- **Fonts:** [Literata](https://github.com/googlefonts/literata),
  [Newsreader](https://github.com/productiontype/Newsreader),
  [Inter](https://github.com/rsms/inter) and
  [iA Writer Quattro and Mono](https://github.com/iaolo/iA-Fonts), all under
  the SIL Open Font License.
- **Icons:** [Lucide](https://lucide.dev), under ISC (some derived from
  Feather, MIT).
- **Photographs:** public-domain (CC0) images from Wikimedia Commons;
  photographers are credited in `assets/photos/CREDITS.md`.
- **US English dictionary:** from SCOWL, under its permissive licence.

The sounds, painted scenes and app icon were made for this project.
