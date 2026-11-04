# Samādhi roadmap

This is the planning file for Samādhi. It records what's next, what's
planned for later, the options behind each plan and the decisions still
open. Keep it current: when plans change, update this file in the same
change. What has shipped is in [`CHANGELOG.md`](CHANGELOG.md).

**Status key:** *next* (the next things to build), *planned* (designed,
not started), *idea* (worth doing, not designed yet).

---

## 1. Easy install for Mac friends: a DMG *(next)*

**Goal:** a friend downloads `Samadhi.dmg` from the website,
double-clicks it, drags Samādhi into Applications and opens it with no
warnings and no Terminal.

**Today:** Homebrew and a shell installer put a bare program on disk.
The app then makes its own `~/Applications/Samādhi.app` launcher. That
works, but people who don't know Homebrew won't get that far.

### What's needed

1. **A real app bundle** built at release time: `Samādhi.app/Contents/`
   with `MacOS/samadhi` (the program itself, not a launcher script),
   `Info.plist`, `Resources/samadhi.icns`, and the bundled assets.
   - Build both Mac targets (Apple silicon and Intel), as the release
     workflow already does, then join them into one universal binary with
     `lipo -create`.
   - `src/macos.rs` already writes the plist and icns, so reuse it from a
     small `xtask` or script.
   - When the program runs from inside a bundle, skip making the
     `~/Applications` launcher.
2. **Code signing and notarisation.** Without these, macOS refuses to
   open the app. On recent macOS versions the old right-click → Open
   workaround is gone too: people have to find "Open Anyway" in System
   Settings → Privacy & Security, which most won't.
   - Join the Apple Developer Program: $99 a year, in your name. Only you
     can do this.
   - Create a **Developer ID Application** certificate and an App Store
     Connect API key for the notary service.
   - Sign with the hardened runtime:
     `codesign --options runtime --timestamp --sign "Developer ID Application: …"`.
     No special entitlements are expected: Samādhi only needs audio
     output, the network and files the user picks.
   - Notarise and staple: `xcrun notarytool submit --wait`, then
     `xcrun stapler staple` on the `.app` and on the DMG.
3. **The DMG.** Use `create-dmg` (Homebrew) or `hdiutil`. Include a
   window with the app icon, an arrow to an `Applications` shortcut and
   a soft background in the app's style.
   - Name it `Samadhi-<version>.dmg`. Also publish a stable
     `Samadhi.dmg`, so the website can always link to
     `releases/latest/download/Samadhi.dmg`.
4. **Release automation.** dist (cargo-dist) doesn't make DMGs, so add a
   job to `.github/workflows/release.yml` on a `macos-latest` runner.
   - It runs after the Mac builds. It downloads them, runs `lipo`,
     bundles, signs, notarises, makes the DMG and uploads it to the
     GitHub Release.
   - Secrets it needs: `APPLE_CERT_P12` (base64), `APPLE_CERT_PASSWORD`,
     `APPLE_TEAM_ID`, `APPLE_NOTARY_KEY` (.p8), `APPLE_NOTARY_KEY_ID`
     and `APPLE_NOTARY_ISSUER`.
   - Keep dist's `dist generate` happy: set `allow-dirty = ["ci"]`, or
     move the job to a separate workflow triggered by
     `release: published`. The separate workflow is simpler.
5. **Updates for DMG users.** They won't have `brew upgrade`. Short
   term, check the GitHub releases API at launch (at most once a day) and
   show a quiet "A new version is ready: Download" note. Later, consider
   [Sparkle](https://sparkle-project.org) for one-click updates; it needs
   an EdDSA key and an appcast feed, which the website can host.

**Stopgap before signing:** an unsigned DMG works if friends follow
"Open Anyway" in Privacy & Security. Publish the steps with screenshots
on the download page.

**Decisions for you:** join the Apple Developer Program now, or use
the stopgap first.

---

## 2. A website with downloads *(done: live on GitHub Pages; domain next)*

**Done in 0.3.0:** the site in `site/` is live at
<https://anirudhi.github.io/samadhi/>. It has a Windows download (`.msi`
and `Samadhi.exe`), Mac and Linux instructions with copy buttons, and
highlights the visitor's system.

