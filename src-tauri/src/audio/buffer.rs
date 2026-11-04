use std::io::Cursor;

/// Convert raw f32 PCM samples to WAV bytes (16kHz mono)
pub fn to_wav(samples: &[f32], sample_rate: u32) -> anyhow::Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buf = Vec::new();
    let cursor = Cursor::new(&mut buf);
    let mut writer = hound::WavWriter::new(cursor, spec)?;

    for &sample in samples {
        let s = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(buf)
}

/// Cut leading/trailing silence so Whisper decodes less audio and has fewer
/// silent stretches to hallucinate over. Frame energy is compared against a
/// threshold relative to the loudest frame, so quiet-but-real speech on a
/// low-gain mic survives. Keeps `PAD_MS` of context on each side.
///
/// Returns the input unchanged when it can't find a confident speech region
/// (all-silence clips must reach the pipeline's dead-mic diagnostics intact).
pub fn trim_silence(samples: &[f32], sample_rate: u32) -> &[f32] {
    const FRAME_MS: usize = 30;
    const PAD_MS: usize = 250;
    let frame = (sample_rate as usize * FRAME_MS / 1000).max(1);
    if samples.len() < frame * 4 {
        return samples;
    }

    let energies: Vec<f32> = samples
        .chunks(frame)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    let loudest = energies.iter().cloned().fold(0.0f32, f32::max);
    if loudest < 0.003 {
        return samples; // effectively silence — leave it for the dead-mic check
    }
    let threshold = (loudest * 0.06).max(0.0015);

    let first = energies.iter().position(|&e| e >= threshold);
    let last = energies.iter().rposition(|&e| e >= threshold);
    let (Some(first), Some(last)) = (first, last) else { return samples };

    let pad = sample_rate as usize * PAD_MS / 1000;
    let start = (first * frame).saturating_sub(pad);
    let end = ((last + 1) * frame + pad).min(samples.len());
    // Don't bother for tiny savings, and never return less than 0.5 s.
    if end - start < sample_rate as usize / 2 || samples.len() - (end - start) < frame * 3 {
        return samples;
    }
    &samples[start..end]
}

#[cfg(test)]
mod tests {
    use super::trim_silence;

    fn tone(len: usize, amp: f32) -> Vec<f32> {
        (0..len).map(|i| amp * (i as f32 * 0.05).sin()).collect()
    }

    #[test]
    fn trims_leading_and_trailing_silence() {
        let rate = 16_000;
        let mut s = vec![0.0; rate * 2];
        s.extend(tone(rate, 0.3));
        s.extend(vec![0.0; rate * 2]);
        let t = trim_silence(&s, rate as u32);
        // 1 s of speech + 2 × 250 ms padding (± one frame)
        assert!(t.len() >= rate + rate / 2 - 480 && t.len() <= rate + rate / 2 + 960, "{}", t.len());
    }

    #[test]
    fn leaves_pure_silence_untouched() {
        let s = vec![0.0001; 32_000];
        assert_eq!(trim_silence(&s, 16_000).len(), s.len());
    }

    #[test]
    fn keeps_quiet_speech() {
        let rate = 16_000;
        let mut s = vec![0.0; rate];
        s.extend(tone(rate, 0.3));
        s.extend(vec![0.0; rate / 4]);
        s.extend(tone(rate / 2, 0.04)); // quiet trailing word
        let t = trim_silence(&s, rate as u32);
        assert!(t.len() >= rate + rate / 4 + rate / 2, "quiet tail was cut: {}", t.len());
    }
}
