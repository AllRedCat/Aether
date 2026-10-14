//! Contiguous binary frame & GOP index (`FrameIndexEntry`) with $O(\log N)$
//! keyframe lookup for zero-latency timeline scrubbing.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use aether_core::timeline::Rational;

use crate::cache::write_atomic;
use crate::MediaError;

/// Magic header bytes for a persisted `.index` binary frame index file.
pub const FRAME_INDEX_MAGIC: &[u8; 8] = b"AETHFIDX";
/// Current binary format version (version 2 adds DTS to FrameIndexEntry for 32-byte layout).
pub const FRAME_INDEX_VERSION: u32 = 2;
/// Fixed header size in bytes before the contiguous `FrameIndexEntry` array.
pub const FRAME_INDEX_HEADER_SIZE: usize = 64;
/// Fixed size in bytes of a single [`FrameIndexEntry`].
pub const FRAME_INDEX_ENTRY_SIZE: usize = 32;
/// Upper safety cap on entries loaded from disk (~16M frames ≈ 77 hours @ 60fps)
/// to prevent crafted headers from triggering multi-gigabyte allocations.
pub const MAX_FRAME_INDEX_ENTRIES: u64 = 16 * 1024 * 1024;

/// Bit 0 of [`FrameIndexEntry::flags`]: packet is an I-Frame / IDR sync point.
pub const FLAG_IS_KEYFRAME: u8 = 1 << 0;
/// Bit 1 of [`FrameIndexEntry::flags`]: packet is disposable (non-reference B-frame).
pub const FLAG_IS_DISPOSABLE: u8 = 1 << 1;

/// Fixed 32-byte binary record describing a single compressed video packet in
/// presentation order.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameIndexEntry {
    /// Presentation timestamp (in the index's timebase).
    pub pts: i64,
    /// Decode timestamp (in the index's timebase).
    pub dts: i64,
    /// Byte offset of the compressed packet in the source media container.
    pub file_offset: u64,
    /// Compressed packet size in bytes.
    pub packet_size: u32,
    /// Bitfield: Bit 0 = `Is_Keyframe` (I/IDR), Bit 1 = `Is_Disposable`.
    pub flags: u8,
    /// Explicit padding to guarantee 32-byte alignment across all platforms.
    pub _pad: [u8; 3],
}

// Compile-time guarantee that `FrameIndexEntry` is exactly 32 bytes with 8-byte alignment.
const _: [(); FRAME_INDEX_ENTRY_SIZE] = [(); std::mem::size_of::<FrameIndexEntry>()];
const _: [(); 8] = [(); std::mem::align_of::<FrameIndexEntry>()];

impl FrameIndexEntry {
    /// Constructs a new 32-byte [`FrameIndexEntry`] with zeroed padding bytes.
    #[inline]
    pub const fn new(
        pts: i64,
        dts: i64,
        file_offset: u64,
        packet_size: u32,
        is_keyframe: bool,
        is_disposable: bool,
    ) -> Self {
        let mut flags = 0u8;
        if is_keyframe {
            flags |= FLAG_IS_KEYFRAME;
        }
        if is_disposable {
            flags |= FLAG_IS_DISPOSABLE;
        }
        Self {
            pts,
            dts,
            file_offset,
            packet_size,
            flags,
            _pad: [0u8; 3],
        }
    }

    /// Returns `true` if Bit 0 (`FLAG_IS_KEYFRAME`) is set.
    #[inline]
    pub const fn is_keyframe(&self) -> bool {
        (self.flags & FLAG_IS_KEYFRAME) != 0
    }

    /// Returns `true` if Bit 1 (`FLAG_IS_DISPOSABLE`) is set.
    #[inline]
    pub const fn is_disposable(&self) -> bool {
        (self.flags & FLAG_IS_DISPOSABLE) != 0
    }

    /// Serializes this entry into 32 little-endian bytes.
    #[inline]
    pub const fn to_le_bytes(self) -> [u8; FRAME_INDEX_ENTRY_SIZE] {
        let pts = self.pts.to_le_bytes();
        let dts = self.dts.to_le_bytes();
        let off = self.file_offset.to_le_bytes();
        let sz = self.packet_size.to_le_bytes();
        [
            pts[0], pts[1], pts[2], pts[3], pts[4], pts[5], pts[6], pts[7],
            dts[0], dts[1], dts[2], dts[3], dts[4], dts[5], dts[6], dts[7],
            off[0], off[1], off[2], off[3], off[4], off[5], off[6], off[7],
            sz[0], sz[1], sz[2], sz[3],
            self.flags,
            self._pad[0], self._pad[1], self._pad[2],
        ]
    }