- `.github/workflows/site.yml` republishes it to the `gh-pages` branch
  whenever `site/` changes on `main`.
- Its screenshots come from the real app:
  `cargo test -- --ignored render_site_images`.
- **Vercel** was the first choice, but this machine isn't signed in to
  Vercel, so GitHub Pages was used. To move it: in Vercel, "Add New
  Project", import `anirudhi/samadhi` and set the root directory to
  `site` (no build step). Or run `npx vercel login` then
  `npx vercel deploy --prod site` here. Then point the domain at Vercel.

**Still to do:** the domain (below), the DMG link once section 1 is done,
and an animated hero (a short loop of the interface fading out).

**Goal:** one page that says what Samādhi is, shows it, and offers the
right download for each visitor.

### Domain

Check availability before choosing; registrars change. Candidates, best
first:

- `samadhi.app`: likely taken or premium.
- `samadhiwriter.com` / `samadhiwriter.app`
- `getsamadhi.app`
- `samadhi.ink`
- `writesamadhi.com`

Register at Cloudflare Registrar (at-cost pricing; DNS sits next to
hosting). Buying the domain and DNS access are yours to do.

### Hosting and source

- A static site in `site/` in this repository: plain HTML and CSS, or
  Astro if it grows. Fonts and colours come from the app (Literata and
  Inter, the light theme palette).
- Hosted on **Cloudflare Pages** (free, fast, custom domain, HTTPS),
  deployed by a GitHub Action on pushes to `site/`. GitHub Pages works
  too.

### Page outline

1. **Top:** the name, the one-line pitch ("Write in a quiet place") and a
   large screenshot or short looping video of a theme with the interface
   fading out.
2. **Download:** detect the visitor's system.
   - **Mac:** a "Download for Mac" button pointing at
     `releases/latest/download/Samadhi.dmg`, with the Homebrew command
     in small print.
   - **Linux:** the curl installer, and an AppImage once it exists (see
     below).
   - **Everyone:** "Other downloads" linking to GitHub Releases.
3. **Features:** themes and parallax, music and typing sounds, live
   markdown, the focus timer, saving and recovery, Google Drive.
4. **Gallery:** a few theme screenshots (from `target/previews/`).
5. **Privacy:** no account, no tracking. Your writing stays in your
   files.
6. **Footer:** open source (GPL), credits, the OmmWriter inspiration
   note (the same wording as the README) and a link to the source.

**Analytics:** none, or privacy-friendly ones (Plausible, or Cloudflare
Web Analytics, which is cookie-free).

### Linux downloads for friends *(idea)*

Publish an **AppImage** (a single download, no install) from the release
workflow with `cargo-appimage` or `linuxdeploy`. Flathub is a further
step.

### Windows *(done in 0.3.0; signing planned)*

Windows 10 and 11 are supported. What 0.3.0 added:

- **CI:** runs on Windows.
- **Releases:**
  - an `.msi` from `wix/main.wxs`, with a Start menu shortcut and the
    icon;
  - a zip;
  - a PowerShell installer;
  - a plain `Samadhi.exe`, attached by `.github/workflows/windows-exe.yml`.
- **The app itself:**
  - the single-instance check uses a loopback TCP port with a random
    token;
  - Google Drive for desktop is found on its drive letter;
  - `build.rs` embeds the icon in the `.exe`.

**Next:** code-sign the `.exe` and `.msi`, so SmartScreen stops showing
"Windows protected your PC". Options:

- **Azure Trusted Signing:** about $10 a month, the simplest for
  individuals. It needs identity validation in your name.
- **An OV or EV certificate:** a few hundred dollars a year.

Either way, a CI step would run `signtool` (or the Trusted Signing
action) before the MSI is built.

Also worth checking on real Windows machines:
- high-DPI scaling;
- the GPU fallback to OpenGL on older laptops;
- the typing-sound latency of WASAPI.

---

## 3. Cloud saves, accounts, web and mobile *(planned, later)*

**Goal:** people who install Samādhi can sign in, have their writing
kept safely online, and read and write it from a web page and a simple
phone app. Possibly with a free tier.

### Principles

