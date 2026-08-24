# CODE QUEST ADVANCE

Turn a local Git repository into a cartridge and learn it through a retro
handheld game. CODE QUEST ADVANCE inspects the repository on your machine,
generates conceptual questions with your selected local Codex or Claude CLI,
and runs the game in a native Tauri app backed by a headless Bevy engine.

![CODE QUEST ADVANCE running the Oracle cartridge](docs/screenshots/oracle-title-shell.png)

| Oracle Datafall | Concept trial |
|---|---|
| ![The Oracle Datafall scene with charged data runes and broken corruption seals](docs/screenshots/oracle-datafall.png) | ![A concept question with wards, flow, Insight Runes, and score](docs/screenshots/oracle-trial.png) |

## What is playable now

The repository's own [`CODEQUEST.toml`](CODEQUEST.toml) is the reference
cartridge. It defines a manifest-driven Oracle quiz with this complete loop:

1. Insert the repository while the console is off, then switch on the power.
2. Watch repository credits and a five-scene Oracle awakening while the first
   question batch is generated in the background.
3. Create a hero by choosing a name, path, and aura.
4. Play **Oracle Datafall** while the next valid question is loading: move left
   or right, collect data shards, and avoid corruption glyphs.
5. Answer conceptual questions about the project's purpose, architecture,
   responsibilities, interactions, invariants, and tradeoffs.
6. Complete an accepted question batch to level up and increase the next
   batch's difficulty. Lose all three wards to end the run.

The current progression model is visible as well as numeric:

- Correct-answer flow awards x1 at streaks 0–2, x2 at 3–5, and x3 at 6+.
- Cumulative scores of 300, 900, and 1800 awaken Insight Runes I, II, and III.
- Datafall charge lights at 3, 6, and 9 collected shards; corruption breaks
  containment seals at 1, 3, and 5 hits. These are expressive Datafall goals
  and do not alter quiz score, wards, or question generation.
- Oracle bond presentation advances from Initiate at level 1, to Adept at
  levels 2–3, to Oracle-bound at level 4 and above.

The game has no filler question deck. If the installed AI provider returns an
invalid batch, the Oracle keeps Datafall playable and retries; press B to leave
the wait safely. Runtime audio is not implemented yet. Audio entries in the
manifest are production requirements, not playable sound.

## Install

