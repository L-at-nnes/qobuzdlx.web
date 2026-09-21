use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::credits::{
    album_title, is_various_artists, joined_main_artists, parental_warning, track_title,
};
use super::quality::{
    AudioQuality, effective, format_name, format_with_hi_res_quality, format_with_quality,
};
use super::sanitize::{
    MAX_COMPONENT_BYTES, collapse_whitespace, is_inside, make_safe_filename, sanitize_path_segment,
    truncate_to_bytes,
};
use crate::qobuz::{Album, Playlist, Track};
use crate::settings::Settings;

/// Matches a `%Variable%`: no blank and no slash inside, so a stray percent sign is just text.
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%([^%\s/]+)%").expect("valid pattern"));

/// What is being downloaded, which decides where a track goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadKind {
    Album,
    /// A track downloaded on its own.
    Track,
    Playlist,
}

/// Builds folder and file names from the templates of the settings.
#[derive(Debug, Clone, Copy)]
pub struct Namer<'a> {
    settings: &'a Settings,
}

impl<'a> Namer<'a> {
    #[must_use]
    pub fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    /// Applies a template.
    ///
    /// Each `/`-separated component of the template is expanded, cleaned and cut to the length a
    /// filesystem accepts on its own, so a template can describe sub-folders while a `/` that comes
    /// from the data (`AC/DC`) never creates one. A variable that does not exist, or that needs
    /// something the context lacks (a track number in an album folder), is left as it was written.
    #[must_use]
    pub fn apply(
        self,
        template: &str,
        album: Option<&Album>,
        track: Option<&Track>,
        playlist: Option<&Playlist>,
    ) -> String {
        let context = Context {
            album,
            track,
            playlist,
        };

        template
            .split('/')
            .map(|component| self.apply_component(component, &context))
            .collect::<Vec<_>>()
            .join("/")
    }

    fn apply_component(self, component: &str, context: &Context<'_>) -> String {
        let expanded = PLACEHOLDER.replace_all(component, |captures: &Captures<'_>| {
            let name = captures[1].to_lowercase();
            self.resolve(&name, context)
                .unwrap_or_else(|| format!("%{name}%"))
        });

