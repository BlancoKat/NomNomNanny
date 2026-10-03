#!/usr/bin/env bash
# Repair a linuxdeploy AppImage so WebKit can create an EGL display on
# current Mesa (Arch, Omarchy, Fedora 40+).
#
# The published v0.1.0 image bundles Ubuntu 22.04's libwayland-client.so.0.
# AppRun puts that directory first on LD_LIBRARY_PATH, so the host's Mesa
# libEGL_mesa.so.0 binds the bundled client. Mesa 26 references
# wl_display_create_queue_with_name, which that client does not export.
# The lookup fails, eglGetDisplay returns EGL_BAD_PARAMETER, and the bundled
# WebKit aborts the web process:
#   Could not create default EGL display: %s. Aborting...
# The UI process and WebKitNetworkProcess stay up, so the window has no renderer.
# Forcing GDK_BACKEND=x11 (which that image also does) is a separate problem:
# the window still maps through XWayland. It does not cause this abort.
#
# Usage:
#   repair-linux-appimage.sh --in-place <file.AppImage>
#   repair-linux-appimage.sh --check [--launch] <file.AppImage>
set -euo pipefail

MODE=repair
LAUNCH=0

usage() {
  cat <<'EOF'
Usage: repair-linux-appimage.sh --in-place <file.AppImage>
       repair-linux-appimage.sh --check [--launch] <file.AppImage>

--in-place   Remove host display libraries, stop forcing GDK_BACKEND=x11,
             record stderr when it is not a terminal, and repack.
--check      Fail if an AppImage still shadows the host Wayland client.
--launch     With --check, start under Xvfb and fail if WebKit aborts EGL init.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --in-place) MODE=repair; shift ;;
    --check) MODE=check; shift ;;
    --launch) LAUNCH=1; shift ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;;
    -*)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *) break ;;
  esac
done

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 2
fi

appimage=$(readlink -f "$1")
if [[ ! -f "$appimage" ]]; then
  echo "AppImage not found: $appimage" >&2
  exit 1
fi

# Host graphics libraries linuxdeploy copied in from the build distro.
# libwayland-server stays bundled: WebKit NEEDs it, and Debian/Ubuntu do not
# always install the server library with the client.
HOST_DISPLAY_LIBS=(
  'libwayland-client.so*'
  'libwayland-cursor.so*'
  'libwayland-egl.so*'
  'libxkbcommon.so*'
  'libxcb-randr.so*'
  'libxcb-render.so*'
  'libxcb-shm.so*'
  'libXau.so*'
  'libXdmcp.so*'
)

workdir=$(mktemp -d)
cleanup() { rm -rf "$workdir"; }
trap cleanup EXIT

extract_appimage() {
  local dest=$1
  mkdir -p "$dest"
  (
    cd "$dest"
    "$appimage" --appimage-extract >/dev/null
  )
}

remove_host_display_libs() {
  local appdir=$1
  local name file
  for name in "${HOST_DISPLAY_LIBS[@]}"; do
    while IFS= read -r -d '' file; do
      rm -f "$file"
    done < <(find "$appdir" -name "$name" -print0)
  done
}

patch_gtk_hook() {
  local hook=$1
  python3 - "$hook" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
if "nomnom-nanny-gdk-backend" in text:
    sys.exit(0)
block = """# nomnom-nanny-gdk-backend
# Do not force x11. An explicit GDK_BACKEND from the environment is preserved.
# On a Wayland session, try the native backend first and fall back to x11
# (XWayland or Xorg). Forcing x11 was not what aborted the web process; it
# does break a Wayland session that has no XWayland.
if [ -z "${GDK_BACKEND:-}" ]; then
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then
    export GDK_BACKEND=wayland,x11
  else
    export GDK_BACKEND=x11
  fi
fi"""
pattern = re.compile(r"^export GDK_BACKEND=.*$", re.M)
new, count = pattern.subn(block, text, count=1)
if count != 1:
    sys.stderr.write(f"{path}: no GDK_BACKEND assignment to replace\n")
    sys.exit(1)
path.write_text(new)
PY
}

