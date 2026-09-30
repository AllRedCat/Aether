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

/// Headless timeline notifier for inspector widget tests.
class HeadlessInspectorTimelineNotifier extends TimelineNotifier {
  HeadlessInspectorTimelineNotifier(Timeline timeline)
      : super(autoInit: false, createTimelineFn: () async => timeline) {
    state = TimelineState.fromTimeline(timeline);
  }
}

/// Headless media pool notifier for inspector widget tests.
class HeadlessInspectorMediaPoolNotifier extends MediaPoolNotifier {
  HeadlessInspectorMediaPoolNotifier(List<MediaItem> items) : super(autoLoad: false) {
    state = MediaPoolState(items: items);
  }
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  // Test UUIDs
  final videoClipId = UuidValue.fromString('c0000000-0000-0000-0000-000000000001');
  final audioClipId = UuidValue.fromString('c0000000-0000-0000-0000-000000000002');
  final videoTrackId = UuidValue.fromString('t0000000-0000-0000-0000-000000000001');
  final audioTrackId = UuidValue.fromString('t0000000-0000-0000-0000-000000000002');
  final videoMediaId = UuidValue.fromString('m0000000-0000-0000-0000-000000000001');
  final audioMediaId = UuidValue.fromString('m0000000-0000-0000-0000-000000000002');

  final videoClip = Clip(
    id: videoClipId,
    sourceId: videoMediaId,
    sourceIn: 0,
    sourceOut: 600,
    timelineIn: 0,
    timelineOut: 600,
  );

  final audioClip = Clip(
    id: audioClipId,
    sourceId: audioMediaId,
    sourceIn: 0,
    sourceOut: 1200,
    timelineIn: 0,
    timelineOut: 1200,
  );

  final videoTrack = Track(
    id: videoTrackId,
    kind: TrackKind.video,
    clips: [videoClip],
  );

  final audioTrack = Track(
    id: audioTrackId,
    kind: TrackKind.audio,
    clips: [audioClip],
  );

  final testTimeline = Timeline(
    id: UuidValue.fromString('a0000000-0000-0000-0000-000000000001'),
    timebase: const Rational(num: 60, den: 1),
    durationPts: 1200,
    tracks: [videoTrack, audioTrack],
  );

  final testMediaItems = [
    MediaItem(
      id: videoMediaId,
      filePath: '/media/landscape.mp4',
      fileName: 'landscape.mp4',
      mediaType: MediaType.video,
      metadata: const MediaMetadata(
        durationPts: 600,
        durationSeconds: 10.0,
        fileSizeBytes: 10485760,
      ),
    ),
    MediaItem(
      id: audioMediaId,
      filePath: '/media/voiceover.wav',
      fileName: 'voiceover.wav',
      mediaType: MediaType.audio,
      metadata: const MediaMetadata(
        durationPts: 1200,
        durationSeconds: 20.0,
        fileSizeBytes: 2097152,
      ),
    ),
  ];

