# Mbiro: build plan

Mbiro is a paid download manager for macOS. "Mbiro" is a working name; keep it in one constants file so it can change.

Tick the checkboxes in this file as work completes. This file is the record of what is done.

## 1. Summary

We are building what Internet Download Manager is on Windows, for the Mac: fast segmented downloads, pause and resume, capture from the browser, and video grabbing from the web.

The free competitors (Motrix, Free Download Manager, AB Download Manager) already download fast. We win on browser integration and polish, not raw speed.

Decisions already made:

- **Stack:** Tauri 2 shell, React and TypeScript UI, Rust core.
- **Helpers:** `aria2c` is bundled as a sidecar. yt-dlp, ffmpeg and Deno are fetched on first use of the video grabber.
- **Distribution:** sold direct with a Developer ID and notarisation. Not the Mac App Store.
- **Video sites:** supported only through yt-dlp's generic extraction. No site-specific code or UI anywhere. No video site is named in any user-facing copy.
- **Pricing model:** one-time licence with a 14-day trial.
- **Platforms:** macOS first. Windows and Linux later from the same codebase.

## 2. Scope

In v1:

1. Segmented HTTP and HTTPS downloads, up to 16 connections per file, with pause, resume and retry.
2. A queue with a concurrency limit, a speed limit, and per-category save folders.
3. Chrome and Firefox extensions that hand browser downloads to the app.
4. A video grabber: paste a link or copy it to the clipboard, pick a quality, download.
5. Trial, licence key activation, and auto-update.
6. Menu bar item with live progress, and a notification on completion.

Not in v1 (do not build, do not scaffold):

- Safari extension.
- Scheduler, site grabber, on-page video overlay.
- BitTorrent and magnet links, even though aria2 supports them.
- Windows and Linux builds.
- Accounts or cloud sync. The app works offline apart from licence activation and update checks.

## 3. Architecture

Everything passes through one Rust core. The browser, the UI and the helper tools never talk to each other directly.

```text
 Browser                       The app (Tauri)                Helper processes
+----------------+           +---------------------+         +------------------+
| Extension      |           | React UI            |         | aria2c           |
| (MV3)          |           |  commands in,       |    +--->|  files, JSON-RPC |
+-------+--------+           |  events out         |    |    +------------------+
        | stdin/stdout       +----------^----------+    |
+-------v--------+   Unix    +----------v----------+    |    +------------------+
| Native         |  socket   | Rust core           +----+--->| yt-dlp           |
| messaging host +---------->|  job queue, engine  |         |  videos, CLI     |
+----------------+           |  trait, supervisor  |         +--------+---------+
                             +----------^----------+                  |
                             +----------v----------+         +--------v---------+
                             | SQLite              |         | Deno and ffmpeg  |
                             +---------------------+         +------------------+
                                        | HTTPS
                             +----------v----------+
                             | Licence and update  |
                             | servers             |
                             +---------------------+
```

What crosses each boundary:

- **Browser to app:** JSON messages with URL, filename, referrer, cookies and user agent. Only an acknowledgement flows back.
- **UI to core:** Tauri commands (`add`, `pause`, `resume`, `remove`) one way; progress and state events the other.
- **Core to helpers:** JSON-RPC to `aria2c` on a random 127.0.0.1 port with a per-launch secret. Command-line arguments and parsed output for yt-dlp.

## 4. Download engine

The app does not download files itself in v1. A Rust job manager owns the queue and drives aria2.

- **Process:** the core starts one `aria2c` child with RPC enabled on a random localhost port and a random secret per launch. It restarts the child if it dies.
- **Control:** `aria2.addUri`, `pause`, `unpause`, `remove`, `tellStatus`. Progress by polling `tellActive` once a second. Completion and errors from aria2's WebSocket notifications.
- **Segmentation:** default 8 connections per file and a 1 MB minimum split, user-adjustable up to 16.
- **Resume:** aria2 keeps a `.aria2` control file beside each partial download. SQLite stores the job record: URL, headers, cookies, target path, category, state, timestamps.
- **Auth context:** jobs from the browser carry cookies, referrer and user agent. Cookies live in memory and the job row only, are never logged, and are deleted when the job finishes.
- **Fallback:** if a server ignores `Range`, the job drops to one connection and the UI says resume is unavailable.

The job manager talks to engines through one Rust trait (`add`, `pause`, `resume`, `remove`, `status`). aria2 and yt-dlp are two implementations. A native Rust engine may replace aria2 later without touching the UI.

To settle in M0: aria2 publishes no ready-made macOS binary, so `scripts/build-aria2.sh` builds a static universal `aria2c`. Check aria2's current release state before committing to it.

## 5. Browser integration

This is the feature that has to beat the free competitors.

**Extension.** One Manifest V3 codebase, built with WXT for Chrome and Firefox. It listens for new downloads. When a file passes the user's rules (automatic catching on, minimum size, file types, excluded sites), it cancels the browser download and sends URL, filename, referrer, cookies and user agent to the app. It adds a "Download with Mbiro" item to the right-click menu for links.

