# Notices

## Samādhi

Copyright (C) 2026 Anirudh Iyer and the Samādhi contributors.

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License, version 3, as published by the
Free Software Foundation. It is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
or FITNESS FOR A PARTICULAR PURPOSE. See [`LICENSE`](LICENSE) for the full
text.

### Inspiration

Samādhi is heavily inspired by OmmWriter (<https://ommwriter.com>). It is an
independent project, not affiliated with or endorsed by OmmWriter or its
makers, and contains none of OmmWriter's code, images or sounds. The name is
used only to describe that inspiration.

### Original work

Samādhi began as a port to Rust of **KoalaWriter** 1.0, Copyright (C) 2010
Alessio Mazzarelli, released under the GNU GPL version 3
(<https://code.google.com/archive/p/koalawriter>). It has since been
rewritten and substantially extended; it remains under the same licence.

## Bundled third-party material

Everything below is included unmodified, under its own licence, which
permits this use. Each licence text sits next to the files it covers.

| Material | Files | Licence | Licence text |
| --- | --- | --- | --- |
| Literata typeface, © The Literata Project Authors | `assets/fonts/Literata.ttf` | SIL Open Font License 1.1 | `assets/fonts/Literata-OFL.txt` |
| Newsreader typeface, © The Newsreader Project Authors | `assets/fonts/Newsreader.ttf` | SIL Open Font License 1.1 | `assets/fonts/Newsreader-OFL.txt` |
| Inter typeface, © The Inter Project Authors | `assets/fonts/Inter.ttf` | SIL Open Font License 1.1 | `assets/fonts/Inter-OFL.txt` |
| iA Writer Quattro and Mono typefaces, © Information Architects Inc. (based on IBM Plex, © IBM Corp.); Reserved Font Names "iA Writer" and "Plex" | `assets/fonts/iAWriter*.ttf` | SIL Open Font License 1.1 | `assets/fonts/iA-Fonts-LICENSE.md` |
| Lucide icons, © Lucide Icons and Contributors; some icons (including `headphones` and `type`) derived from Feather, © Cole Bemis | `assets/icons/lucide/*.svg` | ISC (Feather-derived icons: MIT) | `assets/icons/lucide/LICENSE` |
| US English word list from SCOWL, © Kevin Atkinson and others | `assets/dictionaries/en_US.*` | SCOWL licence (permissive, notice required) | `assets/dictionaries/en_US-COPYRIGHT.txt` |
| Landscape photographs by Marcelo Quinan, Polina Kirilenko, Joe Mania, Kate Chikina, James Bloedel, Fré Sonneveld, Adam Mescher and Peter Lewis (via Wikimedia Commons) | `assets/photos/*.jpg` (resized) | CC0 1.0 (public domain) | `assets/photos/CREDITS.md` |

The OFL allows these fonts to be bundled with any software; the fonts
themselves stay under the OFL. The photographs were resized to 2560 px wide;
CC0 places no conditions on this.

Made for this project and covered by the GPL along with the code: the app
icon (`assets/icon/`, a watercolour by Anirudh Iyer), all sounds (synthesised
in `src/audio/synth.rs`) and the painted landscapes (generated in
`src/scene.rs`).

## Rust dependencies

Samadhi is built from crates published on crates.io, each under its own
licence; the full list is in `Cargo.lock`. All of them are compatible with
GPL-3.0:

- Most are MIT and/or Apache-2.0; others are BSD-2/3-Clause, ISC, Zlib,
  Unlicense or Unicode-3.0.
- The audio decoders (`symphonia*`), the spell checker (`spellbook`) and
  `option-ext` are under the Mozilla Public License 2.0, which is
  GPL-compatible. They are used unmodified, and their source is available
  from crates.io.
- egui embeds fallback fonts under the SIL OFL 1.1 and the Ubuntu Font
  Licence 1.0, both of which permit this.

The source repository contains no third-party code beyond the files listed
above. If you distribute compiled binaries, include this file and the
licence texts of the bundled crates; `cargo about` or `cargo license` can
generate that list.
