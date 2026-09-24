use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use md5::{Digest, Md5};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// Where the MD5 of the audio sits in a FLAC file: after the marker, the header of the first
/// metadata block, and the 18 bytes of STREAMINFO that precede it.
const MD5_OFFSET: usize = 4 + 4 + 18;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read the file: {0}")]
    Io(#[from] io::Error),
    #[error("not a FLAC file")]
    NotFlac,
    #[error("{0}")]
    Damaged(String),
}

/// Decodes a whole FLAC file to check it.
///
/// A download that reaches its announced size can still hold damaged frames, and it would only be
/// noticed when listening to it. Every frame carries a checksum that the decoder verifies, the
/// checksum of the whole audio is compared when the file has one (Qobuz files often have none), and
/// the number of samples decoded must be the number announced, which is what a file cut short lacks.
///
/// Blocking: the whole file is read, run it where blocking is acceptable.
pub fn verify_flac(path: &Path) -> Result<(), Error> {
    decode(path, None)
}

/// Writes the MD5 of the audio into the header of a FLAC file.
///
/// Files from Qobuz often carry none, which players that verify the audio then report. The audio is
/// decoded to compute it, so the file is checked on the way, and only the 16 bytes of the checksum
/// are rewritten: nothing is re-encoded.
///
/// Blocking, like [`verify_flac`].
pub fn fix_md5(path: &Path) -> Result<(), Error> {
    let mut hasher = Md5::new();
    // The header is read first: only a file that starts with the marker and STREAMINFO is patched
    let mut header = [0u8; 8];
    File::open(path)?
        .read_exact(&mut header)
        .map_err(|_| Error::NotFlac)?;
    if &header[..4] != b"fLaC" || header[4] & 0x7F != 0 {
        return Err(Error::NotFlac);
    }

    decode(path, Some(&mut hasher))?;

    let mut file = OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(MD5_OFFSET as u64))?;
    file.write_all(&hasher.finalize())?;
    file.sync_all()?;
    Ok(())
}

/// Decodes the file, feeding the audio to `hasher` when there is one.
fn decode(path: &Path, mut hasher: Option<&mut Md5>) -> Result<(), Error> {
    let source = MediaSourceStream::new(
        Box::new(File::open(path)?),
        MediaSourceStreamOptions::default(),
    );
    let mut hint = Hint::new();
    hint.with_extension("flac");

    let mut reader = symphonia::default::get_probe()
        .probe(
            &hint,
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| match error {
            DecodeError::Unsupported(_) => Error::NotFlac,
            other => damaged(&other),
        })?;

    let track = reader
        .default_track(TrackType::Audio)
        .ok_or(Error::NotFlac)?;
    let track_id = track.id;
    let expected_frames = track.num_frames;
    let parameters = track
        .codec_params
        .as_ref()
        .and_then(|parameters| parameters.audio())
        .ok_or(Error::NotFlac)?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(parameters, &AudioDecoderOptions::default().verify(true))
        .map_err(|_| Error::NotFlac)?;

    let mut decoded_frames: u64 = 0;
    let Some(bits @ 1..=32) = parameters.bits_per_sample else {
        return Err(Error::NotFlac);
    };
    let sample_width = bits.div_ceil(8) as usize;
    let mut samples: Vec<i32> = Vec::new();
    let mut bytes = Vec::new();
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(error) => return Err(damaged(&error)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let audio = decoder.decode(&packet).map_err(|error| damaged(&error))?;
        decoded_frames += audio.frames() as u64;
        if let Some(hasher) = hasher.as_deref_mut() {
            // What FLAC computes its MD5 over: the samples interleaved, little-endian, in as many
            // whole bytes as they need. The decoder hands them out scaled up to 32 bits.
            audio.copy_to_vec_interleaved::<i32>(&mut samples);
            bytes.clear();
            for sample in &samples {
                bytes.extend_from_slice(&(sample >> (32 - bits)).to_le_bytes()[..sample_width]);
            }
            hasher.update(&bytes);
        }
    }
    if decoder.finalize().verify_ok == Some(false) {
        return Err(Error::Damaged(
            "the audio does not match its checksum".to_owned(),
        ));
    }
    if let Some(expected) = expected_frames
        && decoded_frames != expected
    {
        return Err(Error::Damaged(format!(
            "{decoded_frames} samples instead of {expected}: the file is incomplete"
        )));
    }
    Ok(())
}

