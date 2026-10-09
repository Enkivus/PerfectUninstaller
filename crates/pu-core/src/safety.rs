use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};
use crate::util;

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

/// Windows app directories may be removed as a single, explicitly discovered
/// install target. The containing Program Files directory remains protected.
const APPROVED_INSTALL_PREFIXES: &[&str] = &["C:\\Program Files", "C:\\Program Files (x86)"];

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
            Some(c) if component_eq(c, expected) => {}
            _ => return false,
        }
    }
    true
}

pub fn same_path(left: &Path, right: &Path) -> bool {
    let normalized_left = normalize(left);
    let normalized_right = normalize(right);
    let mut left = normalized_left.components();
    let mut right = normalized_right.components();
    loop {
        match (left.next(), right.next()) {
            (Some(a), Some(b)) if component_eq(a, b) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

pub fn is_within(path: &Path, root: &Path) -> bool {
    under(&normalize(path), &normalize(root))
}

#[cfg(windows)]
fn component_eq(left: Component<'_>, right: Component<'_>) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn component_eq(left: Component<'_>, right: Component<'_>) -> bool {
    left == right
}

fn protected_prefixes() -> Vec<PathBuf> {
    let prefixes = PROTECTED_PREFIXES
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    #[cfg(windows)]
    {
        let mut prefixes = prefixes;
        for variable in ["SystemRoot", "WINDIR"] {
            if let Some(path) = std::env::var_os(variable).filter(|value| !value.is_empty()) {
                prefixes.push(PathBuf::from(path));
            }
        }
        if let Some(program_data) = std::env::var_os("ProgramData") {
            prefixes.push(PathBuf::from(program_data).join("Microsoft"));
        }
        prefixes
    }
    #[cfg(not(windows))]
    prefixes
}

fn approved_install_prefixes() -> Vec<PathBuf> {
    let prefixes = APPROVED_INSTALL_PREFIXES
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    #[cfg(windows)]
    {
        let mut prefixes = prefixes;
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(path) = std::env::var_os(variable).filter(|value| !value.is_empty()) {
                prefixes.push(PathBuf::from(path));
            }
        }
        prefixes
    }
    #[cfg(not(windows))]
    prefixes
}

fn never_delete_paths() -> Vec<PathBuf> {
    let paths = NEVER_DELETE.iter().map(PathBuf::from).collect::<Vec<_>>();
    #[cfg(windows)]
    {
        let mut paths = paths;
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
            if let Some(path) = std::env::var_os(variable).filter(|value| !value.is_empty()) {
                paths.push(PathBuf::from(path));
            }
        }
        if let Some(home) = util::home_dir() {
            if let Some(users_dir) = home.parent().filter(|parent| {
                parent
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Users"))
            }) {
                paths.push(users_dir.to_path_buf());
            }
        }
        paths
    }
    #[cfg(not(windows))]
    paths
}

/// Returns `Ok(())` only when deleting `path` cannot brick the system.
pub fn ensure_safe(path: &Path) -> Result<()> {
    ensure_safe_with_install_exceptions(path, &[])
}

/// Applies the normal safety rules, with a narrow exception for deleting a
/// complete app directory directly discovered under Program Files.
pub fn ensure_safe_install_target(path: &Path, install_paths: &[PathBuf]) -> Result<()> {
    ensure_safe_with_install_exceptions(path, install_paths)
}

fn ensure_safe_with_install_exceptions(path: &Path, install_paths: &[PathBuf]) -> Result<()> {
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
    if count < 2 || is_windows_drive_root(&path) {
        return Err(Error::UnsafePath(path));
    }

    for never in never_delete_paths() {
        if same_path(&never, &path) {
            return Err(Error::UnsafePath(path));
        }
    }

    for exception in PROTECTED_EXCEPTIONS {
        if under(&path, Path::new(exception)) {
            return Ok(());
        }
    }

    for prefix in protected_prefixes() {
        if under(&path, &prefix) {
            let is_approved_install = approved_install_prefixes()
                .iter()
                .any(|install_prefix| under(&path, install_prefix))
                && install_paths
                    .iter()
                    .any(|install_path| same_path(install_path, &path));
            if is_approved_install {
                continue;
            }
            return Err(Error::UnsafePath(path));
        }
    }

    for (prefix, max_components) in HOME_PREFIXES {
        if under(&path, Path::new(prefix)) && count <= *max_components {
            return Err(Error::UnsafePath(path));
        }
    }

    if let Some(home) = util::home_dir() {
        if same_path(&path, &home) {
            return Err(Error::UnsafePath(path));
        }
        for name in HOME_CONTAINER_NAMES {
            if same_path(&path, &home.join(name)) {
                return Err(Error::UnsafePath(path));
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
fn is_windows_drive_root(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Prefix(_)))
        && matches!(components.next(), Some(Component::RootDir))
        && components.next().is_none()
}

#[cfg(not(windows))]
fn is_windows_drive_root(_path: &Path) -> bool {
    false
}

/// Every candidate removed during a run must live inside one of the roots the
/// engine is allowed to operate in: the platform's known trace roots plus the
/// app's own install paths.
pub fn ensure_within_roots(path: &Path, roots: &[PathBuf]) -> Result<()> {
    let normalized = normalize(path);
    let allowed = roots.iter().any(|root| is_within(&normalized, root));
    if allowed {
        Ok(())
    } else {
        Err(Error::UnsafePath(normalized))
    }
}

/// Rejects symlinks in the selected path's parent chain. The selected leaf
/// itself may be a symlink because removal unlinks the symlink rather than
/// following it. Callers should use canonical platform paths for known OS
/// aliases (for example, `/private/var` instead of macOS `/var`).
pub fn ensure_no_symlink_escape(path: &Path, roots: &[PathBuf]) -> Result<()> {
    let path = normalize(path);
    let allowed = roots
        .iter()
        .map(|root| normalize(root))
        .any(|root| under(&path, &root));
    if !allowed {
        return Err(Error::UnsafePath(path));
    }

    let parent = path
        .parent()
        .ok_or_else(|| Error::UnsafePath(path.clone()))?;
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        if is_symlink(&current)? {
            return Err(Error::UnsafePath(path));
        }
    }
    Ok(())
}

fn is_symlink(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(is_link_or_reparse_point(&metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Error::io(path, error)),
    }
}

#[cfg(windows)]
fn is_link_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
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
        let home = util::home_dir().expect("home directory set in test environment");
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
        let home = util::home_dir().unwrap();
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

    #[cfg(windows)]
    #[test]
    fn rejects_windows_drive_roots() {
        assert!(ensure_safe(Path::new("C:\\")).is_err());
        assert!(ensure_safe(Path::new("D:\\")).is_err());
        assert!(ensure_safe(Path::new("c:\\WINDOWS\\System32")).is_err());
        assert!(same_path(
            Path::new("C:\\Program Files\\Example"),
            Path::new("c:\\program files\\example")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn allows_only_explicit_program_files_install_targets() {
        let install = PathBuf::from("C:\\Program Files\\Example App");
        assert!(ensure_safe(&install).is_err());
        assert!(ensure_safe_install_target(&install, std::slice::from_ref(&install)).is_ok());
        assert!(ensure_safe_install_target(
            Path::new("C:\\Program Files\\Other App"),
            std::slice::from_ref(&install)
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_parent_escape_but_allows_a_symlink_leaf() {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join(format!("pu-symlink-{}", std::process::id()));
        let root = base.join("root");
        let outside = base.join("outside");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let outside = std::fs::canonicalize(outside).unwrap();
        symlink(&outside, root.join("redirect")).unwrap();
        symlink(&outside, root.join("leaf-link")).unwrap();

        assert!(
            ensure_no_symlink_escape(&root.join("redirect/file"), std::slice::from_ref(&root))
                .is_err()
        );
        assert!(
            ensure_no_symlink_escape(&root.join("leaf-link"), std::slice::from_ref(&root)).is_ok()
        );
        let _ = std::fs::remove_dir_all(base);
    }
}
