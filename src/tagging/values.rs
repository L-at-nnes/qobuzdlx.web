use std::sync::LazyLock;

use regex::Regex;

use super::comment::build_comment;
use crate::naming::{album_title, joined_main_artists, track_title};
use crate::qobuz::{Album, Track};
use crate::settings::{MarkerPosition, Tags, TitleMarker};

/// Store URLs carry a locale segment (`/fr-fr/`, `/us-en/`...) that has nothing to do with the
/// release: the link must be the same whatever language it was read in.
static LOCALE_SEGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(qobuz\.com)/[a-z]{2}-[a-z]{2}/").expect("valid pattern"));

/// What the tags of one file will say, before any file format gets involved.
///
/// A field is `None` when its tag is switched off or when Qobuz has no value for it: a tag without
/// a value is left out rather than written empty, which players show as a blank field instead of
/// "unknown".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagValues {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub total_tracks: Option<u32>,
    pub disc_number: Option<u32>,
    pub total_discs: Option<u32>,
    pub genre: Option<String>,
    /// Full release date, as Qobuz gives it (`2013-05-17`).
    pub release_date: Option<String>,
    pub year: Option<u32>,
    pub composer: Option<String>,
    pub copyright: Option<String>,
    pub label: Option<String>,
    pub isrc: Option<String>,
    pub barcode: Option<String>,
    /// `album`, `single`, `ep`...
    pub release_type: Option<String>,
    pub explicit: Option<bool>,
    pub url: Option<String>,
    pub comment: Option<String>,
}

/// Works out the tags of a track from the settings, the album and the track.
#[must_use]
pub fn build(tags: &Tags, merge_artists: bool, album: &Album, track: &Track) -> TagValues {
    let present = |enabled: bool, value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|text| enabled && !text.is_empty())
            .map(str::to_owned)
    };

    let date = release_date(album, track);

    TagValues {
        title: tags.title.then(|| marked_title(tags, track)),
        artist: present(
            tags.artist,
            track.performer.as_ref().and_then(|p| p.name.as_deref()),
        ),
        album: tags.album.then(|| album_title(album)),
        album_artist: present(tags.album_artist, Some(&album_artist(album, merge_artists))),
        track_number: tags.track_number.then_some(track.track_number),
        total_tracks: tags.total_tracks.then_some(album.tracks_count),
        disc_number: tags.disc_number.then_some(track.media_number),
        total_discs: tags.total_discs.then_some(album.media_count),
        genre: present(tags.genre, Some(&album.genre.name)),
        release_date: present(tags.release_date, date.as_deref()),
        year: date
            .as_deref()
            .and_then(|date| date.get(..4))
            .and_then(|year| year.parse().ok())
            .filter(|_| tags.year),
        composer: present(
            tags.composer,
            track.composer.as_ref().and_then(|p| p.name.as_deref()),
        ),
        copyright: present(tags.copyright, album.copyright.as_deref()),
        label: present(tags.label, Some(&album.label.name)),
        isrc: present(tags.isrc, track.isrc.as_deref()),
        barcode: present(tags.upc, album.upc.as_deref()),
        release_type: present(tags.release_type, Some(&album.product_type)),
        explicit: tags.explicit.then_some(track.parental_warning),
        url: present(tags.url, store_url(album, track).as_deref()),
        comment: tags
            .comment
            .then(|| build_comment(&tags.comment_text, album.description.as_deref()))
            .filter(|comment| !comment.trim().is_empty()),
    }
}

/// The track title, with the explicit or clean marker when one is asked for.
fn marked_title(tags: &Tags, track: &Track) -> String {
    let title = track_title(track);
    let marker = if track.parental_warning {
        &tags.explicit_marker
    } else {
        &tags.clean_marker
    };
    apply_marker(&title, marker)
}

fn apply_marker(title: &str, marker: &TitleMarker) -> String {
    if !marker.enabled || marker.text.trim().is_empty() {
        return title.to_owned();
    }
    // The spaces are kept exactly as typed: it is up to whoever writes `[E] ` or ` (Explicit)` to
    // provide them
    match marker.position {
        MarkerPosition::Prefix => format!("{}{title}", marker.text),
        MarkerPosition::Suffix => format!("{title}{}", marker.text),
    }
}

/// The album artist: the release's main artist, or every main artist joined (`A, B & C`) when the
/// "merge artist names" option is on.
#[must_use]
pub fn album_artist(album: &Album, merge_artists: bool) -> String {
    if merge_artists {
        if let Some(joined) = joined_main_artists(&album.artists) {
            return joined;
        }
    }
    album.artist.name.clone().unwrap_or_default()
}

