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
  dest_file="$dest/mac.xpl"
  src_file="$repo_root/target/debug/libhot_reload_xpl.dylib"
else
  dest_file="$dest/lin.xpl"
  src_file="$repo_root/target/debug/libhot_reload_xpl.so"
fi

is_fresh_install=1
[ -f "$dest_file" ] && is_fresh_install=0

cp -f "$src_file" "$dest_file"
echo "Installed loader to $dest_file"

# X-Plane only scans Resources/plugins at startup. If the loader wasn't
# installed before this run and X-Plane is already running, it won't notice
# the new plugin without a restart -- unless we tell it to rescan over UDP.
if [ "$is_fresh_install" -eq 1 ] && pgrep -f 'X-Plane' > /dev/null; then
  echo 'Fresh install with X-Plane already running; asking it to reload plugins over UDP...'
  if ! (cd "$repo_root" && cargo run -q -p flybywireless-xplm-reloader --bin reload-plugins); then
    echo 'Reload request failed; restart X-Plane manually to pick up the new plugin.'
  fi
fi
