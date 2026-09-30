use axum::http::{HeaderMap, HeaderValue, header};

use crate::access::LIFETIME_SECONDS;

/// The cookie that tells the server a browser has unlocked the interface.
pub const NAME: &str = "qbdlx_access";

/// The token the browser sends, if it sends one.
#[must_use]
pub fn read(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == NAME)
        .map(|(_, value)| value.to_owned())
}

/// The `Set-Cookie` that remembers an unlocked device.
///
/// `HttpOnly`: scripts never read it. `Lax` rather than `None`: it only has to travel on navigations
/// of this site, and `None` would require `Secure`, which cannot be guaranteed over plain HTTP on a
/// private address. `Secure` is the operator's to declare (`COOKIE_SECURE`): behind a reverse proxy
/// that ends TLS the server only ever sees HTTP, and trusting `X-Forwarded-Proto` would mean trusting
/// anyone who can reach the server.
#[must_use]
pub fn set(token: &str, secure: bool) -> HeaderValue {
    build(token, LIFETIME_SECONDS, secure)
}

/// The `Set-Cookie` that makes the browser forget it.
#[must_use]
pub fn clear(secure: bool) -> HeaderValue {
    build("", 0, secure)
}

fn build(value: &str, max_age: u64, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    let text = format!("{NAME}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}");
    // Tokens are URL-safe base64 and a dot: always a valid header value
    HeaderValue::from_str(&text).expect("a cookie made of safe characters is a valid header")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookies: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for cookie in cookies {
            headers.append(header::COOKIE, HeaderValue::from_str(cookie).unwrap());
        }
        headers
    }

    #[test]
    fn the_token_is_found_among_other_cookies() {
        let found = read(&headers(&["theme=dark; qbdlx_access=abc.def; lang=fr"]));

        assert_eq!(found.as_deref(), Some("abc.def"));
    }

    #[test]
    fn the_token_is_found_in_a_second_header() {
        let found = read(&headers(&["theme=dark", "qbdlx_access=abc.def"]));

        assert_eq!(found.as_deref(), Some("abc.def"));
    }

    #[test]
    fn a_cookie_with_a_similar_name_is_not_the_token() {
        assert_eq!(
            read(&headers(&["not_qbdlx_access=abc", "qbdlx_access_2=abc"])),
            None
        );
        assert_eq!(read(&headers(&[])), None);
    }

    #[test]
    fn the_cookie_is_private_to_scripts_and_to_other_sites() {
        let text = set("abc.def", false).to_str().unwrap().to_owned();

        assert!(text.starts_with("qbdlx_access=abc.def;"));
        assert!(text.contains("HttpOnly"));
        assert!(text.contains("SameSite=Lax"));
        assert!(text.contains("Path=/"));
        assert!(text.contains(&format!("Max-Age={LIFETIME_SECONDS}")));
        assert!(!text.contains("Secure"));
    }

    #[test]
    fn the_operator_can_make_it_secure() {
        assert!(set("abc", true).to_str().unwrap().ends_with("; Secure"));
        assert!(clear(true).to_str().unwrap().ends_with("; Secure"));
    }

    #[test]
    fn clearing_expires_it_at_once() {
        let text = clear(false).to_str().unwrap().to_owned();

        assert!(text.starts_with("qbdlx_access=;"));
        assert!(text.contains("Max-Age=0"));
    }
}
