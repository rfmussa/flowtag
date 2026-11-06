use anyhow::{bail, Error};

/// Max length for a tag or group name
pub const MAX_NAME_LEN: usize = 64;

/// Separator used when joining tag values into a single file field (ID3 / MP4).
/// The scanner MUST split with this same separator (see scan.rs / flush.rs).
pub const FILE_TAG_SEPARATOR: &str = "; ";

/// Characters forbidden inside tag names (they collide with the file-field separator
/// and with hashtag export)
const FORBIDDEN_CHARS: [char; 2] = [';', '/'];

/// Validate and normalize a tag name: trimmed, non-empty, bounded, no separator chars,
/// no leading '#'. Returns the normalized (trimmed) name.
pub fn validate_tag_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Tag name cannot be empty");
    }
    if name.chars().count() > MAX_NAME_LEN {
        bail!("Tag name too long (max {MAX_NAME_LEN} characters)");
    }
    if name.starts_with('#') {
        bail!("Tag name cannot start with '#'");
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN_CHARS.contains(c)) {
        bail!("Tag name cannot contain '{c}'");
    }
    Ok(name.to_string())
}

/// Validate a hex color like `#1a2b3c`
pub fn validate_color(color: &str) -> Result<(), Error> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        bail!("Color must be in #rrggbb format, got: {color}");
    }
    Ok(())
}

/// Validate a library root folder path: must exist, be a directory, and not be
/// nested inside (or contain) any existing root. Paths are canonicalized (symlinks
/// and `..` resolved) before comparison. Returns the canonical path to store.
pub fn validate_folder(path: &str, existing: &[String]) -> Result<String, Error> {
    let p = dunce::canonicalize(path)
        .map_err(|_| anyhow::anyhow!("Folder does not exist: {path}"))?;
    if !p.is_dir() {
        bail!("Not a directory: {path}");
    }
    for root in existing {
        // An existing root that no longer resolves can't conflict
        let r = match dunce::canonicalize(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if p.starts_with(&r) {
            bail!("Folder is already covered by library root: {root}");
        }
        if r.starts_with(&p) {
            bail!("Folder contains existing library root: {root}");
        }
    }
    Ok(p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_name_trimmed_and_accepted() {
        assert_eq!(validate_tag_name("  Peak Time  ").unwrap(), "Peak Time");
    }

    #[test]
    fn tag_name_rejections() {
        assert!(validate_tag_name("").is_err());
        assert!(validate_tag_name("   ").is_err());
        assert!(validate_tag_name("#dark").is_err());
        assert!(validate_tag_name("a;b").is_err());
        assert!(validate_tag_name("a/b").is_err());
        assert!(validate_tag_name(&"x".repeat(65)).is_err());
        assert!(validate_tag_name(&"x".repeat(64)).is_ok());
    }

    #[test]
    fn color_validation() {
        assert!(validate_color("#00d2bf").is_ok());
        assert!(validate_color("#ABCDEF").is_ok());
        assert!(validate_color("00d2bf").is_err());
        assert!(validate_color("#00d2b").is_err());
        assert!(validate_color("#00d2bg").is_err());
        assert!(validate_color("#00d2bf0").is_err());
    }

    #[test]
    fn tag_name_length_counts_characters_not_bytes() {
        // 40 Cyrillic chars = 80 UTF-8 bytes; must be accepted
        assert!(validate_tag_name(&"д".repeat(40)).is_ok());
        assert!(validate_tag_name(&"д".repeat(65)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn folder_validation_canonicalizes_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let real_s = real.to_string_lossy().to_string();
        let link_s = link.to_string_lossy().to_string();
        // The symlink points at an existing root: must be rejected as duplicate coverage
        assert!(validate_folder(&link_s, &[real_s]).is_err());
    }

    #[test]
    fn folder_validation() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let sub = sub.to_string_lossy().to_string();

        // Valid on empty library
        assert!(validate_folder(&root, &[]).is_ok());
        // Missing folder rejected
        assert!(validate_folder("/nonexistent/path/xyz", &[]).is_err());
        // Nested inside existing root rejected
        assert!(validate_folder(&sub, &[root.clone()]).is_err());
        // Containing an existing root rejected
        assert!(validate_folder(&root, &[sub]).is_err());
    }
}
