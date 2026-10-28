<p align="center"><img src="assets/icon/samadhi.png" width="112" alt=""></p>

# Samādhi

*Samādhi* (Pali): concentration, one-pointed attention.

A calm, distraction-free, full-screen writing app for Linux and macOS,
heavily inspired by [OmmWriter](https://ommwriter.com). You get one page,
a living landscape behind it, ambient music and soft typing sounds. The
interface fades away the moment you start typing and comes back when you
move the mouse.

New writing starts as a draft. Save it once to give it a name, and from then
on it saves itself a moment after you stop typing. Unsaved drafts survive
crashes and power cuts, and Samādhi reopens where you left off. Documents
are plain text, saved as Markdown (`.md`) by default, with optional live
markdown styling.

## Features

- **Always full screen.** There is no window chrome. Esc saves and quits.
- **One window.** Launching Samādhi again, or opening a file with it from
  your file manager, brings the running app forward (and opens that file)
  instead of starting a second copy.
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
  - Hover the open button for your eight most recent files; click one to
    reopen it.
  - Files open in the background and file dialogs never block the app, so
    a slow or network drive can't freeze the window.
- **Google Drive, and writing on several computers:**
  - If Google Drive is synced to your computer, new documents go to a
    `Samadhi` folder in it (inside `Writing`, if your Drive has that
    folder), so your writing is backed up and on every machine. Samādhi
    finds Google Drive for desktop on macOS, and a Drive mounted at
    `~/GoogleDrive` or `~/Google Drive` on Linux.
  - When the open file changes on another computer, Samādhi picks up the
    new version within seconds of it syncing.
  - If you have unsaved edits when that happens, neither version is lost:
    yours stays open and the other is saved beside it as
    `Name (conflicted copy).md`.
- **Focus timer:**
  - Set a length from 1 to 60 minutes with the slider, or start 5 or 25
    minutes with one click.
  - A small ring and countdown sit under the page's corner and stay
    faintly visible while you write. Click them to pause.
  - A soft bowl chime sounds when the time is up.
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
- **Sound:**
  - Five calm music tracks.
  - Three endless generated soundscapes (an ambient pad, gentle rain, ocean
    swell), levelled to the same loudness.
  - Nine key sounds, from barely-there (Felt, Whisper, Pencil, Raindrop,
    Wood tap) to classic (Soft tick, Typewriter, Bamboo, Droplet). Each
    keystroke varies slightly, so fast typing sounds natural.
  - Mute and volume controls.
- **Typography.** A few curated, bundled faces:
  - Literata (default), Newsreader and Inter (Sans), set Light, with true
    bold.
  - iA Writer Quattro and Mono.

  Lines are generously spaced, and a centred word count sits under the page.
- **The page.** A dashed frame you can move and resize by dragging its
  edge. Handles appear when you approach it.
- **Frosted sheet.** The *Sheet* slider in the theme menu puts frosted
  glass behind the text: the landscape blurs softly behind the page
  (moving with the parallax), then whitens towards paper further along.
- **Safe saving:**
  - Saves are atomic: a crash can never leave a half-written file.
  - Files you open keep their original encoding (UTF-8 with or without BOM,
    UTF-16, legacy Windows-1252) and line endings, so text written elsewhere
    round-trips unchanged.

## Install

**macOS (Homebrew):**

```sh
brew install anirudhi/tap/samadhi
```

**Linux and macOS (installer script):**

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/anirudhi/samadhi/releases/latest/download/samadhi-installer.sh | sh
```

This puts `samadhi` in `~/.local/bin`, with no root needed.

The first time you run it (type `samadhi` in a terminal), Samādhi adds
itself to your apps, with its icon:

- **macOS:** `~/Applications/Samādhi.app`, which appears in Launchpad and
  Spotlight and can be kept in the Dock.
- **Linux:** an entry in your application menu.

You can also download a build for your system from the
[releases page](https://github.com/anirudhi/samadhi/releases).

### Updating

- **Homebrew:** `brew upgrade samadhi`
- **Installer script:** run `samadhi-update`, or run the installer again.

### Building from source

You need a Rust toolchain (1.95 or newer), plus the ALSA development
headers on Linux:

```sh
sudo apt install libasound2-dev        # Debian, Ubuntu, Pop!_OS
./packaging/install.sh
```

The script builds a release binary and installs it, with the launcher
and icons, for your user only. You can also run it straight from the source
tree with `cargo run --release -- [file.md]`.

## Use

| Keys | Action |
| --- | --- |
| Esc | Quit (saved files are already saved; asks about unsaved drafts) |
| Ctrl+N / Ctrl+O | New document / Open |
| Ctrl+S | Save (names a new draft; later just saves) |
| Ctrl+Shift+S | Save as (a new name; autosave continues there) |
| Ctrl+= / Ctrl+- | Larger / smaller text |
| Ctrl+M | Minimise |

On macOS, use Cmd in place of Ctrl.

Move the mouse to bring the dock back, then hover an icon to see its
options. From top to bottom the dock holds: font (with the markdown and
spell-check switches), text size, theme (with motion and the frosted
sheet), music, typing sounds, the focus timer, and the document (new,
open with recent files, save, save as).

### Your own backgrounds, music and clicks

Drop files into these folders and they appear after the built-in ones:

| Folder | Formats |
| --- | --- |
| `~/.local/share/samadhi/backgrounds/` | jpg, png, webp, gif, bmp, tiff |
| `~/.local/share/samadhi/music/` | mp3, ogg, flac, wav (loops seamlessly) |
| `~/.local/share/samadhi/clicks/` | mp3, ogg, flac, wav (first 2 s used) |

Your own images become themes, and Samādhi picks light or dark text from
their brightness.

### Settings

Settings live in `~/.config/samadhi/settings.toml` on Linux and
`~/Library/Application Support/samadhi/settings.toml` on macOS. Everything
in them is set from the app, but you can edit the file by hand while
Samādhi is closed:

- `documents_dir` sets the folder new documents are saved in.
- `default_extension` sets the extension for new documents (`md` by
  default; letters and digits only).
- `timer_minutes` is the focus timer's length (1 to 60).
- `recent_files` is the list behind the open button (at most eight).

Out-of-range or malformed values fall back to sensible defaults, so a bad
edit never stops the app from starting.

### If something goes wrong

Samādhi keeps a log, which records any error and notes when the interface
was unresponsive for more than half a second, with what it was doing at
the time:

- Linux: `~/.local/state/samadhi/samadhi.log`
- macOS: `~/Library/Application Support/samadhi/samadhi.log`

`samadhi --help` prints its location. Please include it when reporting a
problem.

## Development

```sh
cargo test                                   # all tests (about 10 s)
cargo clippy --all-targets -- -D warnings    # pedantic lints are enabled
cargo test -- --ignored render_previews      # writes UI and theme renders to target/previews/
cargo run --release --example render_icons   # re-renders the icon sizes from assets/icon/samadhi.png
cargo run --release --example prepare_photos -- <dir>  # resizes source photos into assets/photos/
cargo run --release --example time_scenes    # how long each theme takes to paint
```

### Tests

Every module has unit tests next to its code. On top of those:

- **Interface tests** (`src/app.rs`, `ui_tests`) drive the real app
  headlessly with [egui_kittest](https://crates.io/crates/egui_kittest):
  typing, the dock, saving, drafts and crash recovery, dialogs, files
  changing on another computer, the timer and more.
- **Stress tests** (`src/app.rs`, `stress_tests`) throw long runs of random
  input at the app, check that no frame takes long, and check that no text
  is ever lost. A second test does the same on a novel-length document
  (about 40,000 words). Runs are reproducible from their seed; make them
  longer with:

  ```sh
  SAMADHI_FUZZ_SEEDS=40 SAMADHI_FUZZ_STEPS=1500 cargo test --release stress_tests
  ```
- **Property tests** (`tests/fuzz/`) use
  [proptest](https://crates.io/crates/proptest) to feed arbitrary input to
  every parser and pure function: file decoding and encoding, markdown,
  spell checking, settings, page geometry, the timer, frosted glass, crash
  recovery and the single-instance socket. For more cases, run:

  ```sh
  PROPTEST_CASES=5000 cargo test --release --test fuzz
  ```

[`tests/README.md`](tests/README.md) has more detail.

### Layout of `src/`

| Module | What it does |
| --- | --- |
| `app` | The interface: input, drawing, saving and dialogs |
| `document` | Reading and writing plain-text and Markdown files |
| `recovery` | The crash-recovery cache for unsaved drafts |
| `markdown` | Live markdown styling |
| `spell` | Spell checking and suggestions |
| `stage` | Parallax, particles, vignette and the frosted sheet |
| `scene`, `background` | Painted and photographic landscapes, loaded off-thread |
| `frost` | The blurred layer copies behind the frosted sheet |
| `audio` | Playback and sound synthesis |
| `timer` | The focus timer |
| `instance` | One running copy; later launches hand over to it |
| `watchdog` | Notices and logs an unresponsive interface |
| `desktop`, `macos` | Adding the app to the Linux menu and macOS Applications |
| `settings`, `page`, `catalog`, `fonts`, `theme`, `icons`, `widgets` | Supporting pieces |

## Releasing a new version

Releases are built and published by GitHub Actions using
[dist](https://github.com/axodotdev/cargo-dist):

1. Bump `version` in `Cargo.toml` and add a section for it at the top of
   `CHANGELOG.md`; that section becomes the release notes.
2. Commit, then tag and push:

   ```sh
   git tag v0.2.0 && git push && git push --tags
   ```

The release workflow then:

- builds Linux and macOS binaries (Intel and ARM);
- publishes a GitHub Release with installers and the updater;
- updates the Homebrew formula in
  [anirudhi/homebrew-tap](https://github.com/anirudhi/homebrew-tap).

Release settings live in `dist-workspace.toml`. After upgrading `dist`, run
`dist generate` to refresh the workflow.

## Credits and licence

Samādhi is free software, released under the GNU General Public License,
version 3 (see [`LICENSE`](LICENSE)). Copyright © 2026 Anirudh Iyer and the
Samādhi contributors.

Samādhi is heavily inspired by [OmmWriter](https://ommwriter.com), whose
calm, immersive approach to writing (soft backgrounds, ambient sound and
gentle typing sounds) it follows closely. It is an independent open-source
project, not affiliated with or endorsed by OmmWriter or its makers.
"OmmWriter" is used here only to describe that inspiration.

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
- **Music:** the five bundled tracks were generated with Google Gemini and
  are dedicated to the public domain (CC0) as far as any rights exist.

The sounds, painted scenes and app icon were made for this project.
