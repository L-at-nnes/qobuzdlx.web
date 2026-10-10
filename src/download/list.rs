//! A list of addresses written in a text file, one per line.

use super::url::{QobuzUrl, parse};

/// What a text file holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddressList {
    /// The Qobuz addresses that can be downloaded, in the order of the file.
    pub addresses: Vec<QobuzUrl>,
    /// Lines that are neither blank, nor a comment, nor a Qobuz address that can be downloaded.
    pub ignored: usize,
}

/// Reads the addresses of a text file.
///
/// A blank line and a line starting with `#` are left out without being counted: they are the
/// writer's own notes. A trailing ` # note` after an address is cut. Every other line that is not a
/// Qobuz address of a downloadable kind (a link to another site, a word) is counted as ignored.
/// An address may be written without its `https://`.
#[must_use]
pub fn parse_list(text: &str) -> AddressList {
    let mut list = AddressList::default();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = without_note(line.trim());
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let address = if line.contains("://") {
            line.to_owned()
        } else {
            format!("https://{line}")
        };
        match parse(&address) {
            Some(parsed) => list.addresses.push(parsed),
            None => list.ignored += 1,
        }
    }
    list
}

/// The line without a note after it: whitespace, then `#`. A `#` inside an address stays.
fn without_note(line: &str) -> &str {
    line.char_indices()
        .zip(line.chars().skip(1).chain(std::iter::once(' ')))
        .find_map(|((at, current), next)| {
            (current.is_whitespace() && next == '#').then(|| line[..at].trim_end())
        })
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::item::ItemKind;

    fn canonical(list: &AddressList) -> Vec<&str> {
        list.addresses
            .iter()
            .map(|address| address.canonical.as_str())
            .collect()
    }

    #[test]
    fn one_address_per_line_in_order() {
        let list = parse_list(
            "https://play.qobuz.com/album/abc\nhttps://play.qobuz.com/playlist/12\nhttps://play.qobuz.com/track/7\n",
        );

        assert_eq!(
            canonical(&list),
            [
                "https://play.qobuz.com/album/abc",
                "https://play.qobuz.com/playlist/12",
                "https://play.qobuz.com/track/7"
            ]
        );
        assert_eq!(list.addresses[1].kind, ItemKind::Playlist);
        assert_eq!(list.ignored, 0);
    }

    #[test]
    fn comments_and_blank_lines_are_left_out_and_not_counted() {
        let list = parse_list(
            "# my favourites\n\n   \nhttps://play.qobuz.com/album/abc\n  # indented note\n",
        );

        assert_eq!(list.addresses.len(), 1);
        assert_eq!(list.ignored, 0);
    }

    #[test]
    fn what_is_not_a_qobuz_address_is_ignored_and_counted() {
        let list = parse_list(
            "www.youtube.com\nhttps://www.youtube.com/watch?v=1\nhello\nhttps://play.qobuz.com/album/abc\nhttps://www.qobuz.com/fr-fr/shop\n",
        );

        assert_eq!(canonical(&list), ["https://play.qobuz.com/album/abc"]);
        assert_eq!(list.ignored, 4);
    }

    #[test]
    fn an_address_may_lack_its_scheme() {
        let list = parse_list("play.qobuz.com/album/abc\nwww.qobuz.com/fr-fr/album/name/def");

        assert_eq!(
            canonical(&list),
            [
                "https://play.qobuz.com/album/abc",
                "https://play.qobuz.com/album/def"
            ]
        );
    }

    #[test]
    fn a_note_after_an_address_is_cut_but_a_fragment_stays() {
        let list = parse_list(
            "https://play.qobuz.com/album/abc  # the good one\nhttps://play.qobuz.com/album/def#section",
        );

        assert_eq!(
            canonical(&list),
            [
                "https://play.qobuz.com/album/abc",
                "https://play.qobuz.com/album/def"
            ]
        );
        assert_eq!(list.ignored, 0);
    }

    #[test]
    fn windows_line_ends_and_a_byte_order_mark_are_harmless() {
        let list = parse_list(
            "\u{feff}# first\r\nhttps://play.qobuz.com/album/abc\r\nhttps://play.qobuz.com/track/9\r\n",
        );

        assert_eq!(list.addresses.len(), 2);
        assert_eq!(list.ignored, 0);
    }

    #[test]
    fn two_addresses_on_one_line_are_neither() {
        let list = parse_list("https://play.qobuz.com/album/a https://play.qobuz.com/album/b");

        assert!(list.addresses.is_empty());
        assert_eq!(list.ignored, 1);
    }

    #[test]
    fn an_empty_file_gives_nothing() {
        assert_eq!(parse_list(""), AddressList::default());
    }
}
