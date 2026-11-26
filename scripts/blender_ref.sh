#!/usr/bin/env bash
# Clone (or update) the read-only Blender reference repo next to this repo and search it.
# Usage: scripts/blender_ref.sh setup | find <name-fragment> | grep <text>
set -euo pipefail
DIR="${BLENDER_REF_DIR:-$(cd "$(dirname "$0")/../.." && pwd)/blender}"
case "${1:-}" in
  setup) if [ -d "$DIR/.git" ]; then git -C "$DIR" pull --ff-only; else git clone --depth 1 https://github.com/aymenguentri1/blender.git "$DIR"; fi; echo "reference repo at $DIR" ;;
  find)  git -C "$DIR" ls-files | grep -i -- "$2" ;;
  grep)  git -C "$DIR" grep -n -i -- "$2" -- source scripts release | head -100 ;;
  *) echo "usage: $0 setup | find <fragment> | grep <text>"; exit 1 ;;
esac
