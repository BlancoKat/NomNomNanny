# NomNom Nanny

A beautiful, private app for tracking daily calories, macronutrients (protein, fat, carbs), dietary fiber, and hydration. It runs on the desktop and, as a debug build, on Android. iOS is not part of this change.

Built with **Tauri 2 + Svelte 5 + Tailwind + Rust + SQLite + USDA FoodData Central API**.

## Features

- Set personal daily goals for calories (kcal), protein/fat/carbs/fiber (g), and water (fl oz)
- Search real foods using the free USDA FoodData Central API (results are cached locally)
- Choose realistic servings and see live nutrient calculations
- Quick water logging + flexible manual/custom entry (including saved custom foods with per-100g scaling)
- Daily running totals with beautiful progress indicators
- 7-day and 30-day rolling averages
- View and edit history for past days
- 100% local data (SQLite) — works great offline after the first food lookups
- No accounts, no telemetry, no cloud sync. Copy the diary to another device by exporting a file and importing it there
- Each device keeps its own database. A phone does not see the Linux database until you import a diary file

## Getting Started

### Install a release

Public installers are attached to the [GitHub release](https://github.com/BlancoKat/NomNomNanny/releases/latest) whose tag is `v` plus the app version (for example `v0.1.1`). Download the asset for your system from that page.

Linux packages need WebKitGTK 4.1 at runtime. Install the `.deb` with apt so that library is pulled in automatically:

```bash
sudo apt update
sudo apt install ./nomnom-nanny_*_amd64.deb
```

`dpkg -i` alone does not install those libraries, and the app then fails to start. On Fedora and openSUSE, install the `.rpm` with `dnf` or `zypper`. On Arch, Omarchy, and Manjaro, use the AppImage (or build from source). Mark an AppImage executable with `chmod +x`. If it exits saying FUSE cannot mount it, start it with `--appimage-extract-and-run`.

The AppImage ships its own WebKitGTK, built on Ubuntu 22.04. It must use the host's Wayland client, EGL, and Mesa rather than copies from that build machine. A build that still carries `libwayland-client.so.0` aborts the web process on Mesa 26 (Arch and Omarchy included): the window stays open, `WebKitWebProcess` dies, and the page never paints. `WebKitNetworkProcess` is a different process and can keep running. From a terminal the abort looks like:

```text
Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
```

Launches from the app menu hide that line because stderr is not a terminal. The AppImage appends it to `~/.local/state/nomnom-nanny/appimage.log`.

`./scripts/install-linux.sh` repacks an AppImage before it installs it, including an older download such as v0.1.0. A source build (`npm run tauri dev`) uses the system WebKit and is unaffected.

### Check an AppImage on Wayland

On the machine where it failed (Wayland session, not only an Ubuntu X11 VM):

```bash
echo "session=$XDG_SESSION_TYPE wayland=${WAYLAND_DISPLAY:-unset}"
chmod +x ./nomnom-nanny_*_amd64.AppImage
./nomnom-nanny_*_amd64.AppImage
```

Leave it in the foreground for a few seconds. The shell should stay occupied and the window should show the tracker, not an empty GTK frame. In another terminal:

```bash
pgrep -a WebKitWebProcess
```

That process should still be alive. Then confirm it loaded the host Wayland client rather than the mounted AppImage copy (`/tmp/.mount_*`):

```bash
pid=$(pgrep -n WebKitWebProcess)
grep -F libwayland-client /proc/"$pid"/maps
```

The path should be under `/usr/lib` or `/lib`, not inside the AppImage mount. Quit the app and, if the window was blank, read `~/.local/state/nomnom-nanny/appimage.log`. A dev build on the same machine (`npm run tauri dev`) is the comparison: it uses system WebKitGTK and does not set `GDK_BACKEND=x11`.

From a clone of this repo, the same steps are wrapped up as:

```bash
./scripts/install-linux.sh
```

Pass a downloaded `.deb`, `.rpm`, or `.AppImage` if you already have one. The script installs WebKitGTK when it is missing, and falls back to extract-and-run when FUSE is unavailable.

Windows: run `nomnom-nanny_*_x64-setup.exe`. macOS: open the Apple Silicon `.dmg`. If Gatekeeper blocks the app, right-click it and choose Open.

### Prerequisites for a source build

- [Node.js](https://nodejs.org/) 22 or newer
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- On Linux: additional system dependencies for Tauri (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

### From Source

```bash
git clone https://github.com/blancokat/nomnomnanny.git
cd nomnomnanny
npm install
npm run tauri dev
```

The first run will take a few minutes while the Rust backend compiles.

### Android debug build

iOS is not part of this change. GitHub Actions does not build or publish Android.

The phone keeps its own SQLite file under the app's private storage. USDA search calls `https://api.nal.usda.gov` from the app, so the Android manifest allows network access. The API key stays in the on-device settings store and falls back to `DEMO_KEY` when the field is empty.

Install [Android Studio](https://developer.android.com/studio) (SDK Platform, SDK Platform-Tools, SDK Build-Tools, SDK Command-line Tools, and NDK Side by side), then point `JAVA_HOME`, `ANDROID_HOME`, and `NDK_HOME` at them. The exact setup is the [Tauri Android prerequisites](https://v2.tauri.app/start/prerequisites/). From this repo:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
npm install
npm run tauri android build -- --debug --apk --target aarch64
```

That produces a debug APK signed with the Android debug keystore Gradle creates automatically. It is not a Play Store release, and this repo does not add a release keystore. `--target aarch64` is the current phone ABI. Omit `--target` to put every ABI in one APK.

The arm64 command writes:

```text
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

The folder is named `universal` even when the APK contains only `arm64-v8a`. If the path differs, list what was built:

```bash
find src-tauri/gen/android/app/build/outputs/apk -name '*.apk'
```

Sideload it either way:

1. USB. On the phone, turn on Developer options and USB debugging. Then, with [platform-tools](https://developer.android.com/tools/releases/platform-tools) on the computer:

   ```bash
   adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
   ```

2. A file you can open on the phone. Copy the APK with a USB file transfer, or put it in Drive, email, or any folder the phone can open. Tap the APK. When Android asks, allow that app to install unknown apps. This is a debug build, so Play Protect may warn that it is not from the Play Store.

`npm run tauri android dev` runs it on a connected phone or emulator with the dev server. The desktop `npm run tauri dev` command is unchanged.

> **Note**: Make sure `cargo` is in your `PATH`. On some systems you may need to run:
> ```bash
> source "$HOME/.cargo/env" && npm run tauri dev
> ```

### Production Build

```bash
npm run tauri build
```

The resulting installers will be in `src-tauri/target/release/bundle/`. On Linux, install the freshly built package with:

```bash
./scripts/install-linux.sh
```

Pushes to `main` publish those installers to GitHub Releases under the `v<version>` tag. Download links on a draft or on a release tagged `main` do not work for a normal browser session.

### First Run

1. Launch the app.
2. Go to the **Goals** section and set your daily targets.
3. (Recommended) Get a free USDA API key at https://fdc.nal.usda.gov/api-key-signup and enter it in Settings. This enables the real food search.
4. Start logging! You can use the USDA search for real foods or Manual Entry for homemade/custom items.

## Moving the diary between devices

Goals, log entries, and custom foods can move as one JSON file named `nomnom-diary.json`. The USDA response cache is left out (search fills it again). The USDA API key is not in the file.

On either Android or the desktop app, open **Goals** and use **Export diary** or **Import diary**. Export uses the system save dialog (on Android, the document picker, so you can put the file in Downloads, Drive, or another app). Import uses the system open dialog. Before anything is replaced, the app shows what is on the device and what is in the file. Nothing is deleted until you choose **Replace diary on this device**. The two copies are not merged.

The file is version 1 JSON (`"format": "nomnom-nanny-diary"`). A later app can still read it after a database migration because import writes through the current tables instead of swapping the live SQLite file.

## Custom Foods

You can save homemade recipes or frequently eaten items as "My Foods". These are stored with nutrients per 100g so you can log any quantity and have the app calculate the values for you. You can also override nutrients on a per-entry basis.

## Tech

- **Desktop runtime**: Tauri 2 (small native installers, ~10-20 MB)
- **Android**: Tauri 2 debug APK. Not built by the desktop release workflow. iOS is not part of this change.
- **Frontend**: Svelte 5 (runes) + Tailwind
- **Backend**: Rust + SQLite
- **Food data**: USDA FoodData Central (user-provided free API key)

## License

MIT

USDA data is public domain / U.S. government data.

---

Built with care. Eat well! 🥦💧
