//! Deterministic sidecar cache path generator and atomic cache writer.
//!
//! Cache keys are derived deterministically from:
//! `SHA-256(SHA-256(partial_header_bytes) || file_size_le || mtime_secs_le || mtime_nanos_le)`.
//!
//! Any change to the leading header bytes, total file size, or modification
//! timestamp automatically produces a new cache key and invalidates stale
//! sidecar artifacts without requiring manual cache purging.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use crate::MediaError;

/// Number of leading bytes hashed from the start of a media file (64 KiB).
pub const PARTIAL_HASH_WINDOW_BYTES: usize = 64 * 1024;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Deterministic fingerprint of a source media file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFingerprint {
    /// SHA-256 of the first `PARTIAL_HASH_WINDOW_BYTES` (or entire file if smaller).
    pub partial_hash: [u8; 32],
    /// Total file size in bytes.
    pub file_size: u64,
    /// Last modification timestamp (seconds since `UNIX_EPOCH`).
    pub mtime_secs: u64,
    /// Sub-second nanoseconds of the modification timestamp.
    pub mtime_nanos: u32,
    /// Combined 32-byte digest over `(partial_hash || file_size || mtime)`.
    pub digest: [u8; 32],
    /// 64-character lowercase hex representation of `digest`.
    pub cache_key: String,
}

/// Computes the deterministic [`SourceFingerprint`] for a file on disk from
/// `partial_header + file_size + mtime`.
pub fn compute_source_fingerprint(path: &Path) -> Result<SourceFingerprint, MediaError> {
    if !path.exists() {
        return Err(MediaError::FileNotFound(path.display().to_string()));
    }
    if !path.is_file() {
        return Err(MediaError::NotAFile(path.display().to_string()));
    }

    let meta = fs::metadata(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let file_size = meta.len();
    if file_size == 0 {
        return Err(MediaError::EmptyFile(path.display().to_string()));
    }

    let (mtime_secs, mtime_nanos) = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| (d.as_secs(), d.subsec_nanos()))
        .unwrap_or((0, 0));

    let to_read = (file_size as usize).min(PARTIAL_HASH_WINDOW_BYTES);
    let mut header_buf = vec![0u8; to_read];
    let mut file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
    let mut read_total = 0usize;
    while read_total < to_read {
        match file.read(&mut header_buf[read_total..]) {
            Ok(0) => break,
            Ok(n) => read_total += n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(MediaError::IoError(e.to_string())),
        }
    }
    header_buf.truncate(read_total);

    Ok(compute_fingerprint_from_parts(
        &header_buf,
        file_size,
        mtime_secs,
        mtime_nanos,
    ))
}

/// Pure, filesystem-independent routine that computes a [`SourceFingerprint`]
/// from the initial header slice, total file size, and modification time.
pub fn compute_fingerprint_from_parts(
    partial_header: &[u8],
    file_size: u64,
    mtime_secs: u64,
    mtime_nanos: u32,
) -> SourceFingerprint {
    let partial_hash = sha256(partial_header);

    let mut composite = [0u8; 32 + 8 + 8 + 4];
    composite[0..32].copy_from_slice(&partial_hash);
    composite[32..40].copy_from_slice(&file_size.to_le_bytes());
    composite[40..48].copy_from_slice(&mtime_secs.to_le_bytes());
    composite[48..52].copy_from_slice(&mtime_nanos.to_le_bytes());

    let digest = sha256(&composite);
    let cache_key = hex_encode(&digest);

    SourceFingerprint {
        partial_hash,
        file_size,
        mtime_secs,
        mtime_nanos,
        digest,
        cache_key,
    }
}

/// Centralizes deterministic paths for derived sidecar caches inside a project.
#[derive(Debug, Clone)]
pub struct CacheManager {
    root: PathBuf,
}

impl CacheManager {
    /// Creates a [`CacheManager`] rooted at `root` (e.g. `<project_path>/cache`).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Creates a [`CacheManager`] for a project directory (`<project_path>/cache`).
    pub fn for_project(project_path: impl AsRef<Path>) -> Self {
        Self {
            root: project_path.as_ref().join("cache"),
        }
    }

    /// Root cache directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Deterministic path for a binary frame index file (`.index`) given a `cache_key`.
    ///
    /// If an existing `.index` file exists, returns that path. If a legacy `.fidx`
    /// file exists and no `.index` is present, returns the legacy path for backwards
    /// compatibility; otherwise defaults to the primary `.index` path.
    pub fn frame_index_path(&self, cache_key: &str) -> PathBuf {
        let index_path = self.root.join("index").join(format!("{}.index", cache_key));
        if !index_path.exists() {
            let legacy_fidx = self.root.join("index").join(format!("{}.fidx", cache_key));
            if legacy_fidx.exists() {
                return legacy_fidx;
            }
        }
        index_path
    }

    /// Primary `.index` path for writing new binary frame index cache files.
    pub fn primary_frame_index_path(&self, cache_key: &str) -> PathBuf {
        self.root.join("index").join(format!("{}.index", cache_key))
    }

