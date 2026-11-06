use std::path::Path;
use anyhow::Error;
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;
use onetagger_tag::{Tag, Field};

use crate::models::ScannedTrack;
use crate::validate::FILE_TAG_SEPARATOR;
use crate::{tracks, Db, FILE_TAG_FIELD};

/// Progress callback cadence (files)
pub const SCAN_PROGRESS_EVERY: usize = 100;

/// Audio extensions the tagger (and player) support
pub const SCAN_EXTENSIONS: [&str; 8] = ["mp3", "flac", "aif", "aiff", "m4a", "mp4", "wav", "ogg"];

/// Outcome of a library scan
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub failed: usize,
}

/// Split raw TAGS field values into clean tag names.
///
/// `get_raw` is inconsistent across formats: ID3 returns ONE unsplit string,
/// FLAC returns separate entries, MP4 returns entries split by its separator.
/// Splitting every element by the canonical separator handles all three.
pub fn split_file_tags(raw: &[String]) -> Vec<String> {
    raw.iter()
        .flat_map(|v| v.split(FILE_TAG_SEPARATOR))
        // Also split values written by other tools with plain ";"
        .flat_map(|v| v.split(';'))
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(String::from)
        .collect()
}

/// Title fallback: the file stem
fn stem_title(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// Parse one file into a ScannedTrack. Never fails on unreadable tags —
/// falls back to the filename stem so every file shows up in the library.
fn parse_file(path: &Path, mtime: i64, size: i64) -> ScannedTrack {
    let format = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut track = ScannedTrack {
        path: path.to_string_lossy().to_string(),
        format,
        title: stem_title(path),
        artist: String::new(),
        duration_ms: None,
        mtime,
        size,
        tag_names: vec![],
    };

    match Tag::load_file(path, false) {
        Ok(tag_wrap) => {
            let tag = tag_wrap.tag();
            if let Some(title) = tag.get_field(Field::Title).and_then(|v| v.into_iter().next()) {
                if !title.trim().is_empty() {
                    track.title = title;
                }
            }
            if let Some(artists) = tag.get_field(Field::Artist) {
                track.artist = artists.join(", ");
            }
            if let Some(raw) = tag.get_raw(FILE_TAG_FIELD) {
                track.tag_names = split_file_tags(&raw);
            }
        }
        Err(e) => debug!("Unreadable tags in {path:?} (using filename): {e}"),
    }

    // Duration via lofty (best effort, only for new/changed files)
    if let Ok(file) = lofty::read_from_path(path) {
        use lofty::file::AudioFile;
        track.duration_ms = Some(file.properties().duration().as_millis() as i64);
    }

    track
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            SCAN_EXTENSIONS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

use crate::file_stats;

/// Scan all library roots: walk files, parse new/changed ones in parallel
/// (everything when `full`), upsert, and mark vanished tracks missing.
/// `progress(done, total)` is called from the scanning thread every
/// SCAN_PROGRESS_EVERY parsed files.
pub fn scan(db: &Db, full: bool, progress: impl Fn(usize, usize) + Sync) -> Result<ScanResult, Error> {
    let mut result = ScanResult::default();

    for folder in tracks::list_folders(db)? {
        // An unreachable root (unplugged drive, renamed mount) must not wipe the
        // folder's library state by marking every track missing
        if !Path::new(&folder.path).is_dir() {
            warn!("Scan: library root unreachable, skipping: {}", folder.path);
            result.failed += 1;
            continue;
        }

        // 1. Walk the tree
        let mut seen: Vec<(String, i64, i64)> = vec![];
        for entry in WalkDir::new(&folder.path).follow_links(true) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Scan: cannot read dir entry under {}: {e}", folder.path);
                    result.failed += 1;
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_audio_file(entry.path()) {
                continue;
            }
            match file_stats(entry.path()) {
                Some((mtime, size)) => seen.push((entry.path().to_string_lossy().to_string(), mtime, size)),
                None => {
                    warn!("Scan: cannot stat {:?}", entry.path());
                    result.failed += 1;
                }
            }
        }

        // 2. Diff against the DB
        let known = tracks::track_stats_for_folder(db, folder.id)?;
        let to_parse: Vec<&(String, i64, i64)> = seen
            .iter()
            .filter(|(path, mtime, size)| full || known.get(path) != Some(&(*mtime, *size)))
            .collect();
        let total = to_parse.len();

        // 3. Parse changed files in parallel, with coarse progress
        let done = std::sync::atomic::AtomicUsize::new(0);
        let parsed: Vec<ScannedTrack> = to_parse
            .par_iter()
            .map(|(path, mtime, size)| {
                let track = parse_file(Path::new(path), *mtime, *size);
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n % SCAN_PROGRESS_EVERY == 0 {
                    progress(n, total);
                }
                track
            })
            .collect();

        // 4. Upsert + mark vanished files
        let (added, updated) = tracks::upsert_tracks(db, folder.id, &parsed)?;
        result.added += added;
        result.updated += updated;
        let seen_paths: Vec<String> = seen.into_iter().map(|(p, _, _)| p).collect();
        result.removed += tracks::mark_missing(db, folder.id, &seen_paths)?;
        progress(total, total);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_db;
    use crate::tracks::{add_folder, list_tracks};

    #[test]
    fn split_file_tags_handles_all_formats() {
        // ID3: one joined string
        assert_eq!(split_file_tags(&["dark; rolling".to_string()]), vec!["dark", "rolling"]);
        // FLAC: separate entries
        assert_eq!(
            split_file_tags(&["dark".to_string(), "rolling".to_string()]),
            vec!["dark", "rolling"]
        );
        // Foreign tools: plain ";" without space, stray whitespace, empties
        assert_eq!(split_file_tags(&["dark;rolling; ".to_string()]), vec!["dark", "rolling"]);
        assert!(split_file_tags(&[]).is_empty());
    }

    #[test]
    fn scan_uses_filename_fallback_and_is_incremental() {
        let (dir, db) = test_db();
        let root = dir.path().join("music");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        // Not real audio: tag parsing fails, the file must still appear (stem title)
        std::fs::write(root.join("My Track.mp3"), b"not really audio").unwrap();
        std::fs::write(root.join("sub/Other.flac"), b"nope").unwrap();
        std::fs::write(root.join("readme.txt"), b"ignored").unwrap();
        add_folder(&db, &root.to_string_lossy()).unwrap();

        let res = scan(&db, false, |_, _| {}).unwrap();
        assert_eq!(res.added, 2);
        assert_eq!(res.failed, 0);
        let tracks = list_tracks(&db).unwrap();
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().any(|t| t.title == "My Track"));

        // Unchanged rescan parses nothing
        let res = scan(&db, false, |_, _| {}).unwrap();
        assert_eq!((res.added, res.updated, res.removed), (0, 0, 0));

        // Touching a file re-parses only it
        let f = root.join("My Track.mp3");
        std::fs::write(&f, b"not really audio, longer now").unwrap();
        let res = scan(&db, false, |_, _| {}).unwrap();
        assert_eq!((res.added, res.updated), (0, 1));

        // Deleting a file marks it missing
        std::fs::remove_file(&f).unwrap();
        let res = scan(&db, false, |_, _| {}).unwrap();
        assert_eq!(res.removed, 1);
        let tracks = list_tracks(&db).unwrap();
        assert!(tracks.iter().find(|t| t.title == "My Track").unwrap().missing);
    }

    #[test]
    fn unreachable_root_does_not_mark_everything_missing() {
        let (dir, db) = test_db();
        let root = dir.path().join("music");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.mp3"), b"x").unwrap();
        add_folder(&db, &root.to_string_lossy()).unwrap();
        assert_eq!(scan(&db, false, |_, _| {}).unwrap().added, 1);

        // The whole root disappears (unplugged drive): tracks must NOT be
        // flagged missing, and the scan reports the failure instead
        std::fs::remove_dir_all(&root).unwrap();
        let res = scan(&db, false, |_, _| {}).unwrap();
        assert_eq!(res.removed, 0, "unplugged root wiped the library state");
        assert!(res.failed >= 1);
        assert!(!list_tracks(&db).unwrap()[0].missing);

        // Root returns with the same file (same stats): track stays present
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.mp3"), b"x").unwrap();
        let res = scan(&db, false, |_, _| {}).unwrap();
        assert_eq!(res.removed, 0);
        assert!(!list_tracks(&db).unwrap()[0].missing);
    }
}
