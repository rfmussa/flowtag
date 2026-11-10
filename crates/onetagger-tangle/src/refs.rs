use unicode_normalization::UnicodeNormalization;

/// Map an absolute local path to a Tangle local ref (`~/...`, NFC-normalised).
/// Returns `None` when the path is not inside `home`.
pub fn local_ref(path: &str, home: &str) -> Option<String> {
    let rest = path.strip_prefix(home)?.strip_prefix('/')?;
    Some(format!("~/{rest}").nfc().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_becomes_tilde() {
        assert_eq!(local_ref("/Users/r/Music/Swinsian/a.mp3", "/Users/r").as_deref(), Some("~/Music/Swinsian/a.mp3"));
    }
    #[test]
    fn nfd_path_matches_nfc_ref() {
        // "Xangô" decomposed (o + U+0302) must come out composed
        assert_eq!(local_ref("/Users/r/Xango\u{302}.mp3", "/Users/r").as_deref(), Some("~/Xang\u{f4}.mp3"));
    }
    #[test]
    fn outside_home_is_none() {
        assert_eq!(local_ref("/Volumes/x/a.mp3", "/Users/r"), None);
    }
    #[test]
    fn home_prefix_needs_separator() {
        assert_eq!(local_ref("/Users/rteez2/a.mp3", "/Users/rteez"), None);
    }
}
