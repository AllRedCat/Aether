import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';
import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/bridge/frb_generated.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

/// Fake implementation of RustLibApi to simulate Rust FFI responses in headless tests.
class FakeRustLibApi implements RustLibApi {
  int clipCounter = 0;
  Timeline timeline = Timeline(
    id: UuidValue.fromString('a0000000-0000-0000-0000-000000000001'),
    timebase: const Rational(num: 60, den: 1),
    durationPts: 0,
    tracks: [
      Track(
        id: UuidValue.fromString('b0000000-0000-0000-0000-000000000001'),
        kind: TrackKind.video,
        clips: const [],
      ),
    ],
  );

  @override
  Future<void> crateApiInitEngine() async {}

  @override
  Future<Timeline> crateApiCreateTimeline() async => timeline;

  @override
  Future<Timeline> crateApiAddTrack({
    required Timeline timeline,
    required TrackKind kind,
  }) async =>
      timeline;

  @override
  Future<Timeline> crateApiAddClipToTrack({
    required Timeline timeline,
    required UuidValue trackId,
    required UuidValue sourceId,
    required int sourceIn,
    required int sourceOut,
    required int timelineIn,
  }) async {
    clipCounter++;
    final duration = sourceOut - sourceIn;
    final clip = Clip(
      id: UuidValue.fromString(
        'c0000000-0000-0000-0000-${clipCounter.toString().padLeft(12, '0')}',
      ),
      sourceId: sourceId,
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: timelineIn,
      timelineOut: timelineIn + duration,
    );

    final updatedTracks = timeline.tracks.map((t) {
      if (t.id == trackId) {
        return Track(id: t.id, kind: t.kind, clips: [...t.clips, clip]);
      }
      return t;
    }).toList();

    timeline = Timeline(
      id: timeline.id,
      timebase: timeline.timebase,
      durationPts: clip.timelineOut,
      tracks: updatedTracks,
    );
    return timeline;
  }
}

/// Mock notifier allowing exact state control for isolated widget testing.
class MockTimelineNotifier extends TimelineNotifier {
  bool addClipCalled = false;

  MockTimelineNotifier(TimelineState initial)
      : super(
          autoInit: false,
          createTimelineFn: () async => throw UnimplementedError(),
          addClipToTrackFn: ({
            required timeline,
            required trackId,
            required sourceId,
            required sourceIn,
            required sourceOut,
            required timelineIn,
          }) async =>
              throw UnimplementedError(),
        ) {
    state = initial;
  }

  @override
  Future<void> addClip({
    UuidValue? trackId,
    UuidValue? sourceId,
    int sourceIn = 0,
    int sourceOut = 60,
    int? timelineIn,
  }) async {
    addClipCalled = true;
    final duration = sourceOut - sourceIn;
    final inPts = timelineIn ?? state.durationPts;
    final outPts = inPts + duration;

    final newClip = Clip(
      id: UuidValue.fromString('c0000000-0000-0000-0000-000000000001'),
      sourceId: sourceId ??
          UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: inPts,
      timelineOut: outPts,
    );

    final updatedTracks = state.tracks.map((t) {
      return Track(id: t.id, kind: t.kind, clips: [...t.clips, newClip]);
    }).toList();

    state = state.copyWith(
      totalClipCount: state.totalClipCount + 1,
      durationPts: outPts,
      tracks: updatedTracks,
    );
  }
}

