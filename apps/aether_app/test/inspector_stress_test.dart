import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/inspector/clip_properties_provider.dart';
import 'package:aether_app/src/features/inspector/inspector_view.dart';
import 'package:aether_app/src/features/inspector/widgets/audio_properties_view.dart';
import 'package:aether_app/src/features/inspector/widgets/sequence_properties_view.dart';
import 'package:aether_app/src/features/inspector/widgets/video_properties_view.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_selection_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

/// Mutable headless timeline notifier for empirical stress testing.
class MutableStressTimelineNotifier extends TimelineNotifier {
  MutableStressTimelineNotifier(Timeline initialTimeline)
      : super(autoInit: false, createTimelineFn: () async => initialTimeline) {
    state = TimelineState.fromTimeline(initialTimeline);
  }

  void updateTimeline(Timeline newTimeline) {
    state = TimelineState.fromTimeline(newTimeline);
  }
}

/// Mutable headless media pool notifier for empirical stress testing.
class MutableStressMediaPoolNotifier extends MediaPoolNotifier {
  MutableStressMediaPoolNotifier(List<MediaItem> initialItems) : super(autoLoad: false) {
    state = MediaPoolState(items: initialItems);
  }

  void updateItems(List<MediaItem> newItems) {
    state = MediaPoolState(items: newItems);
  }
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  // Test UUIDs
  final videoClipAId = UuidValue.fromString('a0000001-0000-0000-0000-000000000001');
  final audioClipBId = UuidValue.fromString('b0000002-0000-0000-0000-000000000002');
  final videoClipCId = UuidValue.fromString('c0000003-0000-0000-0000-000000000003');
  final overlayClipDId = UuidValue.fromString('d0000004-0000-0000-0000-000000000004');

  final videoTrackId = UuidValue.fromString('10000001-0000-0000-0000-000000000001');
  final audioTrackId = UuidValue.fromString('20000002-0000-0000-0000-000000000002');
  final overlayTrackId = UuidValue.fromString('30000003-0000-0000-0000-000000000003');

  final mediaVideoAId = UuidValue.fromString('40000001-0000-0000-0000-000000000001');
  final mediaAudioBId = UuidValue.fromString('50000002-0000-0000-0000-000000000002');
  final mediaVideoCId = UuidValue.fromString('60000003-0000-0000-0000-000000000003');
  final missingMediaId = UuidValue.fromString('deadbeef-0000-0000-0000-000000000001');

  final clipA = Clip(
    id: videoClipAId,
    sourceId: mediaVideoAId,
    sourceIn: 0,
    sourceOut: 500,
    timelineIn: 0,
    timelineOut: 500,
  );

  final clipB = Clip(
    id: audioClipBId,
    sourceId: mediaAudioBId,
    sourceIn: 0,
    sourceOut: 1000,
    timelineIn: 0,
    timelineOut: 1000,
  );

  final clipC = Clip(
    id: videoClipCId,
    sourceId: mediaVideoCId,
    sourceIn: 0,
    sourceOut: 300,
    timelineIn: 500,
    timelineOut: 800,
  );

  final clipD = Clip(
    id: overlayClipDId,
    sourceId: mediaVideoAId,
    sourceIn: 0,
    sourceOut: 400,
    timelineIn: 100,
    timelineOut: 500,
  );

  final videoTrack = Track(
    id: videoTrackId,
    kind: TrackKind.video,
    clips: [clipA, clipC],
  );

  final audioTrack = Track(
    id: audioTrackId,
    kind: TrackKind.audio,
    clips: [clipB],
  );

  final overlayTrack = Track(
    id: overlayTrackId,
    kind: TrackKind.overlay,
    clips: [clipD],
  );

  final defaultTimeline = Timeline(
    id: UuidValue.fromString('90000000-0000-0000-0000-000000000001'),
    timebase: const Rational(num: 60, den: 1),
    durationPts: 1000,
    tracks: [videoTrack, audioTrack, overlayTrack],
  );

