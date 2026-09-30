import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/editor/editor_screen.dart';
import 'package:aether_app/src/features/editor/layout/editor_layout_state.dart';
import 'package:aether_app/src/features/inspector/clip_properties_provider.dart';
import 'package:aether_app/src/features/inspector/inspector_view.dart';
import 'package:aether_app/src/features/inspector/widgets/audio_properties_view.dart';
import 'package:aether_app/src/features/inspector/widgets/sequence_properties_view.dart';
import 'package:aether_app/src/features/inspector/widgets/video_properties_view.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_selection_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

/// Mutable headless timeline notifier for empirical viewport and integration testing.
class MutableIntegrationTimelineNotifier extends TimelineNotifier {
  MutableIntegrationTimelineNotifier(Timeline initialTimeline)
      : super(autoInit: false, createTimelineFn: () async => initialTimeline) {
    state = TimelineState.fromTimeline(initialTimeline);
  }

  void setTimeline(Timeline newTimeline) {
    state = TimelineState.fromTimeline(newTimeline);
  }
}

/// Mutable headless media pool notifier for empirical viewport and integration testing.
class MutableIntegrationMediaPoolNotifier extends MediaPoolNotifier {
  MutableIntegrationMediaPoolNotifier(List<MediaItem> initialItems) : super(autoLoad: false) {
    state = MediaPoolState(items: initialItems);
  }

  void setItems(List<MediaItem> newItems) {
    state = MediaPoolState(items: newItems);
  }
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  // Test UUIDs
  final videoClipId = UuidValue.fromString('11111111-0000-0000-0000-000000000001');
  final audioClipId = UuidValue.fromString('22222222-0000-0000-0000-000000000002');
  final overlayClipId = UuidValue.fromString('33333333-0000-0000-0000-000000000003');

  final track1VideoId = UuidValue.fromString('aaaa0001-0000-0000-0000-000000000001');
  final track2AudioId = UuidValue.fromString('bbbb0002-0000-0000-0000-000000000002');
  final track3OverlayId = UuidValue.fromString('cccc0003-0000-0000-0000-000000000003');

  final mediaVideoId = UuidValue.fromString('eeee0001-0000-0000-0000-000000000001');
  final mediaAudioId = UuidValue.fromString('eeee0002-0000-0000-0000-000000000002');
  final mediaOverlayId = UuidValue.fromString('eeee0003-0000-0000-0000-000000000003');

  final clipVideo = Clip(
    id: videoClipId,
    sourceId: mediaVideoId,
    sourceIn: 0,
    sourceOut: 600,
    timelineIn: 0,
    timelineOut: 600,
  );

  final clipAudio = Clip(
    id: audioClipId,
    sourceId: mediaAudioId,
    sourceIn: 0,
    sourceOut: 1200,
    timelineIn: 0,
    timelineOut: 1200,
  );

  final clipOverlay = Clip(
    id: overlayClipId,
    sourceId: mediaOverlayId,
    sourceIn: 0,
    sourceOut: 300,
    timelineIn: 100,
    timelineOut: 400,
  );

  final track1Video = Track(
    id: track1VideoId,
    kind: TrackKind.video,
    clips: [clipVideo],
  );

  final track2Audio = Track(
    id: track2AudioId,
    kind: TrackKind.audio,
    clips: [clipAudio],
  );

  final track3Overlay = Track(
    id: track3OverlayId,
    kind: TrackKind.overlay,
    clips: [clipOverlay],
  );

  final multiTrackTimeline = Timeline(
    id: UuidValue.fromString('99999999-0000-0000-0000-000000000001'),
    timebase: const Rational(num: 60, den: 1),
    durationPts: 1200,
    tracks: [track1Video, track2Audio, track3Overlay],
  );

  final mediaItems = [
    MediaItem(
      id: mediaVideoId,
      filePath: '/media/main_camera.mp4',
      fileName: 'main_camera.mp4',
      mediaType: MediaType.video,
      metadata: const MediaMetadata(
        durationPts: 600,
        durationSeconds: 10.0,
        fileSizeBytes: 20971520,
      ),
    ),
    MediaItem(
      id: mediaAudioId,
      filePath: '/media/soundtrack.wav',
      fileName: 'soundtrack.wav',
      mediaType: MediaType.audio,
      metadata: const MediaMetadata(
        durationPts: 1200,
        durationSeconds: 20.0,
        fileSizeBytes: 5242880,
      ),
    ),
    MediaItem(
      id: mediaOverlayId,
      filePath: '/media/lower_third.mov',
      fileName: 'lower_third.mov',
      mediaType: MediaType.video,
      metadata: const MediaMetadata(
        durationPts: 300,
        durationSeconds: 5.0,
        fileSizeBytes: 8388608,
      ),
    ),
  ];