fn damaged(error: &DecodeError) -> Error {
    Error::Damaged(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    use tempfile::TempDir;

    /// A stereo sine wave, as a FLAC file. Ten full blocks: the encoder records the size of a short
    /// last block as the smallest block of the stream, which the decoder does not accept.
    #[allow(clippy::cast_possible_truncation)] // The amplitude fits the bit depth
    fn flac_with_depth(bits: usize) -> Vec<u8> {
        let amplitude = f64::from((1u32 << (bits - 2)) - 1);
        let samples: Vec<i32> = (0..40_960)
            .flat_map(|index| {
                let value = (f64::from(index) * 0.05).sin() * amplitude;
                [value as i32, (value * 0.5) as i32]
            })
            .collect();
        let config = flacenc::config::Encoder::default()
            .into_verified()
            .expect("valid configuration");
        let source = flacenc::source::MemSource::from_samples(&samples, 2, bits, 44_100);
        let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
            .expect("encoding works");
        let mut sink = flacenc::bitsink::ByteSink::new();
        stream.write(&mut sink).expect("writing to memory works");
        sink.as_slice().to_vec()
    }

    fn flac_bytes() -> Vec<u8> {
        flac_with_depth(16)
    }

    fn check(bytes: &[u8]) -> Result<(), Error> {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("track.flac");
        std::fs::write(&path, bytes).unwrap();
        verify_flac(&path)
    }

    #[test]
    fn an_intact_file_passes() {
        check(&flac_bytes()).unwrap();
    }

    #[test]
    fn a_file_cut_short_is_refused() {
        let bytes = flac_bytes();

        let error = check(&bytes[..bytes.len() / 2]).unwrap_err();

        assert!(matches!(error, Error::Damaged(_)), "{error}");
    }

    #[test]
    fn a_damaged_frame_is_refused() {
        let mut bytes = flac_bytes();
        let middle = bytes.len() / 2;
        for byte in &mut bytes[middle..middle + 16] {
            *byte ^= 0xA5;
        }

        let error = check(&bytes).unwrap_err();

        assert!(matches!(error, Error::Damaged(_)), "{error}");
    }

    #[test]
    fn something_that_is_not_flac_is_refused() {
        let error = check(b"this is certainly not audio, just some text").unwrap_err();

        assert!(matches!(error, Error::NotFlac), "{error}");
    }

    /// The MD5 the encoder wrote, and the file with that field zeroed like Qobuz's.
    fn without_md5(bytes: &[u8]) -> (Vec<u8>, [u8; 16]) {
        let start = MD5_OFFSET;
        let mut original = [0u8; 16];
        original.copy_from_slice(&bytes[start..start + 16]);
        let mut stripped = bytes.to_vec();
        stripped[start..start + 16].fill(0);
        (stripped, original)
    }

    #[test]
    fn a_missing_md5_is_computed_and_written() {
        let bytes = flac_bytes();
        let (stripped, original) = without_md5(&bytes);
        assert_ne!(original, [0u8; 16], "the encoder writes a checksum");
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("track.flac");
        std::fs::write(&path, &stripped).unwrap();

        fix_md5(&path).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        verify_flac(&path).unwrap();
    }

    #[test]
    fn the_md5_is_computed_for_24_bit_audio_too() {
        // Hi-res is what most downloads are
        let bytes = flac_with_depth(24);
        let (stripped, original) = without_md5(&bytes);
        assert_ne!(original, [0u8; 16]);
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("track.flac");
        std::fs::write(&path, &stripped).unwrap();

        fix_md5(&path).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn a_wrong_md5_is_reported_by_the_check() {
        let mut bytes = flac_bytes();
        bytes[MD5_OFFSET] ^= 0xFF;

        let error = check(&bytes).unwrap_err();

        assert!(matches!(error, Error::Damaged(_)), "{error}");
    }

    #[test]
    fn a_damaged_file_is_not_given_a_checksum() {
        let mut bytes = flac_bytes();
        let middle = bytes.len() / 2;
        bytes[middle..middle + 16].fill(0x5A);
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("track.flac");
        std::fs::write(&path, &bytes).unwrap();

        assert!(fix_md5(&path).is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "the file is left as it was"
        );
    }

    #[test]
    fn the_md5_of_something_that_is_not_flac_is_refused() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("track.flac");
        std::fs::write(&path, b"RIFF....WAVEfmt not a flac file at all").unwrap();

        assert!(matches!(fix_md5(&path), Err(Error::NotFlac)));
    }

    #[test]
    fn a_missing_file_is_an_io_error() {
        let directory = TempDir::new().unwrap();

        let error = verify_flac(&directory.path().join("absent.flac")).unwrap_err();

        assert!(matches!(error, Error::Io(_)));
    }
}
