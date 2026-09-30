import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/editor/editor_screen.dart';
import 'package:aether_app/src/features/editor/layout/widgets/aether_split_divider.dart';
import 'package:aether_app/src/features/editor/layout/widgets/resizable_split_view.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/features/inspector/inspector_view.dart';
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

Widget createStressTestApp({
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

  group('M1 Empirical Stress & Adversarial Suite', () {
    // -------------------------------------------------------------------------
    // TEST 1: Rapid Preset Switching Lifecycle
    // -------------------------------------------------------------------------
    testWidgets('1. Rapid sequential preset switching: 30 cycles (editing -> color -> audio -> editing)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressTestApp());
      await tester.pumpAndSettle();

      final editingBtn = find.byKey(const Key('preset_button_editing'));
      final colorBtn = find.byKey(const Key('preset_button_color'));
      final audioBtn = find.byKey(const Key('preset_button_audio'));

      // Perform 30 rapid sequential cycles
      for (int cycle = 0; cycle < 30; cycle++) {
        await tester.tap(colorBtn);
        await tester.pump();

        await tester.tap(audioBtn);
        await tester.pump();

        await tester.tap(editingBtn);
        await tester.pump();
      }

      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.byType(InspectorView), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);
      expect(find.byKey(const Key('color_grading_placeholder')), findsNothing);
    });

    // -------------------------------------------------------------------------
    // TEST 2: Double-tap Reset Under Modified Proportions
    // -------------------------------------------------------------------------
    testWidgets('2. Double-tap reset restores default preset proportions across all presets',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressTestApp());
      await tester.pumpAndSettle();

      // === EDITING PRESET ===
      final defaultMediaPoolWidth = tester.getSize(find.byType(MediaPoolView)).width;
      final defaultTimelineHeight = tester.getSize(find.byType(TimelineView)).height;

      final divider0 = find.byKey(const Key('split_divider_horizontal_0'));
      await tester.drag(divider0, const Offset(150.0, 0.0));
      final vDivider = find.byKey(const Key('split_divider_vertical_0'));
      await tester.drag(vDivider, const Offset(0.0, -100.0));
      await tester.pumpAndSettle();

      // Double-tap horizontal divider 0 to reset
      await tester.tap(divider0);
      await tester.pump(const Duration(milliseconds: 40));
      await tester.tap(divider0);
      await tester.pumpAndSettle();

      final restoredMediaPoolWidth = tester.getSize(find.byType(MediaPoolView)).width;
      final restoredTimelineHeight = tester.getSize(find.byType(TimelineView)).height;
      expect((restoredMediaPoolWidth - defaultMediaPoolWidth).abs(), lessThan(2.0));
      expect((restoredTimelineHeight - defaultTimelineHeight).abs(), lessThan(2.0));

      // === COLOR PRESET (disambiguating descendant divider) ===
      await tester.tap(find.byKey(const Key('preset_button_color')));
      await tester.pumpAndSettle();

      final defaultColorPreviewWidth = tester.getSize(find.byType(PreviewView)).width;

      final colorTopDivider = find.descendant(
        of: find.byType(ResizableSplitView).at(1),
        matching: find.byType(AetherSplitDivider),
      );
      await tester.drag(colorTopDivider, const Offset(-120.0, 0.0));
      await tester.pumpAndSettle();

      await tester.tap(colorTopDivider);
      await tester.pump(const Duration(milliseconds: 40));
      await tester.tap(colorTopDivider);
      await tester.pumpAndSettle();

      final resetColorPreviewWidth = tester.getSize(find.byType(PreviewView)).width;
      expect((resetColorPreviewWidth - defaultColorPreviewWidth).abs(), lessThan(2.0));

      // === AUDIO PRESET + RESET BUTTON IN PRESET BAR ===
      await tester.tap(find.byKey(const Key('preset_button_audio')));
      await tester.pumpAndSettle();

      final defaultAudioTimelineHeight = tester.getSize(find.byType(TimelineView)).height;

      final audioVDivider = find.byKey(const Key('split_divider_vertical_0'));
      await tester.drag(audioVDivider, const Offset(0.0, 150.0));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('reset_layout_button')));
      await tester.pumpAndSettle();

      final resetAudioTimelineHeight = tester.getSize(find.byType(TimelineView)).height;
      expect((resetAudioTimelineHeight - defaultAudioTimelineHeight).abs(), lessThan(2.0));
    });

    // -------------------------------------------------------------------------
    // TEST 3: Remediation Verification - Unique Divider Keys in Color Preset
    // -------------------------------------------------------------------------
    testWidgets('3. RESOLVED: Divider keys in Color preset are uniquely namespaced',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressTestApp());
      await tester.pumpAndSettle();

      // Switch to Color preset
      await tester.tap(find.byKey(const Key('preset_button_color')));
      await tester.pumpAndSettle();

      // Verify no ambiguous or duplicate split_divider_horizontal_0 key
      expect(find.byKey(const Key('split_divider_horizontal_0')), findsNothing);

      // Verify namespaced keys are unique and single
      final topDividerFinder = find.byKey(const Key('top_split_divider_horizontal_0'));
      final bottomDividerFinder = find.byKey(const Key('bottom_split_divider_horizontal_0'));
      expect(topDividerFinder, findsOneWidget);
      expect(bottomDividerFinder, findsOneWidget);

      // Both dividers can be dragged cleanly without ambiguity or exceptions
      await tester.drag(topDividerFinder, const Offset(50.0, 0.0));
      await tester.pumpAndSettle();
      await tester.drag(bottomDividerFinder, const Offset(-50.0, 0.0));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
    });

    // -------------------------------------------------------------------------
    // TEST 4: Remediation Verification - Extreme Horizontal Drag Clamping & Responsiveness
    // -------------------------------------------------------------------------
    testWidgets('4. RESOLVED: Extreme negative horizontal drag (-2000px) does not overflow',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressTestApp());
      await tester.pumpAndSettle();

      final divider0 = find.byKey(const Key('split_divider_horizontal_0'));

      // Drag extreme left to shrink MediaPool towards its declared minWidth (300px)
      await tester.drag(divider0, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      final mediaPoolWidth = tester.getSize(find.byType(MediaPoolView)).width;
      expect(mediaPoolWidth, greaterThanOrEqualTo(299.0));
    });

    // -------------------------------------------------------------------------
    // TEST 5: Remediation Verification - Extreme Vertical Drag Clamping & Scrollability
    // -------------------------------------------------------------------------
    testWidgets('5. RESOLVED: Extreme upward vertical drag (-2000px) does not overflow',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createStressTestApp());
      await tester.pumpAndSettle();

      final vDivider = find.byKey(const Key('split_divider_vertical_0'));

      // Drag extreme upward to shrink top workspace towards minFirstSize (240px)
      await tester.drag(vDivider, const Offset(0.0, -2000.0));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
    });
  });
}
