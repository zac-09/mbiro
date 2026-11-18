# Working on Mbiro

Mbiro is a paid download manager for macOS, built with Tauri 2 (Rust core, React and TypeScript UI) plus a browser extension. Isaac owns the product and makes the decisions.

Read these before writing code, in this order:

1. `PLAN.md`: what to build, the architecture, the milestones.
2. `DESIGN.md`: how it looks, with the designed screens in `design/reference/`.

## Hard rules

These come from store policy, licensing and security. Do not trade them away for convenience. If a task seems to need breaking one, stop and ask.

1. **The extension has no video code.** No video-site detection, no lists of video hosts, no media sniffing. It only hands ordinary file downloads to the app.
2. **No site-specific extraction code anywhere.** Video support is yt-dlp, called as a separate process, and nothing more.
3. **No video site is named in user-facing text:** UI strings, extension listing, help pages, website, changelog, screenshots.
4. **Never attempt DRM-protected streams.**
5. **Only `aria2c` ships inside the app bundle.** yt-dlp, Deno and ffmpeg are fetched at runtime into Application Support, over HTTPS, and verified against a checksum before they are executed.
6. **GPL and LGPL tools stay separate processes.** Never link aria2 or ffmpeg code into our binaries.
7. **Engines sit behind the one Rust trait** in `PLAN.md` section 4. The UI and job manager never call aria2 or yt-dlp directly.
8. **The queue lives in Rust.** Downloads must keep running with no window open. The UI only sends commands and renders events.
9. **aria2's RPC binds to 127.0.0.1** on a random port with a new random secret on every launch.
10. **Cookies and auth headers** stay in memory and the job row, are never written to logs, and are deleted when the job finishes.
11. **No network calls from the app** except downloads the user started, licence activation, update checks and tool fetches. No analytics. Crash reports are opt-in.
12. **No BitTorrent or magnet support** in v1.
13. **Never handle secrets.** Do not ask for, store or print Apple certificates, signing keys, the updater key, or payment keys. Write scripts that read them from environment variables; Isaac runs those scripts.

## How to work

- **One milestone at a time, starting with M0.** Within M0, one spike at a time. When a spike or milestone meets its "done when", stop, tick its box in `PLAN.md`, and report what you built, how to run it, and what you could not verify.
- **If a spike fails, stop.** Explain what failed and what the options are. Do not redesign around it on your own.
- **Stay in scope.** `PLAN.md` section 2 lists what is not in v1. Do not build or scaffold those. Suggest additions in your report instead.
- **Ask, do not guess,** when work depends on an item in `PLAN.md` section 14.
- **Verify against current docs.** Tauri, WXT, aria2, yt-dlp and Apple's notarisation tooling all change. Check their current documentation before relying on an API or flag, and say when you could not check.
- **Pin versions** of every dependency and every fetched tool. Note each new dependency and why in your report.
- **Small commits** with plain messages. Keep the build, lints and tests passing on every commit.
- **Tests come with the code.** Engine behaviour is tested against the local test server described in `PLAN.md` section 11, never against real sites.
- **Follow `DESIGN.md` exactly** for the designed screens. For undesigned screens, use its tokens and components and keep them plain.

## Conventions

- The product name, bundle identifier, native messaging host name and URL scheme live in one constants file per language. Until Isaac confirms them, use `Mbiro`, `dev.mbiro.app`, `dev.mbiro.host` and `mbiro://`, and treat them as placeholders.
- All messages between the extension, the host and the app are typed in `packages/protocol` and validated on receipt.
- Rust: `cargo fmt` and `cargo clippy` clean. TypeScript: strict mode, ESLint and Prettier clean.
- User-facing strings live in one place per app, ready for translation later.
