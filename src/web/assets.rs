//! The interface itself: the files of the web application, carried inside the executable.

use std::borrow::Cow;

use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use super::error::ApiError;

/// Where the interface is built to. The folder may be missing (the interface was not built yet): the
/// server then starts anyway and says so.
#[derive(RustEmbed)]
#[folder = "web/build"]
#[allow_missing = true]
struct Embedded;

/// A file of the interface.
#[derive(Debug)]
pub struct Asset {
    pub bytes: Cow<'static, [u8]>,
    pub mime: String,
}

fn embedded(path: &str) -> Option<Asset> {
    Embedded::get(path).map(|file| Asset {
        bytes: file.data,
        mime: mime_of(path),
    })
}

/// `.webmanifest` is missing from many tables: without it the file would go as a download and
/// browsers would ignore the manifest, and with it the installation on a home screen.
fn mime_of(path: &str) -> String {
    if path.ends_with(".webmanifest") {
        return "application/manifest+json".to_owned();
    }
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

/// What a path leads to.
#[derive(Debug)]
pub enum Resolved {
    /// A file, and whether its name changes with its content (it can then be kept for ever).
    File { asset: Asset, immutable: bool },
    /// A page of the interface that exists only in the browser: the entry page answers for it.
    EntryPage(Asset),
    /// Nothing there.
    Missing,
    /// The interface was not built.
    NotBuilt,
}

/// Finds what a request path leads to, among the files `lookup` knows.
///
/// A path with an extension that is not there is missing, not the entry page: a script that fails to
/// load must fail loudly rather than come back as HTML. A path without one is a page of the
/// interface: its prerendered file if it has one, the entry page otherwise.
pub fn resolve(path: &str, lookup: &impl Fn(&str) -> Option<Asset>) -> Resolved {
    let path = path.trim_matches('/');
    if path == "api" || path.starts_with("api/") {
        return Resolved::Missing;
    }

    let wanted = if path.is_empty() { "index.html" } else { path };
    if let Some(asset) = lookup(wanted) {
        // SvelteKit puts everything it can cache for ever under this folder, named by content
        let immutable = wanted.starts_with("_app/immutable/");
        return Resolved::File { asset, immutable };
    }
    if path
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
    {
        return Resolved::Missing;
    }

    for candidate in [format!("{path}.html"), format!("{path}/index.html")] {
        if !path.is_empty()
            && let Some(asset) = lookup(&candidate)
        {
            return Resolved::File {
                asset,
                immutable: false,
            };
        }
    }
    match lookup("index.html") {
        Some(entry) => Resolved::EntryPage(entry),
        None => Resolved::NotBuilt,
    }
}

/// Answers every request that is not for the API.
pub async fn serve(uri: Uri) -> Response {
    match resolve(uri.path(), &embedded) {
        Resolved::File { asset, immutable } => file_response(asset, immutable),
        Resolved::EntryPage(asset) => file_response(asset, false),
        Resolved::Missing => ApiError::not_found("not_found", "nothing here").into_response(),
        Resolved::NotBuilt => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "The interface is not built: run `npm run build` in the web folder, then build again.",
        )
            .into_response(),
    }
}

fn file_response(asset: Asset, immutable: bool) -> Response {
    let cache = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        // Revalidated every time: a new version of the application is seen at the next load
        "no-cache"
    };
    let mut response = asset.bytes.into_owned().into_response();
    let headers = response.headers_mut();
    if let Ok(mime) = HeaderValue::from_str(&asset.mime) {
        headers.insert(header::CONTENT_TYPE, mime);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn files(names: &[&str]) -> impl Fn(&str) -> Option<Asset> {
        let known: HashMap<String, String> = names
            .iter()
            .map(|name| ((*name).to_owned(), format!("content of {name}")))
            .collect();
        move |path| {
            known.get(path).map(|content| Asset {
                bytes: Cow::Owned(content.clone().into_bytes()),
                mime: mime_of(path),
            })
        }
    }

    fn text(asset: &Asset) -> String {
        String::from_utf8(asset.bytes.to_vec()).unwrap()
    }

    #[test]
    fn the_root_is_the_entry_page() {
        let lookup = files(&["index.html"]);

        let Resolved::File { asset, immutable } = resolve("/", &lookup) else {
            panic!("the root leads to a file");
        };

        assert_eq!(text(&asset), "content of index.html");
        assert!(!immutable);
    }

    #[test]
    fn a_file_is_served_as_it_is() {
        let lookup = files(&["index.html", "favicon.svg"]);

        let Resolved::File { asset, .. } = resolve("/favicon.svg", &lookup) else {
            panic!("a file");
        };

        assert_eq!(asset.mime, "image/svg+xml");
    }

    #[test]
    fn what_is_named_by_its_content_is_kept_for_ever() {
        let lookup = files(&["index.html", "_app/immutable/chunks/abc123.js"]);

        let Resolved::File { immutable, asset } =
            resolve("/_app/immutable/chunks/abc123.js", &lookup)
        else {
            panic!("a file");
        };

        assert!(immutable);
        assert!(asset.mime.contains("javascript"), "{}", asset.mime);
    }

    #[test]
    fn a_prerendered_page_is_found_without_its_extension() {
        let lookup = files(&["index.html", "downloads.html", "settings/index.html"]);

        let Resolved::File { asset, .. } = resolve("/downloads", &lookup) else {
            panic!("a file");
        };
        assert_eq!(text(&asset), "content of downloads.html");
        let Resolved::File { asset, .. } = resolve("/settings", &lookup) else {
            panic!("a file");
        };
        assert_eq!(text(&asset), "content of settings/index.html");
    }

    #[test]
    fn a_page_that_exists_only_in_the_browser_gets_the_entry_page() {
        let lookup = files(&["index.html"]);

        assert!(matches!(
            resolve("/albums/abc", &lookup),
            Resolved::EntryPage(_)
        ));
    }

    #[test]
    fn a_missing_file_is_missing_rather_than_the_entry_page() {
        let lookup = files(&["index.html"]);

        assert!(matches!(resolve("/app.js", &lookup), Resolved::Missing));
        assert!(matches!(
            resolve("/_app/immutable/gone.css", &lookup),
            Resolved::Missing
        ));
    }

    #[test]
    fn the_api_is_never_answered_with_a_page() {
        let lookup = files(&["index.html"]);

        assert!(matches!(
            resolve("/api/nothing", &lookup),
            Resolved::Missing
        ));
        assert!(matches!(resolve("/api", &lookup), Resolved::Missing));
    }

    #[test]
    fn without_a_built_interface_it_says_so() {
        assert!(matches!(resolve("/", &files(&[])), Resolved::NotBuilt));
        assert!(matches!(
            resolve("/downloads", &files(&[])),
            Resolved::NotBuilt
        ));
    }

    #[test]
    fn the_manifest_has_its_own_type() {
        assert_eq!(mime_of("manifest.webmanifest"), "application/manifest+json");
        assert_eq!(mime_of("x.css"), "text/css");
        assert_eq!(mime_of("x.woff2"), "font/woff2");
    }
}
