# CODE QUEST ADVANCE

![Title screen](docs/screenshots/shot1-title.png)
![Battle](docs/screenshots/shot3-battle.png)

## Controls

| Input | GBA | Action |
|-------|-----|--------|
| Arrow keys | D-pad | Navigate menus |
| D | A | Confirm / answer / fight / jump |
| S | B | Back / abort quest |
| Enter | START | Start / confirm |
| Shift | SELECT | Reserved |
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

**Game modes.** A repo with a `CODEQUEST.md` loads the quest-battle mode;
without one, it loads the **ENDLESS REPO QUIZ**. Cartridge contents are data
only: Rust derives a title, mode, and quest list from the repo, then gives
that data to Bevy. A cartridge cannot add JavaScript or replace the game
loop.

Quiz cartridges contain no preloaded questions. Inserting one makes the Bevy
engine ask the `claude` CLI for the first batch immediately. Character
creation, Oracle travel, and level-up screens keep the game moving while
Claude writes or prefetches the next batch; the Oracle waits and retries if a
batch fails instead of substituting generic questions. Generated questions
cover the project's purpose, architecture, responsibilities, interactions,
invariants, and tradeoffs. The prompt and accepted-output policy live in
`src-tauri/src/lib.rs`: file and repository trivia is rejected, questions
must fit four 37-character lines, and each of four distinct choices must fit
35 characters. Set `CQA_NO_AI=1` to disable generation for diagnostics (the
Oracle will continue waiting), or `CQA_CLAUDE_MODEL` to select the model.
Quest-battle commands are selected from the Rust-derived cartridge quest list
and are started, streamed, and stopped by the Bevy runtime.

## Architecture

- `src-tauri/src/engine.rs` — the game. A headless Bevy app owns navigation,
  input edges, fixed-step timing, quiz/battle state, command execution, and a
  CPU-rendered 240×160 RGBA framebuffer.
- `src-tauri/src/lib.rs` — the platform adapter. It validates git-repo
  cartridges, prepares data, and exposes only power, boot completion,
  cartridge, input, and framebuffer operations to the device UI.
- `src/` — the JavaScript device shell. It owns the physical controls,
  cartridge tray, window fitting, the fixed device-firmware boot overlay, and
  copies the fixed-size Rust framebuffer into one canvas. It contains no
  gameplay state or game rendering. Bevy remains at its boot boundary until
  the device animation explicitly completes.

The framebuffer is always exactly 240×160 (153,600 RGBA bytes). Each pixel is
32-bit RGBA (8 bits per channel). Game text uses a purpose-built 5×7 glyph in
a 6×8 cell, the smallest size that keeps letters, digits, punctuation, and
repository paths distinct while fitting 40 columns. Button
presses can change its pixels, but cannot change its dimensions or the CSS
layout of the device, which prevents the mid-game rasterization shift that
the DOM-rendered version could trigger.

## Install

Grab the AppImage, make it executable, run it — there is nothing to install and
no toolchain required:

```bash
chmod +x code-quest-advance_0.1.0_amd64.AppImage
./code-quest-advance_0.1.0_amd64.AppImage
```

It carries GTK3 and WebKitGTK 4.1 inside it (~200 libraries), which matters most
on RHEL 9: that distro ships only the webkit2gtk **4.0** API, in BaseOS,
AppStream, CRB and EPEL alike, so Tauri v2 cannot be built or installed there at
all from stock packages. The AppImage sidesteps that entirely. Verified to boot
and render on RHEL 9 (glibc 2.34) and Fedora 43 (glibc 2.42).

The host supplies only its own graphics stack and fonts. On a machine with a
desktop session those are already present; on a bare/minimal host you need
`libGLESv2.so.2` (`libglvnd-gles`) and some fonts. If the host has no FUSE, run
it as `./code-quest-advance_0.1.0_amd64.AppImage --appimage-extract-and-run`.

At runtime the app shells out, so `git` — and `claude`, for AI question
generation — must be on `PATH`. Those are the only other requirements.

## Build & run

```bash
npm install
npm run tauri dev     # dev window
npm run tauri build   # deb / rpm / AppImage under src-tauri/target/release/bundle/
```

This builds against the host's libraries and is the right thing for development.
It is NOT what you ship: a binary built on Fedora needs GLIBC_2.39 and will not
start on RHEL 9, which has 2.34.

## Packaging a release

```bash
./packaging/build-appimage.sh    # -> dist/*.AppImage + SHA256SUMS + portability.txt
```

That builds inside an Ubuntu 22.04 container — the oldest base carrying
webkit2gtk-4.1, which pins the portability floor at glibc 2.35 — then patches
the bundle down to RHEL 9's glibc 2.34 and repackages it. `packaging/Containerfile`
is the authoritative list of build dependencies and documents each compatibility
fix and why it exists. The build fails rather than emitting an artifact that
would die on a RHEL 9 loader.

Verify a change by rebuilding and re-running the smoke test in
`docs/runbooks/headless-gui-smoke-test/`, which drives the gameplay loop without
touching the desktop.

Prototype status: built as a design prototype — expect rough edges.
