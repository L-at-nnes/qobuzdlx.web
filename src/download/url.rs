use std::sync::LazyLock;

use regex::Regex;
use url::Url;

use super::ItemKind;

/// The locale a store link starts with: `/fr-fr/album/...`.
static LOCALE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]{2}-[a-z]{2}$").expect("valid pattern"));

const PLAYER: &str = "https://play.qobuz.com";

/// A Qobuz address, understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QobuzUrl {
    /// The same address written the one canonical way. Two spellings of one address (with or without
    /// a tracking query, a store link or a player link) give the same value, which is what makes
    /// them count as one item.
    pub canonical: String,
    pub kind: ItemKind,
    pub ref_id: String,
}

impl QobuzUrl {
    fn new(kind: ItemKind, ref_id: &str) -> Self {
        let canonical = match kind {
            ItemKind::Album => format!("{PLAYER}/album/{ref_id}"),
            ItemKind::Track => format!("{PLAYER}/track/{ref_id}"),
            ItemKind::Playlist => format!("{PLAYER}/playlist/{ref_id}"),
            ItemKind::Artist => format!("{PLAYER}/artist/{ref_id}"),
            ItemKind::Label => format!("{PLAYER}/label/{ref_id}"),
            ItemKind::FavoriteAlbums => format!("{PLAYER}/user/library/favorites/albums"),
            ItemKind::FavoriteTracks => format!("{PLAYER}/user/library/favorites/tracks"),
            ItemKind::FavoriteArtists => format!("{PLAYER}/user/library/favorites/artists"),
        };
        Self {
            canonical,
            kind,
            ref_id: ref_id.to_owned(),
        }
    }
}

/// The canonical address of something, as the queue knows it by.
#[must_use]
pub fn canonical(kind: ItemKind, ref_id: &str) -> String {
    QobuzUrl::new(kind, ref_id).canonical
}

/// Reads an address of the web player or of the store.
///
/// A query and a fragment are ignored (`?utm_source=share`), a trailing `/releases` too, and store
/// links (`/fr-fr/album/name/id`, `/interpreter/name/id`) give the same result as the player's.
/// Returns `None` for anything that is not a Qobuz address of a kind that can be downloaded.
#[must_use]
pub fn parse(input: &str) -> Option<QobuzUrl> {
    let input = input.trim();
    // Two addresses pasted one after the other are neither of them
    if input.is_empty() || input.contains(char::is_whitespace) {
        return None;
    }

    let url = Url::parse(input).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host != "qobuz.com" && !host.ends_with(".qobuz.com") {
        return None;
    }

    let mut segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.first().is_some_and(|first| LOCALE.is_match(first)) {
        segments.remove(0);
    }
    if segments.last() == Some(&"releases") {
        segments.pop();
    }

    match segments.as_slice() {
        ["user", .., last] => match *last {
            "albums" => Some(QobuzUrl::new(ItemKind::FavoriteAlbums, "")),
            "tracks" => Some(QobuzUrl::new(ItemKind::FavoriteTracks, "")),
            "artists" => Some(QobuzUrl::new(ItemKind::FavoriteArtists, "")),
            _ => None,
        },
        ["album", _, ..] => segments
            .last()
            .filter(|id| is_album_id(id))
            .map(|id| QobuzUrl::new(ItemKind::Album, id)),
        ["track", _, ..] => numeric(&segments).map(|id| QobuzUrl::new(ItemKind::Track, id)),
        ["playlist", _, ..] => numeric(&segments).map(|id| QobuzUrl::new(ItemKind::Playlist, id)),
        ["artist" | "interpreter", _, ..] => {
            numeric(&segments).map(|id| QobuzUrl::new(ItemKind::Artist, id))
        }
        ["label", _, ..] => numeric(&segments).map(|id| QobuzUrl::new(ItemKind::Label, id)),
        _ => None,
    }
}

fn is_album_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

