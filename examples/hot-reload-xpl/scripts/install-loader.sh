#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"

xp_root="$("$script_dir/find-xplane-root.sh")"
dest="$xp_root/Resources/plugins/hot-reload-xpl/64"

os="$(uname -s)"
if [ "$os" = "Darwin" ]; then
  dest_file="$dest/mac.xpl"
  src_file="$repo_root/target/debug/libhot_reload_xpl.dylib"
else
  dest_file="$dest/lin.xpl"
  src_file="$repo_root/target/debug/libhot_reload_xpl.so"
fi

xplane_running=0
pgrep -f 'X-Plane' > /dev/null && xplane_running=1

if [ "$xplane_running" -eq 1 ] && [ -f "$dest_file" ]; then
  # X-Plane already has this file open -- overwriting it out from under a
  # running process is unsafe/pointless, and even if it succeeded,
  # sim/operation/reload_plugins can't make an already-running X-Plane pick up
  # a changed file for a plugin it already loaded at boot without a restart.
  echo 'X-Plane is already running with the loader loaded; leaving it as-is (restart X-Plane to apply loader changes).'
  exit 0
fi

cd "$repo_root"
cargo build -p hot-reload-xpl

mkdir -p "$dest"
cp -f "$src_file" "$dest_file"
echo "Installed loader to $dest_file"

if [ "$xplane_running" -eq 1 ]; then
  # dest_file didn't exist yet above, so X-Plane never scanned this plugin
  # folder at its own boot and won't notice it exists without a restart.
  echo 'X-Plane is already running: restart it to pick up this newly-installed loader -- there is no way to make a running X-Plane notice a new plugin without a restart.'
fi
