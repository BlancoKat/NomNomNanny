#!/usr/bin/env bash
# Install NomNom Nanny on a fresh Linux machine.
#
# Prefers a native package (deb on apt, rpm on dnf/zypper). Arch and other
# distros get the AppImage. Runtime libraries are installed first so the app
# does not fail with a missing WebKitGTK loader. An AppImage is repacked so it
# does not shadow the host Wayland client (that abort leaves a window with no
# renderer on current Mesa). AppImages that cannot mount FUSE are launched
# with --appimage-extract-and-run.
#
# Usage:
#   ./scripts/install-linux.sh [options] [package.deb|package.rpm|package.AppImage]
#
# With no package path, a bundle under src-tauri/target/release/bundle is used.
# If none exists, the latest public GitHub release asset is downloaded.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

REPO="BlancoKat/NomNomNanny"
PREFIX="${HOME}/.local"
SKIP_DEPS=0
DOWNLOAD_ONLY=""

usage() {
  cat <<'EOF'
Usage: install-linux.sh [options] [package]

Options:
  --prefix DIR       Install AppImage launchers under DIR (default: ~/.local)
  --no-deps          Do not install WebKitGTK / GTK / FUSE packages
  --download-only DIR
                     Download the latest public release asset into DIR and exit
  -h, --help         Show this help

A .deb is installed with apt so dependencies are pulled in. Do not use dpkg -i
on its own; that leaves libwebkit2gtk-4.1-0 uninstalled and the app will not start.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"
      shift 2
      ;;
    --no-deps)
      SKIP_DEPS=1
      shift
      ;;
    --download-only)
      DOWNLOAD_ONLY="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      break
      ;;
  esac
done

package="${1:-}"
arch=$(uname -m)

case "$arch" in
  x86_64) deb_suffix="_amd64.deb"; rpm_suffix=".x86_64.rpm"; appimage_suffix="_amd64.AppImage" ;;
  aarch64|arm64) deb_suffix="_arm64.deb"; rpm_suffix=".aarch64.rpm"; appimage_suffix="_aarch64.AppImage" ;;
  *)
    echo "unsupported architecture: $arch" >&2
    exit 1
    ;;
esac

have() { command -v "$1" >/dev/null 2>&1; }

is_apt() { have apt-get; }
is_dnf() { have dnf; }
is_zypper() { have zypper; }
is_pacman() { have pacman; }

preferred_kind() {
  if is_apt; then
    echo deb
  elif is_dnf || is_zypper; then
    echo rpm
  else
    echo appimage
  fi
}

