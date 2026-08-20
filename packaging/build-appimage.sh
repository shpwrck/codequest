#!/usr/bin/env bash
# Build the portable release AppImage in a container and drop it in dist/.
#
# Why a container: see packaging/Containerfile. A host build on Fedora/Arch
# produces a binary that requires a glibc newer than RHEL 9's and will not start
# there; this pins the floor to Ubuntu 22.04 (glibc 2.35).
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
image=${IMAGE:-code-quest-advance-builder}
out_dir=$repo_root/dist

command -v podman >/dev/null || { echo "podman is required" >&2; exit 1; }

podman build \
  --ignorefile "$repo_root/packaging/containerignore" \
  --file "$repo_root/packaging/Containerfile" \
  --tag "$image" \
  "$repo_root"

mkdir -p "$out_dir"
cid=$(podman create "$image")
trap 'podman rm -f "$cid" >/dev/null 2>&1 || true' EXIT
podman cp "$cid:/out/." "$out_dir/"

echo
echo "artifacts in $out_dir:"
ls -lh "$out_dir"
echo
cat "$out_dir/portability.txt"