/// The last segment, when it is a number.
fn numeric<'a>(segments: &[&'a str]) -> Option<&'a str> {
    segments
        .last()
        .copied()
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(input: &str) -> Option<String> {
        parse(input).map(|parsed| parsed.canonical)
    }

    #[test]
    fn player_links_are_understood_for_every_kind() {
        let cases = [
            (
                "https://play.qobuz.com/album/0886444350747",
                ItemKind::Album,
                "0886444350747",
            ),
            (
                "https://play.qobuz.com/album/abc123def",
                ItemKind::Album,
                "abc123def",
            ),
            (
                "https://play.qobuz.com/track/26803097",
                ItemKind::Track,
                "26803097",
            ),
            (
                "https://play.qobuz.com/playlist/71588272",
                ItemKind::Playlist,
                "71588272",
            ),
            (
                "https://play.qobuz.com/artist/25046",
                ItemKind::Artist,
                "25046",
            ),
            (
                "https://play.qobuz.com/label/9582352",
                ItemKind::Label,
                "9582352",
            ),
        ];

        for (input, kind, ref_id) in cases {
            let parsed = parse(input).unwrap_or_else(|| panic!("{input} was not understood"));
            assert_eq!(
                (parsed.kind, parsed.ref_id.as_str()),
                (kind, ref_id),
                "{input}"
            );
            assert_eq!(
                parsed.canonical, input,
                "a player link is already canonical"
            );
        }
    }

    #[test]
    fn store_links_give_the_player_address() {
        assert_eq!(
            canonical("https://www.qobuz.com/fr-fr/album/kind-of-blue-miles-davis/5099749522428")
                .as_deref(),
            Some("https://play.qobuz.com/album/5099749522428")
        );
        assert_eq!(
            canonical("https://www.qobuz.com/us-en/interpreter/miles-davis/25046").as_deref(),
            Some("https://play.qobuz.com/artist/25046")
        );
        assert_eq!(
            canonical("https://www.qobuz.com/gb-en/label/columbia/10106072").as_deref(),
            Some("https://play.qobuz.com/label/10106072")
        );
    }

    #[test]
    fn a_query_a_fragment_and_a_trailing_slash_are_ignored() {
        let expected = Some("https://play.qobuz.com/album/abc".to_owned());

        assert_eq!(
            canonical("https://play.qobuz.com/album/abc?utm_source=share"),
            expected
        );
        assert_eq!(canonical("https://play.qobuz.com/album/abc#top"), expected);
        assert_eq!(canonical("https://play.qobuz.com/album/abc/"), expected);
        assert_eq!(canonical("  https://play.qobuz.com/album/abc  "), expected);
    }

    #[test]
    fn a_trailing_releases_page_of_an_artist_is_the_artist() {
        assert_eq!(
            canonical("https://www.qobuz.com/fr-fr/interpreter/nina/123/releases").as_deref(),
            Some("https://play.qobuz.com/artist/123")
        );
        assert_eq!(
            canonical("https://play.qobuz.com/artist/123/releases").as_deref(),
            Some("https://play.qobuz.com/artist/123")
        );
    }

    #[test]
    fn the_favourites_of_a_user_are_understood_in_both_spellings() {
        assert_eq!(
            parse("https://play.qobuz.com/user/library/favorites/albums")
                .unwrap()
                .kind,
            ItemKind::FavoriteAlbums
        );
        assert_eq!(
            parse("https://play.qobuz.com/user/15273219/tracks")
                .unwrap()
                .kind,
            ItemKind::FavoriteTracks
        );
        assert_eq!(
            canonical("https://play.qobuz.com/user/15273219/artists").as_deref(),
            Some("https://play.qobuz.com/user/library/favorites/artists")
        );
    }

    #[test]
    fn other_hosts_are_refused() {
        assert_eq!(parse("https://example.com/album/abc"), None);
        assert_eq!(parse("https://notqobuz.com/album/abc"), None);
        assert_eq!(parse("https://qobuz.com.evil.example/album/abc"), None);
        assert_eq!(parse("ftp://play.qobuz.com/album/abc"), None);
    }

    #[test]
    fn text_that_is_not_an_address_is_refused() {
        for input in [
            "",
            "   ",
            "not a url",
            "qobuz",
            "https://",
            "javascript:alert(1)",
        ] {
            assert_eq!(parse(input), None, "{input:?}");
        }
    }

    #[test]
    fn two_addresses_pasted_together_are_refused() {
        assert_eq!(
            parse("https://play.qobuz.com/album/a https://play.qobuz.com/album/b"),
            None
        );
    }

    #[test]
    fn a_page_that_cannot_be_downloaded_is_refused() {
        for input in [
            "https://play.qobuz.com/",
            "https://play.qobuz.com/discover",
            "https://play.qobuz.com/album",
            "https://play.qobuz.com/track/not-a-number",
            "https://play.qobuz.com/user/library/favorites/labels",
        ] {
            assert_eq!(parse(input), None, "{input}");
        }
    }

    #[test]
    fn the_canonical_address_of_something_parses_back_to_it() {
        for (kind, id) in [
            (ItemKind::Album, "0886444350747"),
            (ItemKind::Track, "26803097"),
            (ItemKind::Playlist, "71588272"),
            (ItemKind::Artist, "25046"),
            (ItemKind::Label, "9582352"),
        ] {
            let address = super::canonical(kind, id);

            let parsed = parse(&address).unwrap();

            assert_eq!(
                (parsed.kind, parsed.ref_id.as_str()),
                (kind, id),
                "{address}"
            );
        }
    }

    #[test]
    fn hosts_are_not_case_sensitive() {
        assert_eq!(
            canonical("https://PLAY.Qobuz.com/album/abc").as_deref(),
            Some("https://play.qobuz.com/album/abc")
        );
    }
}
