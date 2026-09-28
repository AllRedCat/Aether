#!/usr/bin/env python3
"""
Challenger Empirical Verification & Stress Oracle
Tests sequential clip additions, duration recalculations, and invariant boundaries.
"""

import sys
import uuid
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent

class Clip:
    def __init__(self, source_id, source_in, source_out, timeline_in):
        if source_out < source_in:
            raise ValueError(f"InvalidSourceBounds: {source_in} > {source_out}")
        if source_in < 0 or source_out < 0:
            raise ValueError("Negative source bounds")
        if timeline_in < 0:
            raise ValueError("Negative timeline_in")
            
        self.id = uuid.uuid4()
        self.source_id = source_id
        self.source_in = source_in
        self.source_out = source_out
        self.timeline_in = timeline_in
        self.timeline_out = timeline_in + (source_out - source_in)

class Track:
    def __init__(self, kind="video"):
        self.id = uuid.uuid4()
        self.kind = kind
        self.clips = []

    def duration_pts(self):
        return max([c.timeline_out for c in self.clips], default=0)

class Timeline:
    def __init__(self):
        self.id = uuid.uuid4()
        self.tracks = [Track("video")]
        self.duration_pts = 0

    def add_track(self, kind="audio"):
        t = Track(kind)
        self.tracks.append(t)
        return t.id

    def recalculate_duration(self):
        all_outs = [c.timeline_out for t in self.tracks for c in t.clips]
        self.duration_pts = max(all_outs, default=0)

    def add_clip(self, track_id, source_in, source_out, timeline_in=None):
        target = next((t for t in self.tracks if t.id == track_id), None)
        if not target:
            raise KeyError(f"TrackNotFound: {track_id}")
            
        insertion_pts = timeline_in if timeline_in is not None else self.duration_pts
        clip = Clip(uuid.uuid4(), source_in, source_out, insertion_pts)
        target.clips.append(clip)
        self.recalculate_duration()
        return clip

    def total_clip_count(self):
        return sum(len(t.clips) for t in self.tracks)

def test_sequential_monotonic_additions():
    """Verify 500 sequential clip additions monotonically increment count and advance duration."""
    tl = Timeline()
    video_id = tl.tracks[0].id
    
    clip_len = 60
    for i in range(1, 501):
        tl.add_clip(video_id, source_in=0, source_out=clip_len)
        expected_pts = i * clip_len
        assert tl.total_clip_count() == i, f"Expected {i} clips, got {tl.total_clip_count()}"
        assert tl.duration_pts == expected_pts, f"Expected {expected_pts} PTS, got {tl.duration_pts}"
        
    print("[PASS] Sequential Monotonic Additions (500 clips): monotonic count & exact cumulative PTS.")

def test_staggered_multitrack_recalculations():
    """Verify multi-track staggered clip additions correctly resolve max duration PTS."""
    tl = Timeline()
    v_id = tl.tracks[0].id
    a_id = tl.add_track("audio")
    o_id = tl.add_track("overlay")
    
    # 1. Video clip: 0..120
    tl.add_clip(v_id, 0, 120, timeline_in=0)
    assert tl.duration_pts == 120
    assert tl.total_clip_count() == 1
    
    # 2. Audio clip: 60..240 (max becomes 240)
    tl.add_clip(a_id, 0, 180, timeline_in=60)
    assert tl.duration_pts == 240
    assert tl.total_clip_count() == 2
    
    # 3. Overlay clip: 10..50 (max remains 240)
    tl.add_clip(o_id, 0, 40, timeline_in=10)
    assert tl.duration_pts == 240
    assert tl.total_clip_count() == 3
    
    # 4. Video clip: 200..350 (max becomes 350)
    tl.add_clip(v_id, 0, 150, timeline_in=200)
    assert tl.duration_pts == 350
    assert tl.total_clip_count() == 4
    
    print("[PASS] Staggered Multi-Track PTS Recalculation: correctly resolves max PTS across 3 tracks.")

def test_bounds_rejection_invariants():
    """Verify bounds violations are strictly rejected without mutating timeline state."""
    tl = Timeline()
    v_id = tl.tracks[0].id
    tl.add_clip(v_id, 0, 100, timeline_in=0)
    assert tl.duration_pts == 100
    assert tl.total_clip_count() == 1
    
    # Negative timeline_in
    try:
        tl.add_clip(v_id, 0, 50, timeline_in=-10)
        assert False, "Should have rejected negative timeline_in"
    except ValueError:
        pass
        
    # Inverted source bounds
    try:
        tl.add_clip(v_id, 100, 50, timeline_in=100)
        assert False, "Should have rejected inverted source bounds"
    except ValueError:
        pass
        
    # Non-existent track
    try:
        tl.add_clip(uuid.uuid4(), 0, 50, timeline_in=100)
        assert False, "Should have rejected non-existent track"
    except KeyError:
        pass
        
    # Verify state remained pristine
    assert tl.duration_pts == 100
    assert tl.total_clip_count() == 1
    print("[PASS] Bounds Rejection Invariants: rejected invalid additions without state corruption.")

if __name__ == "__main__":
    test_sequential_monotonic_additions()
    test_staggered_multitrack_recalculations()
    test_bounds_rejection_invariants()
    print("All Challenger Empirical Oracles Passed!")
