#!/usr/bin/env bash
# Check a NomNom Nanny AppImage before it is published.
# Confirms the host Wayland client is what WebKit will load.
# --launch is an Xvfb smoke test on the build machine. It does not boot
# Mesa 26 or a Wayland session; use the README steps for that.
set -euo pipefail

if [[ $# -lt 1 || "$1" == "-h" || "$1" == "--help" ]]; then
  echo "Usage: $0 [--launch] <package.AppImage>" >&2
  exit 2
fi

launch=()
if [[ "$1" == "--launch" ]]; then
  launch=(--launch)
  shift
fi

root=$(cd "$(dirname "$0")" && pwd)
exec "$root/repair-linux-appimage.sh" --check "${launch[@]}" "$1"
