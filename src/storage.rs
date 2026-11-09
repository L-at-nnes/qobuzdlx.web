//! Small JSON files kept in the configuration directory.
//!
//! A file that cannot be read must never stop the application from starting, and a power cut in
//! the middle of a write must never leave a truncated file behind. Writes therefore go to a
//! neighbouring file that then replaces the original, keeping the previous version as `.bak`;
//! reads that fail set the faulty file aside and fall back to that backup.

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Who may read the file on a shared machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The usual permissions of the process.
    Normal,
    /// Readable by the owner only. For files holding secrets such as the Qobuz session.
    OwnerOnly,
}

#[derive(Debug, Clone)]
pub struct JsonFile {
    path: PathBuf,
    access: Access,
}

impl JsonFile {
    pub fn new(path: impl Into<PathBuf>, access: Access) -> Self {
        Self {
            path: path.into(),
            access,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(suffix);
        self.path.with_file_name(name)
    }

    /// Reads the value, recovering from a damaged file when it can.
    ///
    /// Returns `None` when there is nothing usable: no file yet, or a file that is unreadable and
    /// has no usable backup. A file that exists but cannot be parsed is renamed to
    /// `<name>.corrupt-<timestamp>` so that the next save does not overwrite the evidence.
    pub fn load<T: DeserializeOwned>(&self) -> Option<T> {
        if self.path.exists() {
            match Self::read(&self.path) {
                Ok(value) => return Some(value),
                Err(error) => {
                    tracing::error!(path = %self.path.display(), %error, "unreadable file");
                    self.quarantine();
                }
            }
        }

        let backup = self.sibling(".bak");
        if backup.exists() {
            match Self::read(&backup) {
                Ok(value) => {
                    tracing::warn!(path = %backup.display(), "restored from the backup");
                    return Some(value);
                }
                Err(error) => {
                    tracing::error!(path = %backup.display(), %error, "unreadable backup");
                }
            }
        }

        None
    }

    fn read<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes).map_err(io::Error::from)
    }

    fn quarantine(&self) {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let target = self.sibling(&format!(".corrupt-{seconds}"));
        match fs::rename(&self.path, &target) {
            Ok(()) => tracing::warn!(path = %target.display(), "unreadable file set aside"),
            Err(error) => {
                tracing::error!(path = %self.path.display(), %error, "could not set the unreadable file aside");
            }
        }
    }

    /// Writes the value atomically, laid out to be read by a person. The previous version, if any,
    /// is kept as `<name>.bak`.
    pub fn save<T: Serialize>(&self, value: &T) -> io::Result<()> {
        self.write(value, Layout::Readable)
    }

    /// Like [`JsonFile::save`], without the line breaks and the indentation: for a file that can
    /// hold a hundred thousand entries and is written again and again, where they would make half
    /// of what is written.
    pub fn save_compact<T: Serialize>(&self, value: &T) -> io::Result<()> {
        self.write(value, Layout::Compact)
    }

    fn write<T: Serialize>(&self, value: &T, layout: Layout) -> io::Result<()> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory)?;
        }

        let temporary = self.sibling(".tmp");
        // Buffered: the encoder writes a few bytes at a time, and each would be a call to the system
        let mut writer = BufWriter::new(create_temporary(&temporary, self.access)?);
        match layout {
            Layout::Readable => serde_json::to_writer_pretty(&mut writer, value),
            Layout::Compact => serde_json::to_writer(&mut writer, value),
        }
        .map_err(io::Error::from)?;
        writer.write_all(b"\n")?;
        let file = writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        file.sync_all()?;
        drop(file);

        self.keep_previous();
        fs::rename(&temporary, &self.path)
    }

    /// Keeps the version about to be replaced as `<name>.bak`. A second name for the same file where
    /// the filesystem can give one, which costs nothing whatever the size; a copy where it cannot.
    /// A failed backup is not worth losing the save for.
    fn keep_previous(&self) {
        if !self.path.exists() {
            return;
        }
        let backup = self.sibling(".bak");
        let _ = fs::remove_file(&backup);
        if fs::hard_link(&self.path, &backup).is_ok() {
            return;
        }
        if let Err(error) = fs::copy(&self.path, &backup) {
            tracing::warn!(path = %self.path.display(), %error, "could not back the file up");
        }
    }
}

