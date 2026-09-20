//! What quality a download really has, as the name variables describe it.
//!
//! A folder must say what its files are, not what the release could have offered: a 24-bit / 96 kHz
//! album downloaded as 16-bit FLAC is not "24bit-96kHz".

use crate::settings::Quality;

const CD_BIT_DEPTH: u32 = 16;
const CD_SAMPLING_RATE: f64 = 44.1;
const HI_RES_CEILING: f64 = 96.0;
const MAX_BIT_DEPTH_STANDARD: u32 = 24;

/// Bit depth and sampling rate (kHz) of an audio file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioQuality {
    pub bit_depth: u32,
    pub sampling_rate: f64,
}

/// The quality of the file a download yields, for a release or a track whose best is
/// (`max_bit_depth`, `max_sampling_rate`).
///
/// Qobuz serves the best quality that does not exceed the one asked for: CD quality for MP3 and for
/// FLAC 16, up to 24-bit / 96 kHz for FLAC 24 (a 176.4 kHz master comes down to 88.2 kHz, its closest
/// multiple of 44.1), the best there is for the highest tier. Missing values count as CD quality for
/// the lower tiers.
#[must_use]
pub fn effective(quality: Quality, max_bit_depth: u32, max_sampling_rate: f64) -> AudioQuality {
    match quality {
        Quality::Mp3 | Quality::Flac16 => AudioQuality {
            bit_depth: if max_bit_depth > 0 {
                max_bit_depth.min(CD_BIT_DEPTH)
            } else {
                CD_BIT_DEPTH
            },
            sampling_rate: if max_sampling_rate > 0.0 {
                max_sampling_rate.min(CD_SAMPLING_RATE)
            } else {
                CD_SAMPLING_RATE
            },
        },
        Quality::Flac24 => AudioQuality {
            bit_depth: max_bit_depth.min(MAX_BIT_DEPTH_STANDARD),
            sampling_rate: if max_sampling_rate <= HI_RES_CEILING {
                max_sampling_rate
            } else if (max_sampling_rate - 176.4).abs() < 0.05 {
                88.2
            } else {
                HI_RES_CEILING
            },
        },
        Quality::Flac24Max => AudioQuality {
            bit_depth: max_bit_depth,
            sampling_rate: max_sampling_rate,
        },
    }
}

impl AudioQuality {
    /// `24bit-96kHz`. The rate is written the same way whatever the interface language: `44.1`, never
    /// `44,1`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}bit-{}kHz", self.bit_depth, self.sampling_rate)
    }

    fn is_known(&self) -> bool {
        self.bit_depth > 0 && self.sampling_rate > 0.0
    }

    /// Hi-res is more than CD quality.
    #[must_use]
    pub fn is_hi_res(&self) -> bool {
        self.bit_depth > CD_BIT_DEPTH
    }
}

/// `MP3` or `FLAC`.
#[must_use]
pub const fn format_name(quality: Quality) -> &'static str {
    if quality.is_flac() { "FLAC" } else { "MP3" }
}

/// The format and its quality: `FLAC (16bit-44.1kHz)`, or just `MP3` which has no such thing.
#[must_use]
pub fn format_with_quality(quality: Quality, audio: AudioQuality) -> String {
    if !quality.is_flac() || !audio.is_known() {
        return format_name(quality).to_owned();
    }
    format!("FLAC ({})", audio.label())
}

/// Like [`format_with_quality`], but only hi-res files get their quality written: CD-quality FLAC is
/// plain `FLAC`.
#[must_use]
pub fn format_with_hi_res_quality(quality: Quality, audio: AudioQuality) -> String {
    if audio.is_hi_res() {
        format_with_quality(quality, audio)
    } else {
        format_name(quality).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four name variables of a release the way a template sees them.
    fn names(quality: Quality, bits: u32, rate: f64) -> String {
        let audio = effective(quality, bits, rate);
        format!(
            "{}_{}_{}_{}",
            format_with_quality(quality, audio),
            format_with_hi_res_quality(quality, audio),
            audio.bit_depth,
            audio.sampling_rate
        )
    }

    #[test]
    fn sixteen_bit_asked_on_a_hi_res_release_is_labelled_cd_quality() {
        // Found live: the folder said 24bit-96kHz and held 16-bit files
        assert_eq!(
            names(Quality::Flac16, 24, 96.0),
            "FLAC (16bit-44.1kHz)_FLAC_16_44.1"
        );
    }

    #[test]
    fn the_standard_hi_res_tier_is_capped_at_96_khz() {
        assert_eq!(
            names(Quality::Flac24, 24, 96.0),
            "FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96"
        );
        assert_eq!(
            names(Quality::Flac24, 24, 192.0),
            "FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96"
        );
        assert_eq!(
            names(Quality::Flac24, 24, 48.0),
            "FLAC (24bit-48kHz)_FLAC (24bit-48kHz)_24_48"
        );
    }

    #[test]
    fn a_176_4_khz_master_comes_down_to_88_2_in_the_standard_tier() {
        assert_eq!(
            names(Quality::Flac24, 24, 176.4),
            "FLAC (24bit-88.2kHz)_FLAC (24bit-88.2kHz)_24_88.2"
        );
    }

    #[test]
    fn the_highest_tier_keeps_what_the_release_has() {
        assert_eq!(
            names(Quality::Flac24Max, 24, 192.0),
            "FLAC (24bit-192kHz)_FLAC (24bit-192kHz)_24_192"
        );
        assert_eq!(
            names(Quality::Flac24Max, 24, 96.0),
            "FLAC (24bit-96kHz)_FLAC (24bit-96kHz)_24_96"
        );
        assert_eq!(
            names(Quality::Flac24Max, 16, 44.1),
            "FLAC (16bit-44.1kHz)_FLAC_16_44.1"
        );
    }

    #[test]
    fn cd_quality_is_never_called_hi_res() {
        assert_eq!(
            names(Quality::Flac16, 16, 44.1),
            "FLAC (16bit-44.1kHz)_FLAC_16_44.1"
        );
        assert_eq!(
            names(Quality::Flac16, 16, 48.0),
            "FLAC (16bit-44.1kHz)_FLAC_16_44.1"
        );
    }

    #[test]
    fn mp3_has_no_quality_to_write() {
        assert_eq!(names(Quality::Mp3, 24, 96.0), "MP3_MP3_16_44.1");
    }

    #[test]
    fn unknown_values_do_not_produce_nonsense() {
        assert_eq!(names(Quality::Flac24Max, 0, 0.0), "FLAC_FLAC_0_0");
        assert_eq!(
            names(Quality::Flac16, 0, 0.0),
            "FLAC (16bit-44.1kHz)_FLAC_16_44.1"
        );
    }
}
