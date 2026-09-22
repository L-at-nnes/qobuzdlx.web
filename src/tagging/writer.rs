use std::fs::File;
use std::path::Path;

use lofty::TextEncoding;
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::flac::FlacFile;
use lofty::id3::v2::{ExtendedUrlFrame, Frame, Id3v2Tag};
use lofty::mpeg::MpegFile;
use lofty::ogg::{OggPictureStorage, VorbisComments};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, ItemKey, Tag, TagType};

use super::TagValues;

/// The description ID3 gives the URL frame, and the name of the matching Vorbis field.
const URL_KEY: &str = "Qobuz URL";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read or write the file: {0}")]
    Io(#[from] std::io::Error),

    #[error("cannot tag the file: {0}")]
    Tag(#[from] lofty::error::LoftyError),

    #[error("only FLAC and MP3 files can be tagged, not {0:?}")]
    UnsupportedFormat(String),
}

/// Writes the tags and the cover into an audio file, whose format is read from its extension.
///
/// Tags that are not part of `values` are left as they are in the file.
pub fn write(path: &Path, values: &TagValues, cover: Option<&[u8]>) -> Result<(), Error> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "flac" => write_flac(path, values, cover),
        "mp3" => write_mp3(path, values, cover),
        other => Err(Error::UnsupportedFormat(other.to_owned())),
    }
}

fn jpeg_cover(data: &[u8]) -> Picture {
    Picture::unchecked(data.to_vec())
        .pic_type(PictureType::CoverFront)
        .mime_type(MimeType::Jpeg)
        .build()
}

// ─── FLAC ───

fn write_flac(path: &Path, values: &TagValues, cover: Option<&[u8]>) -> Result<(), Error> {
    let mut flac = FlacFile::read_from(&mut File::open(path)?, ParseOptions::new())?;

    let mut comments = flac.vorbis_comments().cloned().unwrap_or_default();
    fill_vorbis(&mut comments, values);
    flac.set_vorbis_comments(comments);

    // A FLAC file keeps its pictures itself, not in the comments
    if let Some(data) = cover {
        flac.remove_picture_type(PictureType::CoverFront);
        flac.insert_picture(jpeg_cover(data), None)?;
    }
    flac.save_to_path(path, WriteOptions::default())?;
    Ok(())
}

fn fill_vorbis(comments: &mut VorbisComments, values: &TagValues) {
    let mut set = |key: &str, value: Option<&str>| {
        if let Some(value) = value {
            comments.insert(key.to_owned(), value.to_owned());
        }
    };

    set("TITLE", values.title.as_deref());
    set("ARTIST", values.artist.as_deref());
    set("ALBUM", values.album.as_deref());
    set("ALBUMARTIST", values.album_artist.as_deref());
    set("GENRE", values.genre.as_deref());
    set("COMPOSER", values.composer.as_deref());
    set("COPYRIGHT", values.copyright.as_deref());
    set("LABEL", values.label.as_deref());
    set("ISRC", values.isrc.as_deref());
    set("BARCODE", values.barcode.as_deref());
    set("DATE", values.release_date.as_deref());
    set("COMMENT", values.comment.as_deref());
    set(URL_KEY, values.url.as_deref());
    set(
        "MEDIATYPE",
        values
            .release_type
            .as_deref()
            .map(str::to_uppercase)
            .as_deref(),
    );
    set(
        "ITUNESADVISORY",
        values
            .explicit
            .map(|explicit| if explicit { "1" } else { "0" }),
    );

    let mut number = |key: &str, value: Option<u32>| {
        if let Some(value) = value {
            comments.insert(key.to_owned(), value.to_string());
        }
    };
    number("TRACKNUMBER", values.track_number);
    number("TRACKTOTAL", values.total_tracks);
    number("DISCNUMBER", values.disc_number);
    number("DISCTOTAL", values.total_discs);
    number("YEAR", values.year);
}

// ─── MP3 ───

fn write_mp3(path: &Path, values: &TagValues, cover: Option<&[u8]>) -> Result<(), Error> {
    let mut mpeg = MpegFile::read_from(&mut File::open(path)?, ParseOptions::new())?;

    // The Qobuz MP3s carry no ID3v1 tag and none is wanted
    mpeg.remove_id3v1();

    let mut tag: Tag = mpeg
        .id3v2()
        .cloned()
        .map_or_else(|| Tag::new(TagType::Id3v2), Tag::from);
    fill_generic(&mut tag, values);
    if let Some(data) = cover {
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(jpeg_cover(data));
    }

    let mut id3 = Id3v2Tag::from(tag);
    if let Some(url) = &values.url {
        id3.insert(Frame::UserUrl(ExtendedUrlFrame::new(
            TextEncoding::UTF8,
            URL_KEY.to_owned(),
            url.clone(),
        )));
    }

    mpeg.set_id3v2(id3);
    mpeg.save_to_path(path, WriteOptions::default())?;
    Ok(())
}

