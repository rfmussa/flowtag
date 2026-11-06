//! Rekordbox-friendly export: tags as a trailing hashtag block in the COMMENT
//! field (searchable in Rekordbox; re-export replaces only our own block).

use std::path::Path;
use anyhow::Error;
use serde::Serialize;
use onetagger_tag::{FrameName, Tag};

use crate::Db;

/// Outcome of a comment export run
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub written: usize,
    pub unchanged: usize,
    pub failed: usize,
}

/// Comment frame per format
fn comment_frame() -> FrameName {
    FrameName::new("COMM", "COMMENT", "©cmt")
}

/// Write every tagged, present track's tags into its COMMENT field as hashtags.
/// Existing comment text is preserved; only our trailing hashtag block changes.
pub fn export_comments(db: &Db, progress: impl Fn(usize, usize)) -> Result<ExportResult, Error> {
    // Every present track: untagged ones must still be visited so a previously
    // written hashtag block is stripped after the last tag is removed
    let jobs: Vec<(i64, String, Vec<String>)> = {
        let conn = db.conn();
        let mut stmt = conn.prepare("SELECT t.id, t.path FROM tracks t WHERE t.missing = 0")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut tag_stmt = conn.prepare(
            "SELECT tg.name FROM track_tags tt JOIN tags tg ON tg.id = tt.tag_id
             WHERE tt.track_id = ?1 ORDER BY tt.position, tg.id",
        )?;
        rows.into_iter()
            .map(|(id, path)| {
                let names = tag_stmt
                    .query_map([id], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((id, path, names))
            })
            .collect::<Result<Vec<_>, Error>>()?
    };

    let total = jobs.len();
    let mut result = ExportResult::default();
    for (i, (track_id, path, tag_names)) in jobs.into_iter().enumerate() {
        match export_one(&path, &tag_names) {
            Ok(true) => {
                result.written += 1;
                // Record the new stats so the next scan doesn't re-parse our write
                if let Some((mtime, size)) = file_stats(&path) {
                    db.conn().execute(
                        "UPDATE tracks SET mtime = ?2, size = ?3 WHERE id = ?1",
                        rusqlite::params![track_id, mtime, size],
                    )?;
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

fn file_stats(path: &str) -> Option<(i64, i64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((mtime, meta.len() as i64))
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
