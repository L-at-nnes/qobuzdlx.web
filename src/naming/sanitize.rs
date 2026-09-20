//! Turning arbitrary text into names a filesystem accepts.
//!
//! The container runs on Linux, but music is often stored on SMB or Windows shares: names are
//! cleaned for the strictest of them while staying readable.

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

/// 255 bytes is the limit of a path component on ext4 and most NAS filesystems. The margin leaves
/// room for the audio extension (`.flac`) and the ` (99)` suffix of the "rename" duplicate mode.
pub const MAX_COMPONENT_BYTES: usize = 240;

const ELLIPSIS: &str = "\u{2026}";

/// A dash glued to the edge of a name by a separator: `Song:` becomes `Song -`, and an empty field in
/// `%TrackNumber% - %TrackTitle%` leaves `- Title`. A dash with no blank next to it (`-OSC-`) is part
/// of the name and stays.
static EDGE_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:-\s+)+|(?:\s+-)+$").expect("valid pattern"));

/// Replaces what Windows and NAS filesystems forbid with readable substitutes.
///
/// `Song: Intro` becomes `Song - Intro`, `AC/DC` becomes `AC-DC`, and `What? Now!` loses its question
/// mark. Spaces are collapsed and dots and spaces trimmed from both ends. A name that is blank is
/// returned as it is: an empty template is a choice, not something to fill in.
#[must_use]
pub fn make_safe_filename(name: &str) -> String {
    if name.trim().is_empty() {
        return name.to_owned();
    }

    let mut safe = String::with_capacity(name.len());
    for character in name.chars() {
        match character {
            ':' => safe.push_str(" -"),
            '/' | '\\' => safe.push('-'),
            '*' | '?' | '|' | '"' | '<' | '>' => {}
            control if control.is_control() => safe.push('_'),
            other => safe.push(other),
        }
    }

    let collapsed = collapse_whitespace(&safe);
    let trimmed = collapsed.trim_matches(|c| c == ' ' || c == '.');
    let without_edges = EDGE_SEPARATOR.replace_all(trimmed, "");
    without_edges
        .trim_matches(|c| c == ' ' || c == '.')
        .to_owned()
}

/// Makes a folder name safe. It may hold several components (a template can contain `/`): each one
/// loses the dots and spaces at its ends, so `.` and `..` vanish and the path cannot climb out of the
/// download folder, while a `..` inside a name (`Vol..2`) is left alone.
///
/// Empty for a blank input (an intentionally blank template), `_` when the input had content that
/// did not survive.
#[must_use]
pub fn sanitize_path_segment(segment: &str) -> String {
    if segment.trim().is_empty() {
        return String::new();
    }

    let components: Vec<&str> = segment
        .split('/')
        .map(|component| component.trim_matches(|c| c == '.' || c == ' '))
        .filter(|component| !component.is_empty())
        .collect();

    if components.is_empty() {
        "_".to_owned()
    } else {
        components.join("/")
    }
}

pub(super) fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cuts `name` to at most `max_bytes` bytes of UTF-8 and marks the cut with an ellipsis.
///
/// The limit is in bytes, not characters, because that is what filesystems count: a title in Japanese
/// or Cyrillic takes two to three bytes per character. The cut falls between user-perceived
/// characters, never between a letter and its combining marks.
#[must_use]
pub fn truncate_to_bytes(name: &str, max_bytes: usize) -> String {
    if name.len() <= max_bytes {
        return name.to_owned();
    }

    let budget = max_bytes.saturating_sub(ELLIPSIS.len());
    let mut kept = String::new();
    for grapheme in name.graphemes(true) {
        if kept.len() + grapheme.len() > budget {
            break;
        }
        kept.push_str(grapheme);
    }

    kept.push_str(ELLIPSIS);
    kept
}

/// Whether `path` is `base` or lies under it.
///
/// Components are compared, not characters: a plain prefix test would accept `/downloads-evil` as
/// being inside `/downloads`. The paths are resolved lexically, so `..` is taken into account without
/// the files having to exist.
#[must_use]
pub fn is_inside(base: &Path, path: &Path) -> bool {
    normalise(path).starts_with(normalise(base))
}