- **Offline first.** The desktop app keeps working with no network and no
  account. Files on disk stay the source of truth on the desktop; sync
  mirrors them.
- **Never lose text.** Same rules as today: a background writer, a local
  recovery cache, and conflicted copies rather than silent overwrites.
- **Private by default.** Encryption in transit and at rest. End-to-end
  encryption is an option for those who want it (see below).
- **Optional.** Google Drive and plain folders keep working for people who
  prefer them.

### Architecture

```
 Desktop app ──┐                    ┌── Web app (browser)
 (files +      │   HTTPS / sync     │
  sync engine) ├──── API ─── DB ────┤
 Phone app ────┘   (auth, docs,     └── Account pages
                    revisions)
```

**Data model:**

- `users`: id, email, plan, created_at.
- `documents`: id, user_id, title, folder, created_at, updated_at,
  deleted_at.
- `revisions`: id, document_id, number, content (or a diff), device_id,
  created_at.
- `devices`: id, user_id, name, last_seen.

**Sync:** each document carries a revision number.

- A client sends "my change, made on top of revision n".
- If the server is still at n, it accepts and returns n + 1.
- If not, the server keeps both: the latest becomes the document, and the
  other is a conflicted copy. These are the same semantics as the
  desktop's Drive handling today.
- This suits writing, which is almost always done on one device at a
  time.

**Real-time merge (later, if wanted):** move to a CRDT such as `yrs`
(Rust Yjs) or Automerge, so edits on two devices merge character by
character. It's more complex, so only if people really edit
simultaneously.

**Desktop sync engine:** a "storage backend" layer behind the saver,
with three backends: a local folder, a synced folder (Drive), and
Samādhi Cloud.

- Uploads reuse the saver's coalescing and spacing.
- Downloads arrive like the "changed elsewhere" reload does now.
- Groundwork worth doing early, even before any server: put saving and
  opening behind that layer.

### Backend options

| Option | Pros | Cons |
| --- | --- | --- |
| **Supabase** (Postgres, auth, storage, row-level security) | Fastest to build; email and Google/Apple sign-in included; good free tier | Another vendor; Postgres-shaped |
| **Own Rust service** (axum + Postgres + Cloudflare R2) | Full control; one language with the app | You build auth, accounts and admin yourself |
| **Firebase** | Mature mobile SDKs | Document-store limits; vendor lock-in |

**Recommendation:** start on Supabase, with the database schema above
and row-level security so each user only ever sees their own rows. A
small Rust or TypeScript edge function handles the sync endpoint's
conflict logic. Move to a self-hosted service later only if costs or
limits demand it.

### Accounts in the desktop app

- **A Settings window**, separate from the full-screen writing window
  (egui supports extra windows with `show_viewport_deferred`). It holds:
  - Account: sign in or out, the plan, storage used, and devices.
  - Sync: on or off, which folder, and the last synced time.
  - The existing preferences (documents folder, fonts and so on), which
    today live only in `settings.toml`.
- **Sign-in happens in the system browser,** never with a password typed
  into the app. It uses OAuth with PKCE and a loopback redirect to
  `http://127.0.0.1:<port>/callback`. It supports Google and Apple, plus
  email magic links.
- Tokens are kept in the system keychain (`keyring` crate): Keychain on
  macOS, Secret Service on Linux.

### Web app

Two routes:

1. **Compile the egui app to WebAssembly** (eframe supports the web).
   - Pros: the same look and the same code, so parity comes fastest.
   - Cons: a heavy first load (fonts, music and photos must stream rather
     than be bundled), and it feels less like a web page (text selection,
     accessibility, browser spellcheck).
2. **A small dedicated web editor** (SvelteKit or React with CodeMirror
   6 and markdown).
   - Pros: a native web feel and fast loading.
   - Cons: a second interface to keep in step.

**Recommendation:** route 2 for the document list, accounts and a clean
editor. Add route 1 later as an "immersive mode" if people want the full
experience in a browser.

### Phone app

- **Step 1: a PWA.** The web app, installable on iOS and Android from the
  browser, with offline storage (IndexedDB) and the same sync. No app
  stores and no review. It covers "read and write on the go".
