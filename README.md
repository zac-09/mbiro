# Mbiro

A download manager for macOS. Tauri 2, a Rust core, a React and TypeScript UI.

Read `AGENTS.md`, then `PLAN.md`, then `DESIGN.md`.

## Set up

Needs Xcode Command Line Tools, Node 20.19 or newer, pnpm 9 and rustup. The Rust
version is pinned in `rust-toolchain.toml` and installs itself on first use.

```sh
pnpm install
pnpm build:aria2   # builds the universal aria2c sidecar from pinned source, about 2 minutes
```

`aria2c` is not committed. The app will not compile until that script has run once.
The native messaging host is built automatically by `pnpm dev` and `pnpm build`;
run `pnpm build:nm-host` once before using `cargo` directly.

## Run

```sh
pnpm dev           # the app, with live reload
pnpm build --target universal-apple-darwin   # unsigned Mbiro.app under target/
```

## Check

```sh
pnpm lint && pnpm typecheck && pnpm test   # TypeScript
pnpm check:rust                            # fmt, clippy, tests (drives the real aria2c against a local server)
```

## Browser extension (development)

Start the app once first: on launch it tells each installed browser where the
native messaging host is.

```sh
pnpm build:extension   # writes apps/extension/dist/chrome-mv3 and firefox-mv3
```

- Chrome: open `chrome://extensions`, turn on Developer mode, choose "Load unpacked"
  and pick `apps/extension/dist/chrome-mv3`.
- Firefox: open `about:debugging#/runtime/this-firefox`, choose "Load Temporary
  Add-on" and pick `apps/extension/dist/firefox-mv3/manifest.json`.

Then right-click any link to a file and choose "Download with Mbiro".