        let safe = make_safe_filename(&expanded).replace("...", "\u{2026}");
        truncate_to_bytes(&safe, MAX_COMPONENT_BYTES)
    }

    /// The artist as folders and the compilation check name the release.
    ///
    /// The main artists are only merged into one name when both merge options are on.
    #[must_use]
    pub fn release_artists(self, album: &Album) -> String {
        let naming = &self.settings.naming;
        if naming.merge_artists && naming.merge_artists_in_folders {
            if let Some(joined) = joined_main_artists(&album.artists) {
                return joined;
            }
        }
        album
            .artist
            .name
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_owned())
    }

    /// The name of a track's file, without its extension.
    ///
    /// A track of a compilation uses the compilation template, in which `%ArtistName%` stands for the
    /// artist of the track rather than the release's. The choice is made here and only here: every
    /// other template (artist, album, playlist folder) applies as written, otherwise a track of a
    /// compilation would turn the name of the playlist folder into a file name.
    #[must_use]
    pub fn track_file_stem(
        self,
        album: &Album,
        track: &Track,
        playlist: Option<&Playlist>,
    ) -> String {
        let naming = &self.settings.naming;

        let template = if is_various_artists(&self.release_artists(album)) {
            rename_variable(&naming.compilation_track, "artistname", "trackartist")
        } else {
            naming.track.clone()
        };
        self.apply(&template, Some(album), Some(track), playlist)
    }

    /// The folder a download goes to: the artist and album folders, or the playlist folder.
    ///
    /// Empty components are skipped (a template left blank is a choice), and the result never leaves
    /// `base`, whatever the templates hold.
    #[must_use]
    pub fn folder(
        self,
        base: &Path,
        album: Option<&Album>,
        track: Option<&Track>,
        playlist: Option<&Playlist>,
    ) -> PathBuf {
        let naming = &self.settings.naming;

        let mut folder = base.to_path_buf();
        match (playlist, album) {
            (None, Some(album)) => {
                for template in [&naming.artist, &naming.album] {
                    let component =
                        sanitize_path_segment(&self.apply(template, Some(album), None, None));
                    push_components(&mut folder, &component);
                }
            }
            (Some(playlist), _) if !naming.playlist.is_empty() => {
                let component = sanitize_path_segment(&self.apply(
                    &naming.playlist,
                    album,
                    track,
                    Some(playlist),
                ));
                push_components(&mut folder, &component);
            }
            _ => {}
        }

        if is_inside(base, &folder) {
            folder
        } else {
            base.to_path_buf()
        }
    }

    /// The file a track is written to, inside `folder`.
    ///
    /// Shared by the download and by the preview of the settings, so that what the preview shows is
    /// what the download does. `suffix` goes between the name and the extension (the id that tells
    /// two tracks with the same name apart).
    #[must_use]
    pub fn track_path(
        self,
        folder: &Path,
        kind: DownloadKind,
        album: &Album,
        track: &Track,
        playlist: Option<&Playlist>,
        suffix: &str,
    ) -> PathBuf {
        // The template is user input and may hold "/", ".." or a leading "/": every component is
        // cleaned like a folder name, so the file cannot leave the download folder.
        let mut name = sanitize_path_segment(&self.track_file_stem(album, track, playlist));
        if name.is_empty() {
            // A blank template still gives a usable name
            name = track.id.to_string();
        }
        name.push_str(suffix);
        name.push_str(self.settings.download.quality.extension());

        let mut path = folder.to_path_buf();
        if let Some(disc_folder) = self.disc_folder(kind, album, track) {
            push_components(&mut path, &disc_folder);
        }
        push_components(&mut path, &name);
        path
    }

    /// Multi-disc releases get one folder per disc, except for a lone track or a playlist entry.
    fn disc_folder(self, kind: DownloadKind, album: &Album, track: &Track) -> Option<String> {
        let template = &self.settings.naming.disc;
        let in_a_playlist = track.playlist_track_id.is_some();
        if in_a_playlist
            || kind == DownloadKind::Track
            || album.media_count <= 1
            || template.is_empty()
        {
            return None;
        }

        let folder = sanitize_path_segment(&replace_ignoring_case(
            template,
            "%DiscNumber%",
            &track.media_number.to_string(),
        ));
        (!folder.is_empty()).then_some(folder)
    }

    // ─── Variables ───

    fn resolve(self, name: &str, context: &Context<'_>) -> Option<String> {
        if let Some(value) = resolve_parental_warning(name, context) {
            return Some(value);
        }

        let settings = self.settings;
        let quality = settings.download.quality;
        let file_format = format_name(quality);

        let track_quality = context.track.map(|track| {
            effective(
                quality,
                track.maximum_bit_depth,
                track.maximum_sampling_rate,
            )
        });
        let album_quality = context.album.map(|album| {
            effective(
                quality,
                album.maximum_bit_depth,
                album.maximum_sampling_rate,
            )
        });

        let value = match name {
            // ── Track ──
            "trackid" => context.track?.id.to_string(),
            // Without a track (album and playlist folders) there is nothing to number a disc with
            "discnumber" => context
                .track
                .map_or_else(String::new, |track| track.media_number.to_string()),
            "trackcomposer" => name_of(context.track?.composer.as_ref()),
            "tracknumber" => {
                let width = self.track_number_width(context);
                format!("{:0>width$}", context.track?.track_number)
            }
            "isrc" => context.track?.isrc.clone().unwrap_or_default(),
            "trackbitdepth" => track_quality?.bit_depth.to_string(),
            "tracksamplerate" => track_quality?.sampling_rate.to_string(),
            "tracktitle" => track_title(context.track?),
            "trackartist" => name_of(context.track?.performer.as_ref()),
            "trackformat" => {
                context.track?;
                file_format.to_owned()
            }
            "trackformatwithquality" => format_with_quality(quality, track_quality?),
            "trackformatwithhiresquality" => format_with_hi_res_quality(quality, track_quality?),

            // ── Album ──
            "albumid" => context.album?.id.clone(),
            "albumurl" => context.album?.url.clone().unwrap_or_default(),
            "artistname" => self.release_artists(context.album?),
            "albumgenre" => context.album?.genre.name.clone(),
            "albumcomposer" => name_of(context.album?.composer.as_ref()),
            "label" => collapse_whitespace(&context.album?.label.name),
            "copyright" => context.album?.copyright.clone().unwrap_or_default(),
            "upc" => context.album?.upc.clone().unwrap_or_default(),
            "releasedate" => context
                .album?
                .release_date_original
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_owned(),
            "year" => year_of(context.album?),
            "releasetype" => capitalised(&context.album?.product_type),
            "bitdepth" => album_quality?.bit_depth.to_string(),
            "samplerate" => album_quality?.sampling_rate.to_string(),
            "albumtitle" => album_title(context.album?),

            // ── Album or playlist ──
            "format" => {
                if context.album.is_none() && context.playlist.is_none() {
                    return None;
                }
                file_format.to_owned()
            }
            "formatwithquality" => self.format_of_release(context, album_quality, false)?,
            "formatwithhiresquality" => self.format_of_release(context, album_quality, true)?,

            // ── Playlist ──
            "playlistid" => context.playlist?.id.to_string(),
            "playlisttitle" => context.playlist?.name.clone(),

            _ => return None,
        };
        Some(value)
    }

    /// `%FormatWithQuality%` of an album folder; of a playlist folder, the album's quality when the
    /// track's album is known, otherwise just the format.
    fn format_of_release(
        self,
        context: &Context<'_>,
        album_quality: Option<AudioQuality>,
        hi_res_only: bool,
    ) -> Option<String> {
        let quality = self.settings.download.quality;
        match album_quality {
            Some(audio) if hi_res_only => Some(format_with_hi_res_quality(quality, audio)),
            Some(audio) => Some(format_with_quality(quality, audio)),
            None if context.playlist.is_some() => Some(format_name(quality).to_owned()),
            None => None,
        }
    }

    /// How many digits a track number is padded to: the size of what the tracks are numbered in.
    ///
    /// With playlist positions the number is a rank in the playlist, not in the album the track comes
    /// from: padding to the album's size would sort `7` before `35` but after `120`.
    fn track_number_width(self, context: &Context<'_>) -> usize {
        let numbered = match context.playlist {
            Some(playlist)
                if self.settings.advanced.playlist_positions && playlist.tracks_count > 0 =>
            {
                Some(playlist.tracks_count)
            }
            _ => context.album.map(|album| album.tracks_count),
        };
        numbered.map_or(2, |count| count.to_string().len())
    }
}

