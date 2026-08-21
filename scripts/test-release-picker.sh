#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/code-quest-advance.AppImage" >&2
  exit 2
fi

task_appimage=$1
task_app_args=()
if [[ "$task_appimage" == *.AppImage ]]; then
  task_app_args+=(--appimage-extract-and-run)
fi
task_display=
for task_display_number in $(seq 97 109); do
  if [[ ! -S "/tmp/.X11-unix/X$task_display_number" ]]; then
    task_display=:$task_display_number
    break
  fi
done
if [[ -z "$task_display" ]]; then
  echo "FAIL: no free X display in the 97-109 test range" >&2
  exit 1
fi
task_tmpdir=$(mktemp -d)
task_xvfb_pid=
task_app_pid=

cleanup() {
  if [[ -n "$task_app_pid" ]]; then
    kill "$task_app_pid" 2>/dev/null || true
  fi
  if [[ -n "$task_xvfb_pid" ]]; then
    kill "$task_xvfb_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

Xvfb "$task_display" -screen 0 1024x720x24 -nolisten tcp \
  >"$task_tmpdir/xvfb.log" 2>&1 &
task_xvfb_pid=$!

DISPLAY="$task_display" \
GDK_BACKEND=x11 \
WEBKIT_DISABLE_COMPOSITING_MODE=1 \
LIBGL_ALWAYS_SOFTWARE=1 \
XDG_DATA_HOME="$task_tmpdir/data" \
XDG_CONFIG_HOME="$task_tmpdir/config" \
"$task_appimage" "${task_app_args[@]}" \
  >"$task_tmpdir/app.log" 2>&1 &
task_app_pid=$!

task_window=
for _ in $(seq 1 100); do
  task_window=$(DISPLAY="$task_display" \
    xdotool search --name "CODE QUEST ADVANCE" 2>/dev/null | tail -1 || true)
  if [[ -n "$task_window" ]]; then
    break
  fi
  sleep .1
done
if [[ -z "$task_window" ]]; then
  echo "FAIL: release window did not open; logs: $task_tmpdir/app.log" >&2
  exit 1
fi

sleep 3
DISPLAY="$task_display" xdotool windowfocus --sync "$task_window"
DISPLAY="$task_display" xdotool key c
sleep .3
DISPLAY="$task_display" xdotool mousemove 416 365 click 1
sleep 1

DISPLAY="$task_display" xwd -root -silent |
  magick xwd:- "$task_tmpdir/after-picker-click.png"
task_named_windows=$(DISPLAY="$task_display" \
  xdotool search --onlyvisible --name '.+' getwindowname %@ 2>/dev/null || true)
task_picker_windows=$(printf '%s\n' "$task_named_windows" |
  grep -F 'SELECT CARTRIDGE (GIT REPO)' || true)

if [[ -z "$task_picker_windows" ]]; then
  echo "FAIL: clicking ADD FROM DISK did not open a native folder picker"
  echo "Screenshot: $task_tmpdir/after-picker-click.png"
  exit 1
fi

echo "PASS: native folder picker opened:"
printf '%s\n' "$task_picker_windows"
