# CODE QUEST ADVANCE

*Every command is a boss.*

A retro Game Boy Advance-themed, gamified desktop frontend for a coding
harness, built with Tauri v2 (Rust backend + vanilla JS webview frontend).
A **quest** is a real shell command: its stdout/stderr streams live into the
game as an RPG battle — output lines land as attack narration and XP, stderr
wounds your hero, exit 0 stamps QUEST CLEARED, and a nonzero exit means GAME
OVER (retry?). XP, levels, titles, streaks, and a procedural nemesis bestiary
persist between sessions.

![Title screen](docs/screenshots/shot1-title.png)
![Battle](docs/screenshots/shot3-battle.png)

## Controls

| Input | GBA | Action |
|-------|-----|--------|
| Arrow keys | D-pad | Navigate menus |
| X | A | Confirm / fight / cheer (hold to fast-forward the log) |
| Z | B | Back / abort quest |
| Enter | START | Start / pause log scroll / rematch |
| Shift | SELECT | Ward a command / hold to FLEE a battle |

All shell buttons are also clickable.

## Quests

Four built-in quests (see `src-tauri/src/lib.rs`) demonstrate the range:
a scripted tutorial battle, a repo-status scry, a real `cargo check` of this
very app (the Borrow Checker boss), and a deliberately doomed run for the
defeat path. **CUSTOM QUEST** accepts any shell command — point it at your
build, your tests, or your agent CLI.

## Architecture

- `src-tauri/src/lib.rs` — the harness driver: `list_quests`, `start_quest`
  (spawns `bash -c`, streams each output line as a `quest://output` event,
  emits `quest://done` exactly once), `abort_quest`.
- `src/` — the entire game (no bundler, no framework, no network):
  `index.html`, `styles.css`, `main.js`. GBA shell chrome, 240×160 logical
  screen, CSS box-shadow pixel sprites, Press Start 2P (bundled), scanlines.

## Build & run

```bash
npm install
npm run tauri dev     # dev window
npm run tauri build   # deb / rpm / AppImage under src-tauri/target/release/bundle/
```

Host prerequisites and this machine's build quirks are documented in
`docs/runbooks/` (see also the headless smoke-test runbook used to verify the
gameplay loop without touching the desktop).

Prototype status: built as a design prototype — expect rough edges.
