#!/usr/bin/env bash
# Check a NomNom Nanny .deb before it is published or installed.
# Confirms the desktop filename is safe and apt can satisfy WebKit/GTK.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

if [[ $# -lt 1 || "$1" == "-h" || "$1" == "--help" ]]; then
  echo "Usage: $0 [--launch] <package.deb>" >&2
  exit 2
fi

launch=0
if [[ "$1" == "--launch" ]]; then
  launch=1
  shift
fi

deb=$(readlink -f "$1")
if [[ ! -f "$deb" ]]; then
  echo "deb not found: $deb" >&2
  exit 1
fi

echo "Checking $deb"
dpkg-deb -I "$deb"

desktop_paths=$(dpkg-deb -c "$deb" | grep -o 'usr/share/applications/.*\.desktop' || true)
if [[ -z "$desktop_paths" ]]; then
  echo "deb has no .desktop file" >&2
  exit 1
fi
if grep -q '[[:space:]]' <<<"$desktop_paths"; then
  echo "desktop filename contains whitespace (breaks some installers):" >&2
  printf '%s\n' "$desktop_paths" >&2
  exit 1
fi
echo "desktop file: $desktop_paths"

depends=$(dpkg-deb -f "$deb" Depends)
echo "Depends: $depends"
grep -q 'libwebkit2gtk-4.1-0' <<<"$depends" || {
  echo "missing libwebkit2gtk-4.1-0 dependency" >&2
  exit 1
}
grep -q 'libgtk-3-0' <<<"$depends" || {
  echo "missing libgtk-3-0 dependency" >&2
  exit 1
}

if ! command -v apt-get >/dev/null 2>&1; then
  echo "apt-get is not available; metadata checks passed"
  exit 0
fi

sudo apt-get update -qq
sudo apt-get install -s -y "$deb"
echo "apt can satisfy $deb"

if [[ "$launch" -eq 1 ]]; then
  sudo apt-get install -y "$deb" xvfb
  set +e
  timeout 15 xvfb-run -a nomnom-nanny
  status=$?
  set -e
  # timeout exits 124 when the app is still running. Any other status means it quit.
  if [[ "$status" -ne 124 ]]; then
    echo "nomnom-nanny exited early (status $status)" >&2
    exit 1
  fi
  echo "nomnom-nanny stayed running under xvfb"
fi