Download the current packages from the
[latest release](https://github.com/shpwrck/codequest/releases/latest).
Tagged packages can lag the source tree; the screenshots and feature
descriptions in this README track the current repository state.

| Platform | Package | Architecture |
|---|---|---|
| Linux | `.AppImage` | x86_64 |
| Windows | NSIS `.exe` or WiX `.msi` | x86_64 |
| macOS | `.dmg` or zipped `.app` | Universal (Apple Silicon + Intel) |

Linux AppImages are portable and need no installed toolchain:

```bash
chmod +x code-quest-advance_0.2.3_amd64.AppImage
./code-quest-advance_0.2.3_amd64.AppImage
```

If FUSE is unavailable, add `--appimage-extract-and-run`. The packaged AppImage
carries GTK3 and WebKitGTK 4.1 and is verified on RHEL 9 and Fedora 43; the host
still supplies its graphics stack and fonts.

The desktop package is only the game runtime. Its cartridges use local command
line tools:

- `git` is required for every cartridge.
- Either `codex` or `claude` is required for quiz question generation and must
  already be authenticated. The installed battery pack selects which one runs.
- `bash` is required for quest-battle commands. Git for Windows supplies the
  supported Windows shell; WSL Bash is deliberately ignored because it cannot
  consume the Windows repository paths used by the quest runner.

On Windows, standard Git for Windows, Codex, and Claude installation locations
are discovered automatically. Custom or portable installs can set `CQA_GIT`,
`CQA_CODEX`, `CQA_CLAUDE`, and `CQA_SHELL` to an executable name on `PATH` or
an absolute path. macOS GUI launches repair their restricted `PATH` before tool
discovery.

## Load AI batteries

The console starts with no AI provider selected. Turn the device over with the
FRONT/BACK switch, press the battery door, and load one of the two-AA packs:

- **Codex:** blue cells using the Codex terminal mark.
- **Claude:** cream, coral, and charcoal cells.

Press the installed pair to remove it and expose the provider choices again.
Battery selection persists between launches, but readiness is proved again for
each application session. On the first power-on, CODE QUEST makes one minimal
non-interactive request to the selected CLI. The engine and boot animation do
not start until that request succeeds. If the pack is missing or the CLI is
unavailable, unauthenticated, or unhealthy, the switch and power LED flash red
and return to the off position. Battery changes are locked while power is on.

## Load a cartridge

A cartridge is a **local Git repository**.

1. Leave the power off and click the top cartridge slot, or press C, to open
   the rack.
2. Choose **+ ADD FROM DISK** and select a repository with the native folder
   picker.
3. Drag the cartridge up to load it, or focus it and press Enter.
4. Close the rack and switch on the power, or press P.

The rack caches at most three repositories. Labels show the current branch and
refresh whenever the rack opens. Drag a cartridge down, press Delete, or use its
recycle action to remove only the rack entry; repository files and saves remain
on disk. A loaded cartridge must be ejected before another can be inserted.

Loading a repository creates a versioned, namespaced save beside it, never
inside it: `/games/demo` uses `/games/demo.sav`. Quiz saves retain validated AI
question batches and answered-question history. Legacy Claude batch saves load
without conversion. Committing an answer records it
immediately, and later runs and launches filter every recorded question so the
cartridge continues with unseen material. Ejecting or recycling a cartridge
does not delete its save.

Loading alone does not modify the selected repository. Quest mode can run the
repository's own lint, build, or test scripts, so those commands have whatever
side effects the project itself defines.

## Game modes

Cartridge selection follows this order:

| Repository contents | Mode |
|---|---|
| Valid `CODEQUEST.toml` | The manifest's `quiz` or `quest` game type |
| No manifest, but `CODEQUEST.md` exists | Legacy quest-battle mode |
| Neither file exists | Endless repository quiz |

### Quiz

Quiz cartridges request six questions from the verified battery provider.
Generated questions must have four distinct
choices, exactly one correct answer, no repository trivia, no more than four
31-character lines for the prompt, and no more than 31 characters per choice.
Valid questions survive a mixed batch, so an accepted batch can contain fewer
than six. Accepted batches are cached in the sibling save and prefetched while
the player continues.

Set `CQA_CODEX_MODEL` or `CQA_CLAUDE_MODEL` to choose the model used by the
corresponding CLI. Set `CQA_NO_AI=1` to disable generation for diagnostics;
`0`, `false`, `no`, and `off` leave it enabled.

### Quest battle

Quest cartridges turn repository operations into streamed command battles.
Every repository receives:

- **Scrying Pool:** `git status --short --branch`
- **The Log Barrow:** a decorated 12-commit history
- **Diff Marsh:** working-tree and staged diff statistics

The engine also adds quests for `lint`, `build`, and `test` scripts in
`package.json`, `cargo check` when `Cargo.toml` exists, and a `make -n` dry run
when a `Makefile` exists. The Bevy runtime starts, streams, and aborts these
commands; B aborts an active battle.

### `CODEQUEST.toml`

`CODEQUEST.toml` is an optional, strict cartridge contract. Schema v2 selects a
trusted game family, a start scene, built-in scene handlers, semantic
transitions, and built-in visual templates. It can reorder and reuse supported
behavior, but it cannot load cartridge JavaScript, add arbitrary conditions,
execute custom commands, or replace the engine loop. Mechanics and template-less
art entries remain validated design metadata.

Schema v1 still loads as metadata and runs the matching built-in quiz or quest
flow. Invalid manifests, unknown keys, unresolved references, unreachable
scenes, and unsupported handler/signal combinations refuse the cartridge
instead of silently falling back.

- [`CODEQUEST.toml` v2 reference](docs/reference/codequest-toml.md)
- [Maintained complete example](docs/examples/CODEQUEST.toml)
- [Oracle quiz design and runtime traceability](docs/game-design/oracle-quiz.md)

## Controls

All physical controls are clickable. The floating FRONT/BACK switch turns the
whole unit over; the rear battery door selects the AI provider, the rear label
lists gameplay bindings, and the serial plate shows the short commit hash of
the running CODE QUEST ADVANCE build.

| Keyboard | Handheld input | Current use |
|---|---|---|
| Arrow keys | D-pad | Navigate; move during Datafall |
| D | A | Confirm, answer, or start a quest |
| S | B | Back, leave the Oracle, or abort a quest |
| Enter | START | Start or confirm |
| Shift | SELECT | Reserved |
| A / F | L / R | Page the quest list |
| P | Power switch | Turn the device on or off |
| C | Cartridge slot | Open or close the rack while powered off |
| F1 | FRONT/BACK switch | Turn the device over |

The engine consumes input edges, not browser key-repeat events. During the
Datafall-to-question handoff and the 45-tick answer review, inactive controls are
explicitly ignored so a held key cannot answer the next question accidentally.

## Architecture

Gameplay and device presentation have a hard boundary:

| Path | Responsibility |
|---|---|
| `src/` | JavaScript/CSS physical shell, cartridge rack, native-dialog bridge, boot overlay, input forwarding, window fitting, and framebuffer canvas |
| `src-tauri/src/engine.rs` | Headless Bevy game state, fixed-step timing, input edges, Oracle/quiz/quest behavior, command effects, and CPU rendering |
| `src-tauri/src/scene_machine.rs` | Executable finite-state machine and built-in quiz/quest templates |
| `src-tauri/src/codequest.rs` | `CODEQUEST.toml` parser, validation, and runtime compilation |
| `src-tauri/src/lib.rs` | Tauri boundary, verified provider state, Git cartridge inspection, provenance, quest construction, AI prompting, and question persistence |
| `src-tauri/src/external_tools.rs` | Cross-platform Git, Codex, Claude, and shell discovery |
| `src-tauri/src/save.rs` | Versioned namespaced saves with atomic file replacement |
| `src-tauri/assets/oracle/` | Authored 240×160 Oracle plates, hero sprites, portraits, and Datafall sprites embedded into the engine |

Bevy always produces one 240×160 RGBA framebuffer: 153,600 bytes at 8 bits per
channel. The shell copies that buffer into a fixed-size, pixelated canvas and
contains no gameplay state or game rendering. Text uses a purpose-built 5×7
font in 6×8 cells, giving the display 40 columns without allowing game content
to change the device layout.

## Develop and verify

Use Node.js 22, stable Rust, and the native Tauri v2 prerequisites for your
platform. Then:

```bash
npm ci
npm run tauri -- dev
```

Run the same core checks used by CI:

```bash
node --check src/main.js
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build --no-bundle
python3 .agents/skills/codequest-game-designer/scripts/validate_codequest.py CODEQUEST.toml
```

Oracle render changes can emit every native scene without launching the shell:

```bash
CQA_VISUAL_PREVIEW_DIR=/tmp/codequest-previews \
  cargo test --manifest-path src-tauri/Cargo.toml \
  oracle_templates_produce_nine_distinct_native_scene_frames --lib
```

Recompile edited Oracle PNG sources before running that preview test:

```bash
./scripts/compile-oracle-assets.sh
```

The [headless GUI smoke-test runbook](docs/runbooks/headless-gui-smoke-test/README.md)
launches the real Tauri app under Xvfb, drives it with XTEST input, and captures
the rendered shell without touching the desktop session.

## Packaging and release gates

Tauri packages the host platform. Windows and macOS artifacts therefore build
on their corresponding GitHub runners. The `Package` workflow builds Linux,
Windows, and macOS packages for every pull request, downloads the exact uploaded
artifacts into fresh jobs, verifies checksums, and launches each packaged app.
A `v*` tag publishes a GitHub release only after all three artifact tests pass.

For a local portable Linux package:

```bash
./packaging/build-appimage.sh
```

The script uses Podman or Docker and writes to `dist/`. Its Ubuntu 22.04 build
container and compatibility checks keep the AppImage usable on RHEL 9's glibc
2.34 floor.

Windows release packages are currently unsigned. macOS CI packages use an
ad-hoc signature; public production distribution still needs a Developer ID
certificate and notarization.
