//! Track inspector: read and write a track's standard metadata fields
//! (title/artist/album/genre/year/bpm/key/comment) straight in the file,
//! keeping the DB row in sync. The custom TAGS field is never touched here —
//! that belongs to the flusher.

use std::path::Path;

use anyhow::Error;
use onetagger_tag::{Field, FrameName, Tag, TagDate};
use serde::{Deserialize, Serialize};

use crate::export::comment_frame;
use crate::{file_stats, tracks, Db};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<i64>,
    pub bpm: Option<i64>,
    pub key: String,
    pub comment: String,
}

fn first(v: Option<Vec<String>>) -> String {
    v.and_then(|v| v.into_iter().next()).unwrap_or_default()
}

/// Read the inspector fields from the file itself (not the DB)
pub fn read_track_meta(path: &str) -> Result<TrackMeta, Error> {
    let tag_wrap = Tag::load_file(Path::new(path), true)?;
    let format = tag_wrap.format();
    let tag = tag_wrap.tag();
    Ok(TrackMeta {
        title: first(tag.get_field(Field::Title)),
        artist: tag.get_field(Field::Artist).map(|v| v.join(", ")).unwrap_or_default(),
        album: first(tag.get_field(Field::Album)),
        genre: tag.get_field(Field::Genre).map(|v| v.join(", ")).unwrap_or_default(),
        year: tag.get_date().map(|d| d.year as i64),
        bpm: first(tag.get_field(Field::BPM)).parse::<f64>().ok().map(|b| b.round() as i64),
        key: first(tag.get_field(Field::Key)),
        comment: first(tag.get_raw(&comment_frame().by_format(&format))),
    })
}

/// Set a field, or remove it when the value is blank
fn set_or_clear(tag: &mut dyn onetagger_tag::TagImpl, format: &onetagger_tag::AudioFileFormat, field: Field, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        tag.remove_raw(field.by_format(format));
    } else {
        tag.set_field(field, vec![value.to_string()], true);
    }
}

/// Write the inspector fields to the file and mirror the list-view columns
/// (title/artist/bpm/key/year) into the DB row. Returns the state as re-read
/// from the file, for the client echo.
pub fn write_track_meta(db: &Db, track_id: i64, meta: &TrackMeta) -> Result<TrackMeta, Error> {
    let path = tracks::get_track_path(db, track_id)?;

    // Only refresh stored stats when the file was untouched outside the app,
    // so a scan still picks up external edits (same guard as flusher/export)
    let (db_mtime, db_size): (i64, i64) = db.conn().query_row(
        "SELECT mtime, size FROM tracks WHERE id = ?1",
        [track_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let externally_unchanged = file_stats(&path) == Some((db_mtime, db_size));

    let mut tag_wrap = Tag::load_file(Path::new(&path), true)?;
    let format = tag_wrap.format();
    {
        let tag = *tag_wrap.tag_mut();
        set_or_clear(tag, &format, Field::Title, &meta.title);
        set_or_clear(tag, &format, Field::Artist, &meta.artist);
        set_or_clear(tag, &format, Field::Album, &meta.album);
        set_or_clear(tag, &format, Field::Genre, &meta.genre);
        set_or_clear(tag, &format, Field::Key, &meta.key);
        match meta.bpm {
            Some(bpm) => tag.set_field(Field::BPM, vec![bpm.to_string()], true),
            None => tag.remove_raw(Field::BPM.by_format(&format)),
        }
        match meta.year {
            Some(year) => tag.set_date(&TagDate { year: year as i32, month: None, day: None }, true),
            None => {
                // There is no Field for dates; drop the frames directly
                // (both ID3 date frames: v2.4 TDRC and v2.3 TYER)
                for frame in [FrameName::new("TDRC", "DATE", "©day"), FrameName::new("TYER", "YEAR", "©day")] {
                    tag.remove_raw(&frame.by_format(&format));
                }
            }
        }
        let comment = meta.comment.trim();
        let frame = comment_frame().by_format(&format);
        if comment.is_empty() {
            tag.remove_raw(&frame);
        } else {
            tag.set_raw(&frame, vec![comment.to_string()], true);
        }
        tag.save_file(Path::new(&path))?;
    }

    // Mirror the list-view columns; DB titles are never empty (filename stem)
    let db_title = match meta.title.trim() {
        "" => Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
        t => t.to_string(),
    };
    db.conn().execute(
        "UPDATE tracks SET title = ?2, artist = ?3, bpm = ?4, key = ?5, year = ?6 WHERE id = ?1",
        rusqlite::params![track_id, db_title, meta.artist.trim(), meta.bpm,
            match meta.key.trim() { "" => None, k => Some(k) }, meta.year],
    )?;
    if externally_unchanged {
        if let Some((mtime, size)) = file_stats(&path) {
            db.conn().execute(
                "UPDATE tracks SET mtime = ?2, size = ?3 WHERE id = ?1",
                rusqlite::params![track_id, mtime, size],
            )?;
        }
    }

    read_track_meta(&path)
}
