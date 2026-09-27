use crate::qobuz::Album;
use crate::settings::ArtworkSize;

/// The size Qobuz serves for every release, and the one used when another is refused.
const DEFAULT_WIDTH: u16 = 600;

#[derive(Debug, thiserror::Error)]
pub enum CoverError {
    #[error("could not download the cover: {0}")]
    Http(reqwest::Error),
    /// Qobuz answered with something else than a picture, an error page for instance.
    #[error("the cover is not a picture")]
    NotAnImage,
}

impl From<reqwest::Error> for CoverError {
    /// The address is removed from the error, as it is everywhere else.
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

/// The address of a cover at a given size.
///
/// Qobuz serves the same picture as `…_50.jpg` up to `…_600.jpg`, and `…_org.jpg` for the file as
/// the label supplied it.
#[must_use]
pub fn cover_url(album: &Album, size: ArtworkSize) -> String {
    let suffix = match size {
        ArtworkSize::Original => "org".to_owned(),
        ArtworkSize::Pixels(width) => width.to_string(),
    };
    album.image.large.replace("_600", &format!("_{suffix}"))
}

/// Whether the bytes start like a JPEG, a PNG or a WebP file.
fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
}

/// Downloads covers.
#[derive(Debug, Clone)]
pub struct Covers {
    http: reqwest::Client,
}

impl Covers {
    #[must_use]
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// The cover at the requested size.
    ///
    /// The original is not available for every release: when Qobuz refuses a size, the 600 pixel
    /// version is used rather than going without a cover.
    pub async fn fetch(&self, album: &Album, size: ArtworkSize) -> Result<Vec<u8>, CoverError> {
        match self.get(&cover_url(album, size)).await {
            Err(error) if size != ArtworkSize::Pixels(DEFAULT_WIDTH) => {
                tracing::warn!(
                    album = %album.id,
                    ?size,
                    %error,
                    "cover size unavailable, using 600 pixels"
                );
                self.get(&cover_url(album, ArtworkSize::Pixels(DEFAULT_WIDTH)))
                    .await
            }
            result => result,
        }
    }

    async fn get(&self, url: &str) -> Result<Vec<u8>, CoverError> {
        let response = self.http.get(url).send().await?.error_for_status()?;
        let bytes = response.bytes().await?;
        if is_image(&bytes) {
            Ok(bytes.to_vec())
        } else {
            Err(CoverError::NotAnImage)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album_with_image(large: &str) -> Album {
        let mut album: Album =
            serde_json::from_str(r#"{"id": "a", "title": "T", "artist": {"id": 1}}"#).unwrap();
        album.image.large = large.to_owned();
        album
    }

    #[test]
    fn a_width_replaces_the_one_in_the_address() {
        let album = album_with_image("https://static.qobuz.com/images/covers/ab/cd/abcd_600.jpg");

        assert_eq!(
            cover_url(&album, ArtworkSize::Pixels(150)),
            "https://static.qobuz.com/images/covers/ab/cd/abcd_150.jpg"
        );
    }

    #[test]
    fn the_original_has_its_own_name() {
        let album = album_with_image("https://static.qobuz.com/images/covers/ab/cd/abcd_600.jpg");

        assert_eq!(
            cover_url(&album, ArtworkSize::Original),
            "https://static.qobuz.com/images/covers/ab/cd/abcd_org.jpg"
        );
    }

    #[test]
    fn the_default_width_is_the_address_as_given() {
        let address = "https://static.qobuz.com/images/covers/ab/cd/abcd_600.jpg";

        assert_eq!(
            cover_url(&album_with_image(address), ArtworkSize::Pixels(600)),
            address
        );
    }
}
