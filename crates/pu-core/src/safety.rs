use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// Directories whose entire subtree must never be touched.
const PROTECTED_PREFIXES: &[&str] = &[
    "/System",
    "/bin",
    "/sbin",
    "/usr",
    "/etc",
    "/private/etc",
    "/dev",
    "/proc",
    "/sys",
    "/boot",
    "/lib",
    "/lib64",
    "/root",
    "/private/var/db",
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
    "C:\\ProgramData\\Microsoft",
    "C:\\$Recycle.Bin",
];

/// Explicit exceptions that live inside a protected prefix.
const PROTECTED_EXCEPTIONS: &[&str] = &["/private/var/db/receipts"];

/// Paths that are never deletable, exactly as written (the containers
/// themselves — contents below them may be targeted individually).
const NEVER_DELETE: &[&str] = &[
    "/",
    "/Applications",
    "/Library",
    "/System",
    "/Users",
    "/home",
    "/private",
    "/private/var",
    "/private/var/db",
    "/var",
    "/var/db",
    "/opt",
    "/tmp",
    "/etc",
    "/usr",
    "/bin",
    "/sbin",
    "/Volumes",
    "/Library/Application Support",
    "/Library/Caches",
    "/Library/Preferences",
    "/Library/LaunchAgents",
    "/Library/LaunchDaemons",
    "C:\\",
    "C:\\Users",
    "C:\\ProgramData",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
];

/// Prefix + max component count: the user's home directory itself (and nothing
/// deeper) can never be deleted, e.g. `/Users/alabbas` or `C:\Users\alabbas`.
const HOME_PREFIXES: &[(&str, usize)] = &[("/Users", 3), ("/home", 3), ("C:\\Users", 3)];

/// Directories inside the user's home that are containers, never targets.
const HOME_CONTAINER_NAMES: &[&str] = &[
    "Library",
    "Documents",
    "Desktop",
    "Downloads",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Applications",
    ".config",
    ".local",
    ".cache",
    ".ssh",
    ".gnupg",
    ".zshrc",
    ".bashrc",
];

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Lexically normalizes a path: collapses `.` and resolves `..` without
/// touching the filesystem, so `..` tricks cannot sneak past us.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn under(child: &Path, root: &Path) -> bool {
    let mut it = child.components();
    for expected in root.components() {
        match it.next() {
            Some(c) if c == expected => {}
            _ => return false,
        }
    }
    true
}

/// Returns `Ok(())` only when deleting `path` cannot brick the system.
pub fn ensure_safe(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(Error::UnsafePath(path.to_path_buf()));
    }

    let path = normalize(path);
    let display = path.to_string_lossy().to_string();
    let count = path.components().count();

    if display.contains('\0') {
        return Err(Error::UnsafePath(path));
    }

    // Never a filesystem root.
    if count < 2 {
        return Err(Error::UnsafePath(path));
    }

    for never in NEVER_DELETE {
        if display.trim_end_matches('/') == *never {
            return Err(Error::UnsafePath(path));
        }
    }

    for exception in PROTECTED_EXCEPTIONS {
        if under(&path, Path::new(exception)) {
            return Ok(());
        }
    }

    for prefix in PROTECTED_PREFIXES {
        if under(&path, Path::new(prefix)) {
            return Err(Error::UnsafePath(path));
        }
    }

    for (prefix, max_components) in HOME_PREFIXES {
        if under(&path, Path::new(prefix)) && count <= *max_components {
            return Err(Error::UnsafePath(path));
        }
    }

    if let Some(home) = home_dir() {
        if path == home {
            return Err(Error::UnsafePath(path));
        }
        for name in HOME_CONTAINER_NAMES {
            if path == home.join(name) {
                return Err(Error::UnsafePath(path));
            }
        }
    }

    Ok(())
}

/// Every candidate removed during a run must live inside one of the roots the
/// engine is allowed to operate in: the platform's known trace roots plus the
/// app's own install paths.
pub fn ensure_within_roots(path: &Path, roots: &[PathBuf]) -> Result<()> {
    let normalized = normalize(path);
    let allowed = roots.iter().any(|root| under(&normalized, &normalize(root)));
    if allowed {
        Ok(())
    } else {
        Err(Error::UnsafePath(normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_paths() {
        assert!(ensure_safe(Path::new("relative/thing")).is_err());
    }

    #[test]
    fn rejects_filesystem_roots_and_containers() {
        assert!(ensure_safe(Path::new("/")).is_err());
        assert!(ensure_safe(Path::new("/Applications")).is_err());
        assert!(ensure_safe(Path::new("/Users")).is_err());
        assert!(ensure_safe(Path::new("/Library")).is_err());
        assert!(ensure_safe(Path::new("/Library/Caches")).is_err());
        assert!(ensure_safe(Path::new("/Volumes")).is_err());
    }

    #[test]
    fn rejects_protected_subtrees() {
        assert!(ensure_safe(Path::new("/System/Library/CoreServices")).is_err());
        assert!(ensure_safe(Path::new("/usr/local/bin")).is_err());
        assert!(ensure_safe(Path::new("/etc/hosts")).is_err());
        assert!(ensure_safe(Path::new("C:\\Windows\\System32")).is_err());
        assert!(ensure_safe(Path::new("C:\\Program Files\\Foo")).is_err());
    }

    #[test]
    fn rejects_home_dir_itself() {
        let home = home_dir().expect("HOME set in test environment");
        assert!(ensure_safe(&home).is_err());
        assert!(ensure_safe(&home.join("Library")).is_err());
        assert!(ensure_safe(&home.join(".config")).is_err());
    }

    #[test]
    fn allows_receipts_inside_protected_db() {
        assert!(ensure_safe(Path::new("/private/var/db/receipts/com.foo.bom")).is_ok());
        assert!(ensure_safe(Path::new("/var/db/receipts/com.foo.plist")).is_ok());
    }

    #[test]
    fn allows_normal_trace_paths() {
        assert!(ensure_safe(Path::new("/Applications/Cool.app")).is_ok());
        let home = home_dir().unwrap();
        assert!(ensure_safe(&home.join("Library/Preferences/com.foo.plist")).is_ok());
        assert!(ensure_safe(&home.join(".config/Foo")).is_ok());
        assert!(ensure_safe(Path::new("/Library/LaunchDaemons/com.foo.plist")).is_ok());
    }

    #[test]
    fn rejects_dotdot_escapes() {
        assert!(ensure_safe(Path::new("/Applications/../../etc/passwd")).is_err());
    }

    #[test]
    fn root_enforcement() {
        let roots = vec![PathBuf::from("/Applications")];
        assert!(ensure_within_roots(Path::new("/Applications/A.app"), &roots).is_ok());
        assert!(ensure_within_roots(Path::new("/etc/passwd"), &roots).is_err());
    }
}
