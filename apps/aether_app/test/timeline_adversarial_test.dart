import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';
import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

void main() {
  const uuidGen = Uuid();

  Track createTrack(TrackKind kind, List<Clip> clips) {
    return Track(
      id: uuidGen.v4obj(),
      kind: kind,
      clips: clips,
    );
  }

  Clip createClip({int sourceIn = 0, int sourceOut = 60, int timelineIn = 0}) {
    return Clip(
      id: uuidGen.v4obj(),
      sourceId: uuidGen.v4obj(),
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: timelineIn,
      timelineOut: timelineIn + (sourceOut - sourceIn),
    );
  }

  group('Adversarial Stress Test: Multi-Track Accumulation & Oracle', () {
    test(
        'TimelineState.fromTimeline correctly aggregates 50 tracks with 250 clips',
        () {
      final List<Track> tracks = [];
      int expectedTotalClips = 0;
      int maxPts = 0;

      for (int t = 0; t < 50; t++) {
        final TrackKind kind = (t % 3 == 0)
            ? TrackKind.video
            : (t % 3 == 1)
                ? TrackKind.audio
                : TrackKind.overlay;

        final int clipCountForTrack = (t % 5 == 0) ? 0 : (t % 10);
        final List<Clip> clips = [];
        int currentTrackPts = 0;

        for (int c = 0; c < clipCountForTrack; c++) {
          final clip = createClip(
            sourceIn: 0,
            sourceOut: 30 + c * 10,
            timelineIn: currentTrackPts,
          );
          clips.add(clip);
          currentTrackPts = clip.timelineOut;
          expectedTotalClips++;
          if (clip.timelineOut > maxPts) {
            maxPts = clip.timelineOut;
          }
        }

        tracks.add(createTrack(kind, clips));
      }

      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: maxPts,
        tracks: tracks,
      );

      final state = TimelineState.fromTimeline(timeline);

      expect(state.totalClipCount, equals(expectedTotalClips));
      expect(state.durationPts, equals(maxPts));
      expect(state.tracks.length, equals(50));
      expect(state.isLoading, isFalse);
      expect(state.errorMessage, isNull);
    });

    test('TimelineState handles extreme empty timeline (0 tracks)', () {
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: const [],
      );

      final state = TimelineState.fromTimeline(timeline);
      expect(state.totalClipCount, equals(0));
      expect(state.durationPts, equals(0));
      expect(state.tracks, isEmpty);
      expect(state.isLoading, isFalse);
      expect(state.errorMessage, isNull);
    });

    test(
        'Stress performance: 5,000 clips across 10 tracks computed without lag',
        () {
      final stopwatch = Stopwatch()..start();
      final List<Track> tracks = [];
      int totalClips = 0;

      for (int t = 0; t < 10; t++) {
        final clips = List.generate(500, (i) {
          totalClips++;
          return createClip(
            sourceIn: 0,
            sourceOut: 60,
            timelineIn: i * 60,
          );
        });
        tracks.add(createTrack(TrackKind.video, clips));
      }

      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 500 * 60,
        tracks: tracks,
      );

      final state = TimelineState.fromTimeline(timeline);
      stopwatch.stop();

      expect(state.totalClipCount, equals(5000));
      expect(totalClips, equals(5000));
      expect(stopwatch.elapsedMilliseconds,
          lessThan(100)); // Must execute in < 100ms
    });
  });

  group('Adversarial Stress Test: Error Recovery & Resilience', () {
    test('TimelineNotifier: FFI init failure sets error state without crashing',
        () async {
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async =>
            throw Exception('Native engine initialization failed: code -1'),
      );

      await notifier.initTimeline();

      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.timeline, isNull);
      expect(notifier.state.errorMessage,
          contains('Native engine initialization failed'));
    });

    test(
        'TimelineNotifier: addClip self-heals by re-initializing if timeline was null',
        () async {
      int initAttempts = 0;
      final defaultTrack = createTrack(TrackKind.video, []);
      final defaultTimeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [defaultTrack],
      );

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async {
          initAttempts++;
          if (initAttempts == 1) {
            throw Exception('Transient initialization timeout');
          }
          return defaultTimeline;
        },
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          final clip = createClip(
              sourceIn: sourceIn, sourceOut: sourceOut, timelineIn: timelineIn);
          final updatedTrack =
              Track(id: trackId, kind: TrackKind.video, clips: [clip]);
          return Timeline(
            id: timeline.id,
            timebase: timeline.timebase,
            durationPts: clip.timelineOut,
            tracks: [updatedTrack],
          );
        },
      );

      // Attempt 1: init fails
      await notifier.initTimeline();
      expect(notifier.state.errorMessage,
          contains('Transient initialization timeout'));
      expect(notifier.state.timeline, isNull);

      // Attempt 2: addClip triggers auto-init and succeeds
      await notifier.addClip();
      expect(initAttempts, equals(2));
      expect(notifier.state.errorMessage, isNull);
      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.totalClipCount, equals(1));
      expect(notifier.state.durationPts, equals(60));
    });

    test(
        'TimelineNotifier: handles empty tracks gracefully without throwing StateError',
        () async {
      final emptyTimeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: const [],
      );

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => emptyTimeline,
      );

      await notifier.initTimeline();
      expect(notifier.state.tracks, isEmpty);

      // Calling addClip on timeline with zero tracks
      await notifier.addClip();
      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.errorMessage,
          equals('No track available to add clip'));
    });

    test(
        'TimelineNotifier: handles addClipToTrack FFI exception and preserves previous state',
        () async {
      final track = createTrack(TrackKind.video, []);
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [track],
      );

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          throw Exception(
              'Rust core error: InvalidSourceBounds (source_in >= source_out)');
        },
      );

      await notifier.initTimeline();
      expect(notifier.state.totalClipCount, equals(0));

      // Attempt to add invalid clip
      await notifier.addClip(sourceIn: 100, sourceOut: 50);

      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.totalClipCount, equals(0));
      expect(notifier.state.durationPts, equals(0));
      expect(notifier.state.errorMessage, contains('InvalidSourceBounds'));

      // Now retry with valid parameters and functional addFn
      final recoveringNotifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          final clip = createClip(sourceIn: 0, sourceOut: 60, timelineIn: 0);
          return Timeline(
            id: timeline.id,
            timebase: timeline.timebase,
            durationPts: 60,
            tracks: [
              Track(id: trackId, kind: TrackKind.video, clips: [clip])
            ],
          );
        },
      );

      await recoveringNotifier.initTimeline();
      await recoveringNotifier.addClip();

      expect(recoveringNotifier.state.isLoading, isFalse);
      expect(recoveringNotifier.state.errorMessage, isNull);
      expect(recoveringNotifier.state.totalClipCount, equals(1));
      expect(recoveringNotifier.state.durationPts, equals(60));
    });
  });

  group('Adversarial Stress Test: Concurrency & Rapid Re-entrancy', () {
    test('Simultaneous addClip invocations are locked by isLoading guard',
        () async {
      final track = createTrack(TrackKind.video, []);
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [track],
      );

      int addCalls = 0;
      final completer = Completer<Timeline>();

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          addCalls++;
          return completer.future;
        },
      );

      await notifier.initTimeline();

      // Launch 10 simultaneous addClip calls
      final futures = List.generate(10, (_) => notifier.addClip());

      // While the first call is suspended on completer.future, state.isLoading must be true
      expect(notifier.state.isLoading, isTrue);

      // Resolve the suspended operation
      final clip = createClip(sourceIn: 0, sourceOut: 60, timelineIn: 0);
      completer.complete(Timeline(
        id: timeline.id,
        timebase: timeline.timebase,
        durationPts: 60,
        tracks: [
          Track(id: track.id, kind: TrackKind.video, clips: [clip])
        ],
      ));

      await Future.wait(futures);

      // Only the first call should have proceeded; 9 were dropped by `if (state.isLoading) return;`
      expect(addCalls, equals(1));
      expect(notifier.state.totalClipCount, equals(1));
      expect(notifier.state.isLoading, isFalse);
    });
  });

  group('Adversarial Stress Test: Immutability & State Contracts', () {
    test('TimelineState copyWith respects clearError flag and field overrides',
        () {
      const state1 = TimelineState(
        totalClipCount: 5,
        durationPts: 300,
        isLoading: true,
        errorMessage: 'Something went wrong',
      );

      final state2 = state1.copyWith(clearError: true, isLoading: false);
      expect(state2.errorMessage, isNull);
      expect(state2.isLoading, isFalse);
      expect(state2.totalClipCount, equals(5));
      expect(state2.durationPts, equals(300));
    });

    test('TimelineState equality reflects all state fields', () {
      final track = createTrack(TrackKind.video, []);
      final stateA = TimelineState(
        totalClipCount: 1,
        durationPts: 60,
        tracks: [track],
        isLoading: false,
        errorMessage: null,
      );
      final stateB = TimelineState(
        totalClipCount: 1,
        durationPts: 60,
        tracks: [track],
        isLoading: false,
        errorMessage: null,
      );

      expect(stateA, equals(stateB));

      expect(stateA == stateB.copyWith(totalClipCount: 2), isFalse);
      expect(stateA == stateB.copyWith(durationPts: 120), isFalse);
      expect(stateA == stateB.copyWith(isLoading: true), isFalse);
      expect(stateA == stateB.copyWith(errorMessage: 'err'), isFalse);
    });
  });

  group('Adversarial Stress Test: TimelineView UI Behavioral Robustness', () {
    testWidgets(
        'Rapid consecutive button taps do not crash or trigger concurrent executions',
        (tester) async {
      int addInvocations = 0;
      final track = createTrack(TrackKind.video, []);
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [track],
      );

      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          addInvocations++;
          // Simulate slight async delay
          await Future.delayed(const Duration(milliseconds: 50));
          final clip = createClip(
              sourceIn: 0, sourceOut: 60, timelineIn: timeline.durationPts);
          return Timeline(
            id: timeline.id,
            timebase: timeline.timebase,
            durationPts: timeline.durationPts + 60,
            tracks: [
              Track(
                  id: trackId,
                  kind: TrackKind.video,
                  clips: [...timeline.tracks.first.clips, clip])
            ],
          );
        },
      );

      await notifier.initTimeline();

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

      final buttonFinder = find.byKey(const Key('add_clip_button'));
      expect(buttonFinder, findsOneWidget);

      // Tap 5 times in rapid succession without pumpAndSettle in-between
      await tester.tap(buttonFinder);
      await tester.pump(); // Start first async action -> sets isLoading: true
      await tester.tap(buttonFinder); // Tap while loading (onPressed is null)
      await tester.tap(buttonFinder); // Tap while loading
      await tester
          .pump(const Duration(milliseconds: 60)); // Let first action complete
      await tester.pumpAndSettle();

      // Only 1 invocation should have executed because button was disabled during loading
      expect(addInvocations, equals(1));
      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('1'),
      );
    });

    testWidgets(
        'UI correctly renders multi-track lanes with Video, Audio, and Overlay',
        (tester) async {
      final clip1 = createClip(sourceIn: 0, sourceOut: 60, timelineIn: 0);
      final clip2 = createClip(sourceIn: 0, sourceOut: 120, timelineIn: 30);

      final videoTrack = createTrack(TrackKind.video, [clip1]);
      final audioTrack = createTrack(TrackKind.audio, [clip2]);
      final overlayTrack = createTrack(TrackKind.overlay, []);

      final state = TimelineState(
        totalClipCount: 2,
        durationPts: 150,
        tracks: [videoTrack, audioTrack, overlayTrack],
        isLoading: false,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => TimelineNotifier(
                  autoInit: false,
                  createTimelineFn: () async => throw UnimplementedError(),
                )..state = state),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );

      expect(find.text('Track 1 (VIDEO)'), findsOneWidget);
      expect(find.text('Track 2 (AUDIO)'), findsOneWidget);
      expect(find.text('Track 3 (OVERLAY)'), findsOneWidget);
      expect(find.text('Clip [0..60 PTS]'), findsOneWidget);
      expect(find.text('Clip [30..150 PTS]'), findsOneWidget);
      expect(find.text('Track is empty. Click "Add Clip" to add media.'),
          findsOneWidget);
    });

    testWidgets('UI displays centered spinner when loading initial timeline',
        (tester) async {
      const state = TimelineState(
        timeline: null,
        tracks: [],
        isLoading: true,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => TimelineNotifier(
                  autoInit: false,
                )..state = state),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );

      expect(find.byType(CircularProgressIndicator),
          findsNWidgets(2)); // Header button + center
      expect(find.text('No tracks available'), findsNothing);
    });

    testWidgets('Error banner is cleared upon initiating a new addClip attempt',
        (tester) async {
      final track = createTrack(TrackKind.video, []);
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [track],
      );

      final completer = Completer<Timeline>();
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async =>
            completer.future,
      );

      await notifier.initTimeline();
      // Set an initial error
      notifier.state =
          notifier.state.copyWith(errorMessage: 'Previous failure occurred');

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

      // Verify error banner is present
      expect(find.text('Previous failure occurred'), findsOneWidget);

      // Tap add clip button
      await tester.tap(find.byKey(const Key('add_clip_button')));
      await tester
          .pump(); // Triggers addClip -> sets isLoading: true, clearError: true

      // Verify error banner is immediately dismissed
      expect(find.text('Previous failure occurred'), findsNothing);

      // Complete the operation
      final clip = createClip(sourceIn: 0, sourceOut: 60, timelineIn: 0);
      completer.complete(Timeline(
        id: timeline.id,
        timebase: timeline.timebase,
        durationPts: 60,
        tracks: [
          Track(id: track.id, kind: TrackKind.video, clips: [clip])
        ],
      ));
      await tester.pumpAndSettle();

      expect(notifier.state.errorMessage, isNull);
      expect(notifier.state.totalClipCount, equals(1));
    });
  });

  group('Adversarial Stress Test: Targeted Track Selection & Edge Cases', () {
    test('addClip targets specified trackId accurately', () async {
      final track1 = createTrack(TrackKind.video, []);
      final track2 = createTrack(TrackKind.audio, []);
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [track1, track2],
      );

      UuidValue? capturedTrackId;
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          capturedTrackId = trackId;
          final clip = createClip(
              sourceIn: sourceIn, sourceOut: sourceOut, timelineIn: timelineIn);
          final updatedTracks = timeline.tracks.map((t) {
            if (t.id == trackId) {
              return Track(id: t.id, kind: t.kind, clips: [...t.clips, clip]);
            }
            return t;
          }).toList();
          return Timeline(
            id: timeline.id,
            timebase: timeline.timebase,
            durationPts: clip.timelineOut,
            tracks: updatedTracks,
          );
        },
      );

      await notifier.initTimeline();
      await notifier.addClip(trackId: track2.id);

      expect(capturedTrackId, equals(track2.id));
      expect(notifier.state.totalClipCount, equals(1));
      expect(notifier.state.tracks[1].clips.length, equals(1));
      expect(notifier.state.tracks[0].clips.length, equals(0));
    });

    test('addClip falls back to first track when non-existent trackId provided',
        () async {
      final track1 = createTrack(TrackKind.video, []);
      final timeline = Timeline(
        id: uuidGen.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [track1],
      );

      UuidValue? capturedTrackId;
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => timeline,
        addClipToTrackFn: ({
          required timeline,
          required trackId,
          required sourceId,
          required sourceIn,
          required sourceOut,
          required timelineIn,
        }) async {
          capturedTrackId = trackId;
          return timeline;
        },
      );

      await notifier.initTimeline();
      final nonExistentId = uuidGen.v4obj();
      await notifier.addClip(trackId: nonExistentId);

      // Falls back to track1.id
      expect(capturedTrackId, equals(track1.id));
    });
  });
}
