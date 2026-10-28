# Samādhi's tests

`cargo test` runs everything below in about ten seconds. Tests never touch
your real settings, documents or audio device: each one works in a
temporary folder with sound switched off.

## Where the tests live

| Where | What it covers |
| --- | --- |
| `src/*.rs` (`mod tests`) | Unit tests for each module, next to the code they test |
| `src/app.rs` (`ui_tests`) | The real app, driven headlessly: typing, the dock and flyouts, drafts and crash recovery, autosave, dialogs (answered by the test), recent files, files changed on another computer, the timer, a second launch handing over a file |
| `src/app.rs` (`stress_tests`) | Random input fuzzing of the whole interface, and a novel-length document |
| `tests/fuzz/` | Property tests of every parser and pure function, with [proptest](https://crates.io/crates/proptest) |

## Stress tests

`fuzz_the_interface_with_random_input` launches the app on a document full
of awkward text: markdown, misspellings, emoji, combining accents,
right-to-left script and zero-width spaces. It then does random things for
a few hundred steps:

- types and pastes text, presses keys with any modifiers, and cuts, copies
  and undoes;
- clicks, double-clicks, drag-selects, right-clicks and scrolls;
- opens each dock flyout and clicks somewhere in it;
- answers draft questions and file dialogs at random;
- toggles settings, rewrites the file "from another computer", and lets
  time pass so autosave runs.

After every step it checks that no frame took over a second and that the
settings are still valid. At the end it checks that the page's text is in
the file (or, for a draft, in the recovery cache).

Each run is reproducible from its seed. For a longer run:

```sh
SAMADHI_FUZZ_SEEDS=40 SAMADHI_FUZZ_STEPS=1500 cargo test --release stress_tests
```

`a_novel_length_document_stays_responsive` does the slow things (select
all, copy, paste a chapter) on about 40,000 words, with the same frame
limit.

## Property tests (`tests/fuzz/`)

One test binary, with a module per area:

| Module | What it checks |
| --- | --- |
| `document` | Any text round-trips through save and open in every supported encoding and line-ending style; arbitrary bytes never crash opening; word counts; conflicted-copy names; atomic writes |
| `markdown` | Parsing and styling never panic on any text or cursor position, and styled spans always cover the text exactly |
| `spell` | Misspelling ranges and suggestions are always valid for any text |
| `settings` | Any settings file loads without panicking; cleaned-up values are always in range; save then load gives the same settings |
| `page` | The page stays on screen and at least the minimum size through any drag, on any screen size |
| `timer` | Any sequence of start, pause, reset and length changes keeps the countdown consistent |
| `frost` | Blurred copies are smaller, valid and colour-preserving for any image |
| `recovery` | Drafts and their file paths come back exactly |
| `instance` | Requests from a second launch arrive intact |
| `catalog`, `scene` | Theme and sound lists, scene building |

For many more cases:

```sh
PROPTEST_CASES=5000 cargo test --release --test fuzz
```

When proptest finds a failing input, it shrinks it to a minimal one and
prints it. Keep that input as an ordinary `#[test]` beside the property,
so the case is checked from then on.

## Regression tests from fuzzing

These bugs were found by the fuzzers and are now fixed, each with a test:

- Deleting the next word right after the document shrank underneath the
  cursor (a reload from another computer) crashed inside egui.
- Undo could bring back the text of the previously open document.
- Resizing the page in a window smaller than the minimum page size crashed.
- A document containing a NUL character could be saved but not reopened.
- A hand-edited extension like `. md` kept a space in new file names.
- A crash-recovered draft lost spaces at the ends of its file's name.
