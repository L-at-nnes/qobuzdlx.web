//! Finding the app credentials the Qobuz web player uses.
//!
//! The player ships them inside its JavaScript bundle, scattered and encoded. The pair changes when
//! Qobuz rebuilds the player, so it is extracted from the live bundle and every candidate is tried
//! against the API; a short list of known pairs is the last resort.

use std::collections::HashSet;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use regex::Regex;

use super::client::Auth;
use super::{AppCredentials, Client, Result, sign_file_url};

/// A track that exists and a format any account may ask for: enough to make Qobuz verify a signature
/// without any user being signed in. Only a valid app pair gets a 200.
const PROBE_TRACK_ID: u64 = 197_432_204;
const PROBE_FORMAT_ID: u32 = 5;

/// The base64 padding and checksum the player appends to the encoded secret.
const SECRET_TAIL_LEN: usize = 44;

/// Known pairs, newest first. Only the first is accepted by Qobuz today (checked on 2026-09-28): the
/// others answer 401 and only serve accounts whose token was issued when they were current.
const FALLBACK_PAIRS: [(&str, &str); 4] = [
    ("312369995", "e79f8b9be485692b0e5f9dd895826368"),
    ("798273057", "abb21364945c0583309667d13ca3d93a"),
    ("579939560", "fa31fc13e7a28e7d70bb61e91aa9e178"),
    ("950096963", "979549437fcc4a3faad4867b5cd25dcb"),
];

macro_rules! pattern {
    ($name:ident, $regex:expr) => {
        static $name: LazyLock<Regex> =
            LazyLock::new(|| Regex::new($regex).expect("valid pattern"));
    };
}

pattern!(
    BUNDLE_IN_SCRIPT_TAG,
    r#"<script src="(/resources/\d+\.\d+\.\d+-[a-z]\d{3}/bundle\.js)""#
);
pattern!(
    BUNDLE_ANYWHERE,
    r#""(/resources/\d+\.\d+\.\d+-[a-z]\d+/bundle\.js)""#
);
pattern!(
    APP_ID_WITH_SECRET,
    r#"production:\{api:\{appId:"(\d+)",appSecret:"(\w+)""#
);
pattern!(APP_ID_ALONE, r#"app_id:\s*"(\d+)""#);
pattern!(
    BERLIN_SEED,
    r#"[a-z]\.initialSeed\("([\w=]+)",window\.utimezone\.berlin\)"#
);
pattern!(
    BERLIN_INFO,
    r#"name:"[A-Za-z/]+/Berlin",info:"([\w=]+)",extras:"([\w=]+)""#
);
pattern!(
    ANY_SEED,
    r#"[a-z]\.initialSeed\("([\w=]+)",window\.utimezone\.([a-z]+)\)"#
);
pattern!(HEX_STRING, r#"['"]([a-f0-9]{32})['"]"#);
pattern!(DIRECT_SECRET, r#"appSecret:"([a-f0-9]{32})""#);

/// The URL of the bundle, relative to the web player, found in its login page.
fn bundle_path(login_page: &str) -> Option<String> {
    BUNDLE_IN_SCRIPT_TAG
        .captures(login_page)
        .or_else(|| BUNDLE_ANYWHERE.captures(login_page))
        .map(|captures| captures[1].to_owned())
}

fn app_id(bundle: &str) -> Option<String> {
    APP_ID_WITH_SECRET
        .captures(bundle)
        .or_else(|| APP_ID_ALONE.captures(bundle))
        .map(|captures| captures[1].to_owned())
}

/// Decodes a secret the player hides in three pieces: a seed, then `info` and `extras` of the matching
/// time zone, concatenated, stripped of their last 44 characters and base64-decoded.
fn decode_secret(seed: &str, info: &str, extras: &str) -> Option<String> {
    let combined = format!("{seed}{info}{extras}");
    let payload = combined.get(..combined.len().checked_sub(SECRET_TAIL_LEN)?)?;
    let bytes = STANDARD.decode(payload).ok()?;
    String::from_utf8(bytes)
        .ok()
        .filter(|secret| !secret.is_empty())
}

/// Every string of the bundle that could be the app secret, most likely first.
fn candidate_secrets(bundle: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    // The Berlin time zone is the one the web player really uses
    if let (Some(seed), Some(info)) = (BERLIN_SEED.captures(bundle), BERLIN_INFO.captures(bundle)) {
        candidates.extend(decode_secret(&seed[1], &info[1], &info[2]));
    }

    // The other time zones, in case Berlin is not where it is any more
    for seed in ANY_SEED
        .captures_iter(bundle)
        .filter(|captures| &captures[2] != "berlin")
    {
        let zone = capitalised(&seed[2]);
        let info = Regex::new(&format!(
            r#"name:"\w+/{}",info:"([\w=]+)",extras:"([\w=]+)""#,
            regex::escape(&zone)
        ));
        if let Some(info) = info.ok().and_then(|pattern| {
            pattern
                .captures(bundle)
                .map(|c| (c[1].to_owned(), c[2].to_owned()))
        }) {
            candidates.extend(decode_secret(&seed[1], &info.0, &info.1));
        }
    }

    // Last resorts: any 32-character hexadecimal string, and a secret written in clear
    candidates.extend(
        HEX_STRING
            .captures_iter(bundle)
            .map(|captures| captures[1].to_owned()),
    );
    candidates.extend(
        DIRECT_SECRET
            .captures_iter(bundle)
            .map(|captures| captures[1].to_owned()),
    );

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.clone()));
    candidates
}

fn capitalised(word: &str) -> String {
    let mut characters = word.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().chain(characters).collect())
        .unwrap_or_default()
}

