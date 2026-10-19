//! Finding the extensions that sit in a directory: one folder each, with an `extension.json`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use super::manifest::{Manifest, check_name};
use super::{CommandExtension, Extension, Info, Source, WebhookExtension};

/// Name of the file that makes a folder an extension.
pub const MANIFEST_FILE: &str = "extension.json";

/// An extension that was found but could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    /// The folder, or the name of the extension.
    pub source: String,
    pub message: String,
}

/// What a directory held.
#[derive(Default)]
pub struct Loaded {
    pub extensions: Vec<Arc<dyn Extension>>,
    pub problems: Vec<Problem>,
}

impl std::fmt::Debug for Loaded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Loaded")
            .field("extensions", &self.extensions.len())
            .field("problems", &self.problems)
            .finish()
    }
}

/// Loads every extension found in the folders of `directory`, in alphabetical order. A directory
/// that does not exist holds none. A folder that cannot be loaded is reported and does not
/// prevent the others from loading. Folders whose name starts with `.` or `_` are ignored, so
/// that one can be set aside without deleting it.
pub fn load_dir(directory: &Path) -> Loaded {
    let mut loaded = Loaded::default();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return loaded;
    };

    let mut folders: Vec<_> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !name.starts_with('.') && !name.starts_with('_')
        })
        .map(|entry| entry.path())
        .collect();
    folders.sort();

    for folder in folders {
        let manifest_path = folder.join(MANIFEST_FILE);
        if !manifest_path.is_file() {
            continue;
        }
        let label = folder
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());

        match load_one(&folder, &manifest_path, &label) {
            Ok(Some(extension)) => loaded.extensions.push(extension),
            Ok(None) => tracing::info!(extension = %label, "extension is disabled"),
            Err(message) => loaded.problems.push(Problem {
                source: label,
                message,
            }),
        }
    }
    loaded
}

/// `None` when the extension is switched off.
fn load_one(
    folder: &Path,
    manifest_path: &Path,
    label: &str,
) -> Result<Option<Arc<dyn Extension>>, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|error| format!("{MANIFEST_FILE} could not be read: {error}"))?;
    let manifest = Manifest::parse(&text).map_err(|error| error.to_string())?;
    if !manifest.enabled {
        return Ok(None);
    }

    // The folder name stands in for a missing name, and must then be usable as one
    let name = if let Some(name) = manifest.name {
        name
    } else {
        check_name(label).map_err(|error| error.to_string())?;
        label.to_owned()
    };
    let info = Info {
        name,
        version: manifest.version,
        description: manifest.description,
        source: Source::Builtin,
        events: manifest.events,
    };
    let timeout = Duration::from_secs(manifest.timeout_seconds);

    match (manifest.command, manifest.webhook) {
        (Some(command), None) => Ok(Some(Arc::new(CommandExtension::new(
            info,
            &command,
            folder.to_path_buf(),
            timeout,
        )))),
        (None, Some(webhook)) => WebhookExtension::new(info, webhook, timeout)
            .map(|extension| Some(Arc::new(extension) as Arc<dyn Extension>))
            .map_err(|error| error.to_string()),
        // Refused by the manifest check already
        _ => Err("give either \"command\" or \"webhook\"".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(root: &Path, folder: &str, manifest: &str) {
        let path = root.join(folder);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(MANIFEST_FILE), manifest).unwrap();
    }

    fn names(loaded: &Loaded) -> Vec<String> {
        loaded
            .extensions
            .iter()
            .map(|extension| extension.info().name)
            .collect()
    }

    #[test]
    fn a_missing_directory_holds_nothing() {
        let loaded = load_dir(Path::new("/this/does/not/exist"));

        assert!(loaded.extensions.is_empty() && loaded.problems.is_empty());
    }

    #[test]
    fn extensions_load_in_alphabetical_order_and_take_the_name_of_their_folder() {
        let root = tempfile::tempdir().unwrap();
        add(root.path(), "beta", r#"{"command": ["run"]}"#);
        add(
            root.path(),
            "alpha",
            r#"{"webhook": {"url": "http://localhost/x"}}"#,
        );
        add(
            root.path(),
            "gamma",
            r#"{"name": "custom", "command": ["run"]}"#,
        );

        let loaded = load_dir(root.path());

        assert_eq!(names(&loaded), ["alpha", "beta", "custom"]);
        assert_eq!(loaded.extensions[0].info().source, Source::Webhook);
        assert_eq!(loaded.extensions[1].info().source, Source::Command);
    }

    #[test]
    fn a_broken_extension_does_not_stop_the_others() {
        let root = tempfile::tempdir().unwrap();
        add(root.path(), "good", r#"{"command": ["run"]}"#);
        add(root.path(), "bad", "{ not json");
        add(root.path(), "worse", r#"{"command": []}"#);

        let loaded = load_dir(root.path());

        assert_eq!(names(&loaded), ["good"]);
        let problems: Vec<_> = loaded.problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(problems, ["bad", "worse"]);
    }

    #[test]
    fn disabled_hidden_and_manifest_less_folders_are_left_alone() {
        let root = tempfile::tempdir().unwrap();
        add(
            root.path(),
            "off",
            r#"{"enabled": false, "command": ["run"]}"#,
        );
        add(root.path(), ".hidden", r#"{"command": ["run"]}"#);
        add(root.path(), "_aside", r#"{"command": ["run"]}"#);
        std::fs::create_dir(root.path().join("no-manifest")).unwrap();
        std::fs::write(root.path().join("stray-file.json"), "{}").unwrap();

        let loaded = load_dir(root.path());

        assert!(loaded.extensions.is_empty());
        assert!(loaded.problems.is_empty());
    }

    #[test]
    fn a_folder_name_that_is_not_a_valid_name_needs_an_explicit_one() {
        let root = tempfile::tempdir().unwrap();
        add(root.path(), "my extension", r#"{"command": ["run"]}"#);
        add(
            root.path(),
            "other one",
            r#"{"name": "other", "command": ["run"]}"#,
        );

        let loaded = load_dir(root.path());

        assert_eq!(names(&loaded), ["other"]);
        assert_eq!(loaded.problems[0].source, "my extension");
    }
}