  Widget buildTestInspectorApp({
    Timeline? timeline,
    List<MediaItem>? mediaItems,
    Widget? customContent,
    List<Override> extraOverrides = const [],
  }) {
    return ProviderScope(
      overrides: [
        timelineProvider.overrideWith(
          (ref) => HeadlessInspectorTimelineNotifier(timeline ?? testTimeline),
        ),
        mediaPoolProvider.overrideWith(
          (ref) => HeadlessInspectorMediaPoolNotifier(mediaItems ?? testMediaItems),
        ),
        ...extraOverrides,
      ],
      child: MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 400,
            height: 800,
            child: InspectorView(customContent: customContent),
          ),
        ),
      ),
    );
  }

  group('M2: Contextual Property Inspector Suite', () {
    testWidgets('1. Empty selection renders SequencePropertiesView with metadata & guide prompt',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      // Verify SequencePropertiesView is visible
      expect(find.byType(SequencePropertiesView), findsOneWidget);
      expect(find.byKey(const Key('sequence_properties_view')), findsOneWidget);
      expect(find.byKey(const Key('sequence_title')), findsOneWidget);
      expect(find.byKey(const Key('sequence_hint')), findsOneWidget);

      // Verify sequence metadata
      expect(find.byKey(const Key('seq_duration_pts')), findsOneWidget);
      expect(find.text('1200 PTS'), findsOneWidget);
      expect(find.byKey(const Key('seq_timebase')), findsOneWidget);
      expect(find.textContaining('60 / 1 (60 fps)'), findsOneWidget);
      expect(find.byKey(const Key('seq_track_count')), findsOneWidget);
      expect(find.textContaining('2 (1 Video, 1 Audio)'), findsOneWidget);
      expect(find.byKey(const Key('seq_clip_count')), findsOneWidget);
      expect(find.text('2 clips'), findsOneWidget);

      // Verify video and audio views are NOT rendered
      expect(find.byType(VideoPropertiesView), findsNothing);
      expect(find.byType(AudioPropertiesView), findsNothing);
    });

    testWidgets('2. Selecting a video clip reactively transitions to VideoPropertiesView',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      // Trigger selection of video clip
      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);

      await tester.pumpAndSettle();

      // Verify SequencePropertiesView is replaced by VideoPropertiesView
      expect(find.byType(SequencePropertiesView), findsNothing);
      expect(find.byType(AudioPropertiesView), findsNothing);
      expect(find.byType(VideoPropertiesView), findsOneWidget);

      // Verify header and badge
      expect(find.byKey(const Key('video_clip_name')), findsOneWidget);
      expect(find.text('landscape.mp4'), findsOneWidget);
      expect(find.text('VIDEO'), findsOneWidget);
      expect(find.textContaining('Range: [0 .. 600 PTS]'), findsOneWidget);

      // Verify all 5 transform sliders are present
      expect(find.byKey(const Key('slider_position_x')), findsOneWidget);
      expect(find.byKey(const Key('slider_position_y')), findsOneWidget);
      expect(find.byKey(const Key('slider_scale')), findsOneWidget);
      expect(find.byKey(const Key('slider_rotation')), findsOneWidget);
      expect(find.byKey(const Key('slider_opacity')), findsOneWidget);

      // Verify default badge values
      expect(find.text('0.0 px'), findsNWidgets(2)); // Position X and Position Y
      expect(find.text('100%'), findsNWidgets(2)); // Scale and Opacity
      expect(find.text('0.0°'), findsOneWidget); // Rotation
    });

    testWidgets('3. Video transform adjustments update badges and individual resets restore defaults',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
      await tester.pumpAndSettle();

      // Adjust Position X and Scale via clipPropertiesProvider
      final propNotifier = container.read(clipPropertiesProvider.notifier);
      propNotifier.updateVideoProperties(videoClipId, positionX: 120.0, scale: 2.5);
      await tester.pumpAndSettle();

      // Verify updated numeric badges
      expect(find.text('120.0 px'), findsOneWidget);
      expect(find.text('250%'), findsOneWidget);

      // Tap individual reset for Position X
      await tester.tap(find.byKey(const Key('reset_position_x')));
      await tester.pumpAndSettle();

      // Position X should be reset to 0.0 px, while Scale remains 250%
      expect(find.text('0.0 px'), findsNWidgets(2));
      expect(find.text('250%'), findsOneWidget);

      // Tap individual reset for Scale
      await tester.tap(find.byKey(const Key('reset_scale')));
      await tester.pumpAndSettle();

      // Scale should be back to 100%
      expect(find.text('100%'), findsNWidgets(2));
    });

    testWidgets('4. Video Reset All button restores all transform parameters simultaneously',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
      await tester.pumpAndSettle();

      // Mutate multiple properties
      final propNotifier = container.read(clipPropertiesProvider.notifier);
      propNotifier.updateVideoProperties(
        videoClipId,
        positionX: 300.0,
        positionY: -150.0,
        scale: 1.8,
        rotation: 45.0,
        opacity: 0.5,
      );
      await tester.pumpAndSettle();

      expect(find.text('300.0 px'), findsOneWidget);
      expect(find.text('-150.0 px'), findsOneWidget);
      expect(find.text('180%'), findsOneWidget);
      expect(find.text('45.0°'), findsOneWidget);
      expect(find.text('50%'), findsOneWidget);

      // Tap Reset All
      await tester.tap(find.byKey(const Key('reset_all_transform')));
      await tester.pumpAndSettle();

      // All properties restored to default
      expect(find.text('0.0 px'), findsNWidgets(2));
      expect(find.text('100%'), findsNWidgets(2));
      expect(find.text('0.0°'), findsOneWidget);
    });

    testWidgets('5. Selecting an audio clip reactively transitions to AudioPropertiesView',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);
      container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
      await tester.pumpAndSettle();

      // Verify AudioPropertiesView is active
      expect(find.byType(SequencePropertiesView), findsNothing);
      expect(find.byType(VideoPropertiesView), findsNothing);
      expect(find.byType(AudioPropertiesView), findsOneWidget);

      // Verify header and badge
      expect(find.byKey(const Key('audio_clip_name')), findsOneWidget);
      expect(find.text('voiceover.wav'), findsOneWidget);
      expect(find.text('AUDIO'), findsOneWidget);
      expect(find.textContaining('Range: [0 .. 1200 PTS]'), findsOneWidget);

      // Verify controls
      expect(find.byKey(const Key('slider_volume_db')), findsOneWidget);
      expect(find.byKey(const Key('slider_pan')), findsOneWidget);
      expect(find.byKey(const Key('switch_mute')), findsOneWidget);

      // Verify initial values
      expect(find.text('+0.0 dB'), findsOneWidget);
      expect(find.text('Center'), findsOneWidget);
      expect(find.text('Track Audio Enabled'), findsOneWidget);
    });

    testWidgets('6. Audio adjustments, mute toggle, and individual resets work seamlessly',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);
      container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
      await tester.pumpAndSettle();

      // Update volume and pan
      final propNotifier = container.read(clipPropertiesProvider.notifier);
      propNotifier.updateAudioProperties(audioClipId, volumeDb: -6.5, pan: -0.75);
      await tester.pumpAndSettle();

      expect(find.text('-6.5 dB'), findsOneWidget);
      expect(find.text('L 75%'), findsOneWidget);

      // Toggle mute switch
      await tester.tap(find.byKey(const Key('switch_mute')));
      await tester.pumpAndSettle();

      expect(find.text('Muted'), findsOneWidget);
      expect(find.text('Audio output is silenced for this clip'), findsOneWidget);

      // Tap reset volume
      await tester.tap(find.byKey(const Key('reset_volume_db')));
      await tester.pumpAndSettle();
      expect(find.text('+0.0 dB'), findsOneWidget);

      // Tap reset pan
      await tester.tap(find.byKey(const Key('reset_pan')));
      await tester.pumpAndSettle();
      expect(find.text('Center'), findsOneWidget);
    });

    testWidgets('7. Audio Reset All restores volume, pan, and mute to defaults',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);
      container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
      await tester.pumpAndSettle();

      final propNotifier = container.read(clipPropertiesProvider.notifier);
      propNotifier.updateAudioProperties(audioClipId, volumeDb: 6.0, pan: 0.5, isMuted: true);
      await tester.pumpAndSettle();

      expect(find.text('+6.0 dB'), findsOneWidget);
      expect(find.text('R 50%'), findsOneWidget);
      expect(find.text('Muted'), findsOneWidget);

      // Tap Reset All
      await tester.tap(find.byKey(const Key('reset_all_audio')));
      await tester.pumpAndSettle();

      expect(find.text('+0.0 dB'), findsOneWidget);
      expect(find.text('Center'), findsOneWidget);
      expect(find.text('Track Audio Enabled'), findsOneWidget);
    });

    testWidgets('8. Deselecting (clearing selection) restores SequencePropertiesView',
        (WidgetTester tester) async {
      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);

      // 1. Select video clip
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
      await tester.pumpAndSettle();
      expect(find.byType(VideoPropertiesView), findsOneWidget);

      // 2. Clear selection
      container.read(timelineSelectionProvider.notifier).clearSelection();
      await tester.pumpAndSettle();

      // SequencePropertiesView restored
      expect(find.byType(SequencePropertiesView), findsOneWidget);
      expect(find.byType(VideoPropertiesView), findsNothing);
      expect(find.byType(AudioPropertiesView), findsNothing);
    });

    testWidgets('9. Custom content override bypasses contextual switching (M3 forward compatibility)',
        (WidgetTester tester) async {
      const customKey = Key('m3_custom_color_grading');
      await tester.pumpWidget(
        buildTestInspectorApp(
          customContent: Container(
            key: customKey,
            child: const Text('Color Grading Mode Active'),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.byKey(customKey), findsOneWidget);
      expect(find.text('Color Grading Mode Active'), findsOneWidget);
      expect(find.byType(SequencePropertiesView), findsNothing);
      expect(find.byType(VideoPropertiesView), findsNothing);
      expect(find.byType(AudioPropertiesView), findsNothing);
    });

    testWidgets('10. Inspector renders cleanly in constrained dimensions without layout overflow',
        (WidgetTester tester) async {
      // Test constrained viewport (200px width x 200px height)
      tester.view.physicalSize = const Size(200, 200);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(buildTestInspectorApp());
      await tester.pumpAndSettle();

      // Empty state fits
      expect(find.byType(SequencePropertiesView), findsOneWidget);
      expect(tester.takeException(), isNull);

      final element = tester.element(find.byType(InspectorView));
      final container = ProviderScope.containerOf(element);

      // Video state fits
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
      await tester.pumpAndSettle();
      expect(find.byType(VideoPropertiesView), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Audio state fits
      container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
      await tester.pumpAndSettle();
      expect(find.byType(AudioPropertiesView), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
