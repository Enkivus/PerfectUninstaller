use std::path::{Path, PathBuf};

use crate::models::{Confidence, TraceCandidate, TraceCategory};
use crate::safety::normalize;
use crate::util::estimate_size;

/// One directory the engine searches for residual data.
#[derive(Debug, Clone)]
pub struct SearchRoot {
    pub path: PathBuf,
    pub category: TraceCategory,
    /// Extra levels to descend below the root's direct children.
    /// `0` = only direct children, `1` = children and grandchildren.
    pub depth: usize,
    /// Human readable location shown in the "reason" field.
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Query<'a> {
    pub identifier: Option<&'a str>,
    pub name: Option<&'a str>,
}

/// Scans every root for paths belonging to the queried app.
///
/// `skip` paths (the app's primary install locations) are never re-reported.
pub fn find_traces(roots: &[SearchRoot], query: &Query, skip: &[PathBuf]) -> Vec<TraceCandidate> {
    let skip: Vec<PathBuf> = skip.iter().map(|p| normalize(p)).collect();
    let mut found = Vec::new();

    for root in roots {
        if !root.path.is_dir() {
            continue;
        }
        scan_dir(&root.path, root, root.depth, query, &skip, &mut found);
    }

    dedupe_nested(found)
}

fn scan_dir(
    dir: &Path,
    root: &SearchRoot,
    depth_left: usize,
    query: &Query,
    skip: &[PathBuf],
    out: &mut Vec<TraceCandidate>,
) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in read.flatten() {
        let path = entry.path();
        let normalized = normalize(&path);

        if skip.iter().any(|s| normalized.starts_with(s)) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        if let Some((confidence, why)) = match_entry(&name, query) {
            let label = root.label;
            out.push(TraceCandidate {
                bytes: estimate_size(&path),
                path,
                category: root.category,
                confidence,
                reason: format!("{label}: {why}"),
                primary: false,
            });
            continue;
        }

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir && depth_left > 0 {
            scan_dir(&path, root, depth_left - 1, query, skip, out);
        }
    }
}

fn match_entry(name: &str, query: &Query) -> Option<(Confidence, String)> {
    let lower = name.to_lowercase();
    let stem = strip_ext(&lower);
    let name_compact = query
        .name
        .map(|n| compact(&n.to_lowercase()))
        .filter(|n| n.len() >= 3);

    if let Some(id) = query.identifier.map(|s| s.to_lowercase()) {
        if !id.is_empty() {
            if lower == id {
                return Some((Confidence::High, "bundle identifier match".into()));
            }
            if starts_with_token(&lower, &id) {
                return Some((Confidence::High, "bundle identifier prefix".into()));
            }
            if stem == id {
                return Some((Confidence::High, "bundle identifier match".into()));
            }
            if starts_with_token(stem, &id) {
                return Some((Confidence::High, "bundle identifier prefix".into()));
            }

            // Helper components of the same vendor: `com.vendor.app` should
            // also catch `com.vendor.app-helper` and `org.vendor.AppThumbnailer`.
            if let Some((vendor, _)) = id.rsplit_once('.') {
                if vendor.contains('.')
                    && (starts_with_token(&lower, vendor) || starts_with_token(stem, vendor))
                {
                    let file_compact = compact(&lower);
                    if let Some(nc) = &name_compact {
                        if file_compact.contains(nc.as_str()) {
                            return Some((Confidence::Medium, "vendor and app name match".into()));
                        }
                    }
                    return Some((Confidence::Low, "same vendor as bundle identifier".into()));
                }
            }
        }
    }

    if let Some(name_query) = query.name {
        let n = name_query.to_lowercase();
        if n.len() >= 3 {
            if stem == n {
                return Some((Confidence::Medium, "app name match".into()));
            }
            if compact(stem) == compact(&n) {
                return Some((Confidence::Medium, "app name match (normalized)".into()));
            }
            if stem.contains(&n) || lower.contains(&n) {
                return Some((Confidence::Low, "partial app name match".into()));
            }
        }
    }

    None
}

/// `com.vendor.app` matches `com.vendor.app.plist` / `com.vendor.app-2`,
/// but `com.vendor.application` does not match `com.vendor.app`.
fn starts_with_token(value: &str, prefix: &str) -> bool {
    match value.strip_prefix(prefix) {
        Some(rest) => !rest.is_empty()
            && rest
                .chars()
                .next()
                .is_some_and(|c| matches!(c, '.' | '-' | '_' | ' ')),
        None => false,
    }
}

fn strip_ext(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((head, ext)) if !ext.is_empty() && ext.len() <= 8 && !head.is_empty() => head,
        _ => name,
    }
}

fn compact(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, ' ' | '-' | '_' | '.')).collect()
}