    /// Deserializes an entry from 32 little-endian bytes.
    #[inline]
    pub const fn from_le_bytes(b: [u8; FRAME_INDEX_ENTRY_SIZE]) -> Self {
        let pts = i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        let dts = i64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]);
        let file_offset =
            u64::from_le_bytes([b[16], b[17], b[18], b[19], b[20], b[21], b[22], b[23]]);
        let packet_size = u32::from_le_bytes([b[24], b[25], b[26], b[27]]);
        let flags = b[28];
        Self {
            pts,
            dts,
            file_offset,
            packet_size,
            flags,
            _pad: [0u8; 3],
        }
    }
}

/// Encodes a slice of [`FrameIndexEntry`] records into a flat contiguous
/// little-endian byte buffer (`entries.len() * 32` bytes, without file header).
pub fn encode_entries_raw(entries: &[FrameIndexEntry]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(entries.len() * FRAME_INDEX_ENTRY_SIZE);
    for e in entries {
        buf.extend_from_slice(&e.to_le_bytes());
    }
    buf
}

/// Decodes a flat contiguous little-endian byte buffer into a `Vec<FrameIndexEntry>`.
pub fn decode_entries_raw(bytes: &[u8]) -> Result<Vec<FrameIndexEntry>, MediaError> {
    if bytes.len() % FRAME_INDEX_ENTRY_SIZE != 0 {
        return Err(MediaError::CorruptFile(format!(
            "Raw frame index buffer size ({}) is not a multiple of {} bytes",
            bytes.len(),
            FRAME_INDEX_ENTRY_SIZE
        )));
    }
    let count = bytes.len() / FRAME_INDEX_ENTRY_SIZE;
    if count as u64 > MAX_FRAME_INDEX_ENTRIES {
        return Err(MediaError::CorruptFile(format!(
            "Frame index entry count ({}) exceeds safety limit ({})",
            count, MAX_FRAME_INDEX_ENTRIES
        )));
    }
    let mut entries = Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(FRAME_INDEX_ENTRY_SIZE) {
        let arr: [u8; FRAME_INDEX_ENTRY_SIZE] = chunk.try_into().unwrap();
        entries.push(FrameIndexEntry::from_le_bytes(arr));
    }
    Ok(entries)
}

/// Performs an $O(\log N)$ binary search over a PTS-sorted slice of
/// [`FrameIndexEntry`] to locate the nearest keyframe at or before `target_pts`.
///
/// Returns `None` if `entries` is empty or if no keyframe exists with `pts <= target_pts`.
pub fn binary_search_keyframe_at_or_before(
    entries: &[FrameIndexEntry],
    target_pts: i64,
) -> Option<(usize, &FrameIndexEntry)> {
    if entries.is_empty() {
        return None;
    }
    // `partition_point` is O(log N) and returns the first index where `e.pts > target_pts`.
    let upper = entries.partition_point(|e| e.pts <= target_pts);
    if upper == 0 {
        return None;
    }
    for idx in (0..upper).rev() {
        if entries[idx].is_keyframe() {
            return Some((idx, &entries[idx]));
        }
    }
    None
}

/// Boundary-tolerant keyframe search over a PTS-sorted slice:
/// - If a keyframe exists at or before `target_pts`, returns that keyframe.
/// - If `target_pts` precedes the earliest keyframe in the stream (e.g. negative
///   timestamp or B-frame pre-roll), clamps to the earliest available keyframe.
/// - Returns `None` only when the slice contains no keyframes at all.
pub fn binary_search_keyframe_clamped(
    entries: &[FrameIndexEntry],
    target_pts: i64,
) -> Option<(usize, &FrameIndexEntry)> {
    if let Some(hit) = binary_search_keyframe_at_or_before(entries, target_pts) {
        return Some(hit);
    }
    entries
        .iter()
        .enumerate()
        .find(|(_, e)| e.is_keyframe())
}

/// Indexed table of video frames sorted by PTS, with an auxiliary keyframe
/// lookup table that guarantees strict $O(\log K)$ keyframe seeking regardless
/// of GOP length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameIndex {
    /// Timebase of [`FrameIndexEntry::pts`].
    pub timebase: Rational,
    /// Optional 32-byte source fingerprint bound to this index.
    pub source_digest: [u8; 32],
    /// Contiguous table of packets sorted by `(pts, file_offset)`.
    entries: Vec<FrameIndexEntry>,
    /// Indices into `entries` where `entry.is_keyframe()` is true, sorted by `pts`.
    keyframe_indices: Vec<u32>,
}