/// How a file is laid out.
#[derive(Debug, Clone, Copy)]
enum Layout {
    /// Indented, one field per line.
    Readable,
    /// On one line.
    Compact,
}

fn create_temporary(path: &Path, access: Access) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    if access == Access::OwnerOnly {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Windows has no mode bits: the profile directory's own ACL is what protects the file
    #[cfg(not(unix))]
    let _ = access;

    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        count: u32,
    }

    fn sample(name: &str, count: u32) -> Sample {
        Sample {
            name: name.to_owned(),
            count,
        }
    }

    fn file_in(directory: &TempDir) -> JsonFile {
        JsonFile::new(directory.path().join("sample.json"), Access::Normal)
    }

    #[test]
    fn a_missing_file_yields_nothing() {
        let directory = TempDir::new().unwrap();

        assert_eq!(file_in(&directory).load::<Sample>(), None);
    }

    #[test]
    fn a_saved_value_is_read_back() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);

        file.save(&sample("one", 1)).unwrap();

        assert_eq!(file.load::<Sample>(), Some(sample("one", 1)));
    }

    #[test]
    fn saving_creates_the_missing_directories() {
        let directory = TempDir::new().unwrap();
        let file = JsonFile::new(directory.path().join("a/b/sample.json"), Access::Normal);

        file.save(&sample("deep", 2)).unwrap();

        assert_eq!(file.load::<Sample>(), Some(sample("deep", 2)));
    }

    #[test]
    fn the_previous_version_is_kept_as_a_backup() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);

        file.save(&sample("first", 1)).unwrap();
        file.save(&sample("second", 2)).unwrap();

        let backup: Sample = JsonFile::read(&directory.path().join("sample.json.bak")).unwrap();
        assert_eq!(backup, sample("first", 1));
        assert!(
            !directory.path().join("sample.json.tmp").exists(),
            "no temporary file is left"
        );
    }

    #[test]
    fn a_compact_file_is_read_back_and_keeps_its_previous_version_too() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);

        file.save_compact(&sample("first", 1)).unwrap();
        file.save_compact(&sample("second", 2)).unwrap();

        assert_eq!(file.load::<Sample>(), Some(sample("second", 2)));
        let written = fs::read_to_string(file.path()).unwrap();
        assert_eq!(written.lines().count(), 1, "{written}");
        let backup: Sample = JsonFile::read(&directory.path().join("sample.json.bak")).unwrap();
        assert_eq!(backup, sample("first", 1));
    }

    #[test]
    fn the_backup_is_not_changed_by_the_saves_that_follow_it() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);

        for count in 1..=3 {
            file.save(&sample("version", count)).unwrap();
        }

        // The backup may share its content with the version it was made from: it must be the one
        // before the last, not follow the file
        let backup: Sample = JsonFile::read(&directory.path().join("sample.json.bak")).unwrap();
        assert_eq!(backup, sample("version", 2));
        assert_eq!(file.load::<Sample>(), Some(sample("version", 3)));
    }

    #[test]
    fn a_corrupted_file_is_set_aside_and_the_backup_restored() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);
        file.save(&sample("good", 1)).unwrap();
        file.save(&sample("newer", 2)).unwrap();
        fs::write(file.path(), "{ \"name\": \"trunc").unwrap();

        assert_eq!(file.load::<Sample>(), Some(sample("good", 1)));

        let set_aside = fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(set_aside, 1);
        assert!(
            !file.path().exists(),
            "the damaged file no longer sits in the way"
        );
    }

    #[test]
    fn a_corrupted_file_without_a_backup_yields_nothing() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);
        fs::write(file.path(), "not json").unwrap();

        assert_eq!(file.load::<Sample>(), None);
    }

    #[test]
    fn a_value_of_the_wrong_shape_counts_as_corrupted() {
        let directory = TempDir::new().unwrap();
        let file = file_in(&directory);
        fs::write(file.path(), r#"{"unrelated": true}"#).unwrap();

        assert_eq!(file.load::<Sample>(), None);
    }
}
