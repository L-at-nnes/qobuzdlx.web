use crate::qobuz::StreamUrl;
use crate::settings::Quality;

/// Qualities that can be asked for, from the highest to the lowest.
const FLAC_QUALITIES: [Quality; 3] = [Quality::Flac24Max, Quality::Flac24, Quality::Flac16];

/// The qualities to try for a track, starting with the one chosen.
///
/// The CDN sometimes cuts the very high resolutions for an account, so a quality that keeps failing
/// gives way to the next one down. MP3 is never a fallback for FLAC: the file keeps the extension of
/// the format chosen, and an MP3 stream saved as `.flac` could not be tagged.
#[must_use]
pub fn fallback_chain(requested: Quality) -> Vec<Quality> {
    if !requested.is_flac() {
        return vec![requested];
    }
    FLAC_QUALITIES
        .into_iter()
        .skip_while(|quality| *quality != requested)
        .collect()
}

/// What a quality is called in the log.
#[must_use]
pub const fn quality_label(quality: Quality) -> &'static str {
    match quality {
        Quality::Mp3 => "MP3 320",
        Quality::Flac16 => "FLAC 16bit/44.1kHz",
        Quality::Flac24 => "FLAC 24bit/96kHz",
        Quality::Flac24Max => "FLAC 24bit/192kHz",
    }
}

/// The quality of the stream Qobuz actually served.
///
/// Asking for a quality a track does not have is not an error: Qobuz answers with the best it has.
/// The log therefore has to describe the answer, not the request.
#[must_use]
pub fn describe_stream(stream: &StreamUrl, requested: Quality) -> String {
    if stream.format_id == 5 || stream.mime_type.as_deref() == Some("audio/mpeg") {
        return quality_label(Quality::Mp3).to_owned();
    }
    if stream.bit_depth > 0 && stream.sampling_rate > 0.0 {
        return format!("FLAC {}bit/{}kHz", stream.bit_depth, stream.sampling_rate);
    }
    quality_label(requested).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(format_id: u32, bit_depth: u32, sampling_rate: f64, mime: Option<&str>) -> StreamUrl {
        StreamUrl {
            url: Some("https://cdn.example/file".into()),
            format_id,
            mime_type: mime.map(str::to_owned),
            sampling_rate,
            bit_depth,
            sample: false,
        }
    }

    #[test]
    fn the_highest_quality_steps_down_through_every_flac() {
        assert_eq!(
            fallback_chain(Quality::Flac24Max),
            [Quality::Flac24Max, Quality::Flac24, Quality::Flac16]
        );
    }

    #[test]
    fn a_middle_quality_only_steps_down() {
        assert_eq!(
            fallback_chain(Quality::Flac24),
            [Quality::Flac24, Quality::Flac16]
        );
    }

    #[test]
    fn cd_quality_has_nothing_below_it() {
        assert_eq!(fallback_chain(Quality::Flac16), [Quality::Flac16]);
    }

    #[test]
    fn mp3_stays_mp3_and_flac_never_falls_back_to_it() {
        assert_eq!(fallback_chain(Quality::Mp3), [Quality::Mp3]);
        assert!(!fallback_chain(Quality::Flac24Max).contains(&Quality::Mp3));
    }

    #[test]
    fn a_flac_stream_is_described_by_what_was_served() {
        let served = stream(7, 24, 96.0, Some("audio/flac"));

        assert_eq!(
            describe_stream(&served, Quality::Flac24Max),
            "FLAC 24bit/96kHz"
        );
    }

    #[test]
    fn a_fractional_sampling_rate_keeps_its_decimals() {
        let served = stream(6, 16, 44.1, None);

        assert_eq!(
            describe_stream(&served, Quality::Flac16),
            "FLAC 16bit/44.1kHz"
        );
    }

    #[test]
    fn an_mp3_answer_is_recognised_by_format_or_by_mime_type() {
        assert_eq!(
            describe_stream(&stream(5, 0, 0.0, None), Quality::Flac16),
            "MP3 320"
        );
        assert_eq!(
            describe_stream(&stream(6, 0, 0.0, Some("audio/mpeg")), Quality::Flac16),
            "MP3 320"
        );
    }

    #[test]
    fn an_answer_without_details_falls_back_to_the_request() {
        assert_eq!(
            describe_stream(&stream(27, 0, 0.0, None), Quality::Flac24Max),
            "FLAC 24bit/192kHz"
        );
    }
}