impl FrameIndex {
    /// Builds a [`FrameIndex`], sorting `entries` by `(pts, file_offset)` (and
    /// preferring keyframes first when timestamps tie) and precomputing the
    /// keyframe lookup table.
    pub fn new(
        timebase: Rational,
        source_digest: [u8; 32],
        mut entries: Vec<FrameIndexEntry>,
    ) -> Self {
        entries.sort_by(|a, b| {
            a.pts
                .cmp(&b.pts)
                .then_with(|| b.is_keyframe().cmp(&a.is_keyframe()))
                .then_with(|| a.dts.cmp(&b.dts))
                .then_with(|| a.file_offset.cmp(&b.file_offset))
        });

        let mut keyframe_indices = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if e.is_keyframe() {
                if let Ok(idx32) = u32::try_from(i) {
                    keyframe_indices.push(idx32);
                }
            }
        }

        Self {
            timebase,
            source_digest,
            entries,
            keyframe_indices,
        }
    }

    /// All indexed frame entries in ascending presentation order.
    #[inline]
    pub fn entries(&self) -> &[FrameIndexEntry] {
        &self.entries
    }

    /// Number of indexed packets.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index contains no packets.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of I/IDR keyframes in the index.
    #[inline]
    pub fn keyframe_count(&self) -> usize {
        self.keyframe_indices.len()
    }

    /// Returns an iterator over all keyframe entries in presentation order.
    pub fn keyframes(&self) -> impl Iterator<Item = &FrameIndexEntry> {
        self.keyframe_indices
            .iter()
            .map(|&idx| &self.entries[idx as usize])
    }

    /// Strict $O(\log K)$ binary search over the keyframe table: returns the
    /// keyframe with the greatest `pts <= target_pts`, or `None` if `target_pts`
    /// is earlier than the first keyframe (or if there are no keyframes).
    pub fn find_keyframe_at_or_before(&self, target_pts: i64) -> Option<&FrameIndexEntry> {
        if self.keyframe_indices.is_empty() {
            return None;
        }
        let upper = self
            .keyframe_indices
            .partition_point(|&idx| self.entries[idx as usize].pts <= target_pts);
        if upper == 0 {
            return None;
        }
        let kf_entry_idx = self.keyframe_indices[upper - 1] as usize;
        Some(&self.entries[kf_entry_idx])
    }

    /// Boundary-tolerant $O(\log K)$ keyframe search for timeline scrubbing:
    /// - Returns the nearest preceding keyframe (`pts <= target_pts`).
    /// - If `target_pts` is less than the first keyframe's PTS, clamps to the
    ///   first keyframe so seeking before stream start still lands on a valid I-Frame.
    /// - If `target_pts` exceeds the stream duration, returns the final keyframe.
    pub fn seek_keyframe(&self, target_pts: i64) -> Option<&FrameIndexEntry> {
        if let Some(kf) = self.find_keyframe_at_or_before(target_pts) {
            return Some(kf);
        }
        self.keyframe_indices
            .first()
            .map(|&idx| &self.entries[idx as usize])
    }

    /// $O(\log N)$ binary search over all packets (I, P, B) returning the frame
    /// with the greatest `pts <= target_pts`.
    pub fn find_frame_at_or_before(&self, target_pts: i64) -> Option<&FrameIndexEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let upper = self.entries.partition_point(|e| e.pts <= target_pts);
        if upper == 0 {
            return None;
        }
        Some(&self.entries[upper - 1])
    }

    /// Serializes the header + contiguous 32-byte [`FrameIndexEntry`] table into bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let total = FRAME_INDEX_HEADER_SIZE + self.entries.len() * FRAME_INDEX_ENTRY_SIZE;
        let mut out = Vec::with_capacity(total);

        // 64-byte header:
        // [0..8]   magic "AETHFIDX"
        // [8..12]  version (u32 LE)
        // [12..16] timebase.num (i32 LE)
        // [16..20] timebase.den (i32 LE)
        // [20..24] reserved (u32 LE = 0)
        // [24..32] entry_count (u64 LE)
        // [32..64] source_digest ([u8; 32])
        out.extend_from_slice(FRAME_INDEX_MAGIC);
        out.extend_from_slice(&FRAME_INDEX_VERSION.to_le_bytes());
        out.extend_from_slice(&self.timebase.num.to_le_bytes());
        out.extend_from_slice(&self.timebase.den.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.source_digest);
        debug_assert_eq!(out.len(), FRAME_INDEX_HEADER_SIZE);

        for entry in &self.entries {
            out.extend_from_slice(&entry.to_le_bytes());
        }
        out
    }

    /// Deserializes a [`FrameIndex`] from a byte buffer (header + contiguous entries).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MediaError> {
        if bytes.len() < FRAME_INDEX_HEADER_SIZE {
            return Err(MediaError::CorruptFile(format!(
                "Frame index file too small ({} bytes < {} header bytes)",
                bytes.len(),
                FRAME_INDEX_HEADER_SIZE
            )));
        }

        if &bytes[0..8] != FRAME_INDEX_MAGIC && &bytes[0..8] != b"AETHINDX" {
            return Err(MediaError::CorruptFile(
                "Invalid frame index magic header".to_string(),
            ));
        }

        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != FRAME_INDEX_VERSION && version != 1 {
            return Err(MediaError::CorruptFile(format!(
                "Unsupported frame index version: {} (expected {})",
                version, FRAME_INDEX_VERSION
            )));
        }

        let tb_num = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let tb_den = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
        if tb_num <= 0 || tb_den <= 0 {
            return Err(MediaError::CorruptFile(format!(
                "Invalid timebase in frame index header: {}/{}",
                tb_num, tb_den
            )));
        }

        let entry_count = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        if entry_count > MAX_FRAME_INDEX_ENTRIES {
            return Err(MediaError::CorruptFile(format!(
                "Frame index entry count ({}) exceeds safety limit",
                entry_count
            )));
        }

        let mut source_digest = [0u8; 32];
        source_digest.copy_from_slice(&bytes[32..64]);

        let payload = &bytes[FRAME_INDEX_HEADER_SIZE..];

        let entries = if version == 1 && payload.len() == (entry_count as usize) * 24 {
            let mut legacy_entries = Vec::with_capacity(entry_count as usize);
            for chunk in payload.chunks_exact(24) {
                let pts = i64::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7]]);
                let file_offset = u64::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11], chunk[12], chunk[13], chunk[14], chunk[15]]);
                let packet_size = u32::from_le_bytes([chunk[16], chunk[17], chunk[18], chunk[19]]);
                let flags = chunk[20];
                legacy_entries.push(FrameIndexEntry {
                    pts,
                    dts: pts,
                    file_offset,
                    packet_size,
                    flags,
                    _pad: [0u8; 3],
                });
            }
            legacy_entries
        } else {
            let expected_payload_bytes = (entry_count as usize)
                .checked_mul(FRAME_INDEX_ENTRY_SIZE)
                .ok_or_else(|| MediaError::CorruptFile("Frame index size overflow".to_string()))?;

            if payload.len() != expected_payload_bytes {
                return Err(MediaError::CorruptFile(format!(
                    "Frame index payload truncated or mismatched: expected {} bytes for {} entries, got {} bytes",
                    expected_payload_bytes,
                    entry_count,
                    payload.len()
                )));
            }

            decode_entries_raw(payload)?
        };

        Ok(Self::new(
            Rational {
                num: tb_num,
                den: tb_den,
            },
            source_digest,
            entries,
        ))
    }

    /// Atomically persists the binary frame index to `path`.
    pub fn save_to_file(&self, path: &Path) -> Result<(), MediaError> {
        let bytes = self.to_bytes();
        write_atomic(path, |file| file.write_all(&bytes))
    }

    /// Loads a persisted binary frame index from `path`.
    pub fn load_from_file(path: &Path) -> Result<Self, MediaError> {
        if !path.exists() {
            return Err(MediaError::FileNotFound(path.display().to_string()));
        }
        if let Ok(mmap_idx) = MmapFrameIndex::open(path) {
            return Ok(Self::new(
                mmap_idx.timebase,
                mmap_idx.source_digest,
                mmap_idx.entries().to_vec(),
            ));
        }
        let mut f = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .map_err(|e| MediaError::IoError(e.to_string()))?;
        Self::from_bytes(&buf)
    }

    /// Loads a persisted binary frame index and verifies that its embedded
    /// `source_digest` matches `expected_digest`.
    pub fn load_validated(
        path: &Path,
        expected_digest: &[u8; 32],
    ) -> Result<Self, MediaError> {
        let idx = Self::load_from_file(path)?;
        if &idx.source_digest != expected_digest {
            return Err(MediaError::CorruptFile(
                "Frame index digest does not match source media fingerprint".to_string(),
            ));
        }
        Ok(idx)
    }
}

