#!/usr/bin/env bash
# Requires `jq` to parse cargo's --message-format=json stream.
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"

cd "$repo_root"
os="$(uname -s)"
ext="so"
[ "$os" = "Darwin" ] && ext="dylib"

# Plain `cargo build`, not a manual `-C extra-filename` — passing that
# through `cargo rustc -- ...` bypasses Cargo's own artifact bookkeeping, so
# `--message-format=json` reports an *empty* filenames array for a target it
# doesn't recognize the (self-chosen) output name of. Get the normal,
# reliably-reported build output instead, then make it unique ourselves by
# copying it to a freshly-named staging file below.
# target.name is the underscored crate name, not the hyphenated package name.
builtDll=$(cargo build -p hot-reload-dll --message-format=json \
  | jq -r --arg name hot_reload_dll 'select(.reason=="compiler-artifact" and .target.name==$name) | .filenames[]' \
  | grep -E "\\.${ext}\$" | tail -n1)
if [ -z "$builtDll" ]; then echo "Could not determine the built payload .$ext path from cargo output." >&2; exit 1; fi

id=$(date +%s%3N)
stagingDir="$repo_root/target/hot-reload-staging"
mkdir -p "$stagingDir"
stagedDll="$stagingDir/libhot_reload_dll-$id.$ext"
cp -f "$builtDll" "$stagedDll"

if [ "$os" = "Darwin" ]; then
  watchDir="$HOME/Library/Application Support/xplm-hotreload"
else
  watchDir="${XDG_DATA_HOME:-$HOME/.local/share}/xplm-hotreload"
fi
mkdir -p "$watchDir"
watchFile="$watchDir/hot-reload-example.json"
jq -n --arg path "$stagedDll" --arg id "$id" '{payload_path: $path, build_id: $id}' > "$watchFile.tmp"
mv -f "$watchFile.tmp" "$watchFile"
echo "Published payload build $id -> $stagedDll"
