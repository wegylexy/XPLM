#!/usr/bin/env bash
# Downloads the X-Plane SDK zip and places it in both vendored locations this
# workspace's build scripts expect: xplm-sys/vendor/xplm-sdk (full SDK) and
# xpmp2-sys/vendor/xplm-sdk/CHeaders/XPLM (headers-only subset).
#
# Both destinations are gitignored — this script is the replacement for a
# manual download-and-unzip step, run once per machine (or whenever the SDK
# version changes).
set -euo pipefail

sdk_url='https://developer.x-plane.com/wp-content/plugins/code-sample-generation/sdk_zip_files/XPSDK430.zip'
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_zip="$(mktemp -t xplm-sdk.XXXXXX.zip)"
tmp_extract="$(mktemp -d -t xplm-sdk-extract.XXXXXX)"

echo "Downloading $sdk_url..."
curl -fsSL -o "$tmp_zip" "$sdk_url"

echo "Extracting..."
unzip -q "$tmp_zip" -d "$tmp_extract"

# The zip's top-level folder is already named "SDK" (all-caps) when
# extracted — no rename needed, unlike what this repo's README used to claim.
extracted_sdk="$tmp_extract/SDK"
if [ ! -d "$extracted_sdk" ]; then
  echo "Expected an 'SDK' folder inside the extracted zip at $tmp_extract, found:" >&2
  ls "$tmp_extract" >&2
  exit 1
fi

xplm_sys_dest="$repo_root/xplm-sys/vendor/xplm-sdk"
xpmp2_sys_dest="$repo_root/xpmp2-sys/vendor/xplm-sdk/CHeaders/XPLM"

echo "Installing full SDK to $xplm_sys_dest..."
rm -rf "$xplm_sys_dest"
mkdir -p "$(dirname "$xplm_sys_dest")"
mv "$extracted_sdk" "$xplm_sys_dest"

echo "Installing XPLM headers subset to $xpmp2_sys_dest..."
rm -rf "$xpmp2_sys_dest"
mkdir -p "$(dirname "$xpmp2_sys_dest")"
cp -r "$xplm_sys_dest/CHeaders/XPLM" "$xpmp2_sys_dest"

rm -f "$tmp_zip"
rm -rf "$tmp_extract"

echo 'Done. Run `git submodule update --init xpmp2-sys/vendor/XPMP2` if you also need xpmp2/xpmp2-sys.'
