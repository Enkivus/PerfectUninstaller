use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Returns the current user's home directory using the platform's standard
/// environment variables.
pub fn home_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(home));
    }

    #[cfg(windows)]
    {
        if let Some(home) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(home));
        }
        let drive = std::env::var_os("HOMEDRIVE")?;
        let path = std::env::var_os("HOMEPATH")?;
        return Some(PathBuf::from(drive).join(path));
    }

    #[cfg(not(windows))]
    None
}

/// Total size in bytes of a file/directory tree. Does not follow symlinks
/// (a symlink contributes only its own length).
pub fn measure(path: &Path) -> u64 {
    let mut total = 0u64;
    measure_into(path, &mut total);
    total
}

/// Fast directory-size estimate for interactive analysis. The walk never
/// follows symlinks and stops after a small time or entry budget so a single
/// enormous app bundle cannot hold the analysis screen open.
pub fn estimate_size(path: &Path) -> u64 {
    const MAX_TIME: Duration = Duration::from_millis(45);
    let deadline = Instant::now() + MAX_TIME;
    let mut entries = 0usize;
    let mut total = 0u64;
    estimate_into(path, deadline, &mut entries, &mut total);
    total
}

fn estimate_into(path: &Path, deadline: Instant, entries: &mut usize, total: &mut u64) {
    if *entries >= 8_000 || Instant::now() >= deadline {
        return;
    }
    *entries += 1;
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return;
    };
    if md.is_dir() {
        let Ok(rd) = std::fs::read_dir(path) else {
            return;
        };
        for entry in rd.flatten() {
            if *entries >= 8_000 || Instant::now() >= deadline {
                break;
            }
            estimate_into(&entry.path(), deadline, entries, total);
        }
    } else {
        *total = total.saturating_add(md.len());
    }
}

fn measure_into(path: &Path, total: &mut u64) {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return;
    };
    if md.is_dir() {
        let Ok(rd) = std::fs::read_dir(path) else {
            return;
        };
        for entry in rd.flatten() {
            measure_into(&entry.path(), total);
        }
    } else {
        *total += md.len();
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}
