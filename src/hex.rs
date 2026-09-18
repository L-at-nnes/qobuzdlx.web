//! Lowercase hexadecimal encoding.

use std::fmt::Write;

/// Encodes `bytes` as lowercase hexadecimal, two digits per byte.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            // Writing into a String cannot fail
            let _ = write!(text, "{byte:02x}");
            text
        })
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn every_byte_takes_two_lowercase_digits() {
        assert_eq!(encode(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn nothing_encodes_to_nothing() {
        assert_eq!(encode(&[]), "");
    }
}
