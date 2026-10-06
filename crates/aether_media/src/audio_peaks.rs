use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use byteorder::{LittleEndian, WriteBytesExt};

use crate::decoder::{SymphoniaAudioDecoder, AudioDecoder};

pub const AETHER_PEAKS_MAGIC: &[u8; 8] = b"AETHPEAK";
pub const AETHER_PEAKS_VERSION: u32 = 1;

/// Extracts audio peaks (min/max pairs) and saves them into a binary cache file.
pub fn generate_waveform_cache(
    input_path: &str,
    output_path: &str,
    samples_per_peak: u32,
) -> Result<(), String> {
    if samples_per_peak == 0 {
        return Err("samples_per_peak must be greater than 0".to_string());
    }

    // Usamos o Symphonia diretamente, pois ele suporta extração de trilhas de áudio
    // de containers multimídia como MP4 e MOV, além dos formatos nativos de áudio.
    let mut decoder = SymphoniaAudioDecoder::open(input_path)
        .map_err(|e| format!("Failed to open audio decoder: {:?}", e))?;

    let channels = decoder.channels() as usize;
    if channels == 0 {
        return Err("Media has no audio channels".to_string());
    }
    
    let sample_rate = decoder.sample_rate();

    let mut peaks: Vec<Vec<(f32, f32)>> = vec![Vec::new(); channels];
    let mut current_samples: Vec<Vec<f32>> = vec![Vec::new(); channels];

    while let Ok(Some(buffer)) = decoder.next_audio_buffer() {
        let frame_count = buffer.frame_count();
        let samples = &buffer.samples_f32;

        for frame in 0..frame_count {
            for ch in 0..channels {
                let sample_idx = frame * channels + ch;
                if sample_idx < samples.len() {
                    let sample = samples[sample_idx];
                    current_samples[ch].push(sample);

                    if current_samples[ch].len() as u32 == samples_per_peak {
                        let (min, max) = compute_min_max(&current_samples[ch]);
                        peaks[ch].push((min, max));
                        current_samples[ch].clear();
                    }
                }
            }
        }
    }

    // Processar o resíduo final (se o áudio não for um múltiplo exato do samples_per_peak)
    for ch in 0..channels {
        if !current_samples[ch].is_empty() {
            let (min, max) = compute_min_max(&current_samples[ch]);
            peaks[ch].push((min, max));
            current_samples[ch].clear();
        }
    }

    save_peaks_binary(output_path, channels as u32, sample_rate, samples_per_peak, &peaks)?;

    Ok(())
}

fn compute_min_max(samples: &[f32]) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for &s in samples {
        if s < min {
            min = s;
        }
        if s > max {
            max = s;
        }
    }
    // Caso de slice vazio (nao deveria ocorrer, mas evita f32::MAX vazar)
    if min > max {
        return (0.0, 0.0);
    }
    (min, max)
}

fn save_peaks_binary(
    path: &str,
    channels: u32,
    sample_rate: u32,
    samples_per_peak: u32,
    peaks: &[Vec<(f32, f32)>],
) -> Result<(), String> {
    // Garante que o diretorio de destino existe
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache dir: {}", e))?;
    }

    let mut file = File::create(path).map_err(|e| format!("Failed to create peak file: {}", e))?;

    // Header
    file.write_all(AETHER_PEAKS_MAGIC).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(AETHER_PEAKS_VERSION).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(channels).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(sample_rate).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(samples_per_peak).map_err(|e| e.to_string())?;

    let peak_count = if peaks.is_empty() { 0 } else { peaks[0].len() as u32 };
    file.write_u32::<LittleEndian>(peak_count).map_err(|e| e.to_string())?;

    // Body: Para cada canal, grava min e max
    for ch_peaks in peaks {
        for &(min, max) in ch_peaks {
            file.write_f32::<LittleEndian>(min).map_err(|e| e.to_string())?;
            file.write_f32::<LittleEndian>(max).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_compute_min_max() {
        let samples = vec![0.1, -0.5, 0.8, -0.2, 0.0];
        let (min, max) = compute_min_max(&samples);
        assert_eq!(min, -0.5);
        assert_eq!(max, 0.8);
    }

    #[test]
    fn test_save_peaks_binary() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.aether_peaks");
        let path_str = file_path.to_str().unwrap();

        let peaks = vec![
            vec![( -0.5, 0.5 ), ( -0.8, 0.9 )], // Channel 0
            vec![( -0.4, 0.4 ), ( -0.7, 0.8 )], // Channel 1
        ];

        let result = save_peaks_binary(path_str, 2, 44100, 1000, &peaks);
        assert!(result.is_ok());

        let metadata = fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_audio_peaks_generation() {
        use hound;
        let dir = tempdir().unwrap();
        
        // 1. Create a synthetic wav file
        let wav_path = dir.path().join("synthetic.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        for t in 0..44100 {
            let sample = ( (t as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 10000.0 ) as i16;
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();

        // 2. Run generate_waveform_cache
        let output_path = dir.path().join("output.aether_peaks");
        let result = generate_waveform_cache(
            wav_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            1000,
        );
        
        assert!(result.is_ok());

        // 3. Verify output
        let metadata = fs::metadata(&output_path).unwrap();
        assert!(metadata.len() > 0);
    }
}
