#!/usr/bin/env bash
# Prints the first X-Plane install root that actually looks like one (has a
# Resources folder), scanning every line of x-plane_install_12.txt/_11.txt —
# per X-Plane's own install-location documentation, a line-1-only read isn't
# safe: the file can list multiple locations, including stale/moved ones.
set -euo pipefail

os="$(uname -s)"
case "$os" in
  Darwin)
    candidates=("$HOME/Library/Preferences/x-plane_install_12.txt" "$HOME/Library/Preferences/x-plane_install_11.txt")
    ;;
  Linux)
    candidates=("$HOME/.x-plane/x-plane_install_12.txt" "$HOME/.x-plane/x-plane_install_11.txt")
    ;;
  *)
    echo "Unsupported OS for this script: $os" >&2
    exit 1
    ;;
esac

for f in "${candidates[@]}"; do
  [ -f "$f" ] || continue
  while IFS= read -r line || [ -n "$line" ]; do
    root="${line%$'\r'}"
    [ -n "$root" ] || continue
    if [ -d "$root/Resources" ]; then
      echo "$root"
      exit 0
    fi
  done < "$f"
done

echo "No valid X-Plane install found (checked: ${candidates[*]})" >&2
exit 1
