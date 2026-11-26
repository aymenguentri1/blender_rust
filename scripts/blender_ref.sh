#!/usr/bin/env bash
# Read-only Blender reference (aymenguentri1/blender), cheap: partial clone, files are fetched only when you read them.
# setup | find <fragment> | tree <dir> | show <path> [from] [to] | grep <text> [pathspec]
set -euo pipefail
DIR="${BLENDER_REF_DIR:-$(cd "$(dirname "$0")/../.." && pwd)/blender}"
case "${1:-}" in
  setup) if [ -d "$DIR/.git" ]; then git -C "$DIR" fetch -q --depth 1 origin main && git -C "$DIR" reset -q --soft FETCH_HEAD 2>/dev/null || true
         else git clone -q --depth 1 --filter=blob:none --no-checkout https://github.com/aymenguentri1/blender.git "$DIR"; fi
         echo "reference ready at $DIR ($(git -C "$DIR" ls-tree -r --name-only HEAD | wc -l) files, content fetched on demand)" ;;
  find)  git -C "$DIR" ls-tree -r --name-only HEAD | grep -i -- "$2" | head -${LIMIT:-60} ;;
  tree)  git -C "$DIR" ls-tree --name-only HEAD "${2%/}/" ;;
  show)  if [ -n "${3:-}" ]; then git -C "$DIR" show "HEAD:$2" | sed -n "${3},${4:-$(( ${3} + 120 ))}p"; else git -C "$DIR" show "HEAD:$2" | head -${LIMIT:-250}; fi ;;
  grep)  git -C "$DIR" grep -n -i -e "$2" HEAD -- "${3:-source/blender/editors}" | sed 's/^HEAD://' | head -${LIMIT:-60} ;;
  *) echo "usage: $0 setup | find <fragment> | tree <dir> | show <path> [from] [to] | grep <text> [pathspec]"; exit 1 ;;
esac
