#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if pgrep -f 'X-Plane' > /dev/null; then
  echo 'X-Plane already running.'
  exit 0
fi

xp_root="$("$script_dir/find-xplane-root.sh")"
os="$(uname -s)"

if [ "$os" = "Darwin" ]; then
  app=$(find "$xp_root" -maxdepth 1 -iname 'X-Plane*.app' | head -n1)
  if [ -z "$app" ]; then echo "Could not find an X-Plane.app under $xp_root" >&2; exit 1; fi
  echo "Starting $app..."
  open "$app"
else
  exe=$(find "$xp_root" -maxdepth 1 -iname 'X-Plane*' -executable -type f | head -n1)
  if [ -z "$exe" ]; then echo "Could not find an X-Plane executable under $xp_root" >&2; exit 1; fi
  echo "Starting $exe..."
  nohup "$exe" > /dev/null 2>&1 &
fi

deadline=$((SECONDS + 120))
until pgrep -f 'X-Plane' > /dev/null; do
  if [ $SECONDS -gt $deadline ]; then echo 'Timed out waiting for X-Plane to start.' >&2; exit 1; fi
  sleep 1
done
echo 'X-Plane process detected; first cold launch can take a while before flight loops start ticking.'
