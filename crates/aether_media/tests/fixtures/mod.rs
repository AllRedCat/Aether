#![allow(dead_code)]

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

pub const SAMPLE_H264_MP4_BASE64: &str = "\
AAAAIGZ0eXBpc29tAAACAGlzb21pc28yYXZjMW1wNDEAAANLbW9vdgAAAGxtdmhkAAAAAAAAAAAAAAAA\
AAAD6AAAA+gAAQAAAQAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAA\
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAnZ0cmFrAAAAXHRraGQAAAADAAAAAAAAAAAAAAAB\
AAAAAAAAA+gAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAA\
AEAAAABAAAAAAAAkZWR0cwAAABxlbHN0AAAAAAAAAAEAAAPoAAAAAAABAAAAAAHubWRpYQAAACBtZGhk\
AAAAAAAAAAAAAAAAAAAoAAAAKABVxAAAAAAALWhkbHIAAAAAAAAAAHZpZGUAAAAAAAAAAAAAAABWaWRl\
b0hhbmRsZXIAAAABmW1pbmYAAAAUdm1oZAAAAAEAAAAAAAAAAAAAACRkaW5mAAAAHGRyZWYAAAAAAAAA\
AQAAAAx1cmwgAAAAAQAAAVlzdGJsAAAAuXN0c2QAAAAAAAAAAQAAAKlhdmMxAAAAAAAAAAEAAAAAAAAA\
AAAAAAAAAAAAAEAAQABIAAAASAAAAAAAAAABFExhdmM2My4xLjEwMSBsaWJ4MjY0AAAAAAAAAAAAAAAAGP//\
AAAAL2F2Y0MBQsAe/+EAF2dCwB7ZBCbARAAAAwAEAAADAFA8WLkgAQAFaMuDyyAAAAAQcGFzcAAAAAEA\
AAABAAAAFGJ0cnQAAAAAAAA/oAAAAAAAAAAYc3R0cwAAAAAAAAABAAAACgAABAAAAAAUc3RzcwAAAAAA\
AAABAAAAAQAAABxzdHNjAAAAAAAAAAEAAAABAAAACgAAAAEAAAA8c3RzegAAAAAAAAAAAAAACgAABgAA\
AAAnAAAAOwAAAE4AAAAvAAAAUgAAAC0AAABJAAAAIAAAAC0AAAAUc3RjbwAAAAAAAAABAAADewAAAGF1\
ZHRhAAAAWW1ldGEAAAAAAAAAIWhkbHIAAAAAAAAAAG1kaXJhcHBsAAAAAAAAAAAAAAAALGlsc3QAAAAk\
cXRvbwAAABxkYXRhAAAAAQAAAABMYXZmNjMuMS4xMDEAAAAIZnJlZQAAB/xtZGF0AAACcQYF//9t3EXp\
vebZSLeWLNgg2SPu73gyNjQgLSBjb3JlIDE2NSByMzIyMiBiMzU2MDVhIC0gSC4yNjQvTVBFRy00IEFW\
QyBjb2RlYyAtIENvcHlsZWZ0IDIwMDMtMjAyNSAtIGh0dHA6Ly93d3cudmlkZW9sYW4ub3JnL3gyNjQu\
aHRtbCAtIG9wdGlvbnM6IGNhYmFjPTAgcmVmPTMgZGVibG9jaz0xOjA6MCBhbmFseXNlPTB4MToweDEx\
MSBtZT1oZXggc3VibWU9NyBwc3k9MSBwc3lfcmQ9MS4wMDowLjAwIG1peGVkX3JlZj0xIG1lX3Jhbmdl\
PTE2IGNocm9tYV9tZT0xIHRyZWxsaXM9MSA4eDhkY3Q9MCBjcW09MCBkZWFkem9uZT0yMSwxMSBmYXN0\
X3Bza2lwPTEgY2hyb21hX3FwX29mZnNldD0tMiB0aHJlYWRzPTIgbG9va2FoZWFkX3RocmVhZHM9MSBz\
bGljZWRfdGhyZWFkcz0wIG5yPTAgZGVjaW1hdGU9MSBpbnRlcmxhY2VkPTAgYmx1cmF5X2NvbXBhdD0w\
IGNvbnN0cmFpbmVkX2ludHJhPTAgYmZyYW1lcz0wIHdlaWdodHA9MCBrZXlpbnQ9MjUwIGtleWludF9t\
aW49MTAgc2NlbmVjdXQ9NDAgaW50cmFfcmVmcmVzaD0wIHJjX2xvb2thaGVhZD00MCByYz1jcmYgbWJ0\
cmVlPTEgY3JmPTIzLjAgcWNvbXA9MC42MCBxcG1pbj0wIHFwbWF4PTY5IHFwc3RlcD00IGlwX3JhdGlv\
PTEuNDAgYXE9MToxLjAwAIAAAAOHZYiEJwxgAICElguLfONxKcCYXlf+ADytes1YKT+uuADwCVQ+kEaU\
Czf66AB52tWauFJ/XQYcQAA0ByIAAIGIAAgKGAEBb0LO4Qjrex93CABozMgITFieqKKBciIgITFCOKKL\
cicQApfjYBI/QAAgHbNn6M12xbP7HYAsPxzsXZsGDKfGaAn98IASAAoAHBAACACAAINoDgKZUFRImfaB\
PzVYB338BQmYwf74gqEiZ94D03EpSw/EPD/hqDgAEwFDRyW/geBgKyAjLTQOvL8IAQAGBoAAghhAAHQQ\
AzpAABIDDJYAvpgsSZwwO6f78quWAL6MBIJGLOAZ8/3/MPrFKeAAAIB2AC2qQKaBPJPWeKAAIB39YADz\
Vp1NzX/QXyJiKmOb8j7IHoAaLxYp26lIpn/9eEAAaAHsBgQAAgYgACDOgAFV6dglIX5wYFPzXGkz9ET8\
wE8YApA9xwYENENlMXOMlag7MxB2h+F4AFxh425fWiP/15mbG7vbK5R1eGhQuZcSxjEoWWH4TEM1/G1a\
vwskMaUS0zMrJnv/Sn0gw4J6wAH0AFxmIjJ1FYj/98E5BPnLfrFdppXzJsToVLFZPIf3R8j4LXCH/+C2\
BwADAAAgJgtBgD4wNlDNMEpMhE9XvKjLjUvLo7LFMRd3abdVLxTzK47LWy///9mC4TAcAAyAGA00mQAk\
awG3kbUP2tR/+d1sv47LxDHhgoBmEBABuDgj/g4gXB98AWG0wkhcwR9FqZ/xxeAAIAEwDVepNEPq//xx\
+FAAEA0AGAB4YAAgJAACAiAAIBgFANCmjQO0Z2wIU1ozz+xKAGhKAGoDDQmk8AnJpHjQoZIlJ/R/kAKA\
IPB4fFHm4n8XE1C0HtAIKzQEjKF+cAOQjAWWwE6MX7PIqXqGEAAWAAECVQ8IAAiAAfAB5kR5i2A6yRRn\
4QDubOAxJnfAZeeFIFUM9nEbXgEAcx7eIEZZf/hEQB3g4S8HCF+DkB8HRfAFjQyqDTBDTOczr8QDIceI\
mOPhcyzoykQICAAIFCwABAMCAAEA0KACbL0HWMrjUADaEI2cD69bgQBvX3waAAawnAAaFR8MtngG6s22\
gQCCf99hh2coAAcVYoeXin4AfS/9Czrv2G6nq/AAABr48EooiWTst4GCDDggEACAewaosw4ACxuMivMs\
b/jxgQTAAOi+TTGvc8fMGYCFn/EBIaXAAAAAI0GaOE41CImUxMGPfQjghLbtvoRwUT2e+aW5oRwTadNI\
kcvEAAAAN0GaVBuNWKi4KCEsYa+ojX5/GsR0WtYrhiF3wuuW9O1VlO5+OxJNZsviIyJ4axqlVQoTEhgf\
f8AAAABKQZpg3Gpw1w2QMZZmfY/LUflv2lCXDBR/35I+WaoxLJMshmnafrEcMQXeWXtfxhr3OKeifrFc\
EGNUqqIMt9ml95zNAqV+xq4MS+AAAAArQZqA3GrERMFBBjyhr+PtfvrEcEhY3K+tYjhsi/y/3zfyaMS6\
xHBFm838wAAAAE5BmqBHGrERMMQxlkHv32/mGDVy+Y2fSdl1iuHMPst8wjUSHM7039YjhinZb4c/Mz3N\
8+p0ETZfETT1BBgxVJRVER74rprwvYrSqn0cSYcAAAApQZrAVxqcNcFEY99x/3Zp/BJj/vhrFcEd9j8b\
3NYjhfGqVVLM/5oNP34AAABFQZrgVxqxETDEYa/Pf/+Y2Py18usRw5JE99hGokmsNJvUOfrEcMR9l8MZ\
ZBlvXhBudtmqJdYjhfGqVVB+NQZb+Z/FPDulAAAAHEGbAHcasRE1fWI4JLj7L52sR1bWI4JJMJeAT4AA\
AAApQZsgJcanDXDFJ8+fLuTfrFcEk9s9zWI+sVwvm83Jc6QYuPy3mYnCfYA=";

/// Ensures `sample_64x64_10frames.mp4` exists in `tests/fixtures/` and returns its `PathBuf`.
pub fn ensure_sample_mp4() -> std::path::PathBuf {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample_64x64_10frames.mp4");

    if !fixture_path.exists() {
        if let Some(parent) = fixture_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let decoded = decode_simple_base64(SAMPLE_H264_MP4_BASE64);
        std::fs::write(&fixture_path, decoded).expect("Failed to write sample_64x64_10frames.mp4 fixture");
    }

    fixture_path
}

fn decode_simple_base64(s: &str) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity((s.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for b in s.bytes() {
        if b == b'=' || b.is_ascii_whitespace() {
            continue;
        }
        if let Some(val) = TABLE.iter().position(|&x| x == b) {
            buf = (buf << 6) | (val as u32);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
    }
    out
}