  final defaultMediaPoolItems = [
    MediaItem(
      id: mediaVideoAId,
      filePath: '/media/video_a.mp4',
      fileName: 'video_a.mp4',
      mediaType: MediaType.video,
      metadata: const MediaMetadata(
        durationPts: 500,
        durationSeconds: 8.33,
        fileSizeBytes: 10000000,
      ),
    ),
    MediaItem(
      id: mediaAudioBId,
      filePath: '/media/audio_b.wav',
      fileName: 'audio_b.wav',
      mediaType: MediaType.audio,
      metadata: const MediaMetadata(
        durationPts: 1000,
        durationSeconds: 16.66,
        fileSizeBytes: 5000000,
      ),
    ),
    MediaItem(
      id: mediaVideoCId,
      filePath: '/media/video_c.mp4',
      fileName: 'video_c.mp4',
      mediaType: MediaType.video,
      metadata: const MediaMetadata(
        durationPts: 300,
        durationSeconds: 5.0,
        fileSizeBytes: 4000000,
      ),
    ),
  ];

  Widget buildStressInspectorHarness({
    required MutableStressTimelineNotifier timelineNotifier,
    required MutableStressMediaPoolNotifier mediaPoolNotifier,
    double width = 360,
    double height = 800,
    Widget? extraChild,
  }) {
    return ProviderScope(
      overrides: [
        timelineProvider.overrideWith((ref) => timelineNotifier),
        mediaPoolProvider.overrideWith((ref) => mediaPoolNotifier),
      ],
      child: MaterialApp(
        home: Scaffold(
          body: extraChild ??
              SizedBox(
                width: width,
                height: height,
                child: const InspectorView(),
              ),
        ),
      ),
    );
  }

  group('Empirical Challenge Area 1: Rapid High-Frequency Selection Stress', () {
    testWidgets(
      '1.1 100 rapid cycles: clip A (video) -> clip B (audio) -> background -> clip C (video) with zero state leakage',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Customize Clip A properties
        container.read(clipPropertiesProvider.notifier).updateVideoProperties(
              videoClipAId,
              positionX: 543.2,
              positionY: -128.4,
              scale: 2.45,
              rotation: 45.0,
              opacity: 0.8,
            );

        // Customize Clip B audio properties
        container.read(clipPropertiesProvider.notifier).updateAudioProperties(
              audioClipBId,
              volumeDb: -18.5,
              pan: -0.75,
              isMuted: true,
            );

        // Clip C is intentionally untouched (pristine defaults)

        // Run 100 rapid stress cycles
        for (int cycle = 0; cycle < 100; cycle++) {
          // 1. Select Clip A (Video)
          container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
          await tester.pump();

          expect(find.byType(VideoPropertiesView), findsOneWidget);
          expect(find.text('video_a.mp4'), findsOneWidget);
          expect(find.text('543.2 px'), findsOneWidget);
          expect(find.text('-128.4 px'), findsOneWidget);
          expect(find.text('245%'), findsOneWidget);
          expect(find.text('45.0°'), findsOneWidget);
          expect(find.text('80%'), findsOneWidget);

          // 2. Select Clip B (Audio)
          container.read(timelineSelectionProvider.notifier).selectClip(audioClipBId, audioTrackId);
          await tester.pump();

          expect(find.byType(AudioPropertiesView), findsOneWidget);
          expect(find.byType(VideoPropertiesView), findsNothing);
          expect(find.text('audio_b.wav'), findsOneWidget);
          expect(find.text('-18.5 dB'), findsOneWidget);
          expect(find.text('L 75%'), findsOneWidget);
          expect(find.text('Muted'), findsOneWidget);

          // 3. Clear selection (Background tap)
          container.read(timelineSelectionProvider.notifier).clearSelection();
          await tester.pump();

          expect(find.byType(SequencePropertiesView), findsOneWidget);
          expect(find.byType(VideoPropertiesView), findsNothing);
          expect(find.byType(AudioPropertiesView), findsNothing);

          // 4. Select Clip C (Video) - MUST show pristine defaults, ZERO leakage from Clip A
          container.read(timelineSelectionProvider.notifier).selectClip(videoClipCId, videoTrackId);
          await tester.pump();

          expect(find.byType(VideoPropertiesView), findsOneWidget);
          expect(find.text('video_c.mp4'), findsOneWidget);
          expect(find.text('0.0 px'), findsNWidgets(2)); // Default X and Y
          expect(find.text('100%'), findsNWidgets(2)); // Default scale and opacity
          expect(find.text('0.0°'), findsOneWidget); // Default rotation
        }

        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '1.2 Interactive TimelineView tap transitions: rapid clip taps & background taps',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
            extraChild: const Column(
              children: [
                SizedBox(
                  height: 300,
                  child: TimelineView(),
                ),
                Expanded(
                  child: InspectorView(),
                ),
              ],
            ),
          ),
        );
        await tester.pumpAndSettle();

        final clipAFinder = find.byKey(Key('timeline_clip_$videoClipAId'));
        final clipBFinder = find.byKey(Key('timeline_clip_$audioClipBId'));
        final clipCFinder = find.byKey(Key('timeline_clip_$videoClipCId'));

        expect(clipAFinder, findsOneWidget);
        expect(clipBFinder, findsOneWidget);
        expect(clipCFinder, findsOneWidget);

        // Initial inspector: sequence view
        expect(find.byType(SequencePropertiesView), findsOneWidget);

        // Tap clip A
        await tester.tap(clipAFinder);
        await tester.pumpAndSettle();
        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('video_a.mp4'), findsOneWidget);

