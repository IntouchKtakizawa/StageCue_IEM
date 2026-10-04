//! Offline waveform and conservative tempo suggestions.
#![forbid(unsafe_code)]

use stagecue_audio_engine::DecodedAudio;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveformPeak {
    pub min: f32,
    pub max: f32,
}

pub fn waveform_peaks(audio: &DecodedAudio, buckets: usize) -> Vec<WaveformPeak> {
    if buckets == 0 || audio.samples.is_empty() {
        return vec![];
    }
    let frames = audio.samples.len() / audio.channels as usize;
    let frames_per_bucket = frames.div_ceil(buckets).max(1);
    (0..buckets)
        .filter_map(|bucket| {
            let start = bucket * frames_per_bucket;
            if start >= frames {
                return None;
            }
            let end = ((bucket + 1) * frames_per_bucket).min(frames);
            let mut min = 1.0_f32;
            let mut max = -1.0_f32;
            for frame in start..end {
                for channel in 0..audio.channels as usize {
                    let value = audio.samples[frame * audio.channels as usize + channel];
                    min = min.min(value);
                    max = max.max(value);
                }
            }
            Some(WaveformPeak { min, max })
        })
        .collect()
}

/// Returns an explicitly low-authority BPM suggestion using onset autocorrelation.
pub fn estimate_bpm(audio: &DecodedAudio, min_bpm: u32, max_bpm: u32) -> Option<f64> {
    if audio.sample_rate == 0 || min_bpm == 0 || min_bpm >= max_bpm {
        return None;
    }
    let hop = 512usize;
    let channels = audio.channels as usize;
    let frames = audio.samples.len() / channels;
    if frames < audio.sample_rate as usize * 2 {
        return None;
    }
    let mut energy = Vec::with_capacity(frames / hop);
    for start in (0..frames).step_by(hop) {
        let end = (start + hop).min(frames);
        let mut sum = 0.0;
        for frame in start..end {
            let mono = (0..channels)
                .map(|c| audio.samples[frame * channels + c].abs())
                .sum::<f32>()
                / channels as f32;
            sum += mono;
        }
        energy.push(sum / (end - start) as f32);
    }
    let flux: Vec<f32> = energy.windows(2).map(|w| (w[1] - w[0]).max(0.0)).collect();
    let lag_min =
        (60.0 * audio.sample_rate as f64 / (max_bpm as f64 * hop as f64)).round() as usize;
    let lag_max =
        (60.0 * audio.sample_rate as f64 / (min_bpm as f64 * hop as f64)).round() as usize;
    (lag_min.max(1)..=lag_max.min(flux.len() / 2))
        .map(|lag| {
            let score = flux
                .iter()
                .skip(lag)
                .zip(&flux)
                .map(|(a, b)| a * b)
                .sum::<f32>();
            (lag, score)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(lag, _)| 60.0 * audio.sample_rate as f64 / (lag as f64 * hop as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_bounded_peaks() {
        let audio = DecodedAudio {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![-1.0, -0.5, 0.5, 1.0],
        };
        assert_eq!(
            waveform_peaks(&audio, 2),
            vec![
                WaveformPeak {
                    min: -1.0,
                    max: -0.5
                },
                WaveformPeak { min: 0.5, max: 1.0 }
            ]
        );
    }
}