  Widget buildConstrainedInspectorHarness({
    required TimelineNotifier timelineNotifier,
    required MediaPoolNotifier mediaPoolNotifier,
    double? width,
    double? height,
    Widget? customContent,
  }) {
    return ProviderScope(
      overrides: [
        timelineProvider.overrideWith((ref) => timelineNotifier),
        mediaPoolProvider.overrideWith((ref) => mediaPoolNotifier),
      ],
      child: MaterialApp(
        home: Scaffold(
          body: width != null && height != null
              ? Center(
                  child: SizedBox(
                    width: width,
                    height: height,
                    child: InspectorView(customContent: customContent),
                  ),
                )
              : SizedBox.expand(
                  child: InspectorView(customContent: customContent),
                ),
        ),
      ),
    );
  }

  Widget buildFullEditorHarness({
    required TimelineNotifier timelineNotifier,
    required MediaPoolNotifier mediaPoolNotifier,
  }) {
    return ProviderScope(
      overrides: [
        timelineProvider.overrideWith((ref) => timelineNotifier),
        mediaPoolProvider.overrideWith((ref) => mediaPoolNotifier),
      ],
      child: const MaterialApp(
        home: EditorScreen(),
      ),
    );
  }

  group('Area 1: Small Viewport & Layout Bounds (200px width x 150px height)', () {
    testWidgets(
      '1.1 SequencePropertiesView inside 200px width x 150px height scrolls smoothly without RenderFlex overflow',
      (WidgetTester tester) async {
        tester.view.physicalSize = const Size(200, 150);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        final capturedErrors = <FlutterErrorDetails>[];
        final previousOnError = FlutterError.onError;
        FlutterError.onError = (details) => capturedErrors.add(details);

        try {
          await tester.pumpWidget(
            buildConstrainedInspectorHarness(
              timelineNotifier: timelineNotifier,
              mediaPoolNotifier: mediaPoolNotifier,
            ),
          );
          await tester.pumpAndSettle();

          // 1. Initial render inside 200x150
          expect(find.byType(SequencePropertiesView), findsOneWidget);
          expect(find.byKey(const Key('sequence_title')), findsOneWidget);
          expect(find.byKey(const Key('seq_duration_pts')), findsOneWidget);

          // 2. Perform vertical scrolling down and up
          final scrollableFinder = find.byType(SingleChildScrollView);
          expect(scrollableFinder, findsOneWidget);

          await tester.drag(scrollableFinder, const Offset(0, -100));
          await tester.pumpAndSettle();

          // Check metadata rows are reached
          expect(find.byKey(const Key('seq_track_count')), findsOneWidget);
          expect(find.byKey(const Key('seq_clip_count')), findsOneWidget);

          // Scroll back up
          await tester.drag(scrollableFinder, const Offset(0, 100));
          await tester.pumpAndSettle();
          expect(find.byKey(const Key('sequence_title')), findsOneWidget);

          // Verify zero RenderFlex overflows occurred
          final renderFlexErrors = capturedErrors.where(
            (e) => e.toString().contains('RenderFlex overflowed'),
          );
          expect(renderFlexErrors, isEmpty, reason: 'SequencePropertiesView overflowed at 200x150');
        } finally {
          FlutterError.onError = previousOnError;
        }
      },
    );

    testWidgets(
      '1.2 VideoPropertiesView (Track 1) inside 200px width x 150px height scrolls smoothly and allows slider manipulation',
      (WidgetTester tester) async {
        tester.view.physicalSize = const Size(200, 150);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        final capturedErrors = <FlutterErrorDetails>[];
        final previousOnError = FlutterError.onError;
        FlutterError.onError = (details) => capturedErrors.add(details);

        try {
          await tester.pumpWidget(
            buildConstrainedInspectorHarness(
              timelineNotifier: timelineNotifier,
              mediaPoolNotifier: mediaPoolNotifier,
            ),
          );
          await tester.pumpAndSettle();

          final element = tester.element(find.byType(InspectorView));
          final container = ProviderScope.containerOf(element);

          // Select Video Clip
          container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, track1VideoId);
          await tester.pumpAndSettle();

          expect(find.byType(VideoPropertiesView), findsOneWidget);
          expect(find.byKey(const Key('video_clip_name')), findsOneWidget);
          expect(find.text('main_camera.mp4'), findsOneWidget);
          expect(find.text('VIDEO'), findsOneWidget);

          // Scroll down to expose all sliders
          final scrollableFinder = find.byType(SingleChildScrollView);
          expect(scrollableFinder, findsOneWidget);

          final scrollableState = tester.state<ScrollableState>(
            find.descendant(
              of: find.byType(VideoPropertiesView),
              matching: find.byType(Scrollable),
            ),
          );
          final maxScroll = scrollableState.position.maxScrollExtent;

          // Drag down to reach Position Y and Scale
          await tester.drag(scrollableFinder, Offset(0, -(maxScroll / 2)));
          await tester.pumpAndSettle();
          expect(find.byKey(const Key('slider_position_y')), findsOneWidget);

          // Drag all the way to the bottom to reach Opacity
          await tester.drag(scrollableFinder, Offset(0, -(maxScroll / 2)));
          await tester.pumpAndSettle();
          expect(find.byKey(const Key('slider_opacity')), findsOneWidget);

          // Tap opacity slider at scrolled position
          await tester.tap(find.byKey(const Key('slider_opacity')));
          await tester.pumpAndSettle();

          // Scroll back up and position Transform section header inside the 150px viewport
          await tester.drag(scrollableFinder, Offset(0, maxScroll));
          await tester.pumpAndSettle();
          await tester.drag(scrollableFinder, const Offset(0, -60));
          await tester.pumpAndSettle();

          // Tap Reset All button
          expect(find.byKey(const Key('reset_all_transform')), findsOneWidget);
          await tester.tap(find.byKey(const Key('reset_all_transform')));
          await tester.pumpAndSettle();

          // Verify zero RenderFlex overflows occurred
          final renderFlexErrors = capturedErrors.where(
            (e) => e.toString().contains('RenderFlex overflowed'),
          );
          expect(renderFlexErrors, isEmpty, reason: 'VideoPropertiesView overflowed at 200x150');
        } finally {
          FlutterError.onError = previousOnError;
        }
      },
    );

    testWidgets(
      '1.3 AudioPropertiesView (Track 2) inside 200px width x 150px height scrolls smoothly and allows mute/slider toggle',
      (WidgetTester tester) async {
        tester.view.physicalSize = const Size(200, 150);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        final capturedErrors = <FlutterErrorDetails>[];
        final previousOnError = FlutterError.onError;
        FlutterError.onError = (details) => capturedErrors.add(details);

        try {
          await tester.pumpWidget(
            buildConstrainedInspectorHarness(
              timelineNotifier: timelineNotifier,
              mediaPoolNotifier: mediaPoolNotifier,
            ),
          );
          await tester.pumpAndSettle();

          final element = tester.element(find.byType(InspectorView));
          final container = ProviderScope.containerOf(element);

          // Select Audio Clip
          container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, track2AudioId);
          await tester.pumpAndSettle();

          expect(find.byType(AudioPropertiesView), findsOneWidget);
          expect(find.byKey(const Key('audio_clip_name')), findsOneWidget);
          expect(find.text('soundtrack.wav'), findsOneWidget);
          expect(find.text('AUDIO'), findsOneWidget);

          // Inspect scrollable state
          final scrollableFinder = find.byType(SingleChildScrollView);
          expect(scrollableFinder, findsOneWidget);

          final scrollableState = tester.state<ScrollableState>(
            find.descendant(
              of: find.byType(AudioPropertiesView),
              matching: find.byType(Scrollable),
            ),
          );
          final maxScroll = scrollableState.position.maxScrollExtent;

          // Drag by exact max scroll extent to bring the bottom into view
          await tester.drag(scrollableFinder, Offset(0, -maxScroll));
          await tester.pumpAndSettle();

          expect(find.byKey(const Key('slider_pan')), findsOneWidget);
          expect(find.byKey(const Key('switch_mute')), findsOneWidget);

          // Toggle Mute switch at scrolled position
          await tester.tap(find.byKey(const Key('switch_mute')));
          await tester.pumpAndSettle();

          expect(find.text('Muted'), findsOneWidget);

          // Untoggle Mute switch
          await tester.tap(find.byKey(const Key('switch_mute')));
          await tester.pumpAndSettle();

          expect(find.text('Track Audio Enabled'), findsOneWidget);

          // Scroll back up to the top
          await tester.drag(scrollableFinder, Offset(0, maxScroll));
          await tester.pumpAndSettle();

          // Verify zero RenderFlex overflows occurred
          final renderFlexErrors = capturedErrors.where(
            (e) => e.toString().contains('RenderFlex overflowed'),
          );
          expect(renderFlexErrors, isEmpty, reason: 'AudioPropertiesView overflowed at 200x150');
        } finally {
          FlutterError.onError = previousOnError;
        }
      },
    );

