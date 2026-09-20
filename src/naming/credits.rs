//! Titles and artist names as releases credit them.

use std::sync::LazyLock;

use regex::Regex;

use crate::qobuz::{Album, ArtistRole, Track};

/// Names Qobuz gives the artist of a compilation, typos included.
const VARIOUS_ARTISTS: [&str; 7] = [
    "Various Artists, Array",
    "Various Artists",
    "Various Aritsts",
    "Various Artist",
    "Various Interpreters",
    "Various Interpreter",
    "Various Interprets",
];

static PARENTHESES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^()]+)\)").expect("valid pattern"));

/// `Title (Version)`, with a version written twice (`Title (Live) (Live)`) collapsed to one.
#[must_use]
pub fn format_title(title: &str, version: Option<&str>) -> String {
    let full = match version {
        Some(version) => format!("{} ({version})", title.trim_end()),
        None => title.to_owned(),
    };
    collapse_repeated_parentheses(&full)
}

#[must_use]
pub fn track_title(track: &Track) -> String {
    format_title(&track.title, track.version.as_deref())
}

/// An album title with its version, `Kind of Blue (Mono Version)`.
#[must_use]
pub fn album_title(album: &Album) -> String {
    match &album.version {
        Some(version) => format!("{} ({version})", album.title.trim_end()),
        None => album.title.clone(),
    }
}

/// Merges consecutive identical parenthesised groups, ignoring case: `(Live) (live)` becomes `(Live)`.
fn collapse_repeated_parentheses(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut copied_up_to = 0;
    let mut previous: Option<(usize, String)> = None; // end of the last group and its content

    for group in PARENTHESES.captures_iter(text) {
        let whole = group.get(0).expect("group 0 always exists");
        let content = group[1].to_lowercase();

        let repeats = previous.as_ref().is_some_and(|(end, last)| {
            *last == content && text[*end..whole.start()].trim().is_empty()
        });

        if repeats {
            // Drop the blank and the repeated group
            result.push_str(&text[copied_up_to..previous.as_ref().map_or(0, |(end, _)| *end)]);
            copied_up_to = whole.end();
        }
        previous = Some((whole.end(), content));
    }

    result.push_str(&text[copied_up_to..]);
    result
}

/// The main artists of a release joined as `A, B & C`, when there are several.
#[must_use]
pub fn joined_main_artists(artists: &[ArtistRole]) -> Option<String> {
    let names: Vec<&str> = artists
        .iter()
        .filter(|artist| artist.is_main_artist())
        .map(|artist| artist.name.as_str())
        .collect();

    match names.as_slice() {
        [] | [_] => None,
        [rest @ .., last] => Some(format!("{} & {last}", rest.join(", "))),
    }
}

/// Whether the release is a compilation credited to "Various Artists".
#[must_use]
pub fn is_various_artists(release_artist: &str) -> bool {
    VARIOUS_ARTISTS
        .iter()
        .any(|name| name.eq_ignore_ascii_case(release_artist))
}