/// OS-level memory-mapped buffer wrapper for high-performance zero-copy index access.
#[cfg(unix)]
pub struct MmapHandle {
    ptr: *mut u8,
    len: usize,
}

#[cfg(unix)]
impl MmapHandle {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        use std::os::unix::io::AsRawFd;
        let file = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
        let meta = file.metadata().map_err(|e| MediaError::IoError(e.to_string()))?;
        let len = meta.len() as usize;
        if len < FRAME_INDEX_HEADER_SIZE {
            return Err(MediaError::CorruptFile(format!(
                "Frame index file too small ({} bytes < {} header bytes)",
                len, FRAME_INDEX_HEADER_SIZE
            )));
        }
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(MediaError::IoError(std::io::Error::last_os_error().to_string()));
        }
        Ok(Self {
            ptr: ptr as *mut u8,
            len,
        })
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

#[cfg(unix)]
impl Drop for MmapHandle {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.len);
        }
    }
}

#[cfg(unix)]
unsafe impl Send for MmapHandle {}
#[cfg(unix)]
unsafe impl Sync for MmapHandle {}

#[cfg(not(unix))]
pub struct MmapHandle {
    buf: Vec<u8>,
}

#[cfg(not(unix))]
impl MmapHandle {
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        let mut f = File::open(path).map_err(|e| MediaError::IoError(e.to_string()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| MediaError::IoError(e.to_string()))?;
        if buf.len() < FRAME_INDEX_HEADER_SIZE {
            return Err(MediaError::CorruptFile(format!(
                "Frame index file too small ({} bytes < {} header bytes)",
                buf.len(), FRAME_INDEX_HEADER_SIZE
            )));
        }
        Ok(Self { buf })
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }
}