fn release_date(album: &Album, track: &Track) -> Option<String> {
    let known = |date: Option<&str>| {
        date.map(str::trim)
            .filter(|date| !date.is_empty())
            .map(str::to_owned)
    };

    known(
        track
            .album
            .as_ref()
            .and_then(|album| album.release_date_original.as_deref()),
    )
    .or_else(|| known(album.release_date_original.as_deref()))
}

fn store_url(album: &Album, track: &Track) -> Option<String> {
    let url = track
        .url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .or(album.url.as_deref())?;
    Some(LOCALE_SEGMENT.replace(url, "$1/").into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qobuz::{ArtistRole, Genre, Label, Person};

    fn album() -> Album {
        Album {
            title: "Kind of Blue".into(),
            artist: Person {
                id: 1,
                name: Some("Miles Davis".into()),
            },
            genre: Genre {
                name: "Jazz".into(),
            },
            label: Label {
                name: "Columbia".into(),
                ..Label::default()
            },
            release_date_original: Some("1959-08-17".into()),
            upc: Some("5099749522428".into()),
            copyright: Some("(P) 1959".into()),
            tracks_count: 5,
            media_count: 1,
            ..Album::default()
        }
    }

    fn track() -> Track {
        Track {
            id: 1,
            title: "So What".into(),
            track_number: 1,
            media_number: 1,
            isrc: Some("USSM15900113".into()),
            performer: Some(Person {
                id: 1,
                name: Some("Miles Davis".into()),
            }),
            composer: Some(Person {
                id: 2,
                name: Some("Miles Davis".into()),
            }),
            ..Track::default()
        }
    }

    fn values(tags: &Tags) -> TagValues {
        build(tags, false, &album(), &track())
    }

    #[test]
    fn every_field_is_filled_in_when_qobuz_has_it_and_the_tag_is_on() {
        let values = values(&Tags::default());

        assert_eq!(values.title.as_deref(), Some("So What"));
        assert_eq!(values.artist.as_deref(), Some("Miles Davis"));
        assert_eq!(values.album.as_deref(), Some("Kind of Blue"));
        assert_eq!(values.album_artist.as_deref(), Some("Miles Davis"));
        assert_eq!(
            (values.track_number, values.total_tracks),
            (Some(1), Some(5))
        );
        assert_eq!((values.disc_number, values.total_discs), (Some(1), Some(1)));
        assert_eq!(values.genre.as_deref(), Some("Jazz"));
        assert_eq!(values.release_date.as_deref(), Some("1959-08-17"));
        assert_eq!(values.year, Some(1959));
        assert_eq!(values.label.as_deref(), Some("Columbia"));
        assert_eq!(values.isrc.as_deref(), Some("USSM15900113"));
        assert_eq!(values.barcode.as_deref(), Some("5099749522428"));
        assert_eq!(values.explicit, Some(false));
    }

    #[test]
    fn a_tag_switched_off_is_left_out() {
        let off = Tags {
            genre: false,
            isrc: false,
            track_number: false,
            ..Tags::default()
        };

        let values = values(&off);

        assert_eq!(
            (values.genre, values.isrc, values.track_number),
            (None, None, None)
        );
        assert!(values.album.is_some());
    }

    #[test]
    fn a_tag_without_a_value_is_left_out_rather_than_written_empty() {
        let bare_album = Album {
            genre: Genre::default(),
            label: Label::default(),
            upc: None,
            copyright: Some("  ".into()),
            ..album()
        };
        let bare_track = Track {
            isrc: None,
            composer: None,
            performer: Some(Person::default()),
            ..track()
        };

        let values = build(&Tags::default(), false, &bare_album, &bare_track);

        assert_eq!(values.genre, None);
        assert_eq!(values.label, None);
        assert_eq!(values.barcode, None);
        assert_eq!(values.copyright, None);
        assert_eq!(values.isrc, None);
        assert_eq!(values.composer, None);
        assert_eq!(values.artist, None);
    }

    #[test]
    fn the_year_comes_from_the_date_and_needs_four_digits() {
        let odd = Album {
            release_date_original: Some("19".into()),
            ..album()
        };

        assert_eq!(build(&Tags::default(), false, &odd, &track()).year, None);
        assert_eq!(
            build(&Tags::default(), false, &odd, &track())
                .release_date
                .as_deref(),
            Some("19")
        );
    }

    #[test]
    fn the_date_of_the_track_s_own_album_wins_over_the_one_of_the_release() {
        let reissue = Track {
            album: Some(Album {
                release_date_original: Some("2009-01-15".into()),
                ..Album::default()
            }),
            ..track()
        };

        assert_eq!(
            build(&Tags::default(), false, &album(), &reissue)
                .release_date
                .as_deref(),
            Some("2009-01-15")
        );
    }

    #[test]
    fn a_marker_is_added_to_the_title_of_the_matching_tracks_only() {
        let mut tags = Tags::default();
        tags.explicit_marker.enabled = true;
        tags.clean_marker.enabled = true;
        let explicit = Track {
            parental_warning: true,
            ..track()
        };

        assert_eq!(
            build(&tags, false, &album(), &explicit).title.as_deref(),
            Some("So What (Explicit)")
        );
        assert_eq!(
            build(&tags, false, &album(), &track()).title.as_deref(),
            Some("So What (Clean)")
        );
    }

    #[test]
    fn a_marker_can_go_before_the_title_and_keeps_its_spaces_as_typed() {
        let mut tags = Tags {
            explicit_marker: TitleMarker {
                enabled: true,
                text: "[E] ".into(),
                position: MarkerPosition::Prefix,
            },
            ..Tags::default()
        };
        let explicit = Track {
            parental_warning: true,
            ..track()
        };

        assert_eq!(
            build(&tags, false, &album(), &explicit).title.as_deref(),
            Some("[E] So What")
        );

        tags.explicit_marker = TitleMarker {
            enabled: true,
            text: "(Explicit)".into(),
            position: MarkerPosition::Suffix,
        };
        assert_eq!(
            build(&tags, false, &album(), &explicit).title.as_deref(),
            Some("So What(Explicit)")
        );
    }

    #[test]
    fn a_disabled_or_blank_marker_changes_nothing() {
        let mut tags = Tags::default();
        let explicit = Track {
            parental_warning: true,
            ..track()
        };
        assert_eq!(
            build(&tags, false, &album(), &explicit).title.as_deref(),
            Some("So What")
        );

        tags.explicit_marker = TitleMarker {
            enabled: true,
            text: "  ".into(),
            position: MarkerPosition::Suffix,
        };
        assert_eq!(
            build(&tags, false, &album(), &explicit).title.as_deref(),
            Some("So What")
        );
    }

    #[test]
    fn the_album_artist_follows_the_merge_option() {
        let duet = Album {
            artists: vec![
                ArtistRole {
                    id: 1,
                    name: "John Coltrane".into(),
                    roles: vec!["main-artist".into()],
                },
                ArtistRole {
                    id: 2,
                    name: "Johnny Hartman".into(),
                    roles: vec!["main-artist".into()],
                },
            ],
            artist: Person {
                id: 1,
                name: Some("John Coltrane".into()),
            },
            ..album()
        };

        assert_eq!(album_artist(&duet, false), "John Coltrane");
        assert_eq!(album_artist(&duet, true), "John Coltrane & Johnny Hartman");
        assert_eq!(album_artist(&Album::default(), true), "");
    }

    #[test]
    fn the_store_url_loses_its_locale_segment_whatever_the_language() {
        let url = |value: &str| {
            let linked = Album {
                url: Some(value.into()),
                ..album()
            };
            build(&Tags::default(), false, &linked, &track()).url
        };

        assert_eq!(
            url("https://www.qobuz.com/fr-fr/album/kind-of-blue/123").as_deref(),
            Some("https://www.qobuz.com/album/kind-of-blue/123")
        );
        assert_eq!(
            url("https://www.qobuz.com/us-en/album/x/1").as_deref(),
            Some("https://www.qobuz.com/album/x/1")
        );
        assert_eq!(
            url("https://www.qobuz.com/DE-de/album/x/1").as_deref(),
            Some("https://www.qobuz.com/album/x/1")
        );
        assert_eq!(
            url("https://www.qobuz.com/album/x/1").as_deref(),
            Some("https://www.qobuz.com/album/x/1")
        );
    }

    #[test]
    fn the_url_of_the_track_wins_over_the_one_of_the_album() {
        let with_urls = Track {
            url: Some("https://www.qobuz.com/fr-fr/track/9".into()),
            ..track()
        };
        let linked = Album {
            url: Some("https://www.qobuz.com/fr-fr/album/1".into()),
            ..album()
        };

        assert_eq!(
            build(&Tags::default(), false, &linked, &with_urls)
                .url
                .as_deref(),
            Some("https://www.qobuz.com/track/9")
        );
    }

    #[test]
    fn a_missing_or_blank_url_is_left_out() {
        assert_eq!(values(&Tags::default()).url, None);
    }

    #[test]
    fn the_comment_is_built_from_its_template_and_left_out_when_empty() {
        let described = Album {
            description: Some("<p>Great &amp; calm</p>".into()),
            ..album()
        };
        let mut tags = Tags {
            comment: true,
            comment_text: "Ripped: %Description%".into(),
            ..Tags::default()
        };

        assert_eq!(
            build(&tags, false, &described, &track()).comment.as_deref(),
            Some("Ripped: Great & calm")
        );

        tags.comment_text = "%Description%".into();
        assert_eq!(build(&tags, false, &album(), &track()).comment, None);

        tags.comment = false;
        assert_eq!(build(&tags, false, &described, &track()).comment, None);
    }
}