/// The `%TrackParental…%` and `%AlbumParental…%` variables, each following the flag of its own scope.
fn resolve_parental_warning(name: &str, context: &Context<'_>) -> Option<String> {
    let (suffix, explicit) = match name.strip_prefix("track") {
        Some(suffix) => (suffix, context.track?.parental_warning),
        None => (name.strip_prefix("album")?, context.album?.parental_warning),
    };

    parental_warning(suffix, explicit).map(str::to_owned)
}

/// What a template is applied to.
struct Context<'a> {
    album: Option<&'a Album>,
    track: Option<&'a Track>,
    playlist: Option<&'a Playlist>,
}

fn name_of(person: Option<&crate::qobuz::Person>) -> String {
    person
        .and_then(|person| person.name.clone())
        .unwrap_or_default()
}

fn year_of(album: &Album) -> String {
    album
        .release_date_original
        .as_deref()
        .map(str::trim)
        .and_then(|date| date.get(..4))
        .unwrap_or_default()
        .to_owned()
}

/// `album` to `Album`, `SINGLE` to `Single`.
fn capitalised(word: &str) -> String {
    let mut characters = word.chars();
    characters
        .next()
        .map(|first| {
            first
                .to_uppercase()
                .chain(characters.flat_map(char::to_lowercase))
                .collect()
        })
        .unwrap_or_default()
}

