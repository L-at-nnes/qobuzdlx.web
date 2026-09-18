use md5::{Digest, Md5};

/// Signature of a `track/getFileUrl` request.
///
/// Qobuz wants the MD5 of the request name and its parameters in alphabetical order, concatenated
/// without separators, followed by the timestamp and the app secret.
#[must_use]
pub fn sign_file_url(track_id: u64, format_id: u32, timestamp: u64, app_secret: &str) -> String {
    md5_hex(&format!(
        "trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{app_secret}"
    ))
}

fn md5_hex(input: &str) -> String {
    crate::hex::encode(&Md5::digest(input.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_the_reference_vectors() {
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn the_signature_covers_the_parameters_the_timestamp_and_the_secret() {
        let signature = sign_file_url(197_432_204, 5, 1_700_000_000, "secret");

        assert_eq!(
            signature,
            md5_hex("trackgetFileUrlformat_id5intentstreamtrack_id1974322041700000000secret")
        );
        assert_eq!(signature.len(), 32);
    }

    #[test]
    fn changing_any_input_changes_the_signature() {
        let base = sign_file_url(1, 6, 100, "s");

        assert_ne!(base, sign_file_url(2, 6, 100, "s"));
        assert_ne!(base, sign_file_url(1, 27, 100, "s"));
        assert_ne!(base, sign_file_url(1, 6, 101, "s"));
        assert_ne!(base, sign_file_url(1, 6, 100, "t"));
    }
}
