//! Runtime configuration: what the operator sets from the outside (environment variables).
//!
//! Everything a user can change from the interface lives in [`crate::settings`] instead.

use std::net::SocketAddr;
use std::path::PathBuf;

/// Shortest access password accepted, whether it comes from the environment or the interface.
pub const MIN_ACCESS_PASSWORD_LEN: usize = 8;

const DEFAULT_CONFIG_DIR: &str = "/config";
const DEFAULT_DOWNLOADS_DIR: &str = "/downloads";
const DEFAULT_BIND: &str = "127.0.0.1:8080";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("BIND must be an address like 0.0.0.0:8080, got {value:?}")]
    InvalidBind { value: String },

    #[error("ACCESS_PASSWORD must be at least {MIN_ACCESS_PASSWORD_LEN} characters long")]
    AccessPasswordTooShort,
}

/// Process-level configuration, read once at startup.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Where settings, session, queue and keys are kept (`CONFIG_PATH`).
    pub config_dir: PathBuf,
    /// Download folder used until the user picks another one (`DOWNLOADS_PATH`).
    pub default_downloads_dir: PathBuf,
    /// Where the extensions are looked for (`EXTENSIONS_PATH`), `extensions` inside the config
    /// folder unless set.
    pub extensions_dir: PathBuf,
    /// Address the server listens on (`BIND`).
    ///
    /// Loopback by default: outside a container the application is not reachable from the network
    /// unless the operator asks for it. The container image sets `0.0.0.0:8080`.
    pub bind: SocketAddr,
    /// Access password imposed by the operator (`ACCESS_PASSWORD`), if any.
    pub access_password: Option<String>,
    /// Mark the unlock cookie `Secure` (`COOKIE_SECURE=true`). Only for deployments served over
    /// HTTPS: browsers refuse a `Secure` cookie on plain HTTP.
    pub cookie_secure: bool,
}

impl RuntimeConfig {
    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the configuration through `lookup`, which maps a variable name to its value.
    /// Taking the lookup as a parameter keeps the parsing testable without touching the process
    /// environment.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let non_empty = |name: &str| lookup(name).filter(|value| !value.is_empty());

        let bind_text = non_empty("BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let bind = bind_text
            .parse()
            .map_err(|_| ConfigError::InvalidBind { value: bind_text })?;

        // A password that is too short stops the start-up: ignoring it would leave the interface
        // open to anyone while its owner believes it is protected.
        let access_password = non_empty("ACCESS_PASSWORD");
        if access_password
            .as_ref()
            .is_some_and(|password| password.chars().count() < MIN_ACCESS_PASSWORD_LEN)
        {
            return Err(ConfigError::AccessPasswordTooShort);
        }

        let config_dir = non_empty("CONFIG_PATH")
            .map_or_else(|| PathBuf::from(DEFAULT_CONFIG_DIR), PathBuf::from);

        Ok(Self {
            extensions_dir: non_empty("EXTENSIONS_PATH")
                .map_or_else(|| config_dir.join("extensions"), PathBuf::from),
            config_dir,
            default_downloads_dir: non_empty("DOWNLOADS_PATH")
                .map_or_else(|| PathBuf::from(DEFAULT_DOWNLOADS_DIR), PathBuf::from),
            bind,
            access_password,
            cookie_secure: non_empty("COOKIE_SECURE")
                .is_some_and(|value| value.eq_ignore_ascii_case("true")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(pairs: &[(&str, &str)]) -> Result<RuntimeConfig, ConfigError> {
        let env: HashMap<_, _> = pairs.iter().copied().collect();
        RuntimeConfig::from_lookup(|name| env.get(name).map(|value| (*value).to_owned()))
    }

    #[test]
    fn defaults_listen_on_loopback_only() {
        let config = config(&[]).unwrap();

        assert_eq!(config.bind.to_string(), "127.0.0.1:8080");
        assert_eq!(config.config_dir, PathBuf::from("/config"));
        assert_eq!(config.default_downloads_dir, PathBuf::from("/downloads"));
        assert_eq!(config.extensions_dir, PathBuf::from("/config/extensions"));
        assert_eq!(config.access_password, None);
        assert!(!config.cookie_secure);
    }

    #[test]
    fn every_variable_is_honoured() {
        let config = config(&[
            ("BIND", "0.0.0.0:9000"),
            ("CONFIG_PATH", "/data/config"),
            ("DOWNLOADS_PATH", "/data/music"),
            ("EXTENSIONS_PATH", "/data/plugins"),
            ("ACCESS_PASSWORD", "long enough"),
            ("COOKIE_SECURE", "TRUE"),
        ])
        .unwrap();

        assert_eq!(config.bind.to_string(), "0.0.0.0:9000");
        assert_eq!(config.config_dir, PathBuf::from("/data/config"));
        assert_eq!(config.default_downloads_dir, PathBuf::from("/data/music"));
        assert_eq!(config.extensions_dir, PathBuf::from("/data/plugins"));
        assert_eq!(config.access_password.as_deref(), Some("long enough"));
        assert!(config.cookie_secure);
    }

    #[test]
    fn extensions_follow_the_config_folder_unless_told_otherwise() {
        let config = config(&[("CONFIG_PATH", "/data/config")]).unwrap();

        assert_eq!(
            config.extensions_dir,
            PathBuf::from("/data/config/extensions")
        );
    }

    #[test]
    fn an_empty_variable_counts_as_unset() {
        let config = config(&[
            ("ACCESS_PASSWORD", ""),
            ("CONFIG_PATH", ""),
            ("COOKIE_SECURE", ""),
        ])
        .unwrap();

        assert_eq!(config.access_password, None);
        assert_eq!(config.config_dir, PathBuf::from("/config"));
    }

    #[test]
    fn a_short_access_password_stops_the_start_up() {
        assert!(matches!(
            config(&[("ACCESS_PASSWORD", "short")]),
            Err(ConfigError::AccessPasswordTooShort)
        ));
    }

    #[test]
    fn the_password_length_counts_characters_not_bytes() {
        // Eight characters, sixteen bytes
        assert!(config(&[("ACCESS_PASSWORD", "éééééééé")]).is_ok());
    }

    #[test]
    fn an_invalid_bind_address_is_reported_with_its_value() {
        let error = config(&[("BIND", "not-an-address")]).unwrap_err();

        assert!(
            matches!(error, ConfigError::InvalidBind { ref value } if value == "not-an-address")
        );
    }

    #[test]
    fn only_the_word_true_enables_secure_cookies() {
        assert!(!config(&[("COOKIE_SECURE", "1")]).unwrap().cookie_secure);
        assert!(!config(&[("COOKIE_SECURE", "false")]).unwrap().cookie_secure);
    }
}
