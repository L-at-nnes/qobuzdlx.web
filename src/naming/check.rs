//! Showing what the templates produce and pointing out what is wrong with them, without having to
//! start a download to find out.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::namer::{DownloadKind, Namer, replace_ignoring_case};
use crate::qobuz::{Album, ArtistRole, Genre, Label, Person, Playlist, Track};
use crate::settings::Settings;

/// A `%tag%` still standing once a template was applied to the sample: unknown, misspelt, or not
/// available in that kind of template (`%TrackNumber%` in an album folder, say).
static UNRESOLVED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%[^%\s/]+%").expect("valid pattern"));

static DRIVE_LETTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z]:").expect("valid pattern"));

static VARIABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"%[^%]*%").expect("valid pattern"));

/// The characters a name cleaner rewrites. A backslash is not a separator: only `/` is.
const REPLACED_CHARACTERS: [char; 8] = ['\\', ':', '*', '?', '"', '<', '>', '|'];

/// The paths the templates produce for an invented release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preview {
    /// A track of an album, on its second disc.
    pub album: String,
    /// A track of a compilation.
    pub compilation: String,
    /// A track of a playlist.
    pub playlist: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateField {
    Artist,
    Album,
    Track,
    CompilationTrack,
    Playlist,
    Disc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    /// A file name template cannot be empty: the files would have no name.
    Empty,
    /// The template starts at the root of a filesystem or a drive.
    Rooted,
    /// The template climbs out of its folder with `..`.
    Parent,
    /// Characters that will be replaced in names.
    IllegalCharacters,
    /// A `%variable%` that is unknown or not available here.
    UnknownVariable,
}

/// A problem with one of the templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub field: TemplateField,
    pub kind: IssueKind,
    /// What exactly: the characters, or the variables, concerned.
    pub detail: Option<String>,
}

// ─── The invented release the templates are tried on ───

/// Big enough to exercise every template: two discs, a version in the title, hi-res.
fn sample_album() -> Album {
    Album {
        id: "0123456789012".into(),
        title: "Blue Hour".into(),
        artist: Person {
            id: 1,
            name: Some("Nina Alvarez".into()),
        },
        artists: vec![ArtistRole {
            id: 1,
            name: "Nina Alvarez".into(),
            roles: vec!["main-artist".into()],
        }],
        genre: Genre {
            name: "Jazz".into(),
        },
        label: Label {
            id: 1,
            name: "Northern Lights".into(),
            ..Label::default()
        },
        release_date_original: Some("2013-05-17".into()),
        upc: Some("0123456789012".into()),
        copyright: Some("(P) 2013 Northern Lights".into()),
        tracks_count: 14,
        media_count: 2,
        maximum_bit_depth: 24,
        maximum_sampling_rate: 96.0,
        ..Album::default()
    }
}

fn sample_compilation() -> Album {
    Album {
        id: "0123456789999".into(),
        title: "Summer Mix".into(),
        artist: Person {
            id: 2,
            name: Some("Various Artists".into()),
        },
        genre: Genre {
            name: "Jazz".into(),
        },
        label: Label {
            id: 1,
            name: "Northern Lights".into(),
            ..Label::default()
        },
        release_date_original: Some("2020-06-01".into()),
        tracks_count: 20,
        media_count: 1,
        maximum_bit_depth: 16,
        maximum_sampling_rate: 44.1,
        ..Album::default()
    }
}

fn sample_playlist() -> Playlist {
    Playlist {
        id: 99,
        name: "Morning Playlist".into(),
        tracks_count: 30,
        ..Playlist::default()
    }
}

fn sample_track(track_number: u32, media_number: u32) -> Track {
    Track {
        id: 4242,
        title: "Midnight Garden".into(),
        version: Some("Live".into()),
        track_number,
        media_number,
        position: 3,
        performer: Some(Person {
            id: 1,
            name: Some("Nina Alvarez".into()),
        }),
        composer: Some(Person {
            id: 2,
            name: Some("R. Castillo".into()),
        }),
        isrc: Some("XX0000000001".into()),
        maximum_bit_depth: 24,
        maximum_sampling_rate: 96.0,
        ..Track::default()
    }
}

