import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';
import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

/// In-memory native core simulator for stress testing TimelineNotifier and TimelineView.
class MemoryCoreSimulator {
  late Timeline timeline;
  int clipIdCounter = 0;

  MemoryCoreSimulator() {
    timeline = Timeline(
      id: UuidValue.fromString('a0000000-0000-0000-0000-000000000001'),
      timebase: const Rational(num: 60, den: 1),
      durationPts: 0,
      tracks: [
        Track(
          id: UuidValue.fromString('b0000000-0000-0000-0000-000000000001'),
          kind: TrackKind.video,
          clips: const [],
        ),
        Track(
          id: UuidValue.fromString('b0000000-0000-0000-0000-000000000002'),
          kind: TrackKind.audio,
          clips: const [],
        ),
      ],
    );
  }

  Future<Timeline> createTimeline() async {
    return timeline;
  }

  Future<Timeline> addClipToTrack({
    required Timeline timeline,
    required UuidValue trackId,
    required UuidValue sourceId,
    required int sourceIn,
    required int sourceOut,
    required int timelineIn,
  }) async {
    if (sourceOut < sourceIn) {
      throw Exception(
          'InvalidSourceBounds: source_in ($sourceIn) > source_out ($sourceOut)');
    }
    if (timelineIn < 0) {
      throw Exception('InvalidClipBounds: timeline_in cannot be negative');
    }

    final targetTrackIndex = timeline.tracks.indexWhere((t) => t.id == trackId);
    if (targetTrackIndex == -1) {
      throw Exception('TrackNotFound: track $trackId does not exist');
    }

    clipIdCounter++;
    final duration = sourceOut - sourceIn;
    final timelineOut = timelineIn + duration;

    final clip = Clip(
      id: UuidValue.fromString(
        'c0000000-0000-0000-0000-${clipIdCounter.toString().padLeft(12, '0')}',
      ),
      sourceId: sourceId,
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: timelineIn,
      timelineOut: timelineOut,
    );

    final updatedTracks = <Track>[];
    for (int i = 0; i < timeline.tracks.length; i++) {
      final t = timeline.tracks[i];
      if (i == targetTrackIndex) {
        updatedTracks.add(
          Track(id: t.id, kind: t.kind, clips: [...t.clips, clip]),
        );
      } else {
        updatedTracks.add(t);
      }
    }

    // Exact Rust core recalculation: max(timeline_out) across all tracks, clamped >= 0
    int maxPts = 0;
    for (final tr in updatedTracks) {
      for (final cl in tr.clips) {
        if (cl.timelineOut > maxPts) {
          maxPts = cl.timelineOut;
        }
      }
    }

    this.timeline = Timeline(
      id: timeline.id,
      timebase: timeline.timebase,
      durationPts: maxPts,
      tracks: updatedTracks,
    );

    return this.timeline;
  }
}

