import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/editor/editor_screen.dart';
import 'package:aether_app/src/features/editor/layout/editor_layout_state.dart';
import 'package:aether_app/src/features/editor/layout/panel_descriptor.dart';
import 'package:aether_app/src/features/editor/layout/panel_registry.dart';
import 'package:aether_app/src/features/editor/layout/widgets/resizable_split_view.dart';
import 'package:aether_app/src/features/editor/layout/widgets/workspace_preset_bar.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';

/// Headless fake notifier that avoids initializing native Rust dynamic libraries in test runs.
class HeadlessTestTimelineNotifier extends TimelineNotifier {
  HeadlessTestTimelineNotifier()
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

Widget createTestApp({
  List<Override> overrides = const [],
}) {
  return ProviderScope(
    overrides: [
      timelineProvider.overrideWith((ref) => HeadlessTestTimelineNotifier()),
      ...overrides,
    ],
    child: const MaterialApp(
      home: EditorScreen(),
    ),
  );
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  group('M1: Modular Editor Layout Suite', () {
    testWidgets('1. Default panel mounting in EditorScreen (Editing Preset)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createTestApp());
      await tester.pumpAndSettle();

      // Verify core top-level structures
      expect(find.byType(EditorScreen), findsOneWidget);
      expect(find.byType(WorkspacePresetBar), findsOneWidget);
      expect(find.byType(ResizableSplitView), findsWidgets);

      // Verify default Editing preset panels
      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);

      // Verify preset bar state
      expect(find.byKey(const Key('preset_button_editing')), findsOneWidget);
      expect(find.byKey(const Key('preset_button_color')), findsOneWidget);
      expect(find.byKey(const Key('preset_button_audio')), findsOneWidget);
      expect(find.byKey(const Key('reset_layout_button')), findsOneWidget);

      // Verify split dividers are mounted
      expect(find.byKey(const Key('split_divider_vertical_0')), findsOneWidget);
      expect(find.byKey(const Key('split_divider_horizontal_0')), findsOneWidget);
      expect(find.byKey(const Key('split_divider_horizontal_1')), findsOneWidget);
    });

    testWidgets('2. Switching between layout presets (Editing -> Color -> Audio)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createTestApp());
      await tester.pumpAndSettle();

      // Initially in Editing: MediaPool is visible
      expect(find.byType(MediaPoolView), findsOneWidget);

      // --- Switch to Color Preset ---
      await tester.tap(find.byKey(const Key('preset_button_color')));
      await tester.pumpAndSettle();

      // In Color mode: MediaPool is omitted from top row, Preview remains
      expect(find.byType(MediaPoolView), findsNothing);
      expect(find.byType(PreviewView), findsOneWidget);
      // Bottom row now mounts Color Grading placeholder and Timeline
      expect(find.byKey(const Key('color_grading_placeholder')), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);

      // --- Switch to Audio Preset ---
      await tester.tap(find.byKey(const Key('preset_button_audio')));
      await tester.pumpAndSettle();

      // In Audio mode: MediaPool returns to top row, Timeline expands
      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);
      expect(find.byKey(const Key('color_grading_placeholder')), findsNothing);

      // --- Switch back to Editing Preset ---
      await tester.tap(find.byKey(const Key('preset_button_editing')));
      await tester.pumpAndSettle();

      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);
    });

    testWidgets('3. Dragging horizontal and vertical splitters + double-tap reset',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createTestApp());
      await tester.pumpAndSettle();

      // 3A: Drag top horizontal divider to expand Media Pool
      final initialMediaPoolWidth = tester.getSize(find.byType(MediaPoolView)).width;
      final dividerFinder = find.byKey(const Key('split_divider_horizontal_0'));
      expect(dividerFinder, findsOneWidget);

      await tester.drag(dividerFinder, const Offset(80.0, 0.0));
      await tester.pumpAndSettle();

      final expandedMediaPoolWidth = tester.getSize(find.byType(MediaPoolView)).width;
      expect(expandedMediaPoolWidth, greaterThan(initialMediaPoolWidth));

      // 3B: Reset via double-tap on divider
      await tester.tap(dividerFinder);
      await tester.pump(const Duration(milliseconds: 50));
      await tester.tap(dividerFinder);
      await tester.pumpAndSettle();

      final resetMediaPoolWidth = tester.getSize(find.byType(MediaPoolView)).width;
      expect(
        (resetMediaPoolWidth - initialMediaPoolWidth).abs(),
        lessThan(2.0),
        reason: 'Double-tapping divider must restore default preset width.',
      );

      // 3C: Drag vertical divider upwards to expand Timeline
      final initialTimelineHeight = tester.getSize(find.byType(TimelineView)).height;
      final verticalDividerFinder = find.byKey(const Key('split_divider_vertical_0'));
      expect(verticalDividerFinder, findsOneWidget);

      await tester.drag(verticalDividerFinder, const Offset(0.0, -100.0));
      await tester.pumpAndSettle();

      final expandedTimelineHeight = tester.getSize(find.byType(TimelineView)).height;
      expect(expandedTimelineHeight, greaterThan(initialTimelineHeight));

      // 3D: Boundary clamping against collapse (extreme drag downward)
      await tester.drag(verticalDividerFinder, const Offset(0.0, 1500.0));
      await tester.pumpAndSettle();

      final clampedTimelineHeight = tester.getSize(find.byType(TimelineView)).height;
      expect(
        clampedTimelineHeight,
        greaterThanOrEqualTo(100.0),
        reason: 'Timeline must clamp to minimum height and not collapse.',
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('4. Dynamic registration of a custom test panel without modifying core layout',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      // Create an isolated registry with standard panels
      final testRegistry = EditorPanelRegistry.standard();

      // Register an entirely new, custom 3rd-party panel descriptor
      const customPanelId = EditorPanelId.custom('waveform_analytics');
      final customDescriptor = EditorPanelDescriptor(
        id: customPanelId,
        title: 'Waveform Analytics',
        icon: Icons.graphic_eq_rounded,
        builder: (context) => const Center(
          key: Key('custom_panel_content'),
          child: Text('Live FFT Spectrum Audio Data'),
        ),
        showHeader: true,
        minWidth: 200.0,
      );
      testRegistry.register(customDescriptor);

      // Create custom layout container mounting the new panel in the top region
      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => HeadlessTestTimelineNotifier()),
          editorPanelRegistryProvider.overrideWith((ref) => testRegistry),
        ],
      );
      addTearDown(container.dispose);

      // Mount custom panel dynamically via layout state
      container.read(editorLayoutProvider.notifier).setPanels(
        topPanels: [
          EditorPanelId.mediaPool,
          EditorPanelId.preview,
          EditorPanelId.inspector,
          customPanelId,
        ],
        topWeights: [0.20, 0.40, 0.20, 0.20],
      );

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: EditorScreen(),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Verify the custom panel is mounted and rendered cleanly
      expect(find.byKey(const Key('custom_panel_content')), findsOneWidget);
      expect(find.text('Live FFT Spectrum Audio Data'), findsOneWidget);
      expect(find.text('WAVEFORM ANALYTICS'), findsOneWidget);

      // Verify that 4 panels now produce 3 horizontal split dividers
      expect(find.byKey(const Key('split_divider_horizontal_0')), findsOneWidget);
      expect(find.byKey(const Key('split_divider_horizontal_1')), findsOneWidget);
      expect(find.byKey(const Key('split_divider_horizontal_2')), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