patch_apprun() {
  local apprun=$1
  python3 - "$apprun" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
text = path.read_text()
if "nomnom-nanny-stderr-log" in text:
    sys.exit(0)
needle = 'source "$this_dir"/apprun-hooks/"linuxdeploy-plugin-gtk.sh"\n'
insert = '''# nomnom-nanny-stderr-log
# gtk-launch and other desktop launchers leave stderr on /dev/null, which
# discarded WebKit's "Could not create default EGL display: …" line.
_nn_log_stderr=0
if [ ! -t 2 ]; then
  _nn_log_stderr=1
  _nn_log_dir="${XDG_STATE_HOME:-${HOME:-}/.local/state}/nomnom-nanny"
  if [ -z "${HOME:-}" ] && [ -z "${XDG_STATE_HOME:-}" ]; then
    _nn_log_dir="${TMPDIR:-/tmp}/nomnom-nanny"
  fi
  mkdir -p "$_nn_log_dir" 2>/dev/null || _nn_log_dir="${TMPDIR:-/tmp}"
  _nn_log_file="$_nn_log_dir/appimage.log"
  if exec 2>>"$_nn_log_file"; then
    printf '\\n--- %s ---\\n' "$(date -Is 2>/dev/null || date)" >&2 || true
  fi
fi
source "$this_dir"/apprun-hooks/"linuxdeploy-plugin-gtk.sh"
if [ "$_nn_log_stderr" -eq 1 ]; then
  echo "nomnom-nanny: logging to ${_nn_log_file:-stderr}" >&2
  echo "nomnom-nanny: GDK_BACKEND=${GDK_BACKEND:-unset} WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset}" >&2
  echo "nomnom-nanny: a WebKit line \\"Could not create default EGL display\\" means the web process aborted." >&2
fi
'''
if needle not in text:
    sys.stderr.write(f"{path}: AppRun hook source line not found\n")
    sys.exit(1)
path.write_text(text.replace(needle, insert, 1))
PY
}

assert_absent() {
  local appdir=$1
  local name file found=0
  for name in "${HOST_DISPLAY_LIBS[@]}"; do
    while IFS= read -r -d '' file; do
      echo "still bundled: $file" >&2
      found=1
    done < <(find "$appdir" -name "$name" -print0)
  done
  [[ "$found" -eq 0 ]]
}