fn normalise(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            other => resolved.push(other),
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn safe(input: &str) -> String {
        make_safe_filename(input)
    }

    #[test]
    fn forbidden_characters_get_readable_substitutes() {
        assert_eq!(safe("Song: Intro"), "Song - Intro");
        assert_eq!(safe("AC/DC"), "AC-DC");
        assert_eq!(safe(r"back\slash"), "back-slash");
        assert_eq!(safe("What? Now!"), "What Now!");
        assert_eq!(safe(r#"a*b|c"d<e>f"#), "abcdef");
    }

    #[test]
    fn a_separator_stuck_to_the_edge_by_a_removed_character_is_dropped() {
        assert_eq!(safe("Song:"), "Song");
        assert_eq!(safe("01 - "), "01");
        assert_eq!(safe("- Intro"), "Intro");
        assert_eq!(safe("01 - - "), "01");
    }

    #[test]
    fn dashes_that_belong_to_the_name_stay() {
        assert_eq!(safe("-OSC-"), "-OSC-");
        assert_eq!(safe("Re-mix"), "Re-mix");
        assert_eq!(safe("A - B"), "A - B");
        assert_eq!(safe("-"), "-");
    }

    #[test]
    fn spaces_are_collapsed_and_dots_and_spaces_trimmed() {
        assert_eq!(safe("  spaced   out  "), "spaced out");
        assert_eq!(safe("Title."), "Title");
        assert_eq!(safe("Mr. "), "Mr");
    }

    #[test]
    fn control_characters_become_underscores() {
        assert_eq!(safe("a\u{1}b"), "a_b");
    }

    #[test]
    fn a_blank_name_is_returned_untouched() {
        assert_eq!(safe(""), "");
        assert_eq!(safe("   "), "   ");
    }

    #[test]
    fn a_path_segment_loses_dots_and_spaces_at_the_ends_of_each_component() {
        assert_eq!(sanitize_path_segment("Mr. "), "Mr");
        assert_eq!(sanitize_path_segment("Artist. /Album .."), "Artist/Album");
    }

    #[test]
    fn traversal_components_vanish() {
        assert_eq!(sanitize_path_segment("A/../B"), "A/B");
        assert_eq!(sanitize_path_segment("../../etc"), "etc");
        assert_eq!(sanitize_path_segment("/etc/passwd"), "etc/passwd");
    }

    #[test]
    fn a_double_dot_inside_a_name_is_kept() {
        assert_eq!(sanitize_path_segment("Vol..2"), "Vol..2");
    }

    #[test]
    fn a_segment_with_nothing_left_becomes_an_underscore_but_a_blank_one_stays_blank() {
        assert_eq!(sanitize_path_segment(".."), "_");
        assert_eq!(sanitize_path_segment(" . . "), "_");
        assert_eq!(sanitize_path_segment(""), "");
        assert_eq!(sanitize_path_segment("   "), "");
    }

    #[test]
    fn a_short_name_is_not_truncated() {
        assert_eq!(truncate_to_bytes("Hello", MAX_COMPONENT_BYTES), "Hello");
        assert_eq!(truncate_to_bytes(&"a".repeat(240), 240).len(), 240);
    }

    #[test]
    fn one_byte_over_the_limit_is_cut_with_an_ellipsis() {
        let cut = truncate_to_bytes(&"a".repeat(241), 240);

        assert!(cut.ends_with('\u{2026}'));
        assert!(cut.len() <= 240);
    }

    #[test]
    fn multibyte_text_stays_within_the_byte_budget() {
        let japanese = "音楽の旅".repeat(40); // 160 characters, 480 bytes

        let cut = truncate_to_bytes(&japanese, MAX_COMPONENT_BYTES);

        assert!(cut.len() <= MAX_COMPONENT_BYTES);
        assert!(cut.chars().count() < japanese.chars().count());
    }

    #[test]
    fn a_letter_is_never_separated_from_its_combining_mark() {
        let combining = "e\u{301}".repeat(200); // e followed by a combining acute accent

        let cut = truncate_to_bytes(&combining, MAX_COMPONENT_BYTES);
        let body = cut.trim_end_matches('\u{2026}');

        assert!(cut.len() <= MAX_COMPONENT_BYTES);
        assert!(!body.starts_with('\u{301}'));
        assert!(
            body.ends_with('\u{301}'),
            "the last letter keeps its accent"
        );
        assert_eq!(body.chars().count() % 2, 0);
    }

    #[test]
    fn a_path_is_inside_its_base_by_whole_components() {
        let inside = |base: &str, path: &str| is_inside(Path::new(base), Path::new(path));

        assert!(inside("/downloads", "/downloads/Artist/Album"));
        assert!(inside("/downloads", "/downloads"));
        assert!(inside("/downloads/", "/downloads/Artist"));
        assert!(inside("/downloads", "/downloads/..hidden/x"));
        assert!(!inside("/downloads", "/downloads-evil/x"));
        assert!(!inside("/downloads/a", "/downloads"));
        assert!(!inside("/downloads", "/downloads/../etc"));
    }
}
