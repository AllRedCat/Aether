import 'dart:math' as math;
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';
import 'package:aether_app/src/bridge/api.dart';

void main() {
  group('Adversarial Dart FFI Contract Verification', () {
    test(
        'Direct field access without casting: tracks, clips, timelineIn, durationPts',
        () {
      final timelineId =
          UuidValue.fromString('a0000000-0000-0000-0000-000000000001');
      final trackId1 =
          UuidValue.fromString('b0000000-0000-0000-0000-000000000001');
      final trackId2 =
          UuidValue.fromString('b0000000-0000-0000-0000-000000000002');
      final clipId1 =
          UuidValue.fromString('c0000000-0000-0000-0000-000000000001');
      final clipId2 =
          UuidValue.fromString('c0000000-0000-0000-0000-000000000002');
      final sourceId =
          UuidValue.fromString('d0000000-0000-0000-0000-000000000001');

      final clip1 = Clip(
        id: clipId1,
        sourceId: sourceId,
        sourceIn: 0,
        sourceOut: 120,
        timelineIn: 0,
        timelineOut: 120,
      );

      final clip2 = Clip(
        id: clipId2,
        sourceId: sourceId,
        sourceIn: 10,
        sourceOut: 70,
        timelineIn: 100,
        timelineOut: 160,
      );

      final track1 = Track(
        id: trackId1,
        kind: TrackKind.video,
        clips: [clip1],
      );

      final track2 = Track(
        id: trackId2,
        kind: TrackKind.audio,
        clips: [clip2],
      );

      const timebase = Rational(num: 60, den: 1);

      final timeline = Timeline(
        id: timelineId,
        timebase: timebase,
        durationPts: 160,
        tracks: [track1, track2],
      );

      // 1. Check timeline.tracks directly read as List<Track>
      final List<Track> tracks = timeline.tracks;
      expect(tracks.length, equals(2));

      // 2. Check track.clips directly read as List<Clip>
      final List<Clip> clipsOnTrack1 = tracks[0].clips;
      final List<Clip> clipsOnTrack2 = tracks[1].clips;
      expect(clipsOnTrack1.length, equals(1));
      expect(clipsOnTrack2.length, equals(1));

      // 3. Check clip.timelineIn directly read as int without casting or conversion
      final int clip1In = clipsOnTrack1[0].timelineIn;
      final int clip2In = clipsOnTrack2[0].timelineIn;
      expect(clip1In, equals(0));
      expect(clip2In, equals(100));

      // 4. Check timeline.durationPts directly read as int without casting or conversion
      final int duration = timeline.durationPts;
      expect(duration, equals(160));

      // 5. Arithmetic operations directly on int timestamps
      final int sumIn = clip1In + clip2In;
      expect(sumIn, equals(100));
      final int delta = duration - clip2In;
      expect(delta, equals(60));

      // 6. Aggregation and fold operations as expected by Riverpod state / UI
      final int totalClipCount = timeline.tracks.fold<int>(
        0,
        (acc, track) => acc + track.clips.length,
      );
      expect(totalClipCount, equals(2));

      final int maxTimelineOut = timeline.tracks
          .expand((track) => track.clips)
          .map((clip) => clip.timelineOut)
          .fold<int>(0, math.max);
      expect(maxTimelineOut, equals(timeline.durationPts));
    });

    test('Value equality and hashCode contracts for mirrored data classes', () {
      final clipA = Clip(
        id: UuidValue.fromString('00000000-0000-0000-0000-000000000001'),
        sourceId: UuidValue.fromString('00000000-0000-0000-0000-000000000002'),
        sourceIn: 10,
        sourceOut: 50,
        timelineIn: 100,
        timelineOut: 140,
      );

      final clipB = Clip(
        id: UuidValue.fromString('00000000-0000-0000-0000-000000000001'),
        sourceId: UuidValue.fromString('00000000-0000-0000-0000-000000000002'),
        sourceIn: 10,
        sourceOut: 50,
        timelineIn: 100,
        timelineOut: 140,
      );

      // Clip value equality
      expect(clipA, equals(clipB));
      expect(clipA.hashCode, equals(clipB.hashCode));

      // Rational value equality
      const ratA = Rational(num: 30, den: 1);
      const ratB = Rational(num: 30, den: 1);
      expect(ratA, equals(ratB));
      expect(ratA.hashCode, equals(ratB.hashCode));

      // Track with identical clip list reference
      final sharedClips = [clipA];
      final trackA = Track(
        id: UuidValue.fromString('00000000-0000-0000-0000-000000000003'),
        kind: TrackKind.video,
        clips: sharedClips,
      );

      final trackB = Track(
        id: UuidValue.fromString('00000000-0000-0000-0000-000000000003'),
        kind: TrackKind.video,
        clips: sharedClips,
      );

      expect(trackA, equals(trackB));
      expect(trackA.hashCode, equals(trackB.hashCode));

      // Note on Dart List equality: Dart List uses identity equality for `==`.
      // Two separate List instances with equal items are not `==` under standard Dart `==`.
      final separateClips1 = [clipA];
      final separateClips2 = [clipB];
      expect(separateClips1 == separateClips2, isFalse);

      final trackWithSeparateLists = Track(
        id: UuidValue.fromString('00000000-0000-0000-0000-000000000003'),
        kind: TrackKind.video,
        clips: separateClips2,
      );
      // Demonstrates FRB's `clips == other.clips` list reference check
      expect(trackA == trackWithSeparateLists, isFalse);
    });

    test('Verify FFI function call type signatures', () {
      expect(initEngine, isA<Future<void> Function()>());
      expect(createTimeline, isA<Future<Timeline> Function()>());
      expect(
        addClipToTrack,
        isA<
            Future<Timeline> Function({
              required Timeline timeline,
              required UuidValue trackId,
              required UuidValue sourceId,
              required int sourceIn,
              required int sourceOut,
              required int timelineIn,
            })>(),
      );
      expect(
        addTrack,
        isA<
            Future<Timeline> Function({
              required Timeline timeline,
              required TrackKind kind,
            })>(),
      );
    });
  });
}
