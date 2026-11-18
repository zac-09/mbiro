# First message to the agent

Paste the text below as your first message, with the handoff files already in the root of an empty repository.

---

You are starting work on Mbiro, a paid download manager for macOS.

Before doing anything else, read `AGENTS.md`, then `PLAN.md`, then `DESIGN.md`, and look through `design/reference/`. `AGENTS.md` has hard rules that override convenience; follow them throughout.

Then do only this:

1. Tell me, in a short list, anything in the plan that is unclear, contradictory or out of date against the current documentation of Tauri 2, WXT, aria2 or yt-dlp. Do not fix the plan yourself.
2. Set up the monorepo from `PLAN.md` section 11, with an empty Tauri 2 app that builds and runs on macOS.
3. Do **Spike 1** from `PLAN.md` section 13: build a static universal `aria2c`, start it from the Rust core, add a download over JSON-RPC, and show live progress in the window. A bare window is fine; do not build the designed UI yet.

Stop when Spike 1 works. Tick its box in `PLAN.md`, then report what you built, the exact commands to run it, and anything you could not verify. Wait for me before starting Spike 2.
