# Changelog

All notable changes to Samādhi are recorded here. Each release's section
becomes its notes on GitHub.

## Unreleased

- **Recent files:** hovering the open button lists the last eight files
  you opened, newest first; click one to reopen it.
- **Frosted sheet:** the sheet behind the text is now frosted glass. The
  slider blurs the scene behind the page in, then whitens it towards
  paper. Snow, stars and other motes turn into soft glows behind it.
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
