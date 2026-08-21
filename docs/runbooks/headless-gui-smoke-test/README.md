---
leave-behind: v1
state-scope: headless-gui-smoke-test
status: current
---

# Headless GUI smoke test leave-behind

## Operability

### State and access

Durable state: three small X11 utility RPMs installed on this Fedora 43 host
via local passwordless `sudo dnf` (no credentials involved): `xwd`, `scrot`,
and `xorg-x11-server-Xvfb`. They exist so this repo's GUI can be launched,
driven, and screenshot-verified without touching the user's desktop session.
The app under test is the current repository checkout (release binary at
`src-tauri/target/release/code-quest-advance`); verification screenshots live
in `docs/screenshots/`.

### Template map

Xvfb :99 virtual framebuffer -> xwd root capture -> docs/screenshots/*.png

### Re-run

```bash
sudo dnf install -y xwd scrot xorg-x11-server-Xvfb   # idempotent
cd /path/to/codequest
Xvfb :99 -screen 0 1024x720x24 & XPID=$!
DISPLAY=:99 GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  LIBGL_ALWAYS_SOFTWARE=1 ./src-tauri/target/release/code-quest-advance & PID=$!
sleep 8
WID=$(DISPLAY=:99 xdotool search --name "QUEST" | head -1)
DISPLAY=:99 xdotool windowfocus --sync "$WID"      # focus FIRST — see Decisions
DISPLAY=:99 xdotool key Return                     # XTEST, NOT `key --window`
DISPLAY=:99 xwd -root -silent | magick xwd:- png:shot.png
kill $PID $XPID
```

After building an AppImage, verify the native cartridge picker through the
same headless display stack:

```bash
scripts/test-release-picker.sh \
  src-tauri/target/release/bundle/appimage/code-quest-advance_0.2.1_amd64.AppImage
```

The script powers no game state and touches no desktop session. It opens the
cartridge tray, clicks **ADD FROM DISK**, and fails unless the native
`SELECT CARTRIDGE (GIT REPO)` window appears.

### Verify and recover

Health check: the pipeline above yields a ~100 KB PNG showing the GBA shell
(a bare Xvfb capture is a few KB of black). Full gameplay verification: send
Return (title → quest select), then `x` (starts a quest), and screenshot after
each — battle log must show streamed command output. Failure diagnosis: an
unchanged title screen across shots means key events were ignored — you used
`xdotool key --window` (synthetic events WebKit drops) or skipped
`windowfocus`; `BadMatch` on `X_GetImage` means you tried to screenshot on the
real Wayland desktop instead of inside Xvfb. Recovery: kill stray `Xvfb :99`
and app processes, remove nothing — rerun from the top. Rollback of the RPMs:
`sudo dnf remove xwd scrot xorg-x11-server-Xvfb`.

## Decision log

### Decisions

- On the live Wayland session, X screenshot tools cannot read an XWayland
  window's pixels (`X_GetImage` → `BadMatch` — pixels live in the
  compositor), and GNOME's D-Bus screenshot API is caller-restricted, so
  in-session capture was rejected in favor of an Xvfb virtual display, where
  root capture always works and nothing flashes on the user's desktop.
- ImageMagick 7.1.1's `import` fails with a misleading "missing an image
  filename" even against Xvfb — capture via `xwd -root | magick xwd:- png:…`
  instead (that is why `xwd` is installed; `scrot` is the kept fallback).
- Key injection must be XTEST (`xdotool key`, no `--window`) after an explicit
  `xdotool windowfocus`: WebKitGTK ignores synthetic `XSendEvent` keys, and a
  WM-less Xvfb has no input focus until you set one.
- `GDK_BACKEND=x11 WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1`
  keeps GTK/WebKit stable on a software-only virtual display.

### How to drive it

Edit the app, rebuild (`npm run tauri build`), rerun the Re-run block, and
eyeball the PNGs (or assert on them) before claiming a UI change works; add a
screenshot to `docs/screenshots/` when it documents a new screen. If the app's
window title changes, update the `xdotool search --name` pattern. When this
becomes CI, the same block runs unchanged on any headless runner with the
three RPMs present.
