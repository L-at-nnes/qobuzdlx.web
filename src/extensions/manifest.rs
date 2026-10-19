//! `extension.json`: how a folder describes the extension it holds.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::EventKind;

/// Longest name accepted.
const NAME_MAX: usize = 40;

/// The default time an extension is given for one event.
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 60;
const TIMEOUT_MAX_SECONDS: u64 = 3600;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("extension.json is not valid: {0}")]
    Invalid(String),
    #[error("the name {0:?} is not valid: use 1 to {NAME_MAX} letters, digits, '-' or '_'")]
    Name(String),
    #[error("give either \"command\" or \"webhook\", not both and not neither")]
    Kind,
    #[error("\"command\" needs at least the program to run")]
    EmptyCommand,
    #[error("\"webhook.url\" must be an http or https address")]
    WebhookUrl,
    #[error("\"timeout_seconds\" must be between 1 and {TIMEOUT_MAX_SECONDS}")]
    Timeout,
}

/// Where an extension of the webhook kind is told about events.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Webhook {
    pub url: String,
    /// Sent with every request, for a token for instance.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

/// The content of an `extension.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Defaults to the name of the folder.
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    /// The events wanted. Left out or empty, all of them.
    #[serde(default)]
    pub events: Vec<EventKind>,
    /// A disabled extension is left alone, without having to delete its folder.
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// How long one event may take before the program is killed or the request given up.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Program and arguments. A program written with a `/` is looked for from the folder of the
    /// extension; any other is looked for on the `PATH`.
    pub command: Option<Vec<String>>,
    pub webhook: Option<Webhook>,
}

const fn enabled() -> bool {
    true
}

const fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}

impl Manifest {
    /// Reads and checks a manifest.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(text)
            .map_err(|error| ManifestError::Invalid(error.to_string()))?;
        manifest.check()?;
        Ok(manifest)
    }

    fn check(&self) -> Result<(), ManifestError> {
        if let Some(name) = &self.name {
            check_name(name)?;
        }
        if !(1..=TIMEOUT_MAX_SECONDS).contains(&self.timeout_seconds) {
            return Err(ManifestError::Timeout);
        }
        match (&self.command, &self.webhook) {
            (Some(command), None) => {
                if command
                    .first()
                    .is_none_or(|program| program.trim().is_empty())
                {
                    return Err(ManifestError::EmptyCommand);
                }
            }
            (None, Some(webhook)) => {
                let valid = url::Url::parse(&webhook.url)
                    .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.has_host());
                if !valid {
                    return Err(ManifestError::WebhookUrl);
                }
            }
            _ => return Err(ManifestError::Kind),
        }
        Ok(())
    }
}

/// A name is used in logs, in the interface and as a key: keep it plain.
pub(super) fn check_name(name: &str) -> Result<(), ManifestError> {
    let valid = !name.is_empty()
        && name.len() <= NAME_MAX
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(ManifestError::Name(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_manifest_with_defaults() {
        let manifest = Manifest::parse(r#"{"command": ["./run.sh", "--fast"]}"#).unwrap();

        assert_eq!(manifest.name, None);
        assert!(manifest.enabled);
        assert!(manifest.events.is_empty());
        assert_eq!(manifest.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
        assert_eq!(manifest.command.unwrap(), ["./run.sh", "--fast"]);
    }

    #[test]
    fn a_webhook_manifest_with_events_and_headers() {
        let manifest = Manifest::parse(
            r#"{
                "name": "notify",
                "events": ["item_finished", "queue_drained"],
                "webhook": {"url": "https://example.org/hook", "headers": {"Authorization": "Bearer x"}}
            }"#,
        )
        .unwrap();

        assert_eq!(
            manifest.events,
            [EventKind::ItemFinished, EventKind::QueueDrained]
        );
        assert_eq!(
            manifest.webhook.unwrap().headers["Authorization"],
            "Bearer x"
        );
    }

    #[test]
    fn exactly_one_kind_is_required() {
        assert_eq!(Manifest::parse("{}"), Err(ManifestError::Kind));
        assert_eq!(
            Manifest::parse(r#"{"command": ["a"], "webhook": {"url": "http://x"}}"#),
            Err(ManifestError::Kind)
        );
    }

    #[test]
    fn an_empty_command_is_refused() {
        assert_eq!(
            Manifest::parse(r#"{"command": []}"#),
            Err(ManifestError::EmptyCommand)
        );
        assert_eq!(
            Manifest::parse(r#"{"command": [" "]}"#),
            Err(ManifestError::EmptyCommand)
        );
    }

    #[test]
    fn only_http_addresses_are_accepted_for_a_webhook() {
        for url in ["ftp://example.org", "not a url", "file:///etc/passwd"] {
            let text = format!(r#"{{"webhook": {{"url": "{url}"}}}}"#);
            assert_eq!(
                Manifest::parse(&text),
                Err(ManifestError::WebhookUrl),
                "{url}"
            );
        }
    }

    #[test]
    fn names_are_plain() {
        for name in ["", "has space", "../escape", "é", &"x".repeat(41)] {
            let text = format!(r#"{{"name": {name:?}, "command": ["a"]}}"#);
            assert!(
                matches!(Manifest::parse(&text), Err(ManifestError::Name(_))),
                "{name}"
            );
        }
        assert!(Manifest::parse(r#"{"name": "ok-name_2", "command": ["a"]}"#).is_ok());
    }

    #[test]
    fn a_misspelled_field_is_reported_instead_of_ignored() {
        assert!(matches!(
            Manifest::parse(r#"{"comand": ["a"]}"#),
            Err(ManifestError::Invalid(_))
        ));
        assert!(matches!(
            Manifest::parse(r#"{"events": ["nope"], "command": ["a"]}"#),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn the_timeout_has_bounds() {
        for seconds in [0, 3601] {
            let text = format!(r#"{{"command": ["a"], "timeout_seconds": {seconds}}}"#);
            assert_eq!(Manifest::parse(&text), Err(ManifestError::Timeout));
        }
    }
}
