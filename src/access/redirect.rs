/// Where to go after unlocking, when the address comes from the request.
///
/// Only a path of this site is accepted; anything else leads home, which closes the open redirect.
#[must_use]
pub fn local_path(candidate: Option<&str>) -> String {
    match candidate {
        Some(path) if is_local(path) => path.to_owned(),
        _ => "/".to_owned(),
    }
}

/// `//` is a protocol-relative address and `:` introduces an absolute one. The backslash and the
/// control characters close the remaining ways round: browsers read a backslash as `/` and drop tabs
/// and line breaks, so `/\evil.example` and `/<TAB>/evil.example` both become `//evil.example`.
fn is_local(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && path
            .chars()
            .all(|character| character != '\\' && character != ':' && !character.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_of_the_site_is_kept() {
        assert_eq!(local_path(Some("/downloads")), "/downloads");
        assert_eq!(
            local_path(Some("/search?q=miles&type=albums")),
            "/search?q=miles&type=albums"
        );
        assert_eq!(local_path(Some("/")), "/");
    }

    #[test]
    fn anything_else_leads_home() {
        for candidate in [
            None,
            Some(""),
            Some("downloads"),
            Some("//evil.example"),
            Some("https://evil.example"),
            Some("javascript:alert(1)"),
            Some("/\\evil.example"),
            Some("/\t/evil.example"),
            Some("/\n/evil.example"),
            Some("/a:b"),
        ] {
            assert_eq!(local_path(candidate), "/", "{candidate:?}");
        }
    }
}