/// Zero-copy, memory-mapped representation of a contiguous binary `.index` sidecar file.
///
/// Directly maps `FrameIndexEntry` records into memory for $O(1)$ startup allocation and
/// $O(\log N)$ timeline scrubbing without reading entire index tables into heap memory.
pub struct MmapFrameIndex {
    handle: MmapHandle,
    pub timebase: Rational,
    pub source_digest: [u8; 32],
    entry_count: usize,
}

impl MmapFrameIndex {
    /// Opens and memory-maps the binary `.index` file at `path`.
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        if !path.exists() {
            return Err(MediaError::FileNotFound(path.display().to_string()));
        }
        let handle = MmapHandle::open(path)?;
        let bytes = handle.as_slice();

        if bytes.len() < FRAME_INDEX_HEADER_SIZE {
            return Err(MediaError::CorruptFile(format!(
                "Frame index file too small ({} bytes < {} header bytes)",
                bytes.len(),
                FRAME_INDEX_HEADER_SIZE
            )));
        }

        if &bytes[0..8] != FRAME_INDEX_MAGIC && &bytes[0..8] != b"AETHINDX" {
            return Err(MediaError::CorruptFile(
                "Invalid frame index magic header".to_string(),
            ));
        }

        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != FRAME_INDEX_VERSION {
            return Err(MediaError::CorruptFile(format!(
                "Unsupported frame index version: {} (expected {})",
                version, FRAME_INDEX_VERSION
            )));
        }

        let tb_num = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let tb_den = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
        if tb_num <= 0 || tb_den <= 0 {
            return Err(MediaError::CorruptFile(format!(
                "Invalid timebase in frame index header: {}/{}",
                tb_num, tb_den
            )));
        }

        let entry_count = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        if entry_count > MAX_FRAME_INDEX_ENTRIES {
            return Err(MediaError::CorruptFile(format!(
                "Frame index entry count ({}) exceeds safety limit",
                entry_count
            )));
        }

        let mut source_digest = [0u8; 32];
        source_digest.copy_from_slice(&bytes[32..64]);

        let payload = &bytes[FRAME_INDEX_HEADER_SIZE..];
        let expected_payload_bytes = (entry_count as usize)
            .checked_mul(FRAME_INDEX_ENTRY_SIZE)
            .ok_or_else(|| MediaError::CorruptFile("Frame index size overflow".to_string()))?;

        if payload.len() != expected_payload_bytes {
            return Err(MediaError::CorruptFile(format!(
                "Frame index payload truncated or mismatched: expected {} bytes for {} entries, got {} bytes",
                expected_payload_bytes,
                entry_count,
                payload.len()
            )));
        }