void main() {
  group('Challenger Stress & Empirical Verification Suite', () {
    testWidgets(
        'Stress: Sequential 10-clip addition verifies monotonic clip count & cumulative PTS recalculation in UI',
        (tester) async {
      final sim = MemoryCoreSimulator();
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: sim.createTimeline,
        addClipToTrackFn: sim.addClipToTrack,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );

      await notifier.initTimeline();
      await tester.pumpAndSettle();

      final countFinder = find.byKey(const Key('timeline_total_clips_count'));
      final buttonFinder = find.byKey(const Key('add_clip_button'));

      expect(tester.widget<Text>(countFinder).data, equals('0'));
      expect(find.text('Duration: 0 PTS'), findsOneWidget);

      // Perform 10 sequential additions via UI button taps
      int expectedDuration = 0;
      for (int i = 1; i <= 10; i++) {
        await tester.tap(buttonFinder);
        await tester.pumpAndSettle();

        expectedDuration += 60; // default duration is 60 PTS

        // 1. Verify monotonic increment of clip count
        expect(
          tester.widget<Text>(countFinder).data,
          equals('$i'),
          reason: 'Clip count must be $i after tap $i',
        );

        // 2. Verify accurate duration recalculation
        expect(
          find.text('Duration: $expectedDuration PTS'),
          findsOneWidget,
          reason: 'Duration must be $expectedDuration PTS after tap $i',
        );

        // 3. Verify notifier state matches UI exactly
        expect(notifier.state.totalClipCount, equals(i));
        expect(notifier.state.durationPts, equals(expectedDuration));
      }
    });

    testWidgets(
        'Heterogeneous Durations & Staggered Multi-Track PTS recalculation',
        (tester) async {
      final sim = MemoryCoreSimulator();
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: sim.createTimeline,
        addClipToTrackFn: sim.addClipToTrack,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );

      await notifier.initTimeline();
      await tester.pumpAndSettle();

      final videoTrackId = sim.timeline.tracks[0].id;
      final audioTrackId = sim.timeline.tracks[1].id;

      // Add Clip 1 to Video: [0..100 PTS]
      await notifier.addClip(
        trackId: videoTrackId,
        sourceIn: 0,
        sourceOut: 100,
        timelineIn: 0,
      );
      await tester.pumpAndSettle();
      expect(notifier.state.durationPts, equals(100));
      expect(notifier.state.totalClipCount, equals(1));

      // Add Clip 2 to Audio: [20..150 PTS] -> durationPts becomes 150
      await notifier.addClip(
        trackId: audioTrackId,
        sourceIn: 0,
        sourceOut: 130,
        timelineIn: 20,
      );
      await tester.pumpAndSettle();
      expect(notifier.state.durationPts, equals(150));
      expect(notifier.state.totalClipCount, equals(2));

      // Add Clip 3 to Video: [100..120 PTS] -> durationPts stays 150 because audio clip ends at 150
      await notifier.addClip(
        trackId: videoTrackId,
        sourceIn: 0,
        sourceOut: 20,
        timelineIn: 100,
      );
      await tester.pumpAndSettle();
      expect(notifier.state.durationPts, equals(150));
      expect(notifier.state.totalClipCount, equals(3));

      // Add Clip 4 to Video: [120..250 PTS] -> durationPts advances to 250
      await notifier.addClip(
        trackId: videoTrackId,
        sourceIn: 0,
        sourceOut: 130,
        timelineIn: 120,
      );
      await tester.pumpAndSettle();
      expect(notifier.state.durationPts, equals(250));
      expect(notifier.state.totalClipCount, equals(4));

      // Check UI rendered labels
      expect(find.text('Duration: 250 PTS'), findsOneWidget);
      expect(find.text('4'), findsOneWidget);
    });

    testWidgets('Rapid Tap Debounce / In-flight Guard Test', (tester) async {
      int addCallCount = 0;
      final sim = MemoryCoreSimulator();

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: sim.createTimeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          addCallCount++;
          // Artificial latency simulating FFI processing
          await Future.delayed(const Duration(milliseconds: 50));
          return sim.addClipToTrack(
            timeline: timeline,
            trackId: trackId,
            sourceId: sourceId,
            sourceIn: sourceIn,
            sourceOut: sourceOut,
            timelineIn: timelineIn,
          );
        },
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );

      await notifier.initTimeline();
      await tester.pumpAndSettle();

      // Trigger first addClip
      final f1 = notifier.addClip();
      expect(notifier.state.isLoading, isTrue);

      // Attempt concurrent second addClip while first is in-flight
      final f2 = notifier.addClip();

      // Advance fake async clock past the 50ms delay
      await tester.pump(const Duration(milliseconds: 60));
      await f1;
      await f2;
      await tester.pump();

      // In-flight guard: concurrent call while isLoading must be ignored
      expect(addCallCount, equals(1));
      expect(notifier.state.totalClipCount, equals(1));
      expect(notifier.state.isLoading, isFalse);
    });

    testWidgets(
        'Error recovery: error banner displays and clears on next successful addition',
        (tester) async {
      bool failNext = true;
      final sim = MemoryCoreSimulator();

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: sim.createTimeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          if (failNext) {
            failNext = false;
            throw Exception('EngineError: OutOfMemory');
          }
          return sim.addClipToTrack(
            timeline: timeline,
            trackId: trackId,
            sourceId: sourceId,
            sourceIn: sourceIn,
            sourceOut: sourceOut,
            timelineIn: timelineIn,
          );
        },
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );

      await notifier.initTimeline();
      await tester.pumpAndSettle();

      // Trigger failing addClip
      await notifier.addClip();
      await tester.pumpAndSettle();

      // Verify error banner is visible
      expect(notifier.state.errorMessage, contains('OutOfMemory'));
      expect(find.textContaining('OutOfMemory'), findsOneWidget);
      expect(notifier.state.totalClipCount, equals(0));

      // Trigger succeeding addClip
      await notifier.addClip();
      await tester.pumpAndSettle();

      // Verify error banner is cleared
      expect(notifier.state.errorMessage, isNull);
      expect(find.textContaining('OutOfMemory'), findsNothing);
      expect(notifier.state.totalClipCount, equals(1));
      expect(notifier.state.durationPts, equals(60));
    });
  });
}
