# Rust Core & Model Survey Report: Aether Timeline & Clip Model

## Executive Summary
This report presents the complete static and architectural analysis of `aether_core` (located at `/Users/gabrielgenaro/Developer/Pessoal/Aether/crates/aether_core`), its data models (`Timeline`, `Track`, `Clip`, `Rational`, `TrackKind`), temporal PTS calculations, and points of extension for implementing the "Add Clip to Track" feature with automatic `duration_pts` recalculation.

---

## 1. Codebase Layout & Workspace Inspection

### 1.1 Root Cargo Workspace
**File**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/Cargo.toml`
```toml
[workspace]
members = [
    "crates/aether_bridge",
    "crates/aether_core",
    "crates/aether_render",
    "crates/aether_media"
]
resolver = "2"
```

### 1.2 `aether_core` Package Configuration
**File**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/crates/aether_core/Cargo.toml`
- Package name: `aether_core`
- Version: `0.1.0`
- Edition: `2021`
- Dependencies:
  - `uuid = { version = "1.10", features = ["v4"] }`
- No other third-party dependencies are currently required or imported.

### 1.3 `aether_core` Module Layout
- `crates/aether_core/src/lib.rs` (Lines 1-2):
  ```rust
  pub mod timeline;
  ```
- `crates/aether_core/src/timeline.rs` (Lines 1-40):
  Contains all existing domain data types.

---

## 2. Analysis of Current Data Structures (`timeline.rs`)

Currently, `crates/aether_core/src/timeline.rs` defines five data types:

```rust
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

#[derive(Debug, Clone)]
pub struct Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone)]
pub enum TrackKind {
    Video,
    Audio,
    Overlay,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone)]
pub struct Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}
```

### Observations & Gaps:
1. **No Methods Implemented**:
   - There are currently **zero** `impl` blocks for `Timeline`, `Track`, `Clip`, or `Rational`.
   - All structs have public fields, but no constructors (`new`, `default`), no validation, no mutation methods, and no query methods.
2. **Missing Traits**:
   - All types only derive `#[derive(Debug, Clone)]`.
   - `PartialEq, Eq` are missing across all types, making unit test assertions (`assert_eq!`) and equality checks awkward.
   - `Copy` is missing for `Rational` and `TrackKind` (both are tiny, trivial copyable types).
3. **Empty Track Initialization**:
   - In `aether_bridge/src/api.rs:4-11`, `create_timeline()` initializes `tracks: vec![]`. An empty timeline contains no tracks to receive a clip unless default tracks are pre-created or a method to add tracks is provided.

---

## 3. PTS, Timebase, and Timeline Duration Calculations

### 3.1 Timebase & Presentation Time Stamps (PTS)
- `Rational { num, den }`:
  Represents timeline units per second (framerate or clock frequency). E.g., `{ num: 60, den: 1 }` = 60 fps (1 frame = 1 PTS unit).
- `Clip` temporal boundaries:
  - `source_in`: start frame/PTS within the underlying source asset.
  - `source_out`: end frame/PTS within the source asset.
  - `source_duration = source_out - source_in`.
  - `timeline_in`: start frame/PTS position where this clip is placed on the timeline track.
  - `timeline_out`: end frame/PTS position where this clip finishes on the timeline.
  - In a standard 1:1 speed playback rate: `timeline_out = timeline_in + (source_out - source_in)`.
  - Invariant requirement: `source_out >= source_in` and `timeline_out >= timeline_in >= 0`.

### 3.2 Track & Timeline Duration Formula
- A timeline is composed of parallel tracks ($T_1, T_2, \dots, T_k$).
- Each track $T_i$ contains clips ($C_{i,1}, C_{i,2}, \dots, C_{i,m}$).
- A track's span on the timeline is defined by its furthest clip boundary:
  $$\text{duration}(T_i) = \max_{j} (C_{i,j}.\text{timeline\_out})$$
  (or 0 if track has no clips).
- The overall `duration_pts` of the `Timeline` is the maximum `timeline_out` across all clips across all tracks:
  $$\text{Timeline}.\text{duration\_pts} = \max_{i, j} (C_{i, j}.\text{timeline\_out})$$
  (or 0 if the timeline contains no clips).