        // Tap clip B
        await tester.tap(clipBFinder);
        await tester.pumpAndSettle();
        expect(find.byType(AudioPropertiesView), findsOneWidget);
        expect(find.text('audio_b.wav'), findsOneWidget);

        // Tap clip C
        await tester.tap(clipCFinder);
        await tester.pumpAndSettle();
        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('video_c.mp4'), findsOneWidget);

        // Tap track body background (empty space) to deselect
        final clipCTopRight = tester.getTopRight(clipCFinder);
        await tester.tapAt(clipCTopRight + const Offset(40, 10));
        await tester.pumpAndSettle();
        expect(find.byType(SequencePropertiesView), findsOneWidget);

        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '1.3 High-frequency microtask burst: 200 rapid synchronous selection mutations before single pump',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Rapidly dispatch 200 state changes without pumping
        for (int i = 0; i < 200; i++) {
          if (i % 3 == 0) {
            container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
          } else if (i % 3 == 1) {
            container.read(timelineSelectionProvider.notifier).selectClip(audioClipBId, audioTrackId);
          } else {
            container.read(timelineSelectionProvider.notifier).clearSelection();
          }
        }

        // Final deterministic selection: Clip A
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);

        // Pump frame
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('video_a.mp4'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  });

  group('Empirical Challenge Area 2: Extreme Slider Boundaries & Badge Formatting Stress', () {
    testWidgets(
      '2.1 Extreme Video Transform values (±100,000px, 100.0x scale, 36,000° rot) format cleanly without crash or overflow',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
            width: 360,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select Clip A
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
        await tester.pumpAndSettle();

        // Apply extreme video properties
        container.read(clipPropertiesProvider.notifier).updateVideoProperties(
              videoClipAId,
              positionX: 100000.0,
              positionY: -100000.0,
              scale: 100.0,
              rotation: 36000.0,
              opacity: 10.0,
            );

        await tester.pumpAndSettle();

        // Verify badge strings format cleanly
        expect(find.text('100000.0 px'), findsOneWidget);
        expect(find.text('-100000.0 px'), findsOneWidget);
        expect(find.text('10000%'), findsOneWidget);
        expect(find.text('36000.0°'), findsOneWidget);
        expect(find.text('1000%'), findsOneWidget);

        // Verify no Flutter assertion errors from Slider (clampedValue avoids assert failures)
        expect(tester.takeException(), isNull);

        // Test individual reset: Position X
        await tester.tap(find.byKey(const Key('reset_position_x')));
        await tester.pumpAndSettle();
        expect(find.text('0.0 px'), findsOneWidget);
        expect(find.text('-100000.0 px'), findsOneWidget); // Position Y still extreme

        // Test Reset All
        await tester.tap(find.byKey(const Key('reset_all_transform')));
        await tester.pumpAndSettle();

        expect(find.text('0.0 px'), findsNWidgets(2)); // Both X and Y back to 0.0 px
        expect(find.text('100%'), findsNWidgets(2)); // Scale and Opacity back to 100%
        expect(find.text('0.0°'), findsOneWidget); // Rotation back to 0.0°
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '2.2 Extreme Audio Controls values (-120 dB to +48 dB, pan ±10.0) format cleanly without crash',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
            width: 360,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select Clip B (Audio)
        container.read(timelineSelectionProvider.notifier).selectClip(audioClipBId, audioTrackId);
        await tester.pumpAndSettle();

        // 1. Extreme positive audio values
        container.read(clipPropertiesProvider.notifier).updateAudioProperties(
              audioClipBId,
              volumeDb: 48.0,
              pan: 10.0,
            );
        await tester.pumpAndSettle();

        expect(find.text('+48.0 dB'), findsOneWidget);
        expect(find.text('R 1000%'), findsOneWidget);
        expect(tester.takeException(), isNull);

        // 2. Extreme negative audio values
        container.read(clipPropertiesProvider.notifier).updateAudioProperties(
              audioClipBId,
              volumeDb: -120.0,
              pan: -10.0,
            );
        await tester.pumpAndSettle();

        expect(find.text('-120.0 dB'), findsOneWidget);
        expect(find.text('L 1000%'), findsOneWidget);
        expect(tester.takeException(), isNull);

        // Reset All Audio
        await tester.tap(find.byKey(const Key('reset_all_audio')));
        await tester.pumpAndSettle();

        expect(find.text('+0.0 dB'), findsOneWidget);
        expect(find.text('Center'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '2.3 Constrained width rendering: width 300px and 250px handle extreme badge values cleanly',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        // Render at 250px width
        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
            width: 250,
            height: 600,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
        container.read(clipPropertiesProvider.notifier).updateVideoProperties(
              videoClipAId,
              positionX: 100000.0,
              positionY: -100000.0,
              scale: 100.0,
              rotation: 36000.0,
            );

        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('100000.0 px'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '2.4 Edge failure: at panel registered minWidth (200px), extreme badge (-100000.0 px) overflows RenderFlex by 12px',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        final capturedErrors = <FlutterErrorDetails>[];
        final previousOnError = FlutterError.onError;
        FlutterError.onError = (details) => capturedErrors.add(details);

        try {
          // Render at EditorPanelRegistry registered minWidth: 200.0
          await tester.pumpWidget(
            buildStressInspectorHarness(
              timelineNotifier: timelineNotifier,
              mediaPoolNotifier: mediaPoolNotifier,
              width: 200,
              height: 600,
            ),
          );
          await tester.pumpAndSettle();

          final element = tester.element(find.byType(InspectorView));
          final container = ProviderScope.containerOf(element);

          container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
          container.read(clipPropertiesProvider.notifier).updateVideoProperties(
                videoClipAId,
                positionX: 100000.0,
                positionY: -100000.0,
              );

          await tester.pumpAndSettle();

          // Empirically confirm that RenderFlex overflow occurred for the extreme position badges
          expect(capturedErrors, isNotEmpty);
          final overflowError = capturedErrors.firstWhere(
            (e) => e.toString().contains('A RenderFlex overflowed by 12 pixels'),
          );
          expect(overflowError, isNotNull);
        } finally {
          FlutterError.onError = previousOnError;
        }
      },
    );

    testWidgets(
      '2.5 Audio properties at registered minWidth (200px) with extreme values (-120 dB, pan ±10.0)',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
            width: 200,
            height: 600,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(audioClipBId, audioTrackId);
        container.read(clipPropertiesProvider.notifier).updateAudioProperties(
              audioClipBId,
              volumeDb: -120.0,
              pan: -10.0,
            );

        await tester.pumpAndSettle();

        expect(find.byType(AudioPropertiesView), findsOneWidget);
        expect(find.text('-120.0 dB'), findsOneWidget);
        expect(find.text('L 1000%'), findsOneWidget);
        // Does Audio overflow at 200px? Let's check!
        final audioException = tester.takeException();
        expect(audioException, isNull);
      },
    );
  });

  group('Empirical Challenge Area 3: Edge Conditions Stress', () {
    testWidgets(
      '3.1 Missing sourceId in MediaPool gracefully falls back to default title without throwing',
      (WidgetTester tester) async {
        // Create orphan clips whose sourceIds do not exist in MediaPool
        final orphanVideoClip = Clip(
          id: UuidValue.fromString('e0000001-0000-0000-0000-000000000001'),
          sourceId: missingMediaId,
          sourceIn: 0,
          sourceOut: 200,
          timelineIn: 0,
          timelineOut: 200,
        );
        final orphanAudioClip = Clip(
          id: UuidValue.fromString('e0000002-0000-0000-0000-000000000002'),
          sourceId: missingMediaId,
          sourceIn: 0,
          sourceOut: 200,
          timelineIn: 0,
          timelineOut: 200,
        );

        final orphanTimeline = Timeline(
          id: UuidValue.fromString('e0000000-0000-0000-0000-000000000001'),
          timebase: const Rational(num: 60, den: 1),
          durationPts: 200,
          tracks: [
            Track(
              id: videoTrackId,
              kind: TrackKind.video,
              clips: [orphanVideoClip],
            ),
            Track(
              id: audioTrackId,
              kind: TrackKind.audio,
              clips: [orphanAudioClip],
            ),
          ],
        );

        final timelineNotifier = MutableStressTimelineNotifier(orphanTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(const []); // Empty media pool!

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select orphan video clip
        container.read(timelineSelectionProvider.notifier).selectClip(orphanVideoClip.id, videoTrackId);
        await tester.pumpAndSettle();

        // Must display fallback title 'Video Clip' without error
        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('Video Clip'), findsOneWidget);
        expect(tester.takeException(), isNull);

        // Select orphan audio clip
        container.read(timelineSelectionProvider.notifier).selectClip(orphanAudioClip.id, audioTrackId);
        await tester.pumpAndSettle();

        // Must display fallback title 'Audio Clip' without error
        expect(find.byType(AudioPropertiesView), findsOneWidget);
        expect(find.text('Audio Clip'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.2 Deleting selected clip from Timeline reactively reverts Inspector to SequencePropertiesView',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select Clip A
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);

        // Now simulate deleting Clip A from the timeline: track now only contains Clip C
        final updatedVideoTrack = Track(
          id: videoTrackId,
          kind: TrackKind.video,
          clips: [clipC], // Clip A is removed
        );

        final updatedTimeline = Timeline(
          id: defaultTimeline.id,
          timebase: defaultTimeline.timebase,
          durationPts: 1000,
          tracks: [updatedVideoTrack, audioTrack, overlayTrack],
        );

        // Push new timeline state
        timelineNotifier.updateTimeline(updatedTimeline);
        await tester.pumpAndSettle();

        // Inspector must immediately and gracefully revert to SequencePropertiesView!
        expect(find.byType(VideoPropertiesView), findsNothing);
        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(tester.takeException(), isNull);

        // Also clean up clip properties
        container.read(clipPropertiesProvider.notifier).removeClip(videoClipAId);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.3 Deleting entire parent track while clip is selected reverts Inspector to SequencePropertiesView',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select Clip B (Audio)
        container.read(timelineSelectionProvider.notifier).selectClip(audioClipBId, audioTrackId);
        await tester.pumpAndSettle();

        expect(find.byType(AudioPropertiesView), findsOneWidget);

        // Delete entire audio track
        final updatedTimeline = Timeline(
          id: defaultTimeline.id,
          timebase: defaultTimeline.timebase,
          durationPts: 1000,
          tracks: [videoTrack, overlayTrack], // audioTrack removed
        );

        timelineNotifier.updateTimeline(updatedTimeline);
        await tester.pumpAndSettle();

        // Inspector reverts to SequencePropertiesView
        expect(find.byType(AudioPropertiesView), findsNothing);
        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.4 Removing media item from MediaPool while clip is selected transitions title to fallback',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select Clip A
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
        await tester.pumpAndSettle();

        expect(find.text('video_a.mp4'), findsOneWidget);

        // Remove mediaVideoA from media pool
        mediaPoolNotifier.updateItems([defaultMediaPoolItems[1], defaultMediaPoolItems[2]]);
        await tester.pumpAndSettle();

        // Name gracefully reverts to fallback 'Video Clip'
        expect(find.text('video_a.mp4'), findsNothing);
        expect(find.text('Video Clip'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.5 Empty Timeline (0 tracks, 0 clips) renders SequencePropertiesView with zeroed metadata',
      (WidgetTester tester) async {
        final emptyTimeline = Timeline(
          id: UuidValue.fromString('00000000-0000-0000-0000-000000000000'),
          timebase: const Rational(num: 30, den: 1),
          durationPts: 0,
          tracks: const [],
        );

        final timelineNotifier = MutableStressTimelineNotifier(emptyTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(const []);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(find.text('0 PTS'), findsOneWidget);
        expect(find.textContaining('30 / 1 (30 fps)'), findsOneWidget);
        expect(find.text('0 (0 Video, 0 Audio)'), findsOneWidget);
        expect(find.text('0 clips'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.6 Overlay track clip renders VideoPropertiesView with OVERLAY badge and layer icon',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select overlay clip D
        container.read(timelineSelectionProvider.notifier).selectClip(overlayClipDId, overlayTrackId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('OVERLAY'), findsOneWidget);
        expect(find.byIcon(Icons.layers_rounded), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.7 Edge case: corrupted Rational timebase with den=0 renders Infinity fps cleanly',
      (WidgetTester tester) async {
        final zeroDenTimeline = Timeline(
          id: UuidValue.fromString('70000000-0000-0000-0000-000000000001'),
          timebase: const Rational(num: 60, den: 0), // Denominator = 0
          durationPts: 100,
          tracks: const [],
        );

        final timelineNotifier = MutableStressTimelineNotifier(zeroDenTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(const []);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(find.textContaining('60 / 0 (Infinity fps)'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.8 Inverted clip PTS range (timelineIn > timelineOut) renders negative duration without throwing',
      (WidgetTester tester) async {
        final invertedClip = Clip(
          id: UuidValue.fromString('f0000001-0000-0000-0000-000000000001'),
          sourceId: mediaVideoAId,
          sourceIn: 1000,
          sourceOut: 500,
          timelineIn: 800,
          timelineOut: 300, // Inverted!
        );

        final invertedTimeline = Timeline(
          id: UuidValue.fromString('f0000000-0000-0000-0000-000000000001'),
          timebase: const Rational(num: 60, den: 1),
          durationPts: 800,
          tracks: [
            Track(
              id: videoTrackId,
              kind: TrackKind.video,
              clips: [invertedClip],
            ),
          ],
        );

        final timelineNotifier = MutableStressTimelineNotifier(invertedTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(invertedClip.id, videoTrackId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.textContaining('Duration: -500 PTS'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.9 Interactive slider dragging: drag position X slider updates value and badge dynamically',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(videoClipAId, videoTrackId);
        await tester.pumpAndSettle();

        final sliderFinder = find.byKey(const Key('slider_position_x'));
        expect(sliderFinder, findsOneWidget);

        // Drag slider to the right
        await tester.drag(sliderFinder, const Offset(60, 0));
        await tester.pumpAndSettle();

        // Position X should no longer be 0.0 px
        final currentProps = container.read(clipPropertiesProvider).videoPropertiesFor(videoClipAId);
        expect(currentProps.positionX, isNot(equals(0.0)));
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '3.10 Rapid mute toggle switch maintains consistent state and styling',
      (WidgetTester tester) async {
        final timelineNotifier = MutableStressTimelineNotifier(defaultTimeline);
        final mediaPoolNotifier = MutableStressMediaPoolNotifier(defaultMediaPoolItems);

        await tester.pumpWidget(
          buildStressInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(audioClipBId, audioTrackId);
        await tester.pumpAndSettle();

        final muteSwitchFinder = find.byKey(const Key('switch_mute'));
        expect(muteSwitchFinder, findsOneWidget);

        // Toggle mute 10 times rapidly
        for (int i = 0; i < 10; i++) {
          await tester.tap(muteSwitchFinder);
          await tester.pumpAndSettle();
        }

        // After 10 toggles (even number), it should be unmuted
        final audioProps = container.read(clipPropertiesProvider).audioPropertiesFor(audioClipBId);
        expect(audioProps.isMuted, isFalse);
        expect(find.text('Track Audio Enabled'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  });
}