void main() {
  group('TimelineView Isolated Widget Tests', () {
    testWidgets(
        'Renders initial state with 0 clips, duration 0, and action button',
        (tester) async {
      final notifier = MockTimelineNotifier(
        const TimelineState(
          totalClipCount: 0,
          durationPts: 0,
          tracks: [],
          isLoading: false,
        ),
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

      final countFinder = find.byKey(const Key('timeline_total_clips_count'));
      expect(countFinder, findsOneWidget);
      expect(tester.widget<Text>(countFinder).data, equals('0'));

      expect(find.text('Duration: 0 PTS'), findsOneWidget);
      expect(find.byKey(const Key('add_clip_button')), findsOneWidget);
      expect(find.text('No tracks available'), findsOneWidget);
    });

    testWidgets('Tapping Add Clip button triggers notifier and updates UI',
        (tester) async {
      final trackId =
          UuidValue.fromString('b0000000-0000-0000-0000-000000000001');
      final notifier = MockTimelineNotifier(
        TimelineState(
          totalClipCount: 0,
          durationPts: 0,
          tracks: [Track(id: trackId, kind: TrackKind.video, clips: const [])],
          isLoading: false,
        ),
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

      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('0'),
      );
      expect(find.text('Track is empty. Click "Add Clip" to add media.'),
          findsOneWidget);

      await tester.tap(find.byKey(const Key('add_clip_button')));
      await tester.pump();

      expect(notifier.addClipCalled, isTrue);
      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('1'),
      );
      expect(find.text('Duration: 60 PTS'), findsOneWidget);
      expect(find.text('Clip [0..60 PTS]'), findsOneWidget);
    });

    testWidgets('Shows error banner when errorMessage is present in state',
        (tester) async {
      final notifier = MockTimelineNotifier(
        const TimelineState(
          totalClipCount: 0,
          durationPts: 0,
          tracks: [],
          isLoading: false,
          errorMessage: 'Invalid clip bounds detected',
        ),
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

      expect(find.text('Invalid clip bounds detected'), findsOneWidget);
    });
  });

  group('Timeline Full Integration with RustLib Mock', () {
    setUp(() {
      RustLib.initMock(api: FakeRustLibApi());
    });

    testWidgets(
        'E2E Flow: auto-init timeline, add clip 1, add clip 2 with recalculation',
        (tester) async {
      await tester.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: Scaffold(
              body: TimelineView(),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Step 1: Initial state
      expect(
          find.byKey(const Key('timeline_total_clips_count')), findsOneWidget);
      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('0'),
      );
      expect(find.text('Duration: 0 PTS'), findsOneWidget);
      expect(find.text('Track 1 (VIDEO)'), findsOneWidget);

      final addBtn = find.byKey(const Key('add_clip_button'));
      expect(addBtn, findsOneWidget);

      // Step 2: Add Clip 1
      await tester.tap(addBtn);
      await tester.pumpAndSettle();

      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('1'),
      );
      expect(find.text('Duration: 60 PTS'), findsOneWidget);
      expect(find.text('Clip [0..60 PTS]'), findsOneWidget);

      // Step 3: Add Clip 2
      await tester.tap(addBtn);
      await tester.pumpAndSettle();

      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('2'),
      );
      expect(find.text('Duration: 120 PTS'), findsOneWidget);
      expect(find.text('Clip [60..120 PTS]'), findsOneWidget);
    });
  });

  group('TimelineState and Notifier Unit Tests', () {
    test('TimelineState.fromTimeline aggregates multiple tracks and clips', () {
      final clip1 = Clip(
        id: UuidValue.fromString('c0000000-0000-0000-0000-000000000001'),
        sourceId: UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
        sourceIn: 0,
        sourceOut: 60,
        timelineIn: 0,
        timelineOut: 60,
      );
      final clip2 = Clip(
        id: UuidValue.fromString('c0000000-0000-0000-0000-000000000002'),
        sourceId: UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
        sourceIn: 0,
        sourceOut: 120,
        timelineIn: 60,
        timelineOut: 180,
      );

      final track1 = Track(
        id: UuidValue.fromString('b0000000-0000-0000-0000-000000000001'),
        kind: TrackKind.video,
        clips: [clip1],
      );
      final track2 = Track(
        id: UuidValue.fromString('b0000000-0000-0000-0000-000000000002'),
        kind: TrackKind.audio,
        clips: [clip2],
      );

      final timeline = Timeline(
        id: UuidValue.fromString('a0000000-0000-0000-0000-000000000001'),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 180,
        tracks: [track1, track2],
      );

      final state = TimelineState.fromTimeline(timeline);
      expect(state.totalClipCount, equals(2));
      expect(state.durationPts, equals(180));
      expect(state.tracks.length, equals(2));
      expect(state.isLoading, isFalse);
      expect(state.errorMessage, isNull);
    });

    test('TimelineNotifier handles FFI error gracefully without crashing',
        () async {
      final notifier = TimelineNotifier(
        autoInit: false,
        createTimelineFn: () async => throw Exception('Bridge network error'),
      );

      await notifier.initTimeline();
      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.errorMessage, contains('Bridge network error'));
    });
  });
}
