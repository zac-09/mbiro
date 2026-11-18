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
