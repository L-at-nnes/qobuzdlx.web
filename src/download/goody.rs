use std::path::Path;

use url::Url;

use crate::naming::{make_safe_filename, truncate_to_bytes};
use crate::qobuz::{Album, Goody};

/// File format id of the goodies that are not files to keep (the interface's own links).
const IGNORED_FORMAT: u32 = 52;

/// Leaves room for the ` (id)` suffix and the extension below the 255 bytes a name may take.
const MAX_NAME_BYTES: usize = 200;

/// The name a goody is saved under, or `None` for one that has nothing to download.
///
/// The id is part of the name: an album can hold several goodies with the same description.
#[must_use]
pub fn goody_file_name(album: &Album, goody: &Goody) -> Option<String> {
    let url = goody.url.as_deref().filter(|url| !url.is_empty())?;
    if goody.file_format_id == IGNORED_FORMAT {
        return None;
    }

    let description = goody
        .description
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(&album.title);
    let name = truncate_to_bytes(&make_safe_filename(description), MAX_NAME_BYTES);
    Some(format!("{name} ({}){}", goody.id, extension(url)))
}

/// The extension in the path of the address. A signed address ends with a query string that must not
/// leak into the name: `.pdf?Expires=...` is not a valid name on Windows or on SMB shares.
fn extension(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            Path::new(url.path())
                .extension()
                .map(|extension| format!(".{}", extension.to_string_lossy()))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album() -> Album {
        serde_json::from_str(r#"{"id": "a", "title": "Kind of Blue", "artist": {"id": 1}}"#)
            .unwrap()
    }

    fn goody(id: u64, format: u32, description: Option<&str>, url: Option<&str>) -> Goody {
        Goody {
            id,
            file_format_id: format,
            description: description.map(str::to_owned),
            url: url.map(str::to_owned),
            ..Goody::default()
        }
    }

    #[test]
    fn a_booklet_is_named_after_its_description_with_its_id() {
        let booklet = goody(
            7,
            21,
            Some("Livret numérique"),
            Some("https://cdn.example/booklet.pdf"),
        );

        assert_eq!(
            goody_file_name(&album(), &booklet).as_deref(),
            Some("Livret numérique (7).pdf")
        );
    }

    #[test]
    fn the_query_of_a_signed_address_stays_out_of_the_name() {
        let booklet = goody(
            3,
            21,
            Some("Booklet"),
            Some("https://cdn.example/files/b.pdf?Expires=1700000000&Signature=abc"),
        );

        assert_eq!(
            goody_file_name(&album(), &booklet).as_deref(),
            Some("Booklet (3).pdf")
        );
    }

    #[test]
    fn without_a_description_the_title_of_the_album_is_used() {
        let booklet = goody(9, 21, None, Some("https://cdn.example/b.pdf"));

        assert_eq!(
            goody_file_name(&album(), &booklet).as_deref(),
            Some("Kind of Blue (9).pdf")
        );
    }

    #[test]
    fn characters_a_filesystem_refuses_are_replaced() {
        let booklet = goody(
            1,
            21,
            Some("Liner notes: part 1/2"),
            Some("https://cdn.example/n.pdf"),
        );

        assert_eq!(
            goody_file_name(&album(), &booklet).as_deref(),
            Some("Liner notes - part 1-2 (1).pdf")
        );
    }

    #[test]
    fn a_very_long_description_is_cut_to_leave_room_for_the_rest() {
        let long = "é".repeat(300);
        let booklet = goody(12, 21, Some(&long), Some("https://cdn.example/n.pdf"));

        let name = goody_file_name(&album(), &booklet).unwrap();

        assert!(name.len() <= 255, "{} bytes", name.len());
        assert!(name.ends_with(" (12).pdf"));
    }

    #[test]
    fn an_address_without_an_extension_gives_a_name_without_one() {
        let booklet = goody(2, 21, Some("Extra"), Some("https://cdn.example/download/2"));

        assert_eq!(
            goody_file_name(&album(), &booklet).as_deref(),
            Some("Extra (2)")
        );
    }

    #[test]
    fn a_goody_without_an_address_or_of_the_ignored_format_is_left_out() {
        assert_eq!(
            goody_file_name(&album(), &goody(1, 21, Some("A"), None)),
            None
        );
        assert_eq!(
            goody_file_name(&album(), &goody(1, 21, Some("A"), Some(""))),
            None
        );
        assert_eq!(
            goody_file_name(
                &album(),
                &goody(1, 52, Some("A"), Some("https://cdn.example/a.pdf"))
            ),
            None
        );
    }
}