/// The path a track of an album, of a compilation and of a playlist would get with these settings.
#[must_use]
pub fn preview(settings: &Settings) -> Preview {
    let namer = Namer::new(settings);
    let base = settings.download.directory.as_path();

    let path_of =
        |kind: DownloadKind, album: &Album, track: &Track, playlist: Option<&Playlist>| {
            // The same calls, in the same order, as the download itself
            let folder = namer.folder(base, Some(album), playlist.map(|_| track), playlist);
            display(&namer.track_path(&folder, kind, album, track, playlist, ""))
        };

    // The download renumbers the tracks of a playlist by their position when asked to
    let mut playlist_track = Track {
        playlist_track_id: Some(1),
        ..sample_track(5, 2)
    };
    if settings.advanced.playlist_positions {
        playlist_track.track_number = playlist_track.position;
    }

    Preview {
        album: path_of(
            DownloadKind::Album,
            &sample_album(),
            &sample_track(5, 2),
            None,
        ),
        compilation: path_of(
            DownloadKind::Album,
            &sample_compilation(),
            &sample_track(7, 1),
            None,
        ),
        playlist: path_of(
            DownloadKind::Playlist,
            &sample_album(),
            &playlist_track,
            Some(&sample_playlist()),
        ),
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The problems with the templates as set, each pointing at a field. Empty when all is well.
#[must_use]
pub fn validate(settings: &Settings) -> Vec<Issue> {
    let naming = &settings.naming;
    let namer = Namer::new(settings);
    let album = sample_album();
    let track = sample_track(5, 2);
    let playlist = sample_playlist();

    let mut issues = Vec::new();
    let mut check = |field, template: &str, required: bool, resolved: String| {
        check_template(&mut issues, field, template, required, &resolved);
    };

    check(
        TemplateField::Artist,
        &naming.artist,
        false,
        namer.apply(&naming.artist, Some(&album), None, None),
    );
    check(
        TemplateField::Album,
        &naming.album,
        false,
        namer.apply(&naming.album, Some(&album), None, None),
    );
    check(
        TemplateField::Track,
        &naming.track,
        true,
        namer.apply(&naming.track, Some(&album), Some(&track), Some(&playlist)),
    );
    check(
        TemplateField::CompilationTrack,
        &naming.compilation_track,
        true,
        namer.apply(
            &naming.compilation_track,
            Some(&album),
            Some(&track),
            Some(&playlist),
        ),
    );
    check(
        TemplateField::Playlist,
        &naming.playlist,
        false,
        namer.apply(
            &naming.playlist,
            Some(&album),
            Some(&track),
            Some(&playlist),
        ),
    );
    // The only variable of a disc folder is the disc number
    check(
        TemplateField::Disc,
        &naming.disc,
        false,
        replace_ignoring_case(&naming.disc, "%DiscNumber%", "2"),
    );

    issues
}

fn check_template(
    issues: &mut Vec<Issue>,
    field: TemplateField,
    template: &str,
    required: bool,
    resolved: &str,
) {
    if template.trim().is_empty() {
        // A folder template left blank is a choice (no such folder); a file name cannot be
        if required {
            issues.push(Issue {
                field,
                kind: IssueKind::Empty,
                detail: None,
            });
        }
        return;
    }

    let trimmed = template.trim();
    if trimmed.starts_with(['/', '\\']) || DRIVE_LETTER.is_match(trimmed) {
        issues.push(Issue {
            field,
            kind: IssueKind::Rooted,
            detail: None,
        });
    }

    if trimmed.split(['/', '\\']).any(|part| part.trim() == "..") {
        issues.push(Issue {
            field,
            kind: IssueKind::Parent,
            detail: None,
        });
    }

    let without_variables = VARIABLE.replace_all(template, "");
    let mut replaced: Vec<char> = Vec::new();
    for character in without_variables
        .chars()
        .filter(|c| REPLACED_CHARACTERS.contains(c))
    {
        if !replaced.contains(&character) {
            replaced.push(character);
        }
    }
    if !replaced.is_empty() {
        let list = replaced
            .iter()
            .map(char::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        issues.push(Issue {
            field,
            kind: IssueKind::IllegalCharacters,
            detail: Some(list),
        });
    }

    let mut unresolved: Vec<&str> = Vec::new();
    for variable in UNRESOLVED.find_iter(resolved).map(|found| found.as_str()) {
        if !unresolved.contains(&variable) {
            unresolved.push(variable);
        }
    }
    if !unresolved.is_empty() {
        issues.push(Issue {
            field,
            kind: IssueKind::UnknownVariable,
            detail: Some(unresolved.join(" ")),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{DownloadSettings, Naming};

    fn settings() -> Settings {
        Settings {
            download: DownloadSettings {
                directory: "/downloads".into(),
                ..DownloadSettings::default()
            },
            ..Settings::default()
        }
    }

    fn with(change: impl FnOnce(&mut Naming)) -> Vec<Issue> {
        let mut settings = settings();
        change(&mut settings.naming);
        validate(&settings)
    }

    fn only(issues: &[Issue]) -> &Issue {
        assert_eq!(issues.len(), 1, "{issues:?}");
        &issues[0]
    }

    // ─── Preview ───

    #[test]
    fn the_preview_shows_an_album_a_compilation_and_a_playlist_track() {
        let preview = preview(&settings());

        assert_eq!(
            preview.album,
            "/downloads/Nina Alvarez/Blue Hour/CD 2/05 - Midnight Garden (Live).flac"
        );
        assert_eq!(
            preview.compilation,
            "/downloads/Various Artists/Summer Mix/07 - Nina Alvarez - Midnight Garden (Live).flac"
        );
        assert_eq!(
            preview.playlist,
            "/downloads/Morning Playlist/05 - Midnight Garden (Live).flac"
        );
    }

    #[test]
    fn the_preview_follows_the_settings() {
        let mut changed = settings();
        changed.naming.track = "%TrackTitle%".into();
        changed.naming.album = "%Year% - %AlbumTitle% [%FormatWithQuality%]".into();

        assert_eq!(
            preview(&changed).album,
            "/downloads/Nina Alvarez/2013 - Blue Hour [FLAC (24bit-96kHz)]/CD 2/Midnight Garden (Live).flac"
        );
    }

    #[test]
    fn the_playlist_preview_follows_the_position_option() {
        let mut by_position = settings();
        by_position.advanced.playlist_positions = true;

        assert_eq!(
            preview(&by_position).playlist,
            "/downloads/Morning Playlist/03 - Midnight Garden (Live).flac"
        );
        assert_eq!(
            preview(&by_position).album,
            preview(&settings()).album,
            "albums are unaffected"
        );
    }

    // ─── Validation ───

    #[test]
    fn the_default_templates_raise_no_issue() {
        assert!(validate(&settings()).is_empty());
    }

    #[test]
    fn an_empty_file_name_template_is_reported_but_empty_folder_templates_are_a_choice() {
        let issues = with(|naming| naming.track = "  ".into());
        assert_eq!(
            only(&issues),
            &Issue {
                field: TemplateField::Track,
                kind: IssueKind::Empty,
                detail: None
            }
        );

        let none = with(|naming| {
            naming.artist.clear();
            naming.album.clear();
            naming.playlist.clear();
            naming.disc.clear();
        });
        assert!(none.is_empty());
    }

    #[test]
    fn a_template_starting_at_the_root_is_reported() {
        assert!(
            with(|n| n.album = "/music/%AlbumTitle%".into())
                .iter()
                .any(|i| i.field == TemplateField::Album && i.kind == IssueKind::Rooted)
        );
        assert!(
            with(|n| n.artist = "C:/music".into())
                .iter()
                .any(|i| i.kind == IssueKind::Rooted)
        );
    }

    #[test]
    fn a_template_climbing_out_of_its_folder_is_reported() {
        let issues = with(|n| n.track = "../%TrackTitle%".into());

        assert!(
            issues
                .iter()
                .any(|i| i.field == TemplateField::Track && i.kind == IssueKind::Parent)
        );
    }

    #[test]
    fn a_misspelt_variable_is_reported_by_name() {
        let issues = with(|n| n.track = "%TrackNumbre% - %TrackTitle%".into());

        let issue = only(&issues);
        assert_eq!(issue.kind, IssueKind::UnknownVariable);
        assert_eq!(issue.detail.as_deref(), Some("%tracknumbre%"));
    }

    #[test]
    fn a_track_variable_in_a_folder_template_is_reported() {
        let issues = with(|n| n.album = "%TrackNumber% %AlbumTitle%".into());

        let issue = only(&issues);
        assert_eq!(
            (issue.field, issue.kind),
            (TemplateField::Album, IssueKind::UnknownVariable)
        );
        assert_eq!(issue.detail.as_deref(), Some("%tracknumber%"));
    }

    #[test]
    fn a_playlist_variable_is_fine_in_the_playlist_template_and_the_disc_number_in_the_disc_one() {
        assert!(with(|n| n.playlist = "%PlaylistTitle% (%PlaylistID%)".into()).is_empty());
        assert!(with(|n| n.disc = "Disc %DiscNumber%".into()).is_empty());
    }

    #[test]
    fn the_disc_template_knows_nothing_but_the_disc_number() {
        let issues = with(|n| n.disc = "%AlbumTitle% %DiscNumber%".into());

        assert!(
            issues
                .iter()
                .any(|i| i.field == TemplateField::Disc && i.kind == IssueKind::UnknownVariable)
        );
    }

    #[test]
    fn characters_that_will_be_rewritten_are_listed_once_each() {
        let issues = with(|n| n.track = "%TrackNumber%: %TrackTitle%?:".into());

        let issue = only(&issues);
        assert_eq!(issue.kind, IssueKind::IllegalCharacters);
        assert_eq!(issue.detail.as_deref(), Some(": ?"));
    }

    #[test]
    fn every_parental_warning_variable_resolves() {
        assert!(
            with(|n| n.track = "%TrackTitle% %TrackPAifExEnclosed[]% %AlbumPAShort%".into())
                .is_empty()
        );
    }

    #[test]
    fn several_issues_accumulate() {
        assert!(with(|n| n.track = "/x/%Oops%?".into()).len() >= 3);
    }
}
