#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/artifact-directory" >&2
  exit 2
fi

task_artifact_dir=$(cd -- "$1" && pwd)
task_dmg_packages=("$task_artifact_dir"/*.dmg)
task_zip_packages=("$task_artifact_dir"/*.app.zip)
if [[ ${#task_dmg_packages[@]} -ne 1 || ! -f "${task_dmg_packages[0]}" ]]; then
  echo "FAIL: expected exactly one DMG package" >&2
  exit 1
fi
if [[ ${#task_zip_packages[@]} -ne 1 || ! -f "${task_zip_packages[0]}" ]]; then
  echo "FAIL: expected exactly one zipped app package" >&2
  exit 1
fi

(cd "$task_artifact_dir" && shasum -a 256 -c SHA256SUMS-macos.txt)

task_tmpdir=$(mktemp -d)
task_mount_dir="$task_tmpdir/dmg"
task_mounted=false
cleanup() {
  if [[ "$task_mounted" == true ]]; then
    hdiutil detach "$task_mount_dir" -quiet || true
  fi
  rm -rf "$task_tmpdir"
}
trap cleanup EXIT

test_packaged_app() {
  local task_app=$1
  local task_package_name=$2
  local task_executable_name
  local task_executable
  local task_architectures
  local task_app_pid
  local task_exit_code

  codesign --verify --deep --strict "$task_app"
  task_executable_name=$(/usr/libexec/PlistBuddy \
    -c "Print :CFBundleExecutable" "$task_app/Contents/Info.plist")
  task_executable="$task_app/Contents/MacOS/$task_executable_name"
  task_architectures=$(lipo -archs "$task_executable")
  [[ " $task_architectures " == *" arm64 "* ]] || {
    echo "FAIL: $task_package_name is missing arm64" >&2
    exit 1
  }
  [[ " $task_architectures " == *" x86_64 "* ]] || {
    echo "FAIL: $task_package_name is missing x86_64" >&2
    exit 1
  }

  CQA_NO_AI=1 "$task_executable" \
    >"$task_tmpdir/$task_package_name.log" 2>&1 &
  task_app_pid=$!
  sleep 5
  if ! kill -0 "$task_app_pid" 2>/dev/null; then
    set +e
    wait "$task_app_pid"
    task_exit_code=$?
    set -e
    echo "FAIL: $task_package_name app exited during startup with code $task_exit_code" >&2
    exit 1
  fi
  kill "$task_app_pid"
  wait "$task_app_pid" 2>/dev/null || true
}

task_zip_dir="$task_tmpdir/zip"
mkdir -p "$task_zip_dir"
ditto -x -k "${task_zip_packages[0]}" "$task_zip_dir"
task_zip_app=$(find "$task_zip_dir" -maxdepth 1 -name '*.app' -print -quit)
[[ -n "$task_zip_app" ]] || {
  echo "FAIL: zipped package did not contain an app" >&2
  exit 1
}
test_packaged_app "$task_zip_app" "zip"

mkdir -p "$task_mount_dir"
hdiutil attach -nobrowse -readonly -mountpoint "$task_mount_dir" \
  "${task_dmg_packages[0]}" >/dev/null
task_mounted=true
task_dmg_app=$(find "$task_mount_dir" -maxdepth 1 -name '*.app' -print -quit)
[[ -n "$task_dmg_app" ]] || {
  echo "FAIL: DMG did not contain an app" >&2
  exit 1
}
test_packaged_app "$task_dmg_app" "dmg"
hdiutil detach "$task_mount_dir" -quiet
task_mounted=false

echo "PASS: uploaded app ZIP and DMG passed checksums, signatures, architectures, and startup tests"