**Native messaging host.** A tiny Rust binary the browser launches. It speaks length-prefixed JSON on stdin and stdout, relays each message to the running app over a Unix socket, and launches the app if it is closed. The app installs the host manifest for each browser on first run:

| Browser | Manifest folder under `~/Library/Application Support/` |
| --- | --- |
| Chrome | `Google/Chrome/NativeMessagingHosts/` |
| Firefox | `Mozilla/NativeMessagingHosts/` |
| Edge | `Microsoft Edge/NativeMessagingHosts/` |
| Brave | `BraveSoftware/Brave-Browser/NativeMessagingHosts/` |

**Clipboard capture.** While running, the app watches the clipboard. When a copied URL looks like a file or a page yt-dlp supports, it offers to download it. This is how video links reach the app.

**The extension contains no video code.** No video-site detection, no host lists of video sites, no media sniffing. Video handling lives only in the app. This keeps the extension within the browser stores' rules.

## 6. Video capture

A thin layer over yt-dlp. We write no site-specific code.

1. The user pastes or copies a URL. The app runs `yt-dlp --dump-single-json` for title, thumbnail and formats.
2. The app shows a short quality picker: best, 720p, 480p, audio only. An advanced toggle lists every format.
3. The app runs yt-dlp with the chosen format, parses its progress lines, and shows the job in the same queue as file downloads.
4. ffmpeg merges separate video and audio streams into one MP4 or MKV.

Playlists are in v1 as "download all" with a per-item checklist. Subtitles, chapters and conversion are later.

**Helper tools are fetched on first use**, into `~/Library/Application Support/<app>/tools/`, never placed in the app bundle:

- yt-dlp (official macOS executable, which includes the solver scripts).
- Deno, which yt-dlp has needed for some sites since version 2025.11.12. See https://github.com/yt-dlp/yt-dlp/issues/15012 and https://github.com/yt-dlp/yt-dlp/wiki/EJS for current requirements; verify before implementing.
- ffmpeg (pick a static macOS build and record its licence).

Rules for the tools manager:

- Every fetch is pinned to an HTTPS source and verified against a published checksum before the file is executed.
- A background check updates yt-dlp daily, independent of app releases.
- Settings has a Tools pane: versions, "update now", and a switch to yt-dlp's nightly channel.

Known limits to state in the help pages: DRM-protected streams are out of scope and must never be attempted. Some sites demand a sign-in check that yt-dlp cannot pass without the user's browser cookies.

## 7. App shell and UI

Downloads must continue with no window open, so the queue, the engine processes and the browser socket live in Rust. The UI is a view: it sends commands and renders events.

| Layer | Choice |
| --- | --- |
| UI | React, TypeScript, Vite, Tailwind |
| UI state | Zustand, fed by Tauri events |
| Core | Rust with Tokio |
| Storage | SQLite through `rusqlite`, in Application Support |
| Tauri plugins | updater, notification, clipboard-manager, single-instance, autostart, deep-link |

Screens in v1. See `DESIGN.md` for the look, and `design/reference/` for the designed screens.

- **Main window:** top bar with filters (all, active, done, failed) and search; a live total-speed readout; pause all, resume all, add; the download list.
- **Add download:** URL field, save location, connection count. A video URL swaps in the quality picker.
- **Menu bar popover:** total speed, active jobs, clipboard prompt, pause all, open window.
- **Settings:** general; connections and speed limit; categories and folders; browser integration (install status per browser); tools; licence. Not yet designed: follow `DESIGN.md`.
- **Onboarding:** three steps on first launch: pick a download folder, install the browser extension, start the trial. Not yet designed.
- **Extension:** popup (connected and app-not-found states), in-page notice after a catch, capture rules page.

A custom URL scheme lets the website open the app with a link and lets the licence email activate in one click.

## 8. Licensing and payments

- **Trial:** 14 days, full features. After that: one download at a time, 2 connections, no video grabber.
- **Keys:** a licence is a small JSON payload (email, plan, issue date) signed with Ed25519. The app holds only the public key and verifies offline.
- **Activation:** first use of a key calls a small HTTPS function that records the device and enforces 3 Macs per licence. A Cloud Function with Firestore is enough.
- **Purchase flow:** checkout on the website; the payment provider's webhook calls our function; the function signs a key and emails it with a one-click activation link.
- **Refunds:** a revocation list fetched weekly marks refunded keys invalid.

The payment provider is undecided (see open questions). Build the key-issuing function so the provider is one adapter.

## 9. Distribution

1. **Apple Developer Program:** Isaac enrols and creates the Developer ID Application certificate.
2. **Build:** `tauri build --target universal-apple-darwin`. The bundled `aria2c` and the native messaging host must be universal too.
3. **Sign:** every executable in the bundle, with the Developer ID and hardened runtime.
4. **Notarise:** submit the DMG with `notarytool`, then staple.
5. **Update:** the Tauri updater reads a JSON manifest on static hosting and verifies updates with its own signing key.
6. **Automate:** a GitHub Actions workflow on a macOS runner does steps 2 to 5 on tagged releases.

