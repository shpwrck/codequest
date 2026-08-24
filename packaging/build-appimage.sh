#!/usr/bin/env bash
# Build the portable release AppImage in a container and drop it in dist/.
#
# Why a container: see packaging/Containerfile. A host build on Fedora/Arch
# produces a binary that requires a glibc newer than RHEL 9's and will not start
# there; this pins the floor to Ubuntu 22.04 (glibc 2.35).
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
container_image=${CQA_CONTAINER_IMAGE:-code-quest-advance-builder}
out_dir=$repo_root/dist
build_revision=${CQA_BUILD_REVISION:-}

if [[ -z "$build_revision" ]]; then
  build_revision=$(git -C "$repo_root" rev-parse HEAD)
fi
if [[ ! "$build_revision" =~ ^[0-9a-fA-F]{7,40}$ ]]; then
  echo "CQA_BUILD_REVISION must be a Git commit SHA" >&2
  exit 1
fi

container_engine=${CONTAINER_ENGINE:-}
if [[ -z "$container_engine" ]]; then
  if command -v podman >/dev/null; then
    container_engine=podman
  elif command -v docker >/dev/null; then
    container_engine=docker
  else
    echo "podman or docker is required" >&2
    exit 1
  fi
fi
command -v "$container_engine" >/dev/null || {
  echo "$container_engine is not available" >&2
  exit 1
}

"$container_engine" build \
  --file "$repo_root/packaging/Containerfile" \
  --build-arg "CQA_BUILD_REVISION=$build_revision" \
  --tag "$container_image" \
  "$repo_root"

mkdir -p "$out_dir"
cid=$("$container_engine" create "$container_image")
cleanup() {
  "$container_engine" rm -f "$cid" >/dev/null 2>&1 || true
}
trap cleanup EXIT
"$container_engine" cp "$cid:/out/." "$out_dir/"

echo
echo "artifacts in $out_dir:"
ls -lh "$out_dir"
echo
cat "$out_dir/portability.txt"
