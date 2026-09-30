import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/inspector/color/color_grading_provider.dart';
import 'package:aether_app/src/features/inspector/color/color_grading_view.dart';
import 'package:aether_app/src/features/inspector/inspector_mode_provider.dart';
import 'package:aether_app/src/features/inspector/inspector_view.dart';
import 'package:aether_app/src/features/inspector/widgets/audio_properties_view.dart';
import 'package:aether_app/src/features/inspector/widgets/inspector_mode_toggle.dart';
import 'package:aether_app/src/features/inspector/widgets/video_properties_view.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_selection_provider.dart';
import 'package:aether_app/src/theme/catppuccin.dart';

/// Headless timeline notifier for color grading widget tests.
class HeadlessColorTimelineNotifier extends TimelineNotifier {
  HeadlessColorTimelineNotifier(Timeline timeline)
      : super(autoInit: false, createTimelineFn: () async => timeline) {
    state = TimelineState.fromTimeline(timeline);
  }
}

/// Headless media pool notifier for color grading widget tests.
class HeadlessColorMediaPoolNotifier extends MediaPoolNotifier {
  HeadlessColorMediaPoolNotifier(List<MediaItem> items) : super(autoLoad: false) {
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

  Widget buildTestColorApp({
    Timeline? timeline,
    List<MediaItem>? mediaItems,
    InspectorMode initialMode = InspectorMode.properties,
    UuidValue? initialSelectedClipId,
    UuidValue? initialSelectedTrackId,
    List<Override> extraOverrides = const [],
    double width = 400,
    double height = 900,
  }) {
    return ProviderScope(
      overrides: [
        timelineProvider.overrideWith(
          (ref) => HeadlessColorTimelineNotifier(timeline ?? testTimeline),
        ),
        mediaPoolProvider.overrideWith(
          (ref) => HeadlessColorMediaPoolNotifier(mediaItems ?? testMediaItems),
        ),
        inspectorModeProvider.overrideWith((ref) => initialMode),
        ...extraOverrides,
      ],
      child: MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: width,
            height: height,
            child: Column(
              children: [
                // Top Header Chrome with InspectorModeToggle
                Container(
                  height: 32,
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  color: CatppuccinMocha.mantle,
                  child: Row(
                    children: [
                      if (width > 240) ...[
                        const Icon(Icons.tune_rounded, size: 14, color: CatppuccinMocha.overlay0),
                        const SizedBox(width: 8),
                        const Expanded(
                          child: Text(
                            'INSPECTOR',
                            overflow: TextOverflow.ellipsis,
                            maxLines: 1,
                            style: TextStyle(
                              fontSize: 11,
                              fontWeight: FontWeight.bold,
                              color: CatppuccinMocha.overlay0,
                              letterSpacing: 0.5,
                            ),
                          ),
                        ),
                      ] else ...[
                        const Spacer(),
                      ],
                      const InspectorModeToggle(),
                    ],
                  ),
                ),
                // Inspector View Body
                const Expanded(
                  child: InspectorView(),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  group('M3: Integrated Color Grading Panel Suite', () {
    // ---------------------------------------------------------------------------
    // Test 1: Inspector header mode toggle [Propriedades | Cor] transitions
    // ---------------------------------------------------------------------------
    testWidgets(
      '1. Inspector header mode toggle [Propriedades | Cor] transitions between property inspector and color grading views',
      (WidgetTester tester) async {
        await tester.pumpWidget(buildTestColorApp());
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select active video clip
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        await tester.pumpAndSettle();

        // Initially in Properties mode: VideoPropertiesView is visible, ColorGradingView is not
        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.byType(ColorGradingView), findsNothing);
        expect(find.byKey(const Key('inspector_mode_toggle')), findsOneWidget);
        expect(find.byKey(const Key('inspector_mode_properties_btn')), findsOneWidget);
        expect(find.byKey(const Key('inspector_mode_color_btn')), findsOneWidget);

        // Tap [Cor] mode segment
        await tester.tap(find.byKey(const Key('inspector_mode_color_btn')));
        await tester.pumpAndSettle();

        // In Color mode: VideoPropertiesView is replaced by ColorGradingView
        expect(find.byType(VideoPropertiesView), findsNothing);
        expect(find.byType(ColorGradingView), findsOneWidget);
        expect(container.read(inspectorModeProvider), equals(InspectorMode.color));

        // Tap [Propriedades] mode segment
        await tester.tap(find.byKey(const Key('inspector_mode_properties_btn')));
        await tester.pumpAndSettle();

        // Restored to Properties mode
        expect(find.byType(VideoPropertiesView), findsOneWidget);
        expect(find.byType(ColorGradingView), findsNothing);
        expect(container.read(inspectorModeProvider), equals(InspectorMode.properties));
      },
    );

    // ---------------------------------------------------------------------------
    // Test 2: Color grading panel with active video clip renders basic correction sliders, 3-way color wheels, and tone curves
    // ---------------------------------------------------------------------------
    testWidgets(
      '2. Color grading panel with active video clip renders basic correction sliders, 3-way color wheels, and tone curves',
      (WidgetTester tester) async {
        await tester.pumpWidget(buildTestColorApp());
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Select video clip and switch to Color mode
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        await tester.pumpAndSettle();

        // 1. Verify Header & Clip Metadata
        expect(find.byKey(const Key('color_clip_name')), findsOneWidget);
        expect(find.text('landscape.mp4'), findsOneWidget);
        expect(find.text('VIDEO'), findsOneWidget);
        expect(find.textContaining('Range: [0 .. 600 PTS]'), findsOneWidget);

        // 2. Verify Section 1: Basic Color Correction
        expect(find.byKey(const Key('basic_color_correction_view')), findsOneWidget);
        expect(find.byKey(const Key('slider_temperature')), findsOneWidget);
        expect(find.byKey(const Key('badge_temperature')), findsOneWidget);
        expect(find.byKey(const Key('slider_tint')), findsOneWidget);
        expect(find.byKey(const Key('badge_tint')), findsOneWidget);
        expect(find.byKey(const Key('slider_exposure')), findsOneWidget);
        expect(find.byKey(const Key('badge_exposure')), findsOneWidget);
        expect(find.byKey(const Key('slider_contrast')), findsOneWidget);
        expect(find.byKey(const Key('badge_contrast')), findsOneWidget);
        expect(find.byKey(const Key('slider_saturation')), findsOneWidget);
        expect(find.byKey(const Key('badge_saturation')), findsOneWidget);

        // 3. Verify Section 2: 3-Way Color Wheels
        expect(find.byKey(const Key('color_wheels_view')), findsOneWidget);
        expect(find.byKey(const Key('color_wheel_lift')), findsOneWidget);
        expect(find.byKey(const Key('color_wheel_gamma')), findsOneWidget);
        expect(find.byKey(const Key('color_wheel_gain')), findsOneWidget);
        expect(find.byKey(const Key('chromatic_disk_lift')), findsOneWidget);
        expect(find.byKey(const Key('chromatic_disk_gamma')), findsOneWidget);
        expect(find.byKey(const Key('chromatic_disk_gain')), findsOneWidget);
        expect(find.byKey(const Key('slider_luma_lift')), findsOneWidget);
        expect(find.byKey(const Key('slider_luma_gamma')), findsOneWidget);
        expect(find.byKey(const Key('slider_luma_gain')), findsOneWidget);

        // 4. Verify Section 3: Tone Curves
        expect(find.byKey(const Key('color_curves_view')), findsOneWidget);
        expect(find.byKey(const Key('color_curves_widget')), findsOneWidget);
        expect(find.byKey(const Key('curve_channel_rgb')), findsOneWidget);
        expect(find.byKey(const Key('curve_channel_red')), findsOneWidget);
        expect(find.byKey(const Key('curve_channel_green')), findsOneWidget);
        expect(find.byKey(const Key('curve_channel_blue')), findsOneWidget);
        expect(find.byKey(const Key('reset_curve_channel')), findsOneWidget);
        expect(find.byKey(const Key('reset_all_curves')), findsOneWidget);

        // 5. Verify Global Reset Button
        expect(find.byKey(const Key('reset_all_color')), findsOneWidget);
      },
    );

    // ---------------------------------------------------------------------------
    // Test 3: Basic adjustment sliders update state, update monospace badges, and reset to defaults
    // ---------------------------------------------------------------------------
    testWidgets(
      '3. Basic adjustment sliders update state, update monospace badges, and reset to defaults',
      (WidgetTester tester) async {
        await tester.pumpWidget(buildTestColorApp());
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        await tester.pumpAndSettle();

        // Mutate basic adjustments via notifier
        final notifier = container.read(colorGradingProvider.notifier);
        notifier.updateBasicAdjustments(
          videoClipId,
          temperature: 35.0,
          tint: -25.0,
          exposure: 1.5,
          contrast: 40.0,
          saturation: 150.0,
        );
        await tester.pumpAndSettle();

        // Verify updated badge text readouts
        expect(find.text('+35.0'), findsOneWidget);
        expect(find.text('-25.0'), findsOneWidget);
        expect(find.text('+1.50 EV'), findsOneWidget);
        expect(find.text('+40.0'), findsOneWidget);
        expect(find.text('150%'), findsOneWidget);

        // Tap individual reset for Exposure
        await tester.tap(find.byKey(const Key('reset_exposure')));
        await tester.pumpAndSettle();

        // Exposure badge restores to default while Temperature remains modified
        expect(find.text('+0.00 EV'), findsOneWidget);
        expect(find.text('+35.0'), findsOneWidget);

        // Tap Reset All Basics
        await tester.tap(find.byKey(const Key('reset_all_basic_color')));
        await tester.pumpAndSettle();

        // All basic badges restored to neutral defaults
        expect(find.text('+0.0'), findsNWidgets(3)); // Temperature, Tint & Contrast
        expect(find.text('+0.00 EV'), findsOneWidget); // Exposure
        expect(find.text('100%'), findsOneWidget); // Saturation
      },
    );

    // ---------------------------------------------------------------------------
    // Test 4: Color wheels manipulation (dragging crosshair, adjusting master luminance, double-tap reset)
    // ---------------------------------------------------------------------------
    testWidgets(
      '4. Color wheels manipulation (dragging crosshair, adjusting master luminance, double-tap reset)',
      (WidgetTester tester) async {
        await tester.pumpWidget(buildTestColorApp());
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        await tester.pumpAndSettle();

        // Initially Lift wheel is at neutral
        final initialLift = container.read(colorGradingProvider).gradingFor(videoClipId).lift;
        expect(initialLift.isZero, isTrue);

        // 1. Drag Lift chromatic disk crosshair
        await tester.drag(find.byKey(const Key('chromatic_disk_lift')), const Offset(25, -25));
        await tester.pumpAndSettle();

        final draggedLift = container.read(colorGradingProvider).gradingFor(videoClipId).lift;
        expect(draggedLift.radius, greaterThan(0.0));

        // 2. Drag Lift luminance slider
        await tester.drag(find.byKey(const Key('slider_luma_lift')), const Offset(30, 0));
        await tester.pumpAndSettle();

        final lumaLift = container.read(colorGradingProvider).gradingFor(videoClipId).lift;
        expect(lumaLift.luminance, greaterThan(0.0));

        // 3. Double-tap Lift chromatic disk to reset chromatic offset
        await tester.tap(find.byKey(const Key('chromatic_disk_lift')));
        await tester.pump(const Duration(milliseconds: 50));
        await tester.tap(find.byKey(const Key('chromatic_disk_lift')));
        await tester.pumpAndSettle();

        final resetOffsetLift = container.read(colorGradingProvider).gradingFor(videoClipId).lift;
        expect(resetOffsetLift.x, closeTo(0.0, 1e-4));
        expect(resetOffsetLift.y, closeTo(0.0, 1e-4));

        // 4. Tap reset luminance icon button
        await tester.tap(find.byKey(const Key('reset_luma_lift')));
        await tester.pumpAndSettle();

        final fullyResetLift = container.read(colorGradingProvider).gradingFor(videoClipId).lift;
        expect(fullyResetLift.isZero, isTrue);
      },
    );

    // ---------------------------------------------------------------------------
    // Test 5: Tone curves channel switching (RGB -> Red -> Green -> Blue) and point manipulation
    // ---------------------------------------------------------------------------
    testWidgets(
      '5. Tone curves channel switching (RGB -> Red -> Green -> Blue) and point manipulation',
      (WidgetTester tester) async {
        await tester.pumpWidget(buildTestColorApp());
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        await tester.pumpAndSettle();

        // 1. Initial active channel is RGB
        expect(container.read(colorGradingProvider).gradingFor(videoClipId).activeCurveChannel, equals(CurveChannel.rgb));

        // 2. Switch to Red channel
        await tester.tap(find.byKey(const Key('curve_channel_red')));
        await tester.pumpAndSettle();
        expect(container.read(colorGradingProvider).gradingFor(videoClipId).activeCurveChannel, equals(CurveChannel.red));

        // 3. Switch to Green channel
        await tester.tap(find.byKey(const Key('curve_channel_green')));
        await tester.pumpAndSettle();
        expect(container.read(colorGradingProvider).gradingFor(videoClipId).activeCurveChannel, equals(CurveChannel.green));

        // 4. Switch to Blue channel
        await tester.tap(find.byKey(const Key('curve_channel_blue')));
        await tester.pumpAndSettle();
        expect(container.read(colorGradingProvider).gradingFor(videoClipId).activeCurveChannel, equals(CurveChannel.blue));

        // 5. Switch back to RGB channel
        await tester.tap(find.byKey(const Key('curve_channel_rgb')));
        await tester.pumpAndSettle();
        expect(container.read(colorGradingProvider).gradingFor(videoClipId).activeCurveChannel, equals(CurveChannel.rgb));

        // 6. Add a control point by tapping the center of the curves canvas
        final initialPointCount = container.read(colorGradingProvider).gradingFor(videoClipId).rgbCurvePoints.length;
        expect(initialPointCount, equals(2)); // default [(0,0), (1,1)]

        await tester.tap(find.byKey(const Key('color_curves_widget')));
        await tester.pumpAndSettle();

        final updatedPoints = container.read(colorGradingProvider).gradingFor(videoClipId).rgbCurvePoints;
        expect(updatedPoints.length, equals(3));

        // Verify point info readout displays coordinate percentage
        expect(find.byKey(const Key('curve_point_info')), findsOneWidget);

        // 7. Reset active curve channel
        await tester.tap(find.byKey(const Key('reset_curve_channel')));
        await tester.pumpAndSettle();

        final resetPoints = container.read(colorGradingProvider).gradingFor(videoClipId).rgbCurvePoints;
        expect(resetPoints.length, equals(2));
        expect(resetPoints.first, equals(const Offset(0.0, 0.0)));
        expect(resetPoints.last, equals(const Offset(1.0, 1.0)));
      },
    );

    // ---------------------------------------------------------------------------
    // Test 6: Context-sensitivity fallbacks: audio clip displays banner; unselected displays guidance prompt
    // ---------------------------------------------------------------------------
    testWidgets(
      '6. Context-sensitivity fallbacks: audio clip displays informative banner; unselected state displays guidance prompt',
      (WidgetTester tester) async {
        await tester.pumpWidget(buildTestColorApp());
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // Enter Color mode with NO clip selected
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        container.read(timelineSelectionProvider.notifier).clearSelection();
        await tester.pumpAndSettle();

        // 1. Verify Unselected Sequence Guidance Prompt
        expect(find.byKey(const Key('color_empty_sequence_prompt')), findsOneWidget);
        expect(find.byKey(const Key('color_grading_placeholder')), findsOneWidget);
        expect(find.text('Correção de Cores'), findsOneWidget);
        expect(find.textContaining('Nenhum clipe de vídeo selecionado'), findsOneWidget);

        // Sliders, wheels, and curves are NOT rendered
        expect(find.byKey(const Key('basic_color_correction_view')), findsNothing);
        expect(find.byKey(const Key('color_wheels_view')), findsNothing);
        expect(find.byKey(const Key('color_curves_view')), findsNothing);

        // 2. Select an Audio Clip while in Color mode
        container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
        await tester.pumpAndSettle();

        // Verify Informative Audio Banner
        expect(find.byKey(const Key('color_empty_sequence_prompt')), findsNothing);
        expect(find.byKey(const Key('color_audio_clip_banner')), findsOneWidget);
        expect(find.byKey(const Key('color_audio_clip_name')), findsOneWidget);
        expect(find.text('voiceover.wav'), findsOneWidget);
        expect(find.textContaining('Ajustes de correção de cores aplicam-se exclusivamente a faixas visuais'), findsOneWidget);
        expect(find.byKey(const Key('color_switch_to_properties_btn')), findsOneWidget);

        // Sliders, wheels, and curves are still NOT rendered
        expect(find.byKey(const Key('basic_color_correction_view')), findsNothing);
        expect(find.byKey(const Key('color_wheels_view')), findsNothing);
        expect(find.byKey(const Key('color_curves_view')), findsNothing);

        // 3. Tap button to switch to audio properties
        await tester.tap(find.byKey(const Key('color_switch_to_properties_btn')));
        await tester.pumpAndSettle();

        // Successfully navigated to AudioPropertiesView
        expect(find.byType(AudioPropertiesView), findsOneWidget);
        expect(container.read(inspectorModeProvider), equals(InspectorMode.properties));

        // 4. Return to Color mode and select a Video Clip
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        await tester.pumpAndSettle();

        // Audio banner & empty prompt disappear, full controls render
        expect(find.byKey(const Key('color_audio_clip_banner')), findsNothing);
        expect(find.byKey(const Key('color_empty_sequence_prompt')), findsNothing);
        expect(find.byKey(const Key('basic_color_correction_view')), findsOneWidget);
        expect(find.byKey(const Key('color_wheels_view')), findsOneWidget);
        expect(find.byKey(const Key('color_curves_view')), findsOneWidget);
      },
    );

    // ---------------------------------------------------------------------------
    // Test 7: Constrained viewport rendering (200px width) runs smoothly without RenderFlex overflow
    // ---------------------------------------------------------------------------
    testWidgets(
      '7. Constrained viewport rendering (200px width) runs smoothly without RenderFlex overflow',
      (WidgetTester tester) async {
        // Enforce tight 200px horizontal constraint
        tester.view.physicalSize = const Size(200, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        await tester.pumpWidget(buildTestColorApp(width: 200, height: 800, initialMode: InspectorMode.color));
        await tester.pumpAndSettle();

        final element = tester.element(find.byType(InspectorView));
        final container = ProviderScope.containerOf(element);

        // A. Unselected state in 200px
        container.read(inspectorModeProvider.notifier).state = InspectorMode.color;
        container.read(timelineSelectionProvider.notifier).clearSelection();
        await tester.pumpAndSettle();
        final err = tester.takeException();
        if (err is FlutterError) {
          debugPrint('ERR DETAILS: ${err.toStringDeep()}');
        }
        expect(err, isNull);

        // B. Audio state in 200px
        container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // C. Full Video Color Grading state (sliders + 3 wheels + curves canvas) in 200px
        container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
        await tester.pumpAndSettle();
        final videoErr = tester.takeException();
        if (videoErr is FlutterError) {
          debugPrint('VIDEO ERR: ${videoErr.toStringDeep()}');
        }
        expect(videoErr, isNull);

        // D. Interact with curves channel selector in 200px
        await tester.tap(find.byKey(const Key('curve_channel_red')));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        await tester.tap(find.byKey(const Key('curve_channel_green')));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        await tester.tap(find.byKey(const Key('curve_channel_rgb')));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      },
    );
  });
}