        Ok(Self {
            handle,
            timebase: Rational {
                num: tb_num,
                den: tb_den,
            },
            source_digest,
            entry_count: entry_count as usize,
        })
    }

    /// Opens and memory-maps the binary `.index` file, asserting digest validation.
    pub fn open_validated(path: &Path, expected_digest: &[u8; 32]) -> Result<Self, MediaError> {
        let idx = Self::open(path)?;
        if &idx.source_digest != expected_digest {
            return Err(MediaError::CorruptFile(
                "Frame index digest does not match source media fingerprint".to_string(),
            ));
        }
        Ok(idx)
    }

    /// Direct zero-copy slice of all [`FrameIndexEntry`] records in presentation order.
    #[inline]
    pub fn entries(&self) -> &[FrameIndexEntry] {
        let payload = &self.handle.as_slice()[FRAME_INDEX_HEADER_SIZE..];
        unsafe {
            std::slice::from_raw_parts(
                payload.as_ptr() as *const FrameIndexEntry,
                self.entry_count,
            )
        }
    }

    /// Number of indexed packets.
    #[inline]
    pub fn len(&self) -> usize {
        self.entry_count
    }

    /// Returns `true` if the index contains zero packets.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// Strict $O(\log N)$ binary search over the memory-mapped index: returns the
    /// keyframe with the greatest `pts <= target_pts`.
    pub fn find_keyframe_at_or_before(&self, target_pts: i64) -> Option<&FrameIndexEntry> {
        binary_search_keyframe_at_or_before(self.entries(), target_pts).map(|(_, e)| e)
    }

    /// Boundary-tolerant $O(\log N)$ keyframe search over memory-mapped index.
    pub fn seek_keyframe(&self, target_pts: i64) -> Option<&FrameIndexEntry> {
        binary_search_keyframe_clamped(self.entries(), target_pts).map(|(_, e)| e)
    }

    /// Binary search over all packets (I, P, B) returning frame with greatest `pts <= target_pts`.
    pub fn find_frame_at_or_before(&self, target_pts: i64) -> Option<&FrameIndexEntry> {
        let entries = self.entries();
        if entries.is_empty() {
            return None;
        }
        let upper = entries.partition_point(|e| e.pts <= target_pts);
        if upper == 0 {
            return None;
        }
        Some(&entries[upper - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn sample_gop_entries() -> Vec<FrameIndexEntry> {
        // 2 GOPs of 5 frames each (at 60fps: PTS = 0, 1, 2, 3, 4 | 5, 6, 7, 8, 9)
        // Keyframes at PTS 0 and PTS 5; B-frames at PTS 1, 2, 6, 7 marked disposable.
        vec![
            FrameIndexEntry::new(0, 0, 1024, 12_000, true, false),
            FrameIndexEntry::new(1, 1, 13_024, 1_500, false, true),
            FrameIndexEntry::new(2, 2, 14_524, 1_600, false, true),
            FrameIndexEntry::new(3, 3, 16_124, 4_200, false, false),
            FrameIndexEntry::new(4, 4, 20_324, 4_100, false, false),
            FrameIndexEntry::new(5, 5, 24_424, 11_800, true, false),
            FrameIndexEntry::new(6, 6, 36_224, 1_400, false, true),
            FrameIndexEntry::new(7, 7, 37_624, 1_450, false, true),
            FrameIndexEntry::new(8, 8, 39_074, 3_900, false, false),
            FrameIndexEntry::new(9, 9, 42_974, 4_000, false, false),
        ]
    }

    #[test]
    fn entry_layout_is_32_bytes_and_fields_roundtrip() {
        assert_eq!(std::mem::size_of::<FrameIndexEntry>(), 32);
        assert_eq!(std::mem::align_of::<FrameIndexEntry>(), 8);

        let kf = FrameIndexEntry::new(120, 118, 65_536, 48_123, true, false);
        assert_eq!(kf.pts, 120);
        assert_eq!(kf.dts, 118);
        assert_eq!(kf.file_offset, 65_536);
        assert_eq!(kf.packet_size, 48_123);
        assert!(kf.is_keyframe());
        assert!(!kf.is_disposable());
        assert_eq!(FrameIndexEntry::from_le_bytes(kf.to_le_bytes()), kf);

        let bframe = FrameIndexEntry::new(121, 119, 113_659, 2_048, false, true);
        assert_eq!(bframe.pts, 121);
        assert_eq!(bframe.dts, 119);
        assert_eq!(bframe.file_offset, 113_659);
        assert_eq!(bframe.packet_size, 2_048);
        assert!(!bframe.is_keyframe());
        assert!(bframe.is_disposable());
        assert_eq!(FrameIndexEntry::from_le_bytes(bframe.to_le_bytes()), bframe);
    }

    #[test]
    fn binary_serialization_and_file_persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index").join("sample.index");
        let digest = [0x42u8; 32];
        let tb = Rational { num: 60, den: 1 };

        // Feed out-of-order entries to verify deterministic PTS sorting on creation
        let mut shuffled = sample_gop_entries();
        shuffled.reverse();

        let index = FrameIndex::new(tb, digest, shuffled);
        assert_eq!(index.len(), 10);
        assert_eq!(index.keyframe_count(), 2);

        index.save_to_file(&path).unwrap();
        let loaded = FrameIndex::load_validated(&path, &digest).unwrap();
        assert_eq!(loaded, index);

        // Wrong digest must be rejected
        assert!(FrameIndex::load_validated(&path, &[0x99u8; 32]).is_err());

        // Truncated payload must error cleanly without panic
        let mut raw = index.to_bytes();
        raw.truncate(raw.len() - 5);
        assert!(FrameIndex::from_bytes(&raw).is_err());

        // Header too short must error cleanly
        assert!(FrameIndex::from_bytes(&raw[..30]).is_err());

        // Bad magic must error cleanly
        let mut bad_magic = index.to_bytes();
        bad_magic[0] = b'X';
        assert!(FrameIndex::from_bytes(&bad_magic).is_err());

        // Invalid timebase must error cleanly
        let mut bad_tb = index.to_bytes();
        bad_tb[12..16].copy_from_slice(&0i32.to_le_bytes());
        assert!(FrameIndex::from_bytes(&bad_tb).is_err());
    }

    #[test]
    fn legacy_v1_24_byte_index_migrates_cleanly() {
        // Construct a valid v1 24-byte index buffer
        let tb = Rational { num: 60, den: 1 };
        let digest = [0x77u8; 32];
        let mut raw = Vec::new();
        raw.extend_from_slice(FRAME_INDEX_MAGIC);
        raw.extend_from_slice(&1u32.to_le_bytes()); // version 1
        raw.extend_from_slice(&tb.num.to_le_bytes());
        raw.extend_from_slice(&tb.den.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&2u64.to_le_bytes()); // 2 entries
        raw.extend_from_slice(&digest);

        // Entry 0 (24 bytes)
        raw.extend_from_slice(&0i64.to_le_bytes()); // pts
        raw.extend_from_slice(&1000u64.to_le_bytes()); // file_offset
        raw.extend_from_slice(&500u32.to_le_bytes()); // packet_size
        raw.push(FLAG_IS_KEYFRAME);
        raw.extend_from_slice(&[0u8; 3]);

        // Entry 1 (24 bytes)
        raw.extend_from_slice(&1i64.to_le_bytes()); // pts
        raw.extend_from_slice(&1500u64.to_le_bytes()); // file_offset
        raw.extend_from_slice(&300u32.to_le_bytes()); // packet_size
        raw.push(FLAG_IS_DISPOSABLE);
        raw.extend_from_slice(&[0u8; 3]);

        let loaded = FrameIndex::from_bytes(&raw).expect("v1 migration should succeed");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.entries()[0].pts, 0);
        assert_eq!(loaded.entries()[0].dts, 0);
        assert!(loaded.entries()[0].is_keyframe());
        assert_eq!(loaded.entries()[1].pts, 1);
        assert_eq!(loaded.entries()[1].dts, 1);
        assert!(loaded.entries()[1].is_disposable());
    }

    #[test]
    fn binary_search_keyframe_handles_all_boundaries() {
        let entries = sample_gop_entries();
        let idx = FrameIndex::new(Rational { num: 60, den: 1 }, [0u8; 32], entries.clone());

        // Exact hit on first keyframe (PTS = 0)
        assert_eq!(idx.find_keyframe_at_or_before(0).unwrap().pts, 0);
        assert_eq!(idx.seek_keyframe(0).unwrap().pts, 0);

        // Between GOP 1 and GOP 2 (PTS = 1..4 -> keyframe at PTS = 0)
        for t in 1..=4 {
            let kf = idx.find_keyframe_at_or_before(t).unwrap();
            assert_eq!(kf.pts, 0);
            assert_eq!(kf.file_offset, 1024);
        }

        // Exact hit on second keyframe (PTS = 5)
        let kf5 = idx.find_keyframe_at_or_before(5).unwrap();
        assert_eq!(kf5.pts, 5);
        assert_eq!(kf5.file_offset, 24_424);

        // Inside GOP 2 (PTS = 6..9 -> keyframe at PTS = 5)
        for t in 6..=9 {
            assert_eq!(idx.find_keyframe_at_or_before(t).unwrap().pts, 5);
        }

        // Past end of stream (PTS = 10_000 and i64::MAX -> last keyframe at PTS = 5)
        assert_eq!(idx.find_keyframe_at_or_before(10_000).unwrap().pts, 5);
        assert_eq!(idx.find_keyframe_at_or_before(i64::MAX).unwrap().pts, 5);

        // Negative PTS before stream start:
        // - strict `find_keyframe_at_or_before` returns None
        // - boundary-tolerant `seek_keyframe` clamps to the first keyframe (PTS = 0)
        assert!(idx.find_keyframe_at_or_before(-1).is_none());
        assert_eq!(idx.seek_keyframe(-1).unwrap().pts, 0);
        assert_eq!(idx.seek_keyframe(i64::MIN).unwrap().pts, 0);

        // Slice-level binary search agrees with FrameIndex methods
        assert_eq!(
            binary_search_keyframe_at_or_before(idx.entries(), 4)
                .unwrap()
                .1
                .pts,
            0
        );
        assert_eq!(
            binary_search_keyframe_clamped(idx.entries(), -50)
                .unwrap()
                .1
                .pts,
            0
        );
    }

    #[test]
    fn binary_search_empty_and_non_keyframe_streams_do_not_panic() {
        let empty = FrameIndex::new(Rational { num: 60, den: 1 }, [0u8; 32], Vec::new());
        assert!(empty.find_keyframe_at_or_before(0).is_none());
        assert!(empty.seek_keyframe(0).is_none());

        let no_kf = FrameIndex::new(
            Rational { num: 60, den: 1 },
            [0u8; 32],
            vec![FrameIndexEntry::new(10, 10, 100, 50, false, false)],
        );
        assert!(no_kf.find_keyframe_at_or_before(10).is_none());
        assert!(no_kf.seek_keyframe(10).is_none());
    }

    #[test]
    fn binary_search_meets_sub_5ms_zero_latency_budget_on_large_index() {
        // 360,000 frames (1.6 hours @ 60fps), keyframe every 60 frames
        let count = 360_000usize;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            entries.push(FrameIndexEntry::new(
                i as i64,
                i as i64,
                (i as u64) * 4096,
                4096,
                i % 60 == 0,
                i % 3 != 0,
            ));
        }
        let idx = FrameIndex::new(Rational { num: 60, den: 1 }, [0u8; 32], entries);

        let t0 = Instant::now();
        for probe in [0i64, 59, 60, 61, 123_456, 359_999, 999_999] {
            let kf = idx.seek_keyframe(probe).unwrap();
            assert!(kf.is_keyframe());
        }
        let elapsed = t0.elapsed();
        assert!(
            elapsed.as_millis() < 5,
            "Keyframe lookup took {:?} (>= 5ms budget)",
            elapsed
        );
    }

    #[test]
    fn test_mmap_frame_index_zero_copy_seeking() {
        let entries = sample_gop_entries();
        let idx = FrameIndex::new(Rational { num: 60, den: 1 }, [0xCC; 32], entries);

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let index_path = temp_dir.path().join("sample.index");
        idx.save_to_file(&index_path).expect("save_to_file");

        let mmap_idx = MmapFrameIndex::open(&index_path).expect("MmapFrameIndex::open");
        assert_eq!(mmap_idx.len(), 10);
        assert_eq!(mmap_idx.timebase, Rational { num: 60, den: 1 });
        assert_eq!(mmap_idx.source_digest, [0xCC; 32]);

        // Keyframe lookups match
        assert_eq!(mmap_idx.find_keyframe_at_or_before(0).unwrap().pts, 0);
        assert_eq!(mmap_idx.find_keyframe_at_or_before(4).unwrap().pts, 0);
        assert_eq!(mmap_idx.find_keyframe_at_or_before(5).unwrap().pts, 5);
        assert_eq!(mmap_idx.find_keyframe_at_or_before(8).unwrap().pts, 5);

        // Clamped lookups
        assert_eq!(mmap_idx.seek_keyframe(-10).unwrap().pts, 0);
        assert_eq!(mmap_idx.seek_keyframe(100).unwrap().pts, 5);
    }
}