fn push_components(path: &mut PathBuf, components: &str) {
    for component in components
        .split('/')
        .filter(|component| !component.is_empty())
    {
        path.push(component);
    }
}

/// Renames a variable of a template, whatever the case it was written in.
fn rename_variable(template: &str, from: &str, to: &str) -> String {
    PLACEHOLDER
        .replace_all(template, |captures: &Captures<'_>| {
            if captures[1].eq_ignore_ascii_case(from) {
                format!("%{to}%")
            } else {
                captures[0].to_owned()
            }
        })
        .into_owned()
}

pub(super) fn replace_ignoring_case(text: &str, pattern: &str, replacement: &str) -> String {
    let lowered = text.to_lowercase();
    let pattern = pattern.to_lowercase();

    let mut result = String::with_capacity(text.len());
    let mut from = 0;
    while let Some(found) = lowered[from..].find(&pattern) {
        result.push_str(&text[from..from + found]);
        result.push_str(replacement);
        from += found + pattern.len();
    }
    result.push_str(&text[from..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qobuz::{ArtistRole, Person};
    use crate::settings::Quality;

    fn settings() -> Settings {
        Settings::default()
    }

    fn album() -> Album {
        Album {
            id: "1".into(),
            title: "Alb".into(),
            artist: Person {
                id: 1,
                name: Some("Art".into()),
            },
            tracks_count: 12,
            media_count: 1,
            ..Album::default()
        }
    }

    fn track() -> Track {
        Track {
            id: 77,
            title: "Song".into(),
            track_number: 3,
            media_number: 1,
            ..Track::default()
        }
    }

    fn apply(settings: &Settings, template: &str, album: &Album, track: &Track) -> String {
        Namer::new(settings).apply(template, Some(album), Some(track), None)
    }

    // ─── Variables ───

    #[test]
    fn variables_are_expanded_whatever_their_case() {
        let name = apply(
            &settings(),
            "%TRACKNUMBER% - %tracktitle% - %ArtistName%",
            &album(),
            &track(),
        );

        assert_eq!(name, "03 - Song - Art");
    }

    #[test]
    fn an_unknown_variable_is_kept_as_written_in_lowercase() {
        assert_eq!(
            apply(
                &settings(),
                "%TrackNumbre% - %TrackTitle%",
                &album(),
                &track()
            ),
            "%tracknumbre% - Song"
        );
    }

    #[test]
    fn a_track_variable_in_an_album_folder_is_not_resolved() {
        let name =
            Namer::new(&settings()).apply("%TrackNumber% %AlbumTitle%", Some(&album()), None, None);

        assert_eq!(name, "%tracknumber% Alb");
    }

    #[test]
    fn the_disc_number_works_with_a_track_and_is_empty_without_one() {
        let namer_settings = settings();
        let multi_disc_track = Track {
            media_number: 2,
            ..track()
        };

        assert_eq!(
            apply(
                &namer_settings,
                "%DiscNumber%-%TrackNumber%",
                &album(),
                &multi_disc_track
            ),
            "2-03"
        );
        assert_eq!(
            Namer::new(&namer_settings).apply(
                "[%DiscNumber%] %AlbumTitle%",
                Some(&album()),
                None,
                None
            ),
            "[] Alb"
        );
    }

    #[test]
    fn the_track_number_is_padded_to_the_size_of_the_album() {
        let small = Album {
            tracks_count: 9,
            ..album()
        };
        let big = Album {
            tracks_count: 120,
            ..album()
        };

        assert_eq!(apply(&settings(), "%TrackNumber%", &small, &track()), "3");
        assert_eq!(
            apply(&settings(), "%TrackNumber%", &album(), &track()),
            "03"
        );
        assert_eq!(apply(&settings(), "%TrackNumber%", &big, &track()), "003");
    }

    #[test]
    fn with_playlist_positions_the_number_is_padded_to_the_size_of_the_playlist() {
        let mut positions = settings();
        positions.advanced.playlist_positions = true;
        let playlist = Playlist {
            id: 5,
            name: "Mix".into(),
            tracks_count: 120,
            ..Playlist::default()
        };
        let ranked = Track {
            track_number: 7,
            ..track()
        };

        let by_album = Namer::new(&settings()).apply(
            "%TrackNumber%",
            Some(&album()),
            Some(&ranked),
            Some(&playlist),
        );
        let by_position = Namer::new(&positions).apply(
            "%TrackNumber%",
            Some(&album()),
            Some(&ranked),
            Some(&playlist),
        );

        assert_eq!(by_album, "07");
        assert_eq!(by_position, "007");
    }

    #[test]
    fn the_year_and_the_release_type_come_from_the_album() {
        let dated = Album {
            release_date_original: Some(" 2013-05-17 ".into()),
            product_type: "SINGLE".into(),
            ..album()
        };

        assert_eq!(
            Namer::new(&settings()).apply(
                "%Year% %ReleaseDate% %ReleaseType%",
                Some(&dated),
                None,
                None
            ),
            "2013 2013-05-17 Single"
        );
    }

    #[test]
    fn a_version_is_part_of_the_titles() {
        let versioned = Album {
            version: Some("Deluxe".into()),
            ..album()
        };
        let live = Track {
            version: Some("Live".into()),
            ..track()
        };

        assert_eq!(
            apply(
                &settings(),
                "%AlbumTitle% - %TrackTitle%",
                &versioned,
                &live
            ),
            "Alb (Deluxe) - Song (Live)"
        );
    }

    #[test]
    fn a_slash_in_the_data_never_makes_a_folder() {
        let acdc = Album {
            artist: Person {
                id: 2,
                name: Some("AC/DC".into()),
            },
            ..album()
        };

        assert_eq!(
            Namer::new(&settings()).apply("%ArtistName%", Some(&acdc), None, None),
            "AC-DC"
        );
    }

    #[test]
    fn a_slash_in_the_template_separates_components() {
        assert_eq!(
            Namer::new(&settings()).apply("%ArtistName%/%AlbumTitle%", Some(&album()), None, None),
            "Art/Alb"
        );
    }

    #[test]
    fn three_dots_become_an_ellipsis() {
        let dotted = Album {
            title: "Wait... What".into(),
            ..album()
        };

        assert_eq!(
            Namer::new(&settings()).apply("%AlbumTitle%", Some(&dotted), None, None),
            "Wait\u{2026} What"
        );
    }

    #[test]
    fn each_component_is_cut_on_its_own() {
        let long = Album {
            title: "b".repeat(400),
            ..album()
        };

        let parts: Vec<String> = Namer::new(&settings())
            .apply("%ArtistName%/%AlbumTitle%", Some(&long), None, None)
            .split('/')
            .map(str::to_owned)
            .collect();

        assert_eq!(parts[0], "Art");
        assert!(parts[1].len() <= MAX_COMPONENT_BYTES);
    }

    // ─── Quality ───

    #[test]
    fn the_quality_variables_describe_the_downloaded_quality() {
        let hi_res = Album {
            maximum_bit_depth: 24,
            maximum_sampling_rate: 96.0,
            ..album()
        };
        let mut flac16 = settings();
        flac16.download.quality = Quality::Flac16;

        assert_eq!(
            Namer::new(&flac16).apply("%FormatWithQuality%/%BitDepth%", Some(&hi_res), None, None),
            "FLAC (16bit-44.1kHz)/16"
        );
        assert_eq!(
            Namer::new(&settings()).apply("%FormatWithQuality%", Some(&hi_res), None, None),
            "FLAC (24bit-96kHz)"
        );
    }

    #[test]
    fn a_playlist_folder_without_an_album_just_names_the_format() {
        let playlist = Playlist {
            id: 1,
            name: "Mix".into(),
            ..Playlist::default()
        };

        assert_eq!(
            Namer::new(&settings()).apply(
                "%PlaylistTitle% [%FormatWithQuality%]",
                None,
                None,
                Some(&playlist)
            ),
            "Mix [FLAC]"
        );
    }

    // ─── Parental warning ───

    #[test]
    fn the_album_and_track_parental_tags_follow_their_own_flag() {
        let clean_album = album();
        let explicit_track = Track {
            parental_warning: true,
            ..track()
        };
        let explicit_album = Album {
            parental_warning: true,
            ..album()
        };
        let clean_track = track();

        assert_eq!(
            apply(
                &settings(),
                "%TrackPA%/%AlbumPA%",
                &clean_album,
                &explicit_track
            ),
            "Explicit/Clean"
        );
        assert_eq!(
            apply(
                &settings(),
                "%TrackPA%/%AlbumPA%",
                &explicit_album,
                &clean_track
            ),
            "Clean/Explicit"
        );
    }

    #[test]
    fn an_album_parental_tag_works_without_a_track() {
        let explicit = Album {
            parental_warning: true,
            ..album()
        };

        assert_eq!(
            Namer::new(&settings()).apply("%AlbumPAShort%", Some(&explicit), None, None),
            "E"
        );
    }

    // ─── Compilations ───

    fn compilation() -> Album {
        Album {
            artist: Person {
                id: 2,
                name: Some("Various Artists".into()),
            },
            ..album()
        }
    }

    #[test]
    fn a_compilation_track_uses_the_compilation_template_with_the_track_artist() {
        let performer = Track {
            performer: Some(Person {
                id: 9,
                name: Some("Nina".into()),
            }),
            ..track()
        };

        assert_eq!(
            Namer::new(&settings()).track_file_stem(&compilation(), &performer, None),
            "03 - Nina - Song"
        );
    }

    #[test]
    fn the_compilation_template_does_not_leak_into_the_playlist_folder() {
        // Found live: the folder of a playlist became "1 - Artist - Title"
        let playlist = Playlist {
            id: 5,
            name: "My Mix".into(),
            ..Playlist::default()
        };
        let performer = Track {
            performer: Some(Person {
                id: 9,
                name: Some("Nina".into()),
            }),
            ..track()
        };

        let folder = Namer::new(&settings()).folder(
            Path::new("/dl"),
            Some(&compilation()),
            Some(&performer),
            Some(&playlist),
        );

        assert_eq!(folder, Path::new("/dl/My Mix"));
    }

    #[test]
    fn a_slash_in_the_compilation_template_makes_a_sub_folder_like_in_the_others() {
        let mut slashed = settings();
        slashed.naming.compilation_track = "%TrackArtist%/%TrackTitle%".into();
        let performer = Track {
            performer: Some(Person {
                id: 9,
                name: Some("Nina".into()),
            }),
            ..track()
        };

        assert_eq!(
            Namer::new(&slashed).track_file_stem(&compilation(), &performer, None),
            "Nina/Song"
        );
    }

    // ─── Release artist ───

    fn duet() -> Album {
        Album {
            artist: Person {
                id: 1,
                name: Some("John Coltrane".into()),
            },
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
            ..album()
        }
    }

    #[test]
    fn the_release_artist_is_merged_only_when_both_options_are_on() {
        let mut merged = settings();
        merged.naming.merge_artists = true;
        assert_eq!(
            Namer::new(&merged).release_artists(&duet()),
            "John Coltrane"
        );

        merged.naming.merge_artists_in_folders = true;
        assert_eq!(
            Namer::new(&merged).release_artists(&duet()),
            "John Coltrane & Johnny Hartman"
        );
        assert_eq!(
            Namer::new(&settings()).release_artists(&duet()),
            "John Coltrane"
        );
    }

    #[test]
    fn a_release_without_an_artist_is_named_unknown() {
        let nameless = Album {
            artist: Person::default(),
            ..album()
        };

        assert_eq!(
            Namer::new(&settings()).release_artists(&nameless),
            "Unknown Artist"
        );
    }

    // ─── Folders ───

    #[test]
    fn an_album_goes_into_an_artist_folder_then_an_album_folder() {
        assert_eq!(
            Namer::new(&settings()).folder(Path::new("/dl"), Some(&album()), None, None),
            Path::new("/dl/Art/Alb")
        );
    }

    #[test]
    fn a_blank_folder_template_is_skipped() {
        let mut flat = settings();
        flat.naming.artist = String::new();

        assert_eq!(
            Namer::new(&flat).folder(Path::new("/dl"), Some(&album()), None, None),
            Path::new("/dl/Alb")
        );

        flat.naming.album = "  ".into();
        assert_eq!(
            Namer::new(&flat).folder(Path::new("/dl"), Some(&album()), None, None),
            Path::new("/dl")
        );
    }

    #[test]
    fn a_playlist_goes_into_its_own_folder() {
        let playlist = Playlist {
            id: 5,
            name: "Morning: Jazz".into(),
            ..Playlist::default()
        };

        assert_eq!(
            Namer::new(&settings()).folder(
                Path::new("/dl"),
                Some(&album()),
                Some(&track()),
                Some(&playlist)
            ),
            Path::new("/dl/Morning - Jazz")
        );
    }

    #[test]
    fn a_hostile_template_cannot_leave_the_download_folder() {
        for hostile in ["../../etc/%AlbumTitle%", "/etc/%AlbumTitle%", "a/../../b"] {
            let mut tricky = settings();
            tricky.naming.album = hostile.into();

            let folder = Namer::new(&tricky).folder(Path::new("/dl"), Some(&album()), None, None);

            assert!(
                is_inside(Path::new("/dl"), &folder),
                "{hostile}: {}",
                folder.display()
            );
        }
    }

    #[test]
    fn an_album_titled_with_two_dots_stays_inside_the_download_folder() {
        let dots = Album {
            title: "..".into(),
            ..album()
        };

        let folder = Namer::new(&settings()).folder(Path::new("/dl"), Some(&dots), None, None);

        assert!(is_inside(Path::new("/dl"), &folder));
        assert_ne!(
            folder.file_name().and_then(|name| name.to_str()),
            Some("..")
        );
    }

    // ─── Track files ───

    fn path(
        settings: &Settings,
        kind: DownloadKind,
        album: &Album,
        track: &Track,
        playlist: Option<&Playlist>,
    ) -> PathBuf {
        Namer::new(settings).track_path(Path::new("/dl/Art/Alb"), kind, album, track, playlist, "")
    }

    #[test]
    fn a_track_file_is_named_after_the_template_with_the_extension_of_the_quality() {
        assert_eq!(
            path(&settings(), DownloadKind::Album, &album(), &track(), None),
            Path::new("/dl/Art/Alb/03 - Song.flac")
        );

        let mut mp3 = settings();
        mp3.download.quality = Quality::Mp3;
        assert_eq!(
            path(&mp3, DownloadKind::Album, &album(), &track(), None),
            Path::new("/dl/Art/Alb/03 - Song.mp3")
        );
    }

    #[test]
    fn a_multi_disc_album_gets_a_folder_per_disc() {
        let two_discs = Album {
            media_count: 2,
            ..album()
        };
        let on_disc_two = Track {
            media_number: 2,
            ..track()
        };

        assert_eq!(
            path(
                &settings(),
                DownloadKind::Album,
                &two_discs,
                &on_disc_two,
                None
            ),
            Path::new("/dl/Art/Alb/CD 2/03 - Song.flac")
        );
    }

    #[test]
    fn a_lone_track_or_a_playlist_entry_gets_no_disc_folder() {
        let two_discs = Album {
            media_count: 2,
            ..album()
        };
        let on_disc_two = Track {
            media_number: 2,
            ..track()
        };
        let entry = Track {
            playlist_track_id: Some(5),
            ..on_disc_two.clone()
        };
        let playlist = Playlist {
            id: 1,
            name: "P".into(),
            ..Playlist::default()
        };

        let expected = Path::new("/dl/Art/Alb/03 - Song.flac");
        assert_eq!(
            path(
                &settings(),
                DownloadKind::Track,
                &two_discs,
                &on_disc_two,
                None
            ),
            expected
        );
        assert_eq!(
            path(
                &settings(),
                DownloadKind::Playlist,
                &two_discs,
                &entry,
                Some(&playlist)
            ),
            expected
        );
    }

    #[test]
    fn the_disc_folder_follows_its_template_or_is_disabled_by_an_empty_one() {
        let two_discs = Album {
            media_count: 2,
            ..album()
        };
        let on_disc_two = Track {
            media_number: 2,
            ..track()
        };

        let mut custom = settings();
        custom.naming.disc = "Disc %discnumber%".into();
        assert_eq!(
            path(&custom, DownloadKind::Album, &two_discs, &on_disc_two, None),
            Path::new("/dl/Art/Alb/Disc 2/03 - Song.flac")
        );

        custom.naming.disc = String::new();
        assert_eq!(
            path(&custom, DownloadKind::Album, &two_discs, &on_disc_two, None),
            Path::new("/dl/Art/Alb/03 - Song.flac")
        );
    }

    #[test]
    fn a_hostile_track_template_cannot_leave_the_album_folder() {
        for hostile in [
            "../../etc/%TrackTitle%",
            "/etc/%TrackTitle%",
            "..\\..\\x",
            "a/../../b/%TrackTitle%",
        ] {
            let mut tricky = settings();
            tricky.naming.track = hostile.into();

            let file = path(&tricky, DownloadKind::Album, &album(), &track(), None);

            assert!(
                is_inside(Path::new("/dl/Art/Alb"), &file),
                "{hostile}: {}",
                file.display()
            );
            assert!(file.to_string_lossy().ends_with(".flac"));
        }
    }

    #[test]
    fn a_blank_track_template_still_gives_a_usable_name() {
        let mut blank = settings();
        blank.naming.track = String::new();

        assert_eq!(
            path(&blank, DownloadKind::Album, &album(), &track(), None),
            Path::new("/dl/Art/Alb/77.flac")
        );
    }

    #[test]
    fn a_slash_in_the_track_template_makes_a_sub_folder() {
        let mut nested = settings();
        nested.naming.track = "%TrackNumber%/%TrackTitle%".into();

        assert_eq!(
            path(&nested, DownloadKind::Album, &album(), &track(), None),
            Path::new("/dl/Art/Alb/03/Song.flac")
        );
    }

    #[test]
    fn the_suffix_goes_before_the_extension() {
        let named = Namer::new(&settings()).track_path(
            Path::new("/dl"),
            DownloadKind::Album,
            &album(),
            &track(),
            None,
            " (77)",
        );

        assert_eq!(named, Path::new("/dl/03 - Song (77).flac"));
    }

    #[test]
    fn a_very_long_title_still_fits_with_its_extension_and_a_rename_suffix() {
        let long = Track {
            title: "c".repeat(400),
            ..track()
        };

        let stem = Namer::new(&settings()).track_file_stem(&album(), &long, None);

        assert!(stem.len() + ".flac".len() + " (99)".len() <= 255);
    }

    #[test]
    fn replacing_a_pattern_ignores_case_and_replaces_every_occurrence() {
        assert_eq!(
            replace_ignoring_case("CD %DISCNUMBER% / %discnumber%", "%DiscNumber%", "2"),
            "CD 2 / 2"
        );
        assert_eq!(replace_ignoring_case("nothing", "%x%", "y"), "nothing");
    }
}
