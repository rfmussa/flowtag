//! Rekordbox-friendly export: tags as a trailing hashtag block in the COMMENT
//! field (searchable in Rekordbox; re-export replaces only our own block).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Error;
use serde::Serialize;
use onetagger_tag::{FrameName, Tag};

use crate::Db;

/// True while a comment export runs; the flush worker pauses so the two never
/// write the same file concurrently (last-save-wins would drop one change set)
pub static EXPORT_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Clears EXPORT_ACTIVE even when the export panics or errors
struct ExportGuard;
impl Drop for ExportGuard {
    fn drop(&mut self) {
        EXPORT_ACTIVE.store(false, Ordering::SeqCst);
    }
}

/// Outcome of a comment export run
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub written: usize,
    pub unchanged: usize,
    pub failed: usize,
    /// Files skipped because they were in use (e.g. currently playing)
    pub skipped: usize,
}

/// Comment frame per format
pub(crate) fn comment_frame() -> FrameName {
    FrameName::new("COMM", "COMMENT", "©cmt")
}

/// Write every present track's tags into its COMMENT field as hashtags.
/// Existing comment text is preserved; only our trailing hashtag block changes.
/// `skip` protects files in use (e.g. currently playing); while this runs, the
/// flush worker pauses (EXPORT_ACTIVE).
pub fn export_comments(
    db: &Db,
    only: Option<&std::collections::HashSet<i64>>,
    skip: &dyn Fn(&str) -> bool,
    progress: impl Fn(usize, usize),
) -> Result<ExportResult, Error> {
    EXPORT_ACTIVE.store(true, Ordering::SeqCst);
    let _guard = ExportGuard;

    // Every present track: untagged ones must still be visited so a previously
    // written hashtag block is stripped after the last tag is removed
    let jobs: Vec<(i64, String, Vec<String>, i64, i64)> = {
        let conn = db.conn();
        let mut stmt = conn.prepare("SELECT t.id, t.path, t.mtime, t.size FROM tracks t WHERE t.missing = 0")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut tag_stmt = conn.prepare(
            "SELECT tg.name FROM track_tags tt JOIN tags tg ON tg.id = tt.tag_id
             WHERE tt.track_id = ?1 ORDER BY tt.position, tg.id",
        )?;
        rows.into_iter()
            .map(|(id, path, mtime, size)| {
                let names = tag_stmt
                    .query_map([id], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((id, path, names, mtime, size))
            })
            .collect::<Result<Vec<_>, Error>>()?
    };

    // Scoped export (a playlist or a tag): touch only the chosen tracks
    let jobs: Vec<_> = match only {
        Some(ids) => jobs.into_iter().filter(|(id, ..)| ids.contains(id)).collect(),
        None => jobs,
    };

    let total = jobs.len();
    let mut result = ExportResult::default();
    for (i, (track_id, path, tag_names, db_mtime, db_size)) in jobs.into_iter().enumerate() {
        if skip(&path) {
            result.skipped += 1;
            continue;
        }
        // Keep old stats when the file was changed outside the app, so the next
        // scan still re-parses those external edits
        let externally_unchanged = crate::file_stats(&path) == Some((db_mtime, db_size));
        match export_one(&path, &tag_names) {
            Ok(true) => {
                result.written += 1;
                if externally_unchanged {
                    if let Some((mtime, size)) = crate::file_stats(&path) {
                        db.conn().execute(
                            "UPDATE tracks SET mtime = ?2, size = ?3 WHERE id = ?1",
                            rusqlite::params![track_id, mtime, size],
                        )?;
                    }
                }
            }
            Ok(false) => result.unchanged += 1,
            Err(e) => {
                warn!("Comment export failed for {path}: {e}");
                result.failed += 1;
            }
        }
        if (i + 1) % 50 == 0 {
            progress(i + 1, total);
        }
    }
    progress(total, total);
    Ok(result)
}

/// Returns Ok(true) when the file was written, Ok(false) when already up to date
fn export_one(path: &str, tag_names: &[String]) -> Result<bool, Error> {
    let mut tag_wrap = Tag::load_file(Path::new(path), true)?;
    let frame = comment_frame().by_format(&tag_wrap.format());
    let tag = tag_wrap.tag_mut();
    let existing = tag
        .get_raw(&frame)
        .and_then(|v| v.into_iter().next())
        .unwrap_or_default();
    let updated = build_comment(&existing, tag_names);
    if updated == existing {
        return Ok(false);
    }
    tag.set_raw(&frame, vec![updated], true);
    tag.save_file(Path::new(path))?;
    Ok(true)
}

/// Turn a tag name into a hashtag token: whitespace removed, leading '#' avoided
/// by validation. "Deep House" -> "#DeepHouse"
fn hashtag(name: &str) -> String {
    let compact: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    format!("#{compact}")
}

/// Whether a token looks like one of our hashtags (starts with '#', no inner '#')
fn is_hashtag_token(token: &str) -> bool {
    token.len() > 1 && token.starts_with('#') && !token[1..].contains('#')
}

/// Build the new comment value: keep the existing text, strip a previously
/// written TRAILING hashtag block (hashtags elsewhere in the text are kept),
/// and append the current tags as hashtags. Idempotent.
pub fn build_comment(existing: &str, tags: &[String]) -> String {
    // Strip the trailing hashtag block: walk tokens from the end while they
    // are hashtags, then cut at that point.
    let trimmed = existing.trim_end();
    let mut cut = trimmed.len();
    for token in trimmed.split_whitespace().rev() {
        if !is_hashtag_token(token) {
            break;
        }
        // Find this token's start (search backwards from the current cut)
        if let Some(pos) = trimmed[..cut].rfind(token) {
            cut = pos;
        } else {
            break;
        }
    }
    let base = trimmed[..cut].trim_end();

    let block: Vec<String> = tags.iter().map(|t| hashtag(t)).collect();
    match (base.is_empty(), block.is_empty()) {
        (_, true) => base.to_string(),
        (true, false) => block.join(" "),
        (false, false) => format!("{} {}", base, block.join(" ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_comment_gets_hashtags() {
        assert_eq!(build_comment("", &tags(&["Dark", "Peak Time"])), "#Dark #PeakTime");
    }

    #[test]
    fn existing_text_is_preserved() {
        assert_eq!(
            build_comment("Bought on Bandcamp", &tags(&["Dark"])),
            "Bought on Bandcamp #Dark"
        );
    }

    #[test]
    fn reexport_is_idempotent() {
        let t = tags(&["Dark", "Rolling"]);
        let once = build_comment("my note", &t);
        let twice = build_comment(&once, &t);
        assert_eq!(once, twice);
        assert_eq!(twice, "my note #Dark #Rolling");
    }

    #[test]
    fn changed_tags_replace_the_old_block() {
        let first = build_comment("note", &tags(&["Old", "Gone"]));
        let second = build_comment(&first, &tags(&["New"]));
        assert_eq!(second, "note #New");
    }

    #[test]
    fn empty_tags_strip_the_block() {
        let first = build_comment("note", &tags(&["Dark"]));
        assert_eq!(build_comment(&first, &[]), "note");
        assert_eq!(build_comment("#OnlyTags #Here", &[]), "");
    }

    #[test]
    fn hashtags_inside_the_text_are_kept() {
        assert_eq!(
            build_comment("love #this track", &tags(&["Dark"])),
            "love #this track #Dark"
        );
        // Re-export still only replaces the trailing block
        assert_eq!(
            build_comment("love #this track #Dark", &tags(&["Warm"])),
            "love #this track #Warm"
        );
    }
}