    /// Deterministic path for an audio waveform peak file (`.peak`) given a `cache_key`.
    pub fn audio_peaks_path(&self, cache_key: &str) -> PathBuf {
        self.root.join("peaks").join(format!("{}.peak", cache_key))
    }

    /// Deterministic path for a thumbnail sprite atlas (`.atlas`) given a `cache_key`.
    pub fn thumbnails_path(&self, cache_key: &str) -> PathBuf {
        self.root.join("thumbs").join(format!("{}.atlas", cache_key))
    }

    /// Computes the [`SourceFingerprint`] of `source_path` and returns both the
    /// fingerprint and the target binary frame index path.
    pub fn resolve_frame_index_for_source(
        &self,
        source_path: &Path,
    ) -> Result<(SourceFingerprint, PathBuf), MediaError> {
        let fp = compute_source_fingerprint(source_path)?;
        let path = self.frame_index_path(&fp.cache_key);
        Ok((fp, path))
    }
}

/// Atomically writes a file by writing to a sibling `.tmp` file, flushing to
/// disk (`sync_all`), and renaming over `target`.
pub fn write_atomic<F>(target: &Path, write_fn: F) -> Result<(), MediaError>
where
    F: FnOnce(&mut File) -> io::Result<()>,
{
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| MediaError::IoError(e.to_string()))?;
        }
    }

    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cache");
    let tmp_path = target.with_file_name(format!(".{}.{}.{}.tmp", file_name, pid, seq));

    let write_res = (|| -> io::Result<()> {
        let mut f = File::create(&tmp_path)?;
        write_fn(&mut f)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(MediaError::IoError(e.to_string()));
    }

    if let Err(e) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(MediaError::IoError(e.to_string()));
    }

    Ok(())
}

/// Formats a byte slice as lowercase hexadecimal.
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Minimal, dependency-free FIPS 180-4 SHA-256 implementation
// ---------------------------------------------------------------------------

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn compress_block(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K256[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// Computes the FIPS 180-4 SHA-256 digest of `data`.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let mut chunks = data.chunks_exact(64);
    for chunk in &mut chunks {
        let block: &[u8; 64] = chunk.try_into().unwrap();
        compress_block(&mut state, block);
    }

    let rem = chunks.remainder();
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;

    let bit_len = (data.len() as u64).wrapping_mul(8);
    if rem.len() < 56 {
        tail[56..64].copy_from_slice(&bit_len.to_be_bytes());
        compress_block(&mut state, tail[0..64].try_into().unwrap());
    } else {
        tail[120..128].copy_from_slice(&bit_len.to_be_bytes());
        compress_block(&mut state, tail[0..64].try_into().unwrap());
        compress_block(&mut state, tail[64..128].try_into().unwrap());
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Sub-module alias so callers can reference `aether_media::cache::manager::*`.
pub mod manager {
    pub use super::{
        compute_fingerprint_from_parts, compute_source_fingerprint, hex_encode, sha256,
        write_atomic, CacheManager, SourceFingerprint, PARTIAL_HASH_WINDOW_BYTES,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_nist_vectors() {
        assert_eq!(
            hex_encode(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_encode(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex_encode(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn fingerprint_is_deterministic_and_sensitive_to_header_size_and_mtime() {
        let fp1 = compute_fingerprint_from_parts(b"ftypqt  ", 1_000_000, 1_700_000_000, 123);
        let fp2 = compute_fingerprint_from_parts(b"ftypqt  ", 1_000_000, 1_700_000_000, 123);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.cache_key.len(), 64);

        // Changing 1 byte in header changes key
        let fp_header = compute_fingerprint_from_parts(b"ftypmp42", 1_000_000, 1_700_000_000, 123);
        assert_ne!(fp1.cache_key, fp_header.cache_key);

        // Changing file_size changes key
        let fp_size = compute_fingerprint_from_parts(b"ftypqt  ", 1_000_001, 1_700_000_000, 123);
        assert_ne!(fp1.cache_key, fp_size.cache_key);

        // Changing mtime changes key
        let fp_mtime = compute_fingerprint_from_parts(b"ftypqt  ", 1_000_000, 1_700_000_001, 123);
        assert_ne!(fp1.cache_key, fp_mtime.cache_key);
    }

    #[test]
    fn cache_manager_resolves_deterministic_paths_for_file() {
        let dir = tempfile::tempdir().unwrap();
        let media_path = dir.path().join("clip.mov");
        fs::write(&media_path, b"0123456789abcdef_fake_mov_header").unwrap();

        let mgr = CacheManager::for_project(dir.path());
        let (fp_a, idx_path_a) = mgr.resolve_frame_index_for_source(&media_path).unwrap();
        let (fp_b, idx_path_b) = mgr.resolve_frame_index_for_source(&media_path).unwrap();
        assert_eq!(fp_a, fp_b);
        assert_eq!(idx_path_a, idx_path_b);
        assert!(
            idx_path_a.ends_with(format!("cache/index/{}.index", fp_a.cache_key))
                || idx_path_a.ends_with(format!("cache/index/{}.fidx", fp_a.cache_key))
        );
    }
}

