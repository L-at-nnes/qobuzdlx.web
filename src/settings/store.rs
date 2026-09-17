use std::io;
use std::path::Path;
use std::sync::{Arc, RwLock};

use super::Settings;
use crate::storage::{Access, JsonFile};

const FILE_NAME: &str = "settings.json";

/// The settings, shared by every part of the application and kept in `settings.json`.
///
/// Cloning the store is cheap: clones share the same settings.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    file: JsonFile,
    settings: RwLock<Settings>,
}

impl SettingsStore {
    /// Opens the store, reading `settings.json` from `config_dir` when it exists.
    ///
    /// A missing or unreadable file gives the defaults, and a download folder that was never chosen
    /// becomes `default_downloads`: the application must always be able to start.
    #[must_use]
    pub fn open(config_dir: &Path, default_downloads: &Path) -> Self {
        let file = JsonFile::new(config_dir.join(FILE_NAME), Access::Normal);

        let mut settings: Settings = file.load().unwrap_or_default();
        if settings.download.directory.as_os_str().is_empty() {
            settings.download.directory = default_downloads.to_path_buf();
        }

        Self {
            inner: Arc::new(Inner {
                file,
                settings: RwLock::new(settings),
            }),
        }
    }

    /// A copy of the current settings.
    #[must_use]
    pub fn get(&self) -> Settings {
        self.read().clone()
    }

    /// Applies `change` and saves the result. The settings only change when the save succeeded, so
    /// memory and disk never disagree.
    pub fn update<R>(&self, change: impl FnOnce(&mut Settings) -> R) -> io::Result<R> {
        let mut guard = self
            .inner
            .settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut updated = guard.clone();
        let result = change(&mut updated);
        self.inner.file.save(&updated)?;

        *guard = updated;
        Ok(result)
    }

    /// Replaces all the settings at once.
    pub fn replace(&self, settings: Settings) -> io::Result<()> {
        self.update(|current| *current = settings)
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Settings> {
        self.inner
            .settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{DownloadSettings, Language, Quality};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn open(directory: &TempDir) -> SettingsStore {
        SettingsStore::open(directory.path(), Path::new("/music"))
    }

    #[test]
    fn a_first_start_gives_the_defaults_with_the_default_download_folder() {
        let directory = TempDir::new().unwrap();

        let settings = open(&directory).get();

        assert_eq!(settings.download.directory, PathBuf::from("/music"));
        assert_eq!(settings.download.quality, Quality::Flac24Max);
    }

    #[test]
    fn an_update_is_visible_at_once_and_after_a_restart() {
        let directory = TempDir::new().unwrap();
        let store = open(&directory);

        store
            .update(|settings| settings.language = Some(Language::Fr))
            .unwrap();

        assert_eq!(store.get().language, Some(Language::Fr));
        assert_eq!(open(&directory).get().language, Some(Language::Fr));
    }

    #[test]
    fn clones_share_the_same_settings() {
        let directory = TempDir::new().unwrap();
        let store = open(&directory);
        let clone = store.clone();

        clone
            .update(|settings| settings.download.quality = Quality::Mp3)
            .unwrap();

        assert_eq!(store.get().download.quality, Quality::Mp3);
    }

    #[test]
    fn an_empty_download_folder_in_the_file_falls_back_to_the_default() {
        let directory = TempDir::new().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            r#"{ "download": { "directory": "" } }"#,
        )
        .unwrap();

        assert_eq!(
            open(&directory).get().download.directory,
            PathBuf::from("/music")
        );
    }

    #[test]
    fn a_corrupted_file_does_not_stop_the_start_up() {
        let directory = TempDir::new().unwrap();
        std::fs::write(directory.path().join("settings.json"), "{ truncated").unwrap();

        let expected = Settings {
            download: DownloadSettings {
                directory: PathBuf::from("/music"),
                ..DownloadSettings::default()
            },
            ..Settings::default()
        };
        assert_eq!(open(&directory).get(), expected);
    }

    #[test]
    fn a_failed_save_leaves_the_settings_unchanged() {
        let directory = TempDir::new().unwrap();
        // A file where the configuration directory should be: saving cannot succeed
        let blocker = directory.path().join("config");
        std::fs::write(&blocker, "").unwrap();
        let store = SettingsStore::open(&blocker, Path::new("/music"));

        let result = store.update(|settings| settings.language = Some(Language::Fr));

        assert!(result.is_err());
        assert_eq!(store.get().language, None);
    }
}