verify_tree() {
  local appdir=$1
  local hook="$appdir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
  local apprun="$appdir/AppRun"
  local webkit="$appdir/usr/lib/libwebkit2gtk-4.1.so.0"
  [[ -f "$hook" && -f "$apprun" && -f "$webkit" ]] || {
    echo "AppDir is missing the GTK hook, AppRun, or WebKit" >&2
    exit 1
  }
  assert_absent "$appdir"
  if [[ ! -e "$appdir/usr/lib/libwayland-server.so.0" ]]; then
    echo "libwayland-server.so.0 was removed; bundled WebKit still needs it" >&2
    exit 1
  fi
  if grep -q '^export GDK_BACKEND=x11' "$hook"; then
    echo "GTK hook still forces GDK_BACKEND=x11" >&2
    exit 1
  fi
  grep -q 'nomnom-nanny-gdk-backend' "$hook"
  grep -q 'GDK_BACKEND=wayland,x11' "$hook"
  grep -q 'nomnom-nanny-stderr-log' "$apprun"
  if ! grep -a -q 'Could not create default EGL display: %s. Aborting...' "$webkit"; then
    echo "bundled WebKit is missing the expected EGL abort string" >&2
    exit 1
  fi

  local resolved
  resolved=$(LD_LIBRARY_PATH="$appdir/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    ldd "$webkit" | awk '/libwayland-client\.so\.0/{print $3; exit}')
  if [[ -z "$resolved" || "$resolved" == "not" ]]; then
    echo "libwebkit2gtk could not resolve a host libwayland-client.so.0" >&2
    exit 1
  fi
  case "$resolved" in
    "$appdir"/*)
      echo "WebKit still binds bundled libwayland-client: $resolved" >&2
      exit 1
      ;;
  esac
  echo "WebKit binds host libwayland-client: $resolved"

  local server
  server=$(LD_LIBRARY_PATH="$appdir/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    ldd "$webkit" | awk '/libwayland-server\.so\.0/{print $3; exit}')
  case "$server" in
    "$appdir"/*) ;;
    *)
      echo "WebKit did not bind the bundled libwayland-server ($server)" >&2
      exit 1
      ;;
  esac
}

launch_check() {
  local image=$1
  if ! command -v xvfb-run >/dev/null 2>&1; then
    echo "xvfb-run is not available" >&2
    exit 1
  fi
  local log
  log=$(mktemp)
  set +e
  timeout 15 xvfb-run -a "$image" >"$log" 2>&1
  local status=$?
  set -e
  if grep -q 'Could not create default EGL display' "$log"; then
    echo "WebKit aborted during EGL display creation" >&2
    cat "$log" >&2
    exit 1
  fi
  if [[ "$status" -ne 124 ]]; then
    echo "AppImage exited early (status $status)" >&2
    cat "$log" >&2
    exit 1
  fi
  echo "AppImage stayed running under xvfb"
  rm -f "$log"
}

download() {
  local url=$1 dest=$2
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$dest" "$url"
  else
    python3 - "$url" "$dest" <<'PY'
import sys, urllib.request
urllib.request.urlretrieve(sys.argv[1], sys.argv[2])
PY
  fi
}

ensure_appimagetool() {
  local arch=$1
  local cache="${XDG_CACHE_HOME:-$HOME/.cache}/nomnom-nanny"
  local tool="$cache/appimagetool-${arch}.AppImage"
  if [[ ! -x "$tool" ]]; then
    mkdir -p "$cache"
    local tool_arch=$arch
    if [[ "$arch" == "aarch64" ]]; then
      tool_arch=aarch64
    fi
    download \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${tool_arch}.AppImage" \
      "$tool"
    chmod +x "$tool"
  fi
  printf '%s\n' "$tool"
}

repack() {
  local appdir=$1
  local arch
  arch=$(uname -m)
  case "$arch" in
    x86_64|aarch64) ;;
    arm64) arch=aarch64 ;;
    *)
      echo "unsupported architecture for repack: $arch" >&2
      exit 1
      ;;
  esac
  local tool out
  tool=$(ensure_appimagetool "$arch")
  out=$(mktemp "$workdir/repacked.XXXXXX.AppImage")
  ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 \
    "$tool" --appimage-extract-and-run --no-appstream "$appdir" "$out"
  chmod +x "$out"
  mv "$out" "$appimage"
}

extract_appimage "$workdir"
appdir="$workdir/squashfs-root"

if [[ "$MODE" == "check" ]]; then
  verify_tree "$appdir"
  if [[ "$LAUNCH" -eq 1 ]]; then
    launch_check "$appimage"
  fi
  echo "AppImage display libraries look safe: $appimage"
  exit 0
fi

remove_host_display_libs "$appdir"
patch_gtk_hook "$appdir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
patch_apprun "$appdir/AppRun"
verify_tree "$appdir"
repack "$appdir"
echo "Repacked $appimage without the host display libraries"

# Confirm the bytes we wrote, not only the extracted tree.
check_dir=$(mktemp -d)
(
  cd "$check_dir"
  "$appimage" --appimage-extract >/dev/null
)
verify_tree "$check_dir/squashfs-root"
rm -rf "$check_dir"
echo "Repacked AppImage verified"
