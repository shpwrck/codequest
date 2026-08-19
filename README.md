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
| D | A | Confirm / fight / cheer (hold to fast-forward the log) |
| S | B | Back / abort quest |
| Enter | START | Start / pause log scroll / rematch |
| Shift | SELECT | Ward a command / hold to FLEE a battle |
| A / F | L / R | Page the quest menu |
| P | POWER | Power on / off (or click the switch on the right edge) |
| C | SLOT | Open the cartridge tray (power must be off; or click the slot) |

All shell buttons are also clickable, including the L/R shoulders and the
power switch. The console starts powered off — flip the switch to boot.

## Cartridges

Cartridges are **local git repositories**. Click the cartridge slot on the
top edge (or press `C`) while the power is off, then **+ ADD FROM DISK** to
pick a directory with the native folder dialog. If the directory is a git
repo it loads as a cartridge — title from the directory name, label color
from a path hash — and is cached in the tray for future launches. If it
isn't a git repo, the cartridge is refused with a message. The loaded cart
peeks out of the top-back slot GBA-style and is remembered between launches;
powering on with an empty slot halts on the boot logo, like real hardware.

A repo cartridge's quests are generated from its contents: repo scrying
(`git status`), history (`git log`), and drift (`git diff`) always; plus a
Lint Gauntlet / Forge / Test Dungeon for `package.json` scripts, a Crate
Forge when `Cargo.toml` exists, and Make Mines for a `Makefile`. Swapping
requires ejecting first — remove and insert animations included.

**Game modes.** A repo with a `CODEQUEST.md` loads as a custom game (schema
TBD — currently the quest-battle mode). Without one, the default game is the
**ENDLESS REPO QUIZ**: title screen, CONTINUE/NEW GAME menu, a procedural
hero customizer (reroll names like GREP THE UNSLEEPING, cycle class and
colors on a generated pixel sprite), then endless multiple-choice questions.
Question generation lives in Rust: it calls the `claude` CLI headlessly to
author questions from the repo's actual contents (files, commits, README and
source excerpts) — the customizer buys time while the oracle writes — and
falls back to a procedural generator when the CLI is unavailable (or
`CQA_NO_AI=1`). All questions are thematic — architecture, purpose, and
design comprehension that stays true as the repo evolves; never counts,
sizes, dates, or other state-in-time trivia. Generation starts the moment a
cartridge clicks in and batches prefetch during play, each one harder; if
the oracle ever falls behind, there is no loading screen — your hero walks a
scrolling travel scene with story beats until A CHALLENGER APPEARS. Three hearts, score
scales with level, per-repo auto-save; `CQA_CLAUDE_MODEL` overrides the
model used for generation.

In quest-battle mode, **CUSTOM QUEST** still accepts any shell command —
point it at your build, your tests, or your agent CLI.

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