fn fill_generic(tag: &mut Tag, values: &TagValues) {
    if let Some(title) = &values.title {
        tag.set_title(title.clone());
    }
    if let Some(artist) = &values.artist {
        tag.set_artist(artist.clone());
    }
    if let Some(album) = &values.album {
        tag.set_album(album.clone());
    }
    if let Some(genre) = &values.genre {
        tag.set_genre(genre.clone());
    }
    if let Some(number) = values.track_number {
        tag.set_track(number);
    }
    if let Some(total) = values.total_tracks {
        tag.set_track_total(total);
    }
    if let Some(number) = values.disc_number {
        tag.set_disk(number);
    }
    if let Some(total) = values.total_discs {
        tag.set_disk_total(total);
    }
    if let Some(comment) = &values.comment {
        tag.set_comment(comment.clone());
    }

    let mut text = |key: ItemKey, value: Option<&String>| {
        if let Some(value) = value {
            tag.insert_text(key, value.clone());
        }
    };
    text(ItemKey::AlbumArtist, values.album_artist.as_ref());
    text(ItemKey::Composer, values.composer.as_ref());
    text(ItemKey::CopyrightMessage, values.copyright.as_ref());
    // ID3 has no label frame: the publisher one (TPUB) is what players read as the label
    text(ItemKey::Publisher, values.label.as_ref());
    text(ItemKey::Isrc, values.isrc.as_ref());
    text(ItemKey::Barcode, values.barcode.as_ref());
    text(ItemKey::ReleaseDate, values.release_date.as_ref());
    text(
        ItemKey::OriginalMediaType,
        values
            .release_type
            .as_ref()
            .map(|kind| capitalised(kind))
            .as_ref(),
    );
    text(
        ItemKey::ParentalAdvisory,
        values
            .explicit
            .map(|explicit| {
                if explicit {
                    "1".to_owned()
                } else {
                    "0".to_owned()
                }
            })
            .as_ref(),
    );

    if let Some(year) = values.year {
        tag.insert_text(ItemKey::Year, year.to_string());
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::file::TaggedFileExt;
    use tempfile::TempDir;

    /// The smallest JPEG a decoder accepts: one white pixel.
    const TINY_JPEG: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06,
        0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B,
        0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31,
        0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF,
        0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00,
        0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x08, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x08,
        0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x7F, 0xFF, 0xD9,
    ];

    /// A FLAC file that is only a header: enough to be opened and tagged.
    fn flac_file(directory: &TempDir) -> std::path::PathBuf {
        let mut bytes = b"fLaC".to_vec();
        bytes.extend([0x80, 0, 0, 34]); // last metadata block: STREAMINFO, 34 bytes
        bytes.extend([0x10, 0x00, 0x10, 0x00, 0, 0, 0, 0, 0, 0]); // block and frame sizes
        let packed: u64 = (44_100 << 44) | (1 << 41) | (15 << 36); // rate, channels - 1, bits - 1, samples
        bytes.extend(packed.to_be_bytes());
        bytes.extend([0; 16]); // MD5

        let path = directory.path().join("track.flac");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// Twenty silent MPEG-1 Layer III frames, 128 kbps at 44.1 kHz.
    fn mp3_file(directory: &TempDir) -> std::path::PathBuf {
        let mut bytes = Vec::new();
        for _ in 0..20 {
            let mut frame = vec![0_u8; 417];
            frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            bytes.extend(frame);
        }
        let path = directory.path().join("track.mp3");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn values() -> TagValues {
        TagValues {
            title: Some("So What".into()),
            artist: Some("Miles Davis".into()),
            album: Some("Kind of Blue".into()),
            album_artist: Some("Miles Davis".into()),
            track_number: Some(1),
            total_tracks: Some(5),
            disc_number: Some(1),
            total_discs: Some(2),
            genre: Some("Jazz".into()),
            release_date: Some("1959-08-17".into()),
            year: Some(1959),
            composer: Some("Miles Davis".into()),
            copyright: Some("(P) 1959".into()),
            label: Some("Columbia".into()),
            isrc: Some("USSM15900113".into()),
            barcode: Some("5099749522428".into()),
            release_type: Some("album".into()),
            explicit: Some(true),
            url: Some("https://www.qobuz.com/album/kind-of-blue/123".into()),
            comment: Some("Ripped with QobuzDLX".into()),
        }
    }

    fn vorbis(path: &Path) -> VorbisComments {
        FlacFile::read_from(&mut File::open(path).unwrap(), ParseOptions::new())
            .unwrap()
            .vorbis_comments()
            .cloned()
            .expect("the file has Vorbis comments")
    }

    fn flac_pictures(path: &Path) -> Vec<Picture> {
        FlacFile::read_from(&mut File::open(path).unwrap(), ParseOptions::new())
            .unwrap()
            .pictures()
            .iter()
            .map(|(picture, _)| picture.clone())
            .collect()
    }

    fn id3(path: &Path) -> Id3v2Tag {
        MpegFile::read_from(&mut File::open(path).unwrap(), ParseOptions::new())
            .unwrap()
            .id3v2()
            .cloned()
            .expect("the file has an ID3v2 tag")
    }

    // ─── FLAC ───

    #[test]
    fn a_flac_file_gets_its_tags_under_the_usual_vorbis_names() {
        let directory = TempDir::new().unwrap();
        let path = flac_file(&directory);

        write(&path, &values(), None).unwrap();

        let comments = vorbis(&path);
        let get = |key: &str| comments.get(key).map(str::to_owned);
        assert_eq!(get("TITLE").as_deref(), Some("So What"));
        assert_eq!(get("ALBUMARTIST").as_deref(), Some("Miles Davis"));
        assert_eq!(get("TRACKNUMBER").as_deref(), Some("1"));
        assert_eq!(get("TRACKTOTAL").as_deref(), Some("5"));
        assert_eq!(get("DISCTOTAL").as_deref(), Some("2"));
        assert_eq!(get("DATE").as_deref(), Some("1959-08-17"));
        assert_eq!(get("YEAR").as_deref(), Some("1959"));
        assert_eq!(get("ISRC").as_deref(), Some("USSM15900113"));
        assert_eq!(get("BARCODE").as_deref(), Some("5099749522428"));
        assert_eq!(get("LABEL").as_deref(), Some("Columbia"));
        assert_eq!(get("MEDIATYPE").as_deref(), Some("ALBUM"));
        assert_eq!(get("ITUNESADVISORY").as_deref(), Some("1"));
        assert_eq!(get("COMMENT").as_deref(), Some("Ripped with QobuzDLX"));
        assert_eq!(
            get("Qobuz URL").as_deref(),
            Some("https://www.qobuz.com/album/kind-of-blue/123")
        );
    }

    #[test]
    fn a_flac_file_gets_no_field_for_a_value_that_is_missing() {
        let directory = TempDir::new().unwrap();
        let path = flac_file(&directory);
        let bare = TagValues {
            title: Some("Only a title".into()),
            ..TagValues::default()
        };

        write(&path, &bare, None).unwrap();

        let comments = vorbis(&path);
        assert_eq!(comments.get("TITLE"), Some("Only a title"));
        for key in [
            "ISRC",
            "LABEL",
            "BARCODE",
            "GENRE",
            "COMPOSER",
            "COMMENT",
            "ITUNESADVISORY",
            "Qobuz URL",
        ] {
            assert_eq!(comments.get(key), None, "{key}");
        }
    }

    #[test]
    fn a_clean_track_is_marked_with_a_zero() {
        let directory = TempDir::new().unwrap();
        let path = flac_file(&directory);
        let clean = TagValues {
            explicit: Some(false),
            ..TagValues::default()
        };

        write(&path, &clean, None).unwrap();

        assert_eq!(vorbis(&path).get("ITUNESADVISORY"), Some("0"));
    }

    #[test]
    fn the_cover_is_embedded_as_the_front_cover() {
        let directory = TempDir::new().unwrap();
        let path = flac_file(&directory);

        write(&path, &values(), Some(TINY_JPEG)).unwrap();

        let pictures = flac_pictures(&path);
        assert_eq!(pictures.len(), 1);
        assert_eq!(pictures[0].pic_type(), PictureType::CoverFront);
        assert_eq!(pictures[0].data(), TINY_JPEG);
    }

    #[test]
    fn tagging_twice_does_not_stack_covers_or_fields() {
        let directory = TempDir::new().unwrap();
        let path = flac_file(&directory);

        write(&path, &values(), Some(TINY_JPEG)).unwrap();
        write(&path, &values(), Some(TINY_JPEG)).unwrap();

        assert_eq!(flac_pictures(&path).len(), 1);
        assert_eq!(vorbis(&path).get_all("TITLE").count(), 1);
    }

    // ─── MP3 ───

    #[test]
    fn an_mp3_file_gets_its_tags_in_an_id3v2_tag() {
        let directory = TempDir::new().unwrap();
        let path = mp3_file(&directory);

        write(&path, &values(), None).unwrap();

        let tagged = lofty::read_from_path(&path).unwrap();
        let tag = tagged.primary_tag().expect("a tag was written");
        assert_eq!(tag.title().as_deref(), Some("So What"));
        assert_eq!(tag.artist().as_deref(), Some("Miles Davis"));
        assert_eq!(tag.album().as_deref(), Some("Kind of Blue"));
        assert_eq!(tag.genre().as_deref(), Some("Jazz"));
        assert_eq!((tag.track(), tag.track_total()), (Some(1), Some(5)));
        assert_eq!((tag.disk(), tag.disk_total()), (Some(1), Some(2)));
        assert_eq!(tag.get_string(ItemKey::AlbumArtist), Some("Miles Davis"));
        assert_eq!(tag.get_string(ItemKey::Isrc), Some("USSM15900113"));
        assert_eq!(tag.get_string(ItemKey::Publisher), Some("Columbia"));
        assert_eq!(tag.get_string(ItemKey::Barcode), Some("5099749522428"));
        assert_eq!(tag.get_string(ItemKey::OriginalMediaType), Some("Album"));
        assert_eq!(tag.get_string(ItemKey::ParentalAdvisory), Some("1"));
        assert_eq!(
            tag.get_string(ItemKey::Comment),
            Some("Ripped with QobuzDLX")
        );
    }

    #[test]
    fn an_mp3_file_gets_the_store_url_in_a_user_url_frame() {
        let directory = TempDir::new().unwrap();
        let path = mp3_file(&directory);

        write(&path, &values(), None).unwrap();

        let url = id3(&path)
            .into_iter()
            .find_map(|frame| match frame {
                Frame::UserUrl(url) if url.description == URL_KEY => Some(url.content),
                _ => None,
            })
            .expect("a WXXX frame named Qobuz URL");
        assert_eq!(url, "https://www.qobuz.com/album/kind-of-blue/123");
    }

    #[test]
    fn an_mp3_file_gets_no_frame_for_a_value_that_is_missing() {
        let directory = TempDir::new().unwrap();
        let path = mp3_file(&directory);
        let bare = TagValues {
            title: Some("Only a title".into()),
            explicit: Some(false),
            ..TagValues::default()
        };

        write(&path, &bare, None).unwrap();

        let tagged = lofty::read_from_path(&path).unwrap();
        let tag = tagged.primary_tag().unwrap();
        for key in [
            ItemKey::Barcode,
            ItemKey::Isrc,
            ItemKey::Publisher,
            ItemKey::Composer,
            ItemKey::Genre,
        ] {
            assert_eq!(tag.get_string(key), None, "{key:?}");
        }
        let urls = id3(&path)
            .into_iter()
            .filter(|frame| matches!(frame, Frame::UserUrl(_)))
            .count();
        assert_eq!(urls, 0);
    }

    #[test]
    fn an_mp3_file_gets_its_cover() {
        let directory = TempDir::new().unwrap();
        let path = mp3_file(&directory);

        write(&path, &values(), Some(TINY_JPEG)).unwrap();

        let tagged = lofty::read_from_path(&path).unwrap();
        let pictures = tagged.primary_tag().unwrap().pictures();
        assert_eq!(pictures.len(), 1);
        assert_eq!(pictures[0].pic_type(), PictureType::CoverFront);
    }

    // ─── Other formats ───

    #[test]
    fn only_flac_and_mp3_are_tagged() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("track.ogg");
        std::fs::write(&path, b"x").unwrap();

        assert!(matches!(
            write(&path, &values(), None),
            Err(Error::UnsupportedFormat(extension)) if extension == "ogg"
        ));
    }

    #[test]
    fn a_file_that_is_not_audio_is_an_error_not_a_panic() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("broken.flac");
        std::fs::write(&path, b"not a flac file").unwrap();

        assert!(write(&path, &values(), None).is_err());
    }
}