/// Drops candidates nested inside another candidate (the parent already
/// covers them) and sorts for stable display.
fn dedupe_nested(mut found: Vec<TraceCandidate>) -> Vec<TraceCandidate> {
    found.sort_by(|a, b| a.path.cmp(&b.path));
    let mut kept: Vec<TraceCandidate> = Vec::new();
    for candidate in found {
        if kept.iter().any(|k| candidate.path.starts_with(&k.path)) {
            continue;
        }
        kept.push(candidate);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("pu-test-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempTree(dir)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn roots(dir: &Path) -> Vec<SearchRoot> {
        vec![
            SearchRoot {
                path: dir.to_path_buf(),
                category: TraceCategory::Support,
                depth: 0,
                label: "test",
            },
            SearchRoot {
                path: dir.join("deep"),
                category: TraceCategory::Cache,
                depth: 1,
                label: "deep",
            },
        ]
    }

    #[test]
    fn matches_identifier_exactly_and_prefixed() {
        let q = Query { identifier: Some("com.vendor.cool"), name: Some("Cool App") };
        assert_eq!(match_entry("com.vendor.cool", &q).unwrap().0, Confidence::High);
        assert_eq!(match_entry("com.vendor.cool.plist", &q).unwrap().0, Confidence::High);
        assert_eq!(match_entry("com.vendor.cool-2", &q).unwrap().0, Confidence::High);
        assert_eq!(match_entry("com.vendor.coolant", &q).unwrap().0, Confidence::Low);
        assert_eq!(match_entry("Cool App", &q).unwrap().0, Confidence::Medium);
        assert_eq!(match_entry("coolapp", &q).unwrap().0, Confidence::Medium);
        assert_eq!(match_entry("Cool App leftovers", &q).unwrap().0, Confidence::Low);
        assert!(match_entry("totally-unrelated", &q).is_none());
    }

    #[test]
    fn same_vendor_helpers_rank_by_name_overlap() {
        let q = Query { identifier: Some("org.aseprite.Aseprite"), name: Some("Aseprite") };
        assert_eq!(
            match_entry("org.aseprite.AsepriteThumbnailer", &q).unwrap().0,
            Confidence::Medium,
            "same vendor and app name"
        );

        let chrome = Query { identifier: Some("com.google.Chrome"), name: Some("Chrome") };
        assert_eq!(
            match_entry("com.google.keystone", &chrome).unwrap().0,
            Confidence::Low,
            "same vendor, unrelated product"
        );
    }

    #[test]
    fn weak_names_never_match() {
        let q = Query { identifier: None, name: Some("Go") };
        assert!(match_entry("Go", &q).is_none());
    }

    #[test]
    fn finds_and_dedupes() {
        let tree = TempTree::new("find");
        file(&tree.0.join("com.vendor.cool.plist"), "x");
        file(&tree.0.join("Cool App"), "x");
        file(&tree.0.join("deep/com.vendor.cool/cache.db"), "xxxx");
        file(&tree.0.join("deep/nested/com.vendor.cool/inner"), "xx");
        file(&tree.0.join("unrelated/file"), "x");

        let q = Query { identifier: Some("com.vendor.cool"), name: Some("Cool App") };
        let found = find_traces(&roots(&tree.0), &q, &[]);

        let paths: Vec<String> = found
            .iter()
            .map(|c| c.path.strip_prefix(&tree.0).unwrap().to_string_lossy().to_string())
            .collect();

        assert!(paths.contains(&"com.vendor.cool.plist".to_string()));
        assert!(paths.contains(&"Cool App".to_string()));
        assert!(paths.contains(&"deep/com.vendor.cool".to_string()));
        assert!(paths.contains(&"deep/nested/com.vendor.cool".to_string()));
        assert!(!paths.iter().any(|p| p.contains("unrelated")));
        assert!(!paths.iter().any(|p| p.contains("cache.db")), "child of matched dir excluded");
    }

    #[test]
    fn skips_install_paths() {
        let tree = TempTree::new("skip");
        file(&tree.0.join("Cool.app/Contents/MacOS/cool"), "x");
        file(&tree.0.join("com.vendor.cool.plist"), "x");

        let q = Query { identifier: Some("com.vendor.cool"), name: Some("Cool App") };
        let found = find_traces(&roots(&tree.0), &q, &[tree.0.join("Cool.app")]);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("com.vendor.cool.plist"));
    }

    #[test]
    fn nests_are_collapsed() {
        let parent = TraceCandidate {
            path: PathBuf::from("/base/Cool Parent"),
            category: TraceCategory::Support,
            confidence: Confidence::Medium,
            bytes: 10,
            reason: "test".into(),
            primary: false,
        };
        let child = TraceCandidate {
            path: PathBuf::from("/base/Cool Parent/child/com.vendor.cool"),
            category: TraceCategory::Cache,
            confidence: Confidence::High,
            bytes: 5,
            reason: "test".into(),
            primary: false,
        };
        let sibling = TraceCandidate {
            path: PathBuf::from("/base/Cool Parental"),
            category: TraceCategory::Cache,
            confidence: Confidence::High,
            bytes: 7,
            reason: "test".into(),
            primary: false,
        };

        let kept = dedupe_nested(vec![child, sibling, parent]);
        let paths: Vec<_> = kept.iter().map(|c| c.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/base/Cool Parent"),
                PathBuf::from("/base/Cool Parental")
            ]
        );
    }
}
