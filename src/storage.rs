//! Small JSON files kept in the configuration directory.
//!
//! A file that cannot be read must never stop the application from starting, and a power cut in
//! the middle of a write must never leave a truncated file behind. Writes therefore go to a
//! neighbouring file that then replaces the original, keeping the previous version as `.bak`;
//! reads that fail set the faulty file aside and fall back to that backup.

use std::fs;
use std::io::{self, Write};
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

    /// Writes the value atomically. The previous version, if any, is kept as `<name>.bak`.
    pub fn save<T: Serialize>(&self, value: &T) -> io::Result<()> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory)?;
        }

        let temporary = self.sibling(".tmp");
        let mut file = create_temporary(&temporary, self.access)?;
        serde_json::to_writer_pretty(&mut file, value).map_err(io::Error::from)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        if self.path.exists() {
            // A failed backup is not worth losing the save for
            if let Err(error) = fs::copy(&self.path, self.sibling(".bak")) {
                tracing::warn!(path = %self.path.display(), %error, "could not back the file up");
            }
        }
        fs::rename(&temporary, &self.path)
    }
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
