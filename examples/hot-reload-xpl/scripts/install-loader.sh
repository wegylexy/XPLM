#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"

cd "$repo_root"
cargo build -p hot-reload-xpl

xp_root="$("$script_dir/find-xplane-root.sh")"
dest="$xp_root/Resources/plugins/hot-reload-xpl/64"
mkdir -p "$dest"

os="$(uname -s)"
if [ "$os" = "Darwin" ]; then
  cp -f "$repo_root/target/debug/libhot_reload_xpl.dylib" "$dest/mac.xpl"
  echo "Installed loader to $dest/mac.xpl"
else
  cp -f "$repo_root/target/debug/libhot_reload_xpl.so" "$dest/lin.xpl"
  echo "Installed loader to $dest/lin.xpl"
fi
