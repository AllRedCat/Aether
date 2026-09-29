use std::io::Cursor;

/// Generates a valid minimal PNG image with given dimensions in memory.
pub fn create_synthetic_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::ImageBuffer::<image::Rgba<u8>, _>::new(width, height);
    let mut cursor = Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode synthetic PNG");
    cursor.into_inner()
}

/// Generates a valid standard 16-bit PCM RIFF/WAVE audio file in memory.
pub fn create_synthetic_wav(sample_rate: u32, channels: u16, duration_seconds: f64) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let num_samples = (sample_rate as f64 * duration_seconds).round() as usize;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;
    let data_size = (num_samples * block_align as usize) as u32;
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + data_size as usize);
    // RIFF Header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt subchunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    buf.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (1 for PCM)
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    buf.resize(44 + data_size as usize, 0); // PCM silence samples

    buf
}

/// Generates a valid minimal ISO-BMFF MP4 video file in memory.
pub fn create_synthetic_mp4(width: u16, height: u16, duration_seconds: u32) -> Vec<u8> {
    use mp4::{AvcConfig, Mp4Config, Mp4Sample, Mp4Writer, TrackConfig};

    let config = Mp4Config {
        major_brand: str::parse("isom").unwrap(),
        minor_version: 512,
        compatible_brands: vec![
            str::parse("isom").unwrap(),
            str::parse("iso2").unwrap(),
            str::parse("avc1").unwrap(),
            str::parse("mp41").unwrap(),
        ],
        timescale: 1000,
    };

    let cursor = Cursor::new(Vec::new());
    let mut writer = Mp4Writer::write_start(cursor, &config).expect("Failed to start MP4 writer");

    let track_config = TrackConfig::from(AvcConfig {
        width,
        height,
        seq_param_set: vec![0x67, 0x64, 0x00, 0x1f], // Minimal valid H.264 SPS prefix
        pic_param_set: vec![0x68, 0xe8, 0x38, 0x80], // Minimal valid H.264 PPS prefix
    });
    writer.add_track(&track_config).expect("Failed to add video track");

    let sample = Mp4Sample {
        start_time: 0,
        duration: duration_seconds * 1000,
        rendering_offset: 0,
        is_sync: true,
        bytes: bytes::Bytes::from_static(&[0x00, 0x00, 0x00, 0x02, 0x09, 0x10]),
    };
    writer.write_sample(1, &sample).expect("Failed to write video sample");
    writer.write_end().expect("Failed to finalize MP4");

    writer.into_writer().into_inner()
}