### 3.3 Recalculation Algorithm
When a clip is added, updated, or removed:
```rust
pub fn recalculate_duration(&mut self) {
    self.duration_pts = self.tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| clip.timeline_out)
        .max()
        .unwrap_or(0);
}
```
**Properties**:
- Idempotent and deterministic.
- Handles arbitrary track counts and gaps between clips.
- Handles multi-track overlapping or staggered clip arrangements.
- Correctly accounts for empty tracks (duration 0).

---

## 4. Non-Destructive DAG Principles & Concurrency

1. **Non-destructive Editing**:
   - Source media referenced by `source_id: Uuid` is strictly read-only and immutable.
   - Clips are lightweight edit decision descriptors (DAG leaves referencing media descriptors).
   - Insertion, trimming, or re-ordering of clips creates or mutates only graph nodes, never touching source assets.
2. **DAG Architecture**:
   - The timeline hierarchy form an evaluation DAG:
     $$\text{Sources} \to \text{Clips} \to \text{Tracks (Layers)} \to \text{Compositor} \to \text{Output Frames}$$
   - For this slice, the tree `Timeline -> Vec<Track> -> Vec<Clip>` models the evaluation graph.
3. **Concurrency & Thread Safety**:
   - All structs use standard Rust types (`Uuid`, `i64`, `i32`, `Vec`, `enum`).
   - Every struct automatically satisfies `Send + Sync`.
   - Safe for multithreaded rendering and bridge interactions without unsafe code or raw pointers.

---

## 5. Existing Tests & Test Harness Status

1. **Current State**:
   - `crates/aether_core` currently has **0 unit tests** and **no test files**.
   - No `tests/` directory exists under `crates/aether_core`.
2. **Acceptance Test Requirement**:
   - Prompt Acceptance Criterion:
     > "O comando `cargo test -p aether_core` deve passar sem falhas, contendo ao menos um teste unitário que valide se um `Clip` foi adicionado com sucesso a uma `Track` e se o tamanho da Timeline (`duration_pts`) foi recalculado corretamente."
3. **Environment Caveat**:
   - Neither `cargo` nor `flutter` is currently found in PATH on the test runner host environment (`which cargo` returned code 127).
   - Tooling setup will be required for automated execution of `cargo test` in later phases.

---

## 6. Proposed Implementation & Extension Points

To implement R1 cleanly without breaking encapsulation, the following additions should be made to `crates/aether_core/src/timeline.rs`:

### 6.1 Trait Derivations & Error Types
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    pub num: i32,
    pub den: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub id: Uuid,
    pub source_id: Uuid,
    pub source_in: i64,
    pub source_out: i64,
    pub timeline_in: i64,
    pub timeline_out: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: Uuid,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    pub id: Uuid,
    pub timebase: Rational,
    pub duration_pts: i64,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    TrackNotFound(Uuid),
    InvalidClipBounds { timeline_in: i64, timeline_out: i64 },
    InvalidSourceBounds { source_in: i64, source_out: i64 },
}

impl std::fmt::Display for TimelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TrackNotFound(id) => write!(f, "Track with ID {} not found", id),
            Self::InvalidClipBounds { timeline_in, timeline_out } => {
                write!(f, "Invalid clip timeline bounds: {}..{}", timeline_in, timeline_out)
            }
            Self::InvalidSourceBounds { source_in, source_out } => {
                write!(f, "Invalid source bounds: {}..{}", source_in, source_out)
            }
        }
    }
}

impl std::error::Error for TimelineError {}
```

### 6.2 Methods on `Clip`, `Track`, and `Timeline`
```rust
impl Clip {
    pub fn new(source_id: Uuid, source_in: i64, source_out: i64, timeline_in: i64) -> Self {
        let duration = source_out.saturating_sub(source_in);
        Self {
            id: Uuid::new_v4(),
            source_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out: timeline_in.saturating_add(duration),
        }
    }

    pub fn with_id(
        id: Uuid,
        source_id: Uuid,
        source_in: i64,
        source_out: i64,
        timeline_in: i64,
        timeline_out: i64,
    ) -> Self {
        Self {
            id,
            source_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out,
        }
    }

