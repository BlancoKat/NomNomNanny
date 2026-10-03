//! Keep host Mesa and the AppImage's bundled WebKit on the same Wayland client.
//!
//! linuxdeploy copies Ubuntu's `libwayland-client.so.0` into the AppImage and
//! `AppRun` puts that directory on `LD_LIBRARY_PATH`. Mesa 25+ (Arch, Omarchy,
//! current Fedora) was built against a newer client and references
//! `wl_display_create_queue_with_name`. The bundled client does not export it,
//! so loading `libEGL_mesa` fails, `eglGetDisplay` returns `EGL_BAD_PARAMETER`,
//! and WebKit aborts the web process after logging
//! "Could not create default EGL display".
//!
//! Packaging removes that bundled client. This relaunch covers an AppImage that
//! still contains it: the host client matches the host Mesa.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const SONAME: &str = "libwayland-client.so.0";
const GUARD: &str = "NOMON_HOST_WAYLAND";

pub fn prefer_host_wayland_client() {
    if std::env::var_os("APPIMAGE").is_none() && std::env::var_os("APPDIR").is_none() {
        return;
    }
    if std::env::var_os(GUARD).is_some() || std::env::var_os("NOMON_SKIP_WAYLAND_PRELOAD").is_some()
    {
        return;
    }
    let Some(bundled) = bundled_client() else {
        return;
    };
    let appdir = std::env::var_os("APPDIR");
    match host_client(appdir.as_deref(), &bundled) {
        Some(host) => reexec(&host),
        None => {
            eprintln!(
                "nomnom-nanny: {} is ahead of the system libwayland-client on LD_LIBRARY_PATH.\n\
                 Host Mesa cannot create an EGL display with that library, and WebKit then aborts\n\
                 the web process (\"Could not create default EGL display\"). Install the system\n\
                 Wayland client library and relaunch. When stderr is not a terminal, startup notes\n\
                 are appended to $XDG_STATE_HOME/nomnom-nanny/appimage.log.",
                bundled.display()
            );
        }
    }
}

fn bundled_client() -> Option<PathBuf> {
    let paths = std::env::var_os("LD_LIBRARY_PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        let candidate = dir.join(SONAME);
        candidate.is_file().then_some(candidate)
    })
}

fn host_client(appdir: Option<&OsStr>, bundled: &Path) -> Option<PathBuf> {
    let appdir = appdir.map(Path::new);
    let mut candidates = Vec::new();
    if let Some(text) = ldconfig_cache() {
        candidates.extend(parse_ldconfig(&text));
    }
    candidates.extend(fallback_paths());
    candidates
        .into_iter()
        .find(|path| path.is_file() && !same_library(path, bundled) && !under_appdir(path, appdir))
}

fn ldconfig_cache() -> Option<String> {
    let output = Command::new("ldconfig").arg("-p").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn fallback_paths() -> Vec<PathBuf> {
    [
        "/usr/lib/libwayland-client.so.0",
        "/usr/lib64/libwayland-client.so.0",
        "/lib64/libwayland-client.so.0",
        "/usr/lib/x86_64-linux-gnu/libwayland-client.so.0",
        "/lib/x86_64-linux-gnu/libwayland-client.so.0",
        "/usr/lib/aarch64-linux-gnu/libwayland-client.so.0",
        "/lib/aarch64-linux-gnu/libwayland-client.so.0",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn arch_token() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86-64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        ""
    }
}

fn parse_ldconfig(text: &str) -> Vec<PathBuf> {
    let arch = arch_token().to_ascii_lowercase();
    let mut matched = Vec::new();
    let mut unqualified = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        let Some(rest) = line.strip_prefix(SONAME) else {
            continue;
        };
        let Some((_, path)) = rest.split_once("=>") else {
            continue;
        };
        let path = PathBuf::from(path.trim());
        if path.as_os_str().is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if arch.is_empty() || lower.contains(&arch) {
            matched.push(path);
        } else if !line.contains('(') {
            unqualified.push(path);
        }
    }
    if matched.is_empty() {
        unqualified
    } else {
        matched
    }
}

fn same_library(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn under_appdir(path: &Path, appdir: Option<&Path>) -> bool {
    let Some(appdir) = appdir else {
        return false;
    };
    if path.starts_with(appdir) {
        return true;
    }
    match (path.canonicalize(), appdir.canonicalize()) {
        (Ok(path), Ok(appdir)) => path.starts_with(appdir),
        _ => false,
    }
}

fn reexec(host: &Path) {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("nomnom-nanny: not restarting with the host libwayland-client");
        return;
    };
    let host_text = host.to_string_lossy();
    let mut preload = std::env::var("LD_PRELOAD").unwrap_or_default();
    if preload
        .split([' ', ':'])
        .any(|entry| entry == host_text.as_ref())
    {
        return;
    }
    if !preload.is_empty() {
        preload.push(' ');
    }
    preload.push_str(host_text.as_ref());
    eprintln!(
        "nomnom-nanny: preloading host {host_text} so Mesa does not bind the bundled libwayland-client"
    );
    let status = Command::new(exe)
        .env("LD_PRELOAD", &preload)
        .env(GUARD, "1")
        .args(std::env::args_os().skip(1))
        .status();
    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(err) => {
            eprintln!("nomnom-nanny: failed to relaunch with the host libwayland-client ({err})");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ldconfig_keeps_this_architecture() {
        let text = "\
\tlibwayland-client.so.0 (libc6,x86-64) => /lib/x86_64-linux-gnu/libwayland-client.so.0
\tlibwayland-client.so.0 (libc6,i386) => /lib/i386-linux-gnu/libwayland-client.so.0
\tlibwayland-egl.so.1 (libc6,x86-64) => /lib/x86_64-linux-gnu/libwayland-egl.so.1
";
        let found = parse_ldconfig(text);
        if cfg!(target_arch = "x86_64") {
            assert_eq!(
                found,
                vec![PathBuf::from(
                    "/lib/x86_64-linux-gnu/libwayland-client.so.0"
                )]
            );
        } else {
            assert!(found.is_empty());
        }
    }

    #[test]
    fn parse_ldconfig_accepts_unqualified_lines() {
        let text = "libwayland-client.so.0 => /usr/lib/libwayland-client.so.0\n";
        assert_eq!(
            parse_ldconfig(text),
            vec![PathBuf::from("/usr/lib/libwayland-client.so.0")]
        );
    }

    #[test]
    fn host_client_skips_the_bundled_copy() {
        let dir = std::env::temp_dir().join("nomnom-nanny-wayland-test");
        std::fs::create_dir_all(&dir).unwrap();
        let bundled = dir.join(SONAME);
        std::fs::write(&bundled, b"not-a-library").unwrap();
        let found = host_client(Some(dir.as_os_str()), &bundled).expect("host client");
        assert_ne!(
            found.canonicalize().unwrap(),
            bundled.canonicalize().unwrap()
        );
        assert!(found.ends_with(SONAME));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
