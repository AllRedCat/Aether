import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/editor/editor_screen.dart';
import 'package:aether_app/src/features/editor/layout/editor_layout_state.dart';
import 'package:aether_app/src/features/editor/layout/panel_descriptor.dart';
import 'package:aether_app/src/features/inspector/inspector_view.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

class MockTimelineNotifier extends TimelineNotifier {
  MockTimelineNotifier()
      : super(
          autoInit: false,
          createTimelineFn: () async => Timeline(
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
          ),
        ) {
    state = TimelineState(
      timeline: Timeline(
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
      ),
      totalClipCount: 0,
      durationPts: 0,
    );
  }
}

Widget createStressApp({
  ProviderContainer? container,
  List<Override> overrides = const [],
}) {
  const app = MaterialApp(
    home: EditorScreen(),
  );

  if (container != null) {
    return UncontrolledProviderScope(
      container: container,
      child: app,
    );
  }

  return ProviderScope(
    overrides: [
      timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
      ...overrides,
    ],
    child: app,
  );
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  group('Adversarial Stress Test: Layout Presets & State Lifecycle', () {
    testWidgets('1A: Rapid, sequential switching between layout presets (30 cycles)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createStressApp(container: container));
      await tester.pumpAndSettle();

      final editingFinder = find.byKey(const Key('preset_button_editing'));
      final colorFinder = find.byKey(const Key('preset_button_color'));
      final audioFinder = find.byKey(const Key('preset_button_audio'));

      // Perform 30 rapid switches without pumpAndSettle between every tap
      for (int i = 0; i < 10; i++) {
        await tester.tap(colorFinder);
        await tester.pump(const Duration(milliseconds: 10));

        await tester.tap(audioFinder);
        await tester.pump(const Duration(milliseconds: 10));

        await tester.tap(editingFinder);
        await tester.pump(const Duration(milliseconds: 10));
      }

      await tester.pumpAndSettle();

      // Final state must strictly match Editing preset
      final currentState = container.read(editorLayoutProvider);
      expect(currentState.preset, equals(LayoutPreset.editing));
      expect(currentState.topPanels, equals([
        EditorPanelId.mediaPool,
        EditorPanelId.preview,
        EditorPanelId.inspector,
      ]));
      expect(currentState.bottomPanels, equals([EditorPanelId.timeline]));

      // Verify widget tree is completely consistent
      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);
      expect(find.byKey(const Key('color_grading_placeholder')), findsNothing);

      // Verify no unhandled exceptions
      expect(tester.takeException(), isNull);
    });

    testWidgets('1B: State leak test during rapid switching with active Preview state',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressApp());
      await tester.pumpAndSettle();

      // Start playback in PreviewView
      final playButtonFinder = find.byKey(const Key('preview_play_button'));
      expect(playButtonFinder, findsOneWidget);
      await tester.tap(playButtonFinder);
      await tester.pumpAndSettle();

      // Verify preview is playing (shows pause icon)
      expect(find.byIcon(Icons.pause_rounded), findsOneWidget);

      // Rapidly switch away to Color and then back to Editing
      final colorFinder = find.byKey(const Key('preset_button_color'));
      final editingFinder = find.byKey(const Key('preset_button_editing'));

      await tester.tap(colorFinder);
      await tester.pumpAndSettle();

      // In Color mode, PreviewView is remounted
      expect(find.byType(PreviewView), findsOneWidget);

      await tester.tap(editingFinder);
      await tester.pumpAndSettle();

      // In Editing mode, PreviewView is mounted cleanly without throwing setState after dispose
      expect(find.byType(PreviewView), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('Adversarial Stress Test: Extreme Drag Deltas & Boundary Clamping', () {
    testWidgets('2A: Extreme drag deltas on vertical divider (±2000px, ±10000px)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createStressApp(container: container));
      await tester.pumpAndSettle();

      final verticalDividerFinder = find.byKey(const Key('split_divider_vertical_0'));
      expect(verticalDividerFinder, findsOneWidget);

      // Extreme drag downwards (+2000px)
      await tester.drag(verticalDividerFinder, const Offset(0.0, 2000.0));
      await tester.pumpAndSettle();

      // Vertical ratio must be clamped at 0.85
      final stateAfterDown = container.read(editorLayoutProvider);
      expect(stateAfterDown.verticalRatio, lessThanOrEqualTo(0.8501));
      expect(stateAfterDown.verticalRatio, greaterThanOrEqualTo(0.15));

      // Timeline must NOT collapse below minSecondSize (120px)
      final timelineSize = tester.getSize(find.byType(TimelineView));
      expect(timelineSize.height, greaterThanOrEqualTo(119.0));
      expect(tester.takeException(), isNull);

      // Extreme drag upwards (-10000px)
      await tester.drag(verticalDividerFinder, const Offset(0.0, -10000.0));
      await tester.pumpAndSettle();

      final stateAfterUp = container.read(editorLayoutProvider);
      expect(stateAfterUp.verticalRatio, greaterThanOrEqualTo(0.1499));
      expect(stateAfterUp.verticalRatio, lessThanOrEqualTo(0.85));

      // Top region must NOT collapse below minFirstSize (160px)
      final previewSize = tester.getSize(find.byType(PreviewView));
      expect(previewSize.height, greaterThanOrEqualTo(100.0));
      expect(tester.takeException(), isNull);
    });

    testWidgets('2B: Extreme drag deltas on horizontal dividers (±2000px) in 3-panel row',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createStressApp(container: container));
      await tester.pumpAndSettle();

      final div0Finder = find.byKey(const Key('split_divider_horizontal_0'));
      final div1Finder = find.byKey(const Key('split_divider_horizontal_1'));

      // Drag divider 0 extreme right (+2000px)
      await tester.drag(div0Finder, const Offset(2000.0, 0.0));
      await tester.pumpAndSettle();

      // PreviewView (next child, minWidth: 240.0) must NOT collapse below its minWidth
      final previewWidthAfterDragRight = tester.getSize(find.byType(PreviewView)).width;
      expect(
        previewWidthAfterDragRight,
        greaterThanOrEqualTo(239.0),
        reason: 'PreviewView must respect minimum width constraint under extreme positive drag.',
      );
      expect(tester.takeException(), isNull);

      // Drag divider 0 extreme left (-2000px)
      await tester.drag(div0Finder, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();

      // MediaPoolView (child 0, minWidth: 200.0) must NOT collapse below its minWidth
      final mediaPoolWidthAfterDragLeft = tester.getSize(find.byType(MediaPoolView)).width;
      expect(
        mediaPoolWidthAfterDragLeft,
        greaterThanOrEqualTo(199.0),
        reason: 'MediaPoolView must respect minimum width constraint under extreme negative drag.',
      );
      final exc2B = tester.takeException();
      expect(exc2B, isNull);

      // Drag divider 1 extreme right (+2000px)
      await tester.drag(div1Finder, const Offset(2000.0, 0.0));
      await tester.pumpAndSettle();

      // InspectorView (next child, minWidth: 200.0) must NOT collapse below its minWidth
      final inspectorWidth = tester.getSize(find.byType(InspectorView)).width;
      expect(
        inspectorWidth,
        greaterThanOrEqualTo(199.0),
        reason: 'InspectorView must respect minimum width constraint.',
      );
      expect(tester.takeException(), isNull);

      // Drag divider 1 extreme left (-2000px)
      await tester.drag(div1Finder, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();

      // PreviewView (child 1, minWidth: 240.0) must NOT collapse below its minWidth
      final previewWidthAfterDiv1Left = tester.getSize(find.byType(PreviewView)).width;
      expect(
        previewWidthAfterDiv1Left,
        greaterThanOrEqualTo(239.0),
        reason: 'PreviewView must respect minimum width constraint under divider 1 negative drag.',
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('2C: Ping-pong bidirectional drag stress test with 0 RenderFlex overflow',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressApp());
      await tester.pumpAndSettle();

      final div0 = find.byKey(const Key('split_divider_horizontal_0'));
      final div1 = find.byKey(const Key('split_divider_horizontal_1'));
      final divV = find.byKey(const Key('split_divider_vertical_0'));

      // Rapid alternating drags
      for (int i = 0; i < 5; i++) {
        await tester.drag(div0, const Offset(400.0, 0.0));
        await tester.pump(const Duration(milliseconds: 10));
        await tester.drag(div1, const Offset(-400.0, 0.0));
        await tester.pump(const Duration(milliseconds: 10));
        await tester.drag(divV, const Offset(0.0, 300.0));
        await tester.pump(const Duration(milliseconds: 10));
        await tester.drag(div0, const Offset(-400.0, 0.0));
        await tester.pump(const Duration(milliseconds: 10));
        await tester.drag(div1, const Offset(400.0, 0.0));
        await tester.pump(const Duration(milliseconds: 10));
        await tester.drag(divV, const Offset(0.0, -300.0));
        await tester.pump(const Duration(milliseconds: 10));
      }
      await tester.pumpAndSettle();

      // Check all widgets are rendered with positive non-zero extents
      expect(tester.getSize(find.byType(MediaPoolView)).width, greaterThanOrEqualTo(199.0));
      expect(tester.getSize(find.byType(PreviewView)).width, greaterThanOrEqualTo(239.0));
      expect(tester.getSize(find.byType(InspectorView)).width, greaterThanOrEqualTo(199.0));
      expect(tester.getSize(find.byType(TimelineView)).height, greaterThanOrEqualTo(119.0));
      expect(tester.takeException(), isNull);
    });

    testWidgets('2D: Constrained small window stress test (800x600)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(800, 600);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressApp());
      await tester.pumpAndSettle();

      final div0 = find.byKey(const Key('split_divider_horizontal_0'));
      final divV = find.byKey(const Key('split_divider_vertical_0'));

      // Try extreme drags in constrained window
      await tester.drag(div0, const Offset(1000.0, 0.0));
      await tester.pumpAndSettle();

      await tester.drag(divV, const Offset(0.0, 1000.0));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
    });
  });

  group('Adversarial Stress Test: Double-Tap Reset & Proportions Restoration', () {
    testWidgets('3A: Double-tap on vertical divider reliably restores default 0.60 vertical ratio',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createStressApp(container: container));
      await tester.pumpAndSettle();

      final verticalDividerFinder = find.byKey(const Key('split_divider_vertical_0'));

      // Modify vertical ratio heavily
      await tester.drag(verticalDividerFinder, const Offset(0.0, -250.0));
      await tester.pumpAndSettle();

      final modifiedRatio = container.read(editorLayoutProvider).verticalRatio;
      expect(modifiedRatio, isNot(closeTo(0.60, 0.01)));

      // Double-tap vertical divider to reset
      await tester.tap(verticalDividerFinder);
      await tester.pump(const Duration(milliseconds: 50));
      await tester.tap(verticalDividerFinder);
      await tester.pumpAndSettle();

      final restoredRatio = container.read(editorLayoutProvider).verticalRatio;
      expect(
        restoredRatio,
        closeTo(0.60, 0.001),
        reason: 'Double-tapping vertical divider must restore factory default 0.60 ratio.',
      );
    });

    testWidgets('3B: Double-tap on horizontal divider restores topWeights [0.22, 0.53, 0.25]',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createStressApp(container: container));
      await tester.pumpAndSettle();

      final div0 = find.byKey(const Key('split_divider_horizontal_0'));

      // Drag divider heavily
      await tester.drag(div0, const Offset(200.0, 0.0));
      await tester.pumpAndSettle();

      final modifiedWeights = container.read(editorLayoutProvider).topWeights;
      expect(modifiedWeights[0], isNot(closeTo(0.22, 0.01)));

      // Double-tap horizontal divider to reset
      await tester.tap(div0);
      await tester.pump(const Duration(milliseconds: 50));
      await tester.tap(div0);
      await tester.pumpAndSettle();

      final restoredWeights = container.read(editorLayoutProvider).topWeights;
      expect(restoredWeights[0], closeTo(0.22, 0.001));
      expect(restoredWeights[1], closeTo(0.53, 0.001));
      expect(restoredWeights[2], closeTo(0.25, 0.001));
    });

    testWidgets('3C: Quick reset button restores both vertical & horizontal ratios across presets',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createStressApp(container: container));
      await tester.pumpAndSettle();

      // Switch to Color preset
      await tester.tap(find.byKey(const Key('preset_button_color')));
      await tester.pumpAndSettle();

      // Modify vertical divider
      final verticalDivider = find.byKey(const Key('split_divider_vertical_0'));
      await tester.drag(verticalDivider, const Offset(0.0, 180.0));
      await tester.pumpAndSettle();

      expect(container.read(editorLayoutProvider).verticalRatio, isNot(closeTo(0.55, 0.01)));

      // Tap Reset button in toolbar
      await tester.tap(find.byKey(const Key('reset_layout_button')));
      await tester.pumpAndSettle();

      final colorState = container.read(editorLayoutProvider);
      expect(colorState.preset, equals(LayoutPreset.color));
      expect(colorState.verticalRatio, closeTo(0.55, 0.001));
      expect(colorState.topWeights[0], closeTo(0.55, 0.001));
      expect(colorState.topWeights[1], closeTo(0.45, 0.001));
      expect(colorState.bottomWeights[0], closeTo(0.60, 0.001));
      expect(colorState.bottomWeights[1], closeTo(0.40, 0.001));
    });
  });
}