    pub fn duration(&self) -> i64 {
        self.timeline_out.saturating_sub(self.timeline_in)
    }
}

impl Track {
    pub fn new(kind: TrackKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            clips: Vec::new(),
        }
    }

    pub fn with_id(id: Uuid, kind: TrackKind) -> Self {
        Self {
            id,
            kind,
            clips: Vec::new(),
        }
    }

    pub fn duration_pts(&self) -> i64 {
        self.clips.iter().map(|c| c.timeline_out).max().unwrap_or(0)
    }

    pub fn add_clip(&mut self, clip: Clip) {
        self.clips.push(clip);
    }
}

impl Timeline {
    pub fn new(timebase: Rational) -> Self {
        Self {
            id: Uuid::new_v4(),
            timebase,
            duration_pts: 0,
            tracks: Vec::new(),
        }
    }

    pub fn new_with_default_tracks(timebase: Rational) -> Self {
        Self {
            id: Uuid::new_v4(),
            timebase,
            duration_pts: 0,
            tracks: vec![
                Track::new(TrackKind::Video),
                Track::new(TrackKind::Audio),
            ],
        }
    }

    pub fn add_track(&mut self, kind: TrackKind) -> Uuid {
        let track = Track::new(kind);
        let id = track.id;
        self.tracks.push(track);
        id
    }

    pub fn recalculate_duration(&mut self) {
        self.duration_pts = self.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.timeline_out)
            .max()
            .unwrap_or(0);
    }

    pub fn add_clip(&mut self, track_id: Uuid, clip: Clip) -> Result<(), TimelineError> {
        if clip.timeline_out < clip.timeline_in {
            return Err(TimelineError::InvalidClipBounds {
                timeline_in: clip.timeline_in,
                timeline_out: clip.timeline_out,
            });
        }
        let track = self.tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;
        track.add_clip(clip);
        self.recalculate_duration();
        Ok(())
    }

    pub fn total_clip_count(&self) -> usize {
        self.tracks.iter().map(|t| t.clips.len()).sum()
    }
}
```

### 6.3 Proposed Unit Tests (`crates/aether_core/src/timeline.rs`)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_clip_to_track_and_recalculate_duration() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let track_id = timeline.add_track(TrackKind::Video);
        assert_eq!(timeline.duration_pts, 0);

        let source_id = Uuid::new_v4();
        let clip = Clip::new(source_id, 0, 120, 0);
        let result = timeline.add_clip(track_id, clip);
        assert!(result.is_ok());

        assert_eq!(timeline.tracks[0].clips.len(), 1);
        assert_eq!(timeline.duration_pts, 120);
    }

    #[test]
    fn test_add_multiple_clips_recalculates_max_pts() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let video_track_id = timeline.add_track(TrackKind::Video);
        let audio_track_id = timeline.add_track(TrackKind::Audio);

        let clip1 = Clip::new(Uuid::new_v4(), 0, 60, 0); // out = 60
        timeline.add_clip(video_track_id, clip1).unwrap();
        assert_eq!(timeline.duration_pts, 60);

        let clip2 = Clip::new(Uuid::new_v4(), 0, 180, 50); // out = 230
        timeline.add_clip(audio_track_id, clip2).unwrap();
        assert_eq!(timeline.duration_pts, 230);
    }

    #[test]
    fn test_add_clip_track_not_found() {
        let mut timeline = Timeline::new(Rational { num: 60, den: 1 });
        let clip = Clip::new(Uuid::new_v4(), 0, 60, 0);
        let non_existent_id = Uuid::new_v4();
        let result = timeline.add_clip(non_existent_id, clip);
        assert_eq!(result, Err(TimelineError::TrackNotFound(non_existent_id)));
    }
}
```

---

## 7. Cross-Crate Dependency Observation
In `crates/aether_bridge/src/api.rs`:
Line 2 imports `use uuid::Uuid;`.
However, `crates/aether_bridge/Cargo.toml` does NOT currently list `uuid` in its dependencies. In Rust 2021 edition, direct imports of undeclared dependencies will cause compilation failure. `crates/aether_bridge/Cargo.toml` must add:
```toml
uuid = { version = "1.10", features = ["v4"] }
```
This observation has been noted for the bridge and worker teams.