impl Client {
    /// Finds an app id and secret that Qobuz accepts, before anyone signs in, so that a login is
    /// attempted once with credentials already known to work: a 401 then means the user's own
    /// credentials are wrong. Returns `None` when no pair is accepted.
    pub async fn resolve_app_credentials(&self) -> Option<AppCredentials> {
        match self.credentials_from_bundle().await {
            Ok(Some(credentials)) => return Some(credentials),
            Ok(None) => tracing::warn!(
                "no app secret of the Qobuz bundle was accepted, trying the known pairs"
            ),
            Err(error) => {
                tracing::warn!(%error, "could not read the Qobuz bundle, trying the known pairs");
            }
        }

        for (app_id, app_secret) in FALLBACK_PAIRS {
            if self.accepts(app_id, app_secret).await {
                return Some(AppCredentials {
                    app_id: app_id.to_owned(),
                    app_secret: app_secret.to_owned(),
                });
            }
        }
        None
    }

    /// Whether Qobuz accepts the app pair: it answers 200 to a correctly signed request, and refuses
    /// anything else (a wrong app id is refused before the signature is even looked at).
    pub async fn app_credentials_are_valid(&self, credentials: &AppCredentials) -> bool {
        self.accepts(&credentials.app_id, &credentials.app_secret)
            .await
    }

    async fn credentials_from_bundle(&self) -> Result<Option<AppCredentials>> {
        let login_page = self.get_web_text("/login").await?;
        let Some(bundle_path) = bundle_path(&login_page) else {
            return Err(super::Error::Unexpected(
                "the login page names no bundle".to_owned(),
            ));
        };
        let bundle = self.get_web_text(&bundle_path).await?;

        let Some(app_id) = app_id(&bundle) else {
            return Err(super::Error::Unexpected(
                "the bundle holds no app id".to_owned(),
            ));
        };

        for secret in candidate_secrets(&bundle) {
            if self.accepts(&app_id, &secret).await {
                return Ok(Some(AppCredentials {
                    app_id,
                    app_secret: secret,
                }));
            }
        }
        Ok(None)
    }