/// The explicit/clean wording of the parental warning tags, by suffix.
///
/// Each variant exists once per scope: `%Track<suffix>%` follows the track's warning and
/// `%Album<suffix>%` the album's.
/// Returns `None` for a suffix that is not a parental warning variant.
#[must_use]
pub fn parental_warning(suffix: &str, explicit: bool) -> Option<&'static str> {
    let (when_explicit, when_clean) = match suffix {
        "pa" => ("Explicit", "Clean"),
        "pashort" => ("E", "C"),
        "paifex" => ("Explicit", ""),
        "paifexshort" => ("E", ""),
        "paifcl" => ("", "Clean"),
        "paifclshort" => ("", "C"),
        "paenclosed" => ("(Explicit)", "(Clean)"),
        "paenclosed[]" => ("[Explicit]", "[Clean]"),
        "paenclosedshort" => ("(E)", "(C)"),
        "paenclosedshort[]" => ("[E]", "[C]"),
        "paifexenclosed" => ("(Explicit)", ""),
        "paifexenclosed[]" => ("[Explicit]", ""),
        "paifexenclosedshort" => ("(E)", ""),
        "paifexenclosedshort[]" => ("[E]", ""),
        "paifclenclosed" => ("", "(Clean)"),
        "paifclenclosed[]" => ("", "[Clean]"),
        "paifclenclosedshort" => ("", "(C)"),
        "paifclenclosedshort[]" => ("", "[C]"),
        _ => return None,
    };
    Some(if explicit { when_explicit } else { when_clean })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artist(name: &str, roles: &[&str]) -> ArtistRole {
        ArtistRole {
            name: name.to_owned(),
            roles: roles.iter().map(ToString::to_string).collect(),
            ..ArtistRole::default()
        }
    }

    #[test]
    fn a_version_is_appended_in_parentheses() {
        assert_eq!(
            format_title("Midnight Garden", Some("Live")),
            "Midnight Garden (Live)"
        );
        assert_eq!(format_title("Plain", None), "Plain");
    }

    #[test]
    fn trailing_spaces_before_the_version_are_dropped() {
        assert_eq!(format_title("Title  ", Some("Remix")), "Title (Remix)");
    }

    #[test]
    fn a_version_already_in_the_title_is_not_written_twice() {
        assert_eq!(format_title("Song (Live)", Some("Live")), "Song (Live)");
        assert_eq!(format_title("Song (Live)", Some("live")), "Song (Live)");
    }

    #[test]
    fn different_parentheses_are_left_alone() {
        assert_eq!(
            format_title("Song (Part 1)", Some("Live")),
            "Song (Part 1) (Live)"
        );
    }

    #[test]
    fn three_repeats_collapse_to_one() {
        assert_eq!(
            collapse_repeated_parentheses("A (x) (x)  (X) end"),
            "A (x) end"
        );
    }

    #[test]
    fn repeats_separated_by_text_are_not_merged() {
        assert_eq!(collapse_repeated_parentheses("(x) and (x)"), "(x) and (x)");
    }

    #[test]
    fn several_main_artists_are_joined_with_an_ampersand_before_the_last() {
        let two = [
            artist("John Coltrane", &["main-artist"]),
            artist("Johnny Hartman", &["main-artist"]),
        ];
        let three = [
            artist("A", &["main-artist"]),
            artist("B", &["main-artist"]),
            artist("C", &["main-artist"]),
        ];

        assert_eq!(
            joined_main_artists(&two).as_deref(),
            Some("John Coltrane & Johnny Hartman")
        );
        assert_eq!(joined_main_artists(&three).as_deref(), Some("A, B & C"));
    }

    #[test]
    fn one_main_artist_or_none_has_nothing_to_join() {
        assert_eq!(
            joined_main_artists(&[artist("Solo", &["main-artist"])]),
            None
        );
        assert_eq!(
            joined_main_artists(&[artist("Guest", &["featured-artist"])]),
            None
        );
        assert_eq!(joined_main_artists(&[]), None);
    }

    #[test]
    fn featured_artists_are_not_part_of_the_joined_name() {
        let artists = [
            artist("A", &["main-artist"]),
            artist("B", &["featured-artist"]),
            artist("C", &["main-artist"]),
        ];

        assert_eq!(joined_main_artists(&artists).as_deref(), Some("A & C"));
    }

    #[test]
    fn compilations_are_recognised_whatever_the_spelling_or_case() {
        for name in [
            "Various Artists",
            "various artists",
            "Various Aritsts",
            "Various Interpreters",
        ] {
            assert!(is_various_artists(name), "{name}");
        }
        assert!(!is_various_artists("Miles Davis"));
    }

    #[test]
    fn a_parental_warning_variant_gives_the_explicit_or_the_clean_wording() {
        assert_eq!(parental_warning("pa", true), Some("Explicit"));
        assert_eq!(parental_warning("pa", false), Some("Clean"));
        assert_eq!(parental_warning("paifexenclosedshort[]", true), Some("[E]"));
        assert_eq!(parental_warning("paifexenclosedshort[]", false), Some(""));
        assert_eq!(parental_warning("paifclenclosed", false), Some("(Clean)"));
    }

    #[test]
    fn something_that_is_not_a_variant_is_none() {
        assert_eq!(parental_warning("title", true), None);
    }

    #[test]
    fn every_variant_of_the_readme_resolves() {
        let all = [
            "pa",
            "pashort",
            "paifex",
            "paifexshort",
            "paifcl",
            "paifclshort",
            "paenclosed",
            "paenclosed[]",
            "paenclosedshort",
            "paenclosedshort[]",
            "paifexenclosed",
            "paifexenclosed[]",
            "paifexenclosedshort",
            "paifexenclosedshort[]",
            "paifclenclosed",
            "paifclenclosed[]",
            "paifclenclosedshort",
            "paifclenclosedshort[]",
        ];

        assert!(
            all.iter()
                .all(|suffix| parental_warning(suffix, true).is_some()
                    && parental_warning(suffix, false).is_some())
        );
    }
}
