#!/usr/bin/env bash
set -euo pipefail

project_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
asset_dir="$project_dir/src-tauri/assets/oracle"

command -v magick >/dev/null || {
  echo "ImageMagick 'magick' is required." >&2
  exit 1
}

plates=(chronicle awakening gateway atelier sanctum trial ascension aftermath)
for plate in "${plates[@]}"; do
  dimensions=$(magick identify -format '%wx%h' "$asset_dir/$plate.png")
  [[ "$dimensions" == "240x160" ]] || {
    echo "$plate.png must be 240x160, got $dimensions" >&2
    exit 1
  }
  magick "$asset_dir/$plate.png" -alpha off -depth 8 "rgb:$asset_dir/$plate.rgb"
  bytes=$(wc -c < "$asset_dir/$plate.rgb")
  [[ "$bytes" -eq 115200 ]] || {
    echo "$plate.rgb must be 115200 bytes, got $bytes" >&2
    exit 1
  }
done

for sprite in "$asset_dir"/hero-*.png; do
  dimensions=$(magick identify -format '%wx%h' "$sprite")
  [[ "$dimensions" == "24x36" ]] || {
    echo "$(basename "$sprite") must be 24x36, got $dimensions" >&2
    exit 1
  }
  magick "$sprite" -depth 8 "rgba:${sprite%.png}.rgba"
  bytes=$(wc -c < "${sprite%.png}.rgba")
  [[ "$bytes" -eq 3456 ]] || {
    echo "$(basename "${sprite%.png}.rgba") must be 3456 bytes, got $bytes" >&2
    exit 1
  }
done

for portrait in "$asset_dir"/portrait-*.png; do
  dimensions=$(magick identify -format '%wx%h' "$portrait")
  [[ "$dimensions" == "24x24" ]] || {
    echo "$(basename "$portrait") must be 24x24, got $dimensions" >&2
    exit 1
  }
  magick "$portrait" -depth 8 "rgba:${portrait%.png}.rgba"
  bytes=$(wc -c < "${portrait%.png}.rgba")
  [[ "$bytes" -eq 2304 ]] || {
    echo "$(basename "${portrait%.png}.rgba") must be 2304 bytes, got $bytes" >&2
    exit 1
  }
done

for drop in "$asset_dir"/drop-*.png; do
  dimensions=$(magick identify -format '%wx%h' "$drop")
  [[ "$dimensions" == "16x16" ]] || {
    echo "$(basename "$drop") must be 16x16, got $dimensions" >&2
    exit 1
  }
  magick "$drop" -depth 8 "rgba:${drop%.png}.rgba"
  bytes=$(wc -c < "${drop%.png}.rgba")
  [[ "$bytes" -eq 1024 ]] || {
    echo "$(basename "${drop%.png}.rgba") must be 1024 bytes, got $bytes" >&2
    exit 1
  }
done

echo "Oracle assets compiled at 240x160."
