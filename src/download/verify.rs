use std::fs::File;
use std::io;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

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
    #[allow(clippy::cast_possible_truncation)] // A sample of 12 000 at most
    fn flac_bytes() -> Vec<u8> {
        let samples: Vec<i32> = (0..40_960)
            .flat_map(|index| {
                let value = (f64::from(index) * 0.05).sin() * 12_000.0;
                [value as i32, (value * 0.5) as i32]
            })
            .collect();
        let config = flacenc::config::Encoder::default()
            .into_verified()
            .expect("valid configuration");
        let source = flacenc::source::MemSource::from_samples(&samples, 2, 16, 44_100);
        let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
            .expect("encoding works");
        let mut sink = flacenc::bitsink::ByteSink::new();
        stream.write(&mut sink).expect("writing to memory works");
        sink.as_slice().to_vec()
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

    #[test]
    fn a_missing_file_is_an_io_error() {
        let directory = TempDir::new().unwrap();

        let error = verify_flac(&directory.path().join("absent.flac")).unwrap_err();

        assert!(matches!(error, Error::Io(_)));
    }
}
