use std::fmt::Write;
use std::path::{Component, Path};

use super::PlaylistEntry;

/// The content of an extended M3U playlist (UTF-8), in the order of the playlist.
///
/// Paths are relative to the folder of the file and use `/`: the library can be moved, or read from
/// another machine, without the playlist breaking.
#[must_use]
pub fn build(folder: &Path, entries: &[PlaylistEntry]) -> String {
    let mut ordered: Vec<&PlaylistEntry> = entries.iter().collect();
    ordered.sort_by_key(|entry| entry.index);

    let mut text = String::from("#EXTM3U\n");
    for entry in ordered {
        let title = entry.title.replace(['\r', '\n'], " ");
        // Writing into a String cannot fail
        let _ = writeln!(text, "#EXTINF:{},{title}", entry.duration_seconds);
        let _ = writeln!(text, "{}", relative(folder, &entry.file));
    }
    text
}

/// `file` relative to `folder`, with `/` separators. A file outside the folder is reached with `..`.
fn relative(folder: &Path, file: &Path) -> String {
    let folder: Vec<Component> = folder.components().collect();
    let file: Vec<Component> = file.components().collect();

    let common = folder.iter().zip(&file).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_owned(); folder.len() - common];
    parts.extend(
        file[common..]
            .iter()
            .map(|component| component.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(index: usize, file: &str, duration: u32, title: &str) -> PlaylistEntry {
        PlaylistEntry {
            index,
            file: PathBuf::from(file),
            duration_seconds: duration,
            title: title.to_owned(),
        }
    }

    #[test]
    fn the_header_comes_first_and_an_empty_playlist_has_nothing_else() {
        assert_eq!(build(Path::new("/dl/Mix"), &[]), "#EXTM3U\n");
    }

    #[test]
    fn each_entry_gives_a_duration_a_title_and_a_relative_path() {
        let text = build(
            Path::new("/dl/Mix"),
            &[entry(0, "/dl/Mix/01 - Intro.flac", 127, "The xx - Intro")],
        );

        assert_eq!(
            text,
            "#EXTM3U\n#EXTINF:127,The xx - Intro\n01 - Intro.flac\n"
        );
    }

    #[test]
    fn entries_are_written_in_the_order_of_the_playlist_whatever_the_order_given() {
        let text = build(
            Path::new("/dl/Mix"),
            &[
                entry(2, "/dl/Mix/c.flac", 3, "C"),
                entry(0, "/dl/Mix/a.flac", 1, "A"),
                entry(1, "/dl/Mix/b.flac", 2, "B"),
            ],
        );

        assert_eq!(
            text.lines()
                .filter(|line| !line.starts_with('#'))
                .collect::<Vec<_>>(),
            ["a.flac", "b.flac", "c.flac"]
        );
    }

    #[test]
    fn a_file_in_a_sub_folder_keeps_its_path_with_forward_slashes() {
        let text = build(
            Path::new("/dl/Mix"),
            &[entry(0, "/dl/Mix/CD 2/05 - Song.flac", 60, "Song")],
        );

        assert!(text.ends_with("CD 2/05 - Song.flac\n"));
    }

    #[test]
    fn a_file_outside_the_folder_is_reached_with_parent_references() {
        assert_eq!(
            relative(Path::new("/dl/Mix"), Path::new("/dl/Other/a.flac")),
            "../Other/a.flac"
        );
    }

    #[test]
    fn a_title_never_breaks_the_line_structure() {
        let text = build(
            Path::new("/dl"),
            &[entry(0, "/dl/a.flac", 1, "two\nlines\r\nhere")],
        );

        assert_eq!(text, "#EXTM3U\n#EXTINF:1,two lines  here\na.flac\n");
    }
}