    testWidgets(
      '1.4 Extreme vertical compression (200px width x 100px height and 200px x 60px height) scrolls cleanly without crash',
      (WidgetTester tester) async {
        tester.view.physicalSize = const Size(200, 100);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        await tester.pumpWidget(
          buildConstrainedInspectorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(tester.takeException(), isNull);

        // Scroll under 100px height
        await tester.drag(find.byType(SingleChildScrollView), const Offset(0, -80));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // Extreme 60px height
        tester.view.physicalSize = const Size(200, 60);
        await tester.pumpAndSettle();
        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  });

  group('Area 2: Multi-Track Switching & Widget Tree Replacement', () {
    testWidgets(
      '2.1 Track 1 (Video) -> Track 2 (Audio) -> Track 3 (Overlay) -> Track 1 (Video) replaces cleanly without key collision',
      (WidgetTester tester) async {
        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        await tester.pumpWidget(
          buildConstrainedInspectorHarness(
            width: 320,
            height: 600,
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // 1. Initial State: SequencePropertiesView
        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(find.byType(VideoPropertiesView), findsNothing);
        expect(find.byType(AudioPropertiesView), findsNothing);

        // 2. Step 1: Select Track 1 (Video)
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, track1VideoId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('main_camera.mp4'), findsOneWidget);
        expect(find.text('VIDEO'), findsOneWidget);
        expect(find.byIcon(Icons.movie_filter_rounded), findsOneWidget);
        expect(tester.takeException(), isNull);

        // 3. Step 2: Select Track 2 (Audio)
        container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, track2AudioId);
        await tester.pumpAndSettle();

        expect(find.byType(AudioPropertiesView), findsOneWidget);
        expect(find.byType(VideoPropertiesView), findsNothing);
        expect(find.text('soundtrack.wav'), findsOneWidget);
        expect(find.text('AUDIO'), findsOneWidget);
        expect(find.byKey(const Key('slider_volume_db')), findsOneWidget);
        expect(tester.takeException(), isNull);

        // 4. Step 3: Select Track 3 (Overlay)
        container.read(timelineSelectionProvider.notifier).selectClip(overlayClipId, track3OverlayId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.byType(AudioPropertiesView), findsNothing);
        expect(find.text('lower_third.mov'), findsOneWidget);
        expect(find.text('OVERLAY'), findsOneWidget);
        expect(find.byIcon(Icons.layers_rounded), findsOneWidget);
        expect(tester.takeException(), isNull);

        // 5. Step 4: Switch directly back to Track 1 (Video)
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, track1VideoId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('main_camera.mp4'), findsOneWidget);
        expect(find.text('VIDEO'), findsOneWidget);
        expect(find.byIcon(Icons.movie_filter_rounded), findsOneWidget);
        expect(tester.takeException(), isNull);

        // 6. Step 5: Switch back to Track 3 (Overlay)
        container.read(timelineSelectionProvider.notifier).selectClip(overlayClipId, track3OverlayId);
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.text('lower_third.mov'), findsOneWidget);
        expect(find.text('OVERLAY'), findsOneWidget);
        expect(tester.takeException(), isNull);

        // 7. Clear selection
        container.read(timelineSelectionProvider.notifier).clearSelection();
        await tester.pumpAndSettle();

        expect(find.byType(SequencePropertiesView), findsOneWidget);
        expect(find.byType(VideoPropertiesView), findsNothing);
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      '2.2 Rapid 60-cycle multi-track switching stress without controller disposal or leak exception',
      (WidgetTester tester) async {
        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        await tester.pumpWidget(
          buildConstrainedInspectorHarness(
            width: 300,
            height: 500,
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);
        final selectionNotifier = container.read(timelineSelectionProvider.notifier);

        for (int cycle = 0; cycle < 30; cycle++) {
          // Track 1
          selectionNotifier.selectClip(videoClipId, track1VideoId);
          await tester.pump();

          // Track 2
          selectionNotifier.selectClip(audioClipId, track2AudioId);
          await tester.pump();

          // Track 3
          selectionNotifier.selectClip(overlayClipId, track3OverlayId);
          await tester.pump();

          // Deselect
          selectionNotifier.clearSelection();
          await tester.pump();
        }

        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byType(SequencePropertiesView), findsOneWidget);
      },
    );

    testWidgets(
      '2.3 Parameter isolation across Track 1 (video), Track 2 (audio), and Track 3 (overlay)',
      (WidgetTester tester) async {
        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        await tester.pumpWidget(
          buildConstrainedInspectorHarness(
            width: 320,
            height: 600,
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);
        final selectionNotifier = container.read(timelineSelectionProvider.notifier);
        final clipPropsNotifier = container.read(clipPropertiesProvider.notifier);

        // 1. Mutate Track 1 (Video) parameters
        selectionNotifier.selectClip(videoClipId, track1VideoId);
        clipPropsNotifier.updateVideoProperties(
          videoClipId,
          positionX: 350.0,
          scale: 1.75,
        );
        await tester.pumpAndSettle();

        expect(find.text('350.0 px'), findsOneWidget);
        expect(find.text('175%'), findsOneWidget);

        // 2. Mutate Track 2 (Audio) parameters
        selectionNotifier.selectClip(audioClipId, track2AudioId);
        clipPropsNotifier.updateAudioProperties(
          audioClipId,
          volumeDb: 4.5,
          pan: -0.75,
        );
        clipPropsNotifier.toggleMute(audioClipId);
        await tester.pumpAndSettle();

        expect(find.text('+4.5 dB'), findsOneWidget);
        expect(find.text('L 75%'), findsOneWidget);
        expect(find.text('Muted'), findsOneWidget);

        // 3. Mutate Track 3 (Overlay) parameters
        selectionNotifier.selectClip(overlayClipId, track3OverlayId);
        clipPropsNotifier.updateVideoProperties(
          overlayClipId,
          positionX: -200.0,
          positionY: 150.0,
          rotation: 45.0,
          opacity: 0.5,
        );
        await tester.pumpAndSettle();

        expect(find.text('-200.0 px'), findsOneWidget);
        expect(find.text('150.0 px'), findsOneWidget);
        expect(find.text('45.0°'), findsOneWidget);
        expect(find.text('50%'), findsOneWidget);

        // 4. Switch back to Track 1 (Video) and verify isolation
        selectionNotifier.selectClip(videoClipId, track1VideoId);
        await tester.pumpAndSettle();

        expect(find.text('350.0 px'), findsOneWidget);
        expect(find.text('175%'), findsOneWidget);
        expect(find.text('0.0°'), findsOneWidget); // Default rotation preserved

        // 5. Reset Track 3 (Overlay) and verify Track 1 is untouched
        selectionNotifier.selectClip(overlayClipId, track3OverlayId);
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const Key('reset_all_transform')));
        await tester.pumpAndSettle();

        expect(find.text('0.0 px'), findsWidgets); // Reset to 0
        expect(find.text('100%'), findsWidgets); // Reset to 1.0

        // Verify Track 1 still has modified values
        selectionNotifier.selectClip(videoClipId, track1VideoId);
        await tester.pumpAndSettle();
        expect(find.text('350.0 px'), findsOneWidget);
        expect(find.text('175%'), findsOneWidget);

        // Verify Track 2 still has modified values
        selectionNotifier.selectClip(audioClipId, track2AudioId);
        await tester.pumpAndSettle();
        expect(find.text('+4.5 dB'), findsOneWidget);
        expect(find.text('L 75%'), findsOneWidget);
        expect(find.text('Muted'), findsOneWidget);
      },
    );
  });

  group('Area 3: Cross-Panel Integration on Full EditorScreen', () {
    testWidgets(
      '3.1 Tapping clips on TimelineView updates InspectorView in real time on the full EditorScreen',
      (WidgetTester tester) async {
        tester.view.physicalSize = const Size(1920, 1080);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        await tester.pumpWidget(
          buildFullEditorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        // 1. Initial State: Inspector shows SequencePropertiesView
        expect(find.byType(EditorScreen), findsOneWidget);
        expect(find.byType(TimelineView), findsOneWidget);
        expect(find.byType(InspectorView), findsOneWidget);

        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(SequencePropertiesView),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(VideoPropertiesView),
          ),
          findsNothing,
        );

        // 2. Tap Video Clip on Track 1 in TimelineView
        final videoClipFinder = find.byKey(Key('timeline_clip_$videoClipId'));
        expect(videoClipFinder, findsOneWidget);
        await tester.tap(videoClipFinder);
        await tester.pumpAndSettle();

        // Inspector updates to VideoPropertiesView
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(VideoPropertiesView),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.text('main_camera.mp4'),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.text('VIDEO'),
          ),
          findsOneWidget,
        );

        // 3. Tap Audio Clip on Track 2 in TimelineView
        final audioClipFinder = find.byKey(Key('timeline_clip_$audioClipId'));
        expect(audioClipFinder, findsOneWidget);
        await tester.tap(audioClipFinder);
        await tester.pumpAndSettle();

        // Inspector updates to AudioPropertiesView
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(AudioPropertiesView),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.text('soundtrack.wav'),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.text('AUDIO'),
          ),
          findsOneWidget,
        );

        // 4. Tap Overlay Clip on Track 3 in TimelineView
        final overlayClipFinder = find.byKey(Key('timeline_clip_$overlayClipId'));
        expect(overlayClipFinder, findsOneWidget);
        await tester.tap(overlayClipFinder);
        await tester.pumpAndSettle();

        // Inspector updates to VideoPropertiesView with OVERLAY badge
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(VideoPropertiesView),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.text('lower_third.mov'),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.text('OVERLAY'),
          ),
          findsOneWidget,
        );

        // 5. Tap empty area in TimelineView to clear selection
        await tester.tap(find.byKey(const Key('timeline_background')));
        await tester.pumpAndSettle();

        // Inspector returns to SequencePropertiesView
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(SequencePropertiesView),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: find.byType(InspectorView),
            matching: find.byType(VideoPropertiesView),
          ),
          findsNothing,
        );
      },
    );

    testWidgets(
      '3.2 Cross-panel split resize: shrinking InspectorView towards minWidth (200px) retains real-time tap integration',
      (WidgetTester tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final timelineNotifier = MutableIntegrationTimelineNotifier(multiTrackTimeline);
        final mediaPoolNotifier = MutableIntegrationMediaPoolNotifier(mediaItems);

        await tester.pumpWidget(
          buildFullEditorHarness(
            timelineNotifier: timelineNotifier,
            mediaPoolNotifier: mediaPoolNotifier,
          ),
        );
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(EditorScreen));
        final container = ProviderScope.containerOf(element);

        // Shrink top split weights to allocate minimum size (~200px) to InspectorView
        // Top panels: [mediaPool, preview, inspector]
        // Distribute weights: 40% media pool, 45% preview, 15% inspector
        container.read(editorLayoutProvider.notifier).updateTopWeights([0.45, 0.40, 0.15]);
        await tester.pumpAndSettle();

        // Tap Video Clip in TimelineView
        await tester.tap(find.byKey(Key('timeline_clip_$videoClipId')));
        await tester.pumpAndSettle();

        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(tester.takeException(), isNull);

        // Tap Audio Clip in TimelineView
        await tester.tap(find.byKey(Key('timeline_clip_$audioClipId')));
        await tester.pumpAndSettle();

        expect(find.byType(AudioPropertiesView), findsOneWidget);
        expect(tester.takeException(), isNull);

        // Reset weights
        container.read(editorLayoutProvider.notifier).resetToPresetDefaults();
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
      },
    );
  });
}