Signing credentials never go to an agent. Scripts read them from environment variables, and Isaac runs the release steps.

The extensions ship through the Chrome Web Store and Firefox Add-ons. The website is a static site with five pages: home, download, buy, help, legal.

## 10. Legal and policy checklist (owner: Isaac)

- [ ] Final name chosen, with trademark and domain checks. No "IDM" or "Internet Download Manager" in it.
- [ ] aria2 (GPL): shipped as a separate executable, licence text included, exact source published.
- [ ] ffmpeg: build recorded, source and licence linked in acknowledgements.
- [ ] yt-dlp and Deno listed in acknowledgements.
- [ ] Terms of use: users are responsible for having the right to download what they fetch.
- [ ] Privacy policy: the app sends only licence activation and update checks.
- [ ] Takedown contact published.
- [ ] Current Chrome Web Store and Firefox Add-ons policies read against the extension.
- [ ] Payment provider confirms in writing that the product is acceptable.
- [ ] Legal review of the licensing and terms items.

## 11. Repo layout and tooling

One pnpm monorepo.

```text
apps/
  desktop/            Tauri app
    src/              React UI
    src-tauri/        Rust core: job manager, engines, socket, licence
  extension/          MV3 extension (WXT), built for Chrome and Firefox
  site/               marketing site and help pages
crates/
  nm-host/            native messaging host (Rust)
packages/
  protocol/           TypeScript types and schemas for all messages
services/
  licence/            activation and key-issuing functions
scripts/
  build-aria2.sh      static universal aria2c
  release.sh          sign, notarise, staple, publish update manifest
```

- **Tests:** Vitest for the UI; `cargo test` for the core; a local HTTP test server that can throttle, drop connections and refuse `Range`, for engine tests.
- **CI:** lint and test on every push; signed release builds on tags.
- **Crash and error reports:** opt-in only, asked during onboarding.

## 12. Milestones

Work one milestone at a time. Stop and report when its "done when" is met.

| Milestone | Builds | Done when |
| --- | --- | --- |
| M0 Spikes | Four throwaway proofs of the riskiest parts | Each spike below works on a clean Mac |
| M1 Engine and core UI | Job manager, aria2 engine, SQLite, main window, add dialog, menu bar | A 2 GB file survives pause, quit, relaunch and a dropped network, and finishes intact |
| M2 Browser capture | Extension, native messaging host, manifest installer, clipboard watcher | A download behind a login in Chrome and Firefox lands in the app and completes |
| M3 Video grabber | Tools fetcher and updater, yt-dlp engine, quality picker, playlists | A pasted link from five different sites downloads and merges to one file |
| M4 Selling | Trial, licence keys, activation function, checkout, updater, release pipeline | A test purchase emails a key that activates a notarised build, which then updates itself |
| M5 Private beta | Fixes from testers, onboarding, help pages, store submissions | Seven days with no data-loss bug, and both extensions approved |
| M6 Launch | Website, legal pages | The legal checklist is fully ticked and the buy button is live |

M2 and M3 are independent after M1 and can swap order.

## 13. M0: the four spikes

If spike 2 or 3 fails, the packaging design changes. Nothing in M1 starts before both pass.

- [x] Monorepo created with the layout above and an empty Tauri 2 app that builds.
- [ ] **Spike 1, engine:** build a static universal `aria2c`, start it from the Rust core, add a download over JSON-RPC, show live progress in the window.
- [ ] **Spike 2, signing:** sign and notarise that build with `aria2c` inside, and open it on a second Mac that has never seen it. Agent writes the scripts; Isaac runs them.
- [ ] **Spike 3, tools:** from the notarised build, fetch yt-dlp, Deno and ffmpeg into Application Support, then download and merge one video.
- [ ] **Spike 4, browser:** a minimal extension sends a download URL through a native messaging host to the running app, in Chrome and in Firefox.

## 14. Open questions (owner: Isaac)

Ask rather than guess when work depends on one of these.

- [ ] Final product name, bundle identifier and URL scheme.
- [ ] Price, and whether a licence is per user or per Mac.
- [ ] Payment provider.
- [ ] Registered under Isaac personally or a company.
- [ ] Who does the legal review.

## 15. Risks

| Risk | Mitigation |
| --- | --- |
| Site changes break yt-dlp | Daily tool updates outside app releases, plus a nightly-channel switch |
| Browser store removes the extension | No video code in the extension; the app works by clipboard and paste without it |
| Payment provider drops the product | Keys issued by our own function; provider is one adapter |
| GPL obligations handled wrongly | Separate executables, published source and notices, legal review |
| aria2 stops being maintained | Engine sits behind one trait |
| Tools fail to run under Gatekeeper | Proved or disproved in spike 3 |