find_local_bundle() {
  local kind="$1"
  local root
  root=$(cd "$(dirname "$0")/.." && pwd)
  local matches=()
  shopt -s nullglob
  case "$kind" in
    deb) matches=("$root"/src-tauri/target/release/bundle/deb/*"$deb_suffix") ;;
    rpm) matches=("$root"/src-tauri/target/release/bundle/rpm/*"$rpm_suffix") ;;
    appimage) matches=("$root"/src-tauri/target/release/bundle/appimage/*"$appimage_suffix") ;;
  esac
  shopt -u nullglob
  if [[ ${#matches[@]} -gt 0 ]]; then
    printf '%s\n' "${matches[-1]}"
  fi
}

download_asset() {
  local kind="$1"
  local dest_dir="$2"
  mkdir -p "$dest_dir"
  python3 - "$REPO" "$kind" "$deb_suffix" "$rpm_suffix" "$appimage_suffix" "$dest_dir" <<'PY'
import json, os, sys, urllib.request
repo, kind, deb_suffix, rpm_suffix, appimage_suffix, dest = sys.argv[1:]
suffix = {"deb": deb_suffix, "rpm": rpm_suffix, "appimage": appimage_suffix}[kind]
url = f"https://api.github.com/repos/{repo}/releases/latest"
req = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json", "User-Agent": "nomnom-nanny-install"})
try:
    with urllib.request.urlopen(req) as resp:
        data = json.load(resp)
except Exception as exc:
    sys.stderr.write(
        "No public GitHub release is available to download "
        f"({exc}).\n"
        "The installer previously published from this repo was an untagged draft "
        "whose files return 404 to a normal browser session. Pass a local "
        ".deb, .rpm, or .AppImage, or build from source with npm run tauri build.\n"
    )
    sys.exit(1)
assets = data.get("assets") or []
chosen = next((a for a in assets if a.get("name", "").endswith(suffix)), None)
if chosen is None:
    names = ", ".join(a.get("name", "") for a in assets) or "(none)"
    sys.stderr.write(f"latest release has no {suffix} asset. Found: {names}\n")
    sys.exit(1)
target = os.path.join(dest, chosen["name"])
print(f"Downloading {chosen['browser_download_url']}", file=sys.stderr)
urllib.request.urlretrieve(chosen["browser_download_url"], target)
print(target)
PY
}

install_apt_deps() {
  sudo apt-get update
  local packages=(libwebkit2gtk-4.1-0)
  if apt-cache show libgtk-3-0t64 >/dev/null 2>&1; then
    packages+=(libgtk-3-0t64)
  else
    packages+=(libgtk-3-0)
  fi
  if apt-cache show libfuse2t64 >/dev/null 2>&1; then
    packages+=(libfuse2t64)
  elif apt-cache show libfuse2 >/dev/null 2>&1; then
    packages+=(libfuse2)
  fi
  sudo apt-get install -y "${packages[@]}"
}

install_runtime_deps() {
  if [[ "$SKIP_DEPS" -eq 1 ]]; then
    return
  fi
  if ldconfig -p 2>/dev/null | grep -q 'libwebkit2gtk-4.1.so.0'; then
    echo "WebKitGTK 4.1 is already installed"
    return
  fi
  echo "Installing WebKitGTK 4.1 and GTK (required to start the app)"
  if is_apt; then
    install_apt_deps
  elif is_dnf; then
    sudo dnf install -y webkit2gtk4.1 gtk3 fuse-libs
  elif is_zypper; then
    sudo zypper --non-interactive install webkit2gtk-4_1 gtk3 libfuse2
  elif is_pacman; then
    sudo pacman -S --needed --noconfirm webkit2gtk-4.1 gtk3 fuse2
  else
    echo "Could not detect a package manager. Install WebKitGTK 4.1 and GTK 3, then re-run." >&2
    exit 1
  fi
}

install_deb() {
  local deb="$1"
  if ! is_apt; then
    echo "a .deb was provided but apt is not available" >&2
    exit 1
  fi
  sudo apt-get update
  sudo apt-get install -y "$deb"
  echo "Installed $(dpkg-deb -f "$deb" Package) $(dpkg-deb -f "$deb" Version)"
  echo "Launch it from the app menu as \"NomNom Nanny\", or run: nomnom-nanny"
}

install_rpm() {
  local rpm="$1"
  if is_dnf; then
    sudo dnf install -y "$rpm"
  elif is_zypper; then
    sudo zypper --non-interactive install "$rpm"
  elif have rpm; then
    sudo rpm -Uvh "$rpm"
  else
    echo "an .rpm was provided but no rpm installer is available" >&2
    exit 1
  fi
  echo "Launch it from the app menu as \"NomNom Nanny\", or run: nomnom-nanny"
}

install_appimage() {
  local src="$1"
  local bin_dir="$PREFIX/bin"
  local apps_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  mkdir -p "$bin_dir" "$apps_dir"
  local dest="$bin_dir/NomNomNanny.AppImage"
  cp "$src" "$dest"
  chmod +x "$dest"
  local repair
  repair=$(cd "$(dirname "$0")" && pwd)/repair-linux-appimage.sh
  if [[ ! -x "$repair" ]]; then
    echo "missing $repair; the AppImage would shadow host libwayland-client" >&2
    exit 1
  fi
  "$repair" --in-place "$dest"

  local mode="direct"
  local probe
  probe=$(mktemp -d)
  set +e
  timeout 8 "$dest" --appimage-mount >"$probe/out" 2>"$probe/err"
  local rc=$?
  set -e
  if [[ "$rc" -ne 124 ]]; then
    mode="extract"
    echo "FUSE could not mount the AppImage (exit $rc). Using extract-and-run."
    if [[ -s "$probe/err" ]]; then
      sed 's/^/  /' "$probe/err"
    fi
  else
    echo "AppImage FUSE mount works"
  fi
  rm -rf "$probe"

  local exec_line
  if [[ "$mode" == "direct" ]]; then
    exec_line="$dest"
  else
    exec_line="$dest --appimage-extract-and-run"
  fi

  cat >"$apps_dir/com.nomnomnanny.app.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=NomNom Nanny
Comment=Daily calorie, nutrient and hydration tracker
Exec=$exec_line
Icon=nomnom-nanny
Terminal=false
Categories=Utility;
StartupWMClass=nomnom-nanny
EOF
  if have update-desktop-database; then
    update-desktop-database "$apps_dir" >/dev/null 2>&1 || true
  fi
  echo "Installed $dest"
  echo "Launch: $exec_line"
}

kind_of() {
  local path="$1"
  case "$path" in
    *.deb) echo deb ;;
    *.rpm) echo rpm ;;
    *.AppImage) echo appimage ;;
    *)
      echo "unrecognized package (want .deb, .rpm, or .AppImage): $path" >&2
      exit 1
      ;;
  esac
}

if [[ -n "$DOWNLOAD_ONLY" ]]; then
  download_asset "$(preferred_kind)" "$DOWNLOAD_ONLY"
  exit
fi

if [[ -z "$package" ]]; then
  kind=$(preferred_kind)
  package=$(find_local_bundle "$kind" || true)
  if [[ -z "$package" && "$kind" != "appimage" ]]; then
    package=$(find_local_bundle appimage || true)
    kind=appimage
  fi
  if [[ -z "$package" ]]; then
    echo "No local bundle found; downloading the latest public GitHub release"
    package=$(download_asset "$kind" "${TMPDIR:-/tmp}/nomnom-nanny-install")
  fi
fi

if [[ ! -f "$package" ]]; then
  echo "package not found: $package" >&2
  exit 1
fi

kind=$(kind_of "$package")
install_runtime_deps
case "$kind" in
  deb) install_deb "$(readlink -f "$package")" ;;
  rpm) install_rpm "$(readlink -f "$package")" ;;
  appimage) install_appimage "$(readlink -f "$package")" ;;
esac