- **Step 2, if wanted: native apps.** SwiftUI first, since your friends
  are on Apple devices, then Kotlin or Compose. Both are thin clients of
  the same API.
  - Keep it simple: a document list, a markdown editor, a theme
    background, maybe the typing sound.
  - An Apple developer account (see section 1) also covers the App Store.

### End-to-end encryption *(option)*

- Text is encrypted on the device, with a key from the user's passphrase
  (Argon2 → XChaCha20-Poly1305), so the server never sees it.
- **Trade-offs:**
  - A forgotten passphrase means the text is lost (offer a recovery
    key).
  - The web app must decrypt in the browser.
  - Server-side search isn't possible.
- Offer it as an opt-in, likely in the paid tier.

### Free tier and costs

Text is tiny: a long novel is about 1 MB. Storage is not the cost
driver; requests and support are.

| Item | Rough cost |
| --- | --- |
| Supabase Free | $0: 500 MB database, 50,000 monthly active users, 1 GB file storage; pauses after a week idle, so not for production |
| Supabase Pro | $25 a month: 8 GB database, 100,000 monthly active users; enough for thousands of writers |
| Cloudflare R2 (revision archives) | about $0.015 per GB-month, no egress fees |
| Domain and website | about $10–30 a year; Pages hosting is free |
| Apple Developer Program | $99 a year |

**A plausible split:**

- **Free:** sync on up to 2 devices, web access, unlimited documents
  within a fair-use limit (about 100 MB of text per person), and 30 days
  of revision history.
- **Supporter or Pro:** about $3–4 a month or $30 a year. Unlimited
  devices, full revision history, end-to-end encryption, phone app
  extras, and priority help. Bill through **Lemon Squeezy** or **Paddle**
  (merchants of record that handle VAT and sales tax), or Stripe if you
  register for tax yourself.

**Break-even:** Pro at $25 a month plus small extras is covered by about
10 paying supporters.

**Abuse protection:** per-user quotas, request rate limits, and text only
(no attachments) at first.

### Privacy and legal

You'll need these before launching accounts:

- A privacy policy and terms on the website.
- Account deletion and full export (a zip of `.md` files) in Settings.
- A data processing agreement with the backend provider.
- Under GDPR, users can see, export and delete their data.
- Writing is never read or used for training, and the site says so
  plainly.

### Phases

| Phase | Deliverable | Depends on |
| --- | --- | --- |
| 0 | Settings window; the storage backend layer in the app (local and Drive behind one interface) | none |
| 1 | Backend MVP (auth, documents, revisions); desktop sync behind a "beta" switch; export and delete | 0 |
| 2 | Web app: sign in, document list, editor, conflicted copies | 1 |
| 3 | PWA for phones (offline, install prompt) | 2 |
| 4 | Plans and billing; quotas | 1, website |
| 5 | Native iOS app, then Android | 3 and demand |
| 6 | Optional end-to-end encryption; optional real-time merge | 1 |

### Open decisions

1. Supabase, or your own service from the start?
2. Is end-to-end encryption a must for launch, or later?
3. Free tier limits and the price of the paid plan.
4. The web editor's route (dedicated, or WebAssembly).
5. The name for the service (for example "Samādhi Cloud" or "Sync").

---

## 4. Smaller items and known issues

- **Google Drive access on this Linux machine** *(do soon)*: rclone's
  shared Google client ID is being retired during 2026. Create your own
  client ID in Google Cloud Console (OAuth consent screen plus a
  "Desktop app" credential), then run `rclone config reconnect gdrive:`.
- **Scripts the bundled fonts don't cover** *(idea)*: Chinese, Japanese,
  Korean and Arabic text shows as boxes. Ship Noto Sans subsets as
  fallbacks (a few MB), or load system fonts on demand when such text
  appears.
- **The clipboard on Linux** *(watch)*: egui waits for clipboard data
  with no time limit. If the watchdog log ever shows a stall in
  clipboard code, move paste off the interface thread.
- **Typewriter scrolling and a focus mode** *(idea)*: keep the current line
  centred, and fade other paragraphs.
- **Statistics** *(idea)*: words per session and a daily goal with the
  timer.
