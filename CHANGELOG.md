# Changelog

All notable changes to Samādhi are recorded here. Each release's section
becomes its notes on GitHub.

## 0.3.0

- **Windows:** Samādhi now runs on Windows 10 and 11. Download an installer
  (.msi, with a Start menu shortcut) or a standalone `Samadhi.exe` from
  the website, which also has Mac and Linux instructions. A second launch
  hands over to the running window there too, and Google Drive for desktop
  is found on its drive letter.
- **A website:** [anirudhi.github.io/samadhi](https://anirudhi.github.io/samadhi/).
- **Always Markdown:** every document is saved as `.md`. Other plain-text
  files still open. When you edit one (`.txt`, or an older document with
  another extension), its text goes to a `.md` beside it and the original
  is left untouched. An existing `.md` of the same name is never
  replaced: the new file gets a free name such as `Essay (2).md`. Save As
  always ends in `.md`, and the `default_extension` setting is gone.
- **Text size controls:** smaller and larger now sit side by side, as a
  small and a large letter A in your writing font, with a quiet "Reset"
  beside them.
- **Fixed:** text could vanish or turn into garbled glyphs for a moment
  after switching themes, until you clicked. egui empties its font atlas
  when it fills up, and the editor kept showing text drawn against the
  old one.
- **Recent files:** hovering the open button lists the last eight files
  you opened, newest first; click one to reopen it.
- **Frosted sheet:** the sheet behind the text is now frosted glass. The
  slider blurs the scene behind the page in, then whitens it towards
  paper. Snow, stars and other motes turn into soft glows behind it.
- **Fixed the long hangs while writing:** saving ran on the interface
  thread, and on an rclone Google Drive mount a save could wait up to a
  minute for an earlier upload of the same file to finish. Now:
  - Files are written by a background writer, so a slow or stuck drive
    can never freeze the window. If a save fails, Samādhi says so and
    tries again.
  - Every pause in typing goes straight to a local recovery cache, one
    slot per document, so nothing is lost even if the app closes while a
    drive is stuck. That text is restored, and saved, next time.
  - Saves of the same file are merged and spaced at least 15 seconds
    apart (Ctrl+S, switching documents and quitting save at once).
    Uploads no longer pile up, overlap and retry.
  - On a FUSE mount (rclone and the like), files are rewritten in place.
    Drive then updates the one file and keeps its version history,
    instead of a brand-new file appearing with every save.
- **No more freezes from files or dialogs:** the open and save dialogs no
  longer block the app, and are attached to its window so they can't open
  hidden behind it. Files open, and the check for changes from other
  computers runs, off the interface thread, so a slow drive can't stall
  typing.
- **Faster with long documents:** the styled text and word count are only
  rebuilt when something changes, and spelling underlines are only worked
  out for the visible part of the page. Idle frames on a 40,000-word
  document are about three times quicker.
- **A log, and a watchdog:** Samādhi now keeps a log file (`samadhi
  --help` shows where), noting errors and any moment the interface stops
  responding, with what it was doing.
- **Fixed** (found by new fuzz tests):
  - A crash when deleting a word right after the document changed on
    another computer.
  - Undo could bring back the previous document's text.
  - A crash when resizing the page in a very small window.
  - A document containing a NUL character couldn't be reopened.
  - A hand-edited extension like `. md` kept a space in new file names.
  - A crash-recovered draft lost spaces at the ends of its file's name.

## 0.2.0

- **Focus timer:** set 1 to 60 minutes with a slider, or start 5 or 25
  minutes with one click. A small ring and countdown under the page tick
  down while you write, click to pause, and a soft chime marks the end.
- **Google Drive:** when Google Drive is synced to your computer, new
  documents go to a `Samadhi` folder in it.
- **Writing on several computers:** a file changed on another computer
  reloads within seconds of syncing. If you also have unsaved edits, the
  other version is kept beside yours as a conflicted copy.
- **One copy at a time:** launching Samādhi again (or opening a file with
  it) brings the running app forward instead of starting a second one.
- **Faster start:** the window appears straight away, while the document
  loads in the background (useful on a network drive), and the graphics
  set up faster.
- **macOS:** the app adds `Samādhi.app` to `~/Applications` on first run,
  so a Homebrew install appears in Launchpad and Spotlight. The open
  dialog no longer greys out text files with other extensions.

## 0.1.0

The first release.

- Full-screen, distraction-free writing with a page you can move and resize.
- Sixteen themes: eight landscape photographs and eight painted scenes, with
  gentle parallax, ambient motion and a vignette that deepens as you write.
- Five bundled music tracks and three generated soundscapes; nine subtle
  typing sounds.
- Live markdown styling and spell check (both optional).
- Drafts until the first save, then autosave; unsaved work survives crashes.
- Word count, including the words in the current selection.
- Curated fonts: Literata, Newsreader, Inter, iA Writer Quattro and Mono.
