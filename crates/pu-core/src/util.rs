use std::path::Path;

/// Total size in bytes of a file/directory tree. Does not follow symlinks
/// (a symlink contributes only its own length).
pub fn measure(path: &Path) -> u64 {
    let mut total = 0u64;
    measure_into(path, &mut total);
    total
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