    async fn accepts(&self, app_id: &str, secret: &str) -> bool {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let params = [
            ("request_ts", timestamp.to_string()),
            (
                "request_sig",
                sign_file_url(PROBE_TRACK_ID, PROBE_FORMAT_ID, timestamp, secret),
            ),
            ("track_id", PROBE_TRACK_ID.to_string()),
            ("format_id", PROBE_FORMAT_ID.to_string()),
            ("intent", "stream".to_owned()),
        ];

        self.get_text_once("track/getFileUrl", &params, Auth::App { app_id })
            .await
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A secret the way the player hides it: base64, then 44 characters of tail, cut in three.
    fn hidden(secret: &str) -> (String, String, String) {
        let payload = format!("{}{}", STANDARD.encode(secret), "T".repeat(SECRET_TAIL_LEN));
        let (seed, rest) = payload.split_at(10);
        let (info, extras) = rest.split_at(rest.len() / 2);
        (seed.to_owned(), info.to_owned(), extras.to_owned())
    }

    #[test]
    fn the_bundle_is_found_in_a_script_tag() {
        let page = r#"<html><script src="/resources/7.1.2-b012/bundle.js"></script></html>"#;

        assert_eq!(
            bundle_path(page).as_deref(),
            Some("/resources/7.1.2-b012/bundle.js")
        );
    }

    #[test]
    fn the_bundle_is_found_anywhere_when_the_tag_looks_different() {
        let page = r#"loader.load("/resources/7.1.2-b9/bundle.js")"#;

        assert_eq!(
            bundle_path(page).as_deref(),
            Some("/resources/7.1.2-b9/bundle.js")
        );
    }

    #[test]
    fn a_page_without_a_bundle_names_none() {
        assert_eq!(bundle_path("<html></html>"), None);
    }

    #[test]
    fn the_app_id_comes_from_the_production_block_or_from_a_bare_assignment() {
        assert_eq!(
            app_id(r#"production:{api:{appId:"123456789",appSecret:"abc"}"#).as_deref(),
            Some("123456789")
        );
        assert_eq!(
            app_id(r#"config = { app_id: "987654321" }"#).as_deref(),
            Some("987654321")
        );
        assert_eq!(app_id("nothing here"), None);
    }

    #[test]
    fn a_hidden_secret_is_decoded() {
        let (seed, info, extras) = hidden("0123456789abcdef0123456789abcdef");

        assert_eq!(
            decode_secret(&seed, &info, &extras).as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
    }

    #[test]
    fn a_payload_too_short_to_hold_a_secret_decodes_to_nothing() {
        assert_eq!(decode_secret("abc", "def", "ghi"), None);
    }

    #[test]
    fn a_payload_that_is_not_base64_decodes_to_nothing() {
        assert_eq!(
            decode_secret("!!!!", "????", &"x".repeat(SECRET_TAIL_LEN)),
            None
        );
    }

    #[test]
    fn the_berlin_secret_comes_first_then_the_other_candidates_without_duplicates() {
        let secret = "0123456789abcdef0123456789abcdef";
        let (seed, info, extras) = hidden(secret);
        let bundle = format!(
            r#"a.initialSeed("{seed}",window.utimezone.berlin) name:"Europe/Berlin",info:"{info}",extras:"{extras}" x="ffffffffffffffffffffffffffffffff" y="{secret}" appSecret:"ffffffffffffffffffffffffffffffff""#
        );

        assert_eq!(
            candidate_secrets(&bundle),
            [
                secret.to_owned(),
                "ffffffffffffffffffffffffffffffff".to_owned()
            ]
        );
    }

    #[test]
    fn another_time_zone_is_decoded_too() {
        let secret = "fedcba9876543210fedcba9876543210";
        let (seed, info, extras) = hidden(secret);
        let bundle = format!(
            r#"a.initialSeed("{seed}",window.utimezone.paris) name:"Europe/Paris",info:"{info}",extras:"{extras}""#
        );

        assert_eq!(candidate_secrets(&bundle), [secret.to_owned()]);
    }

    #[test]
    fn a_bundle_without_secrets_gives_no_candidate() {
        assert!(candidate_secrets("var x = 1;").is_empty());
    }
}
