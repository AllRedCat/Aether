import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/editor/editor_screen.dart';
import 'package:aether_app/src/features/editor/layout/editor_layout_state.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';

class MockHeadlessTimelineNotifier extends TimelineNotifier {
  MockHeadlessTimelineNotifier()
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

Widget createDeepAdversarialApp({
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
      timelineProvider.overrideWith((ref) => MockHeadlessTimelineNotifier()),
      ...overrides,
    ],
    child: app,
  );
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  group('Challenger R3 Deep Adversarial Verification Suite', () {
    testWidgets('1. Bug 1 Re-test & Color Preset: Extreme horizontal & vertical drags with namespaced dividers',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockHeadlessTimelineNotifier()),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(createDeepAdversarialApp(container: container));
      await tester.pumpAndSettle();

      // Switch to Color preset
      await tester.tap(find.byKey(const Key('preset_button_color')));
      await tester.pumpAndSettle();

      // Ensure no colliding key exists
      expect(find.byKey(const Key('split_divider_horizontal_0')), findsNothing);

      final topDividerFinder = find.byKey(const Key('top_split_divider_horizontal_0'));
      final bottomDividerFinder = find.byKey(const Key('bottom_split_divider_horizontal_0'));
      final verticalDividerFinder = find.byKey(const Key('split_divider_vertical_0'));

      expect(topDividerFinder, findsOneWidget);
      expect(bottomDividerFinder, findsOneWidget);
      expect(verticalDividerFinder, findsOneWidget);

      // Extreme drag top divider left (-2000px) and right (+2000px)
      await tester.drag(topDividerFinder, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.drag(topDividerFinder, const Offset(2000.0, 0.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      // Extreme drag bottom divider left (-2000px) and right (+2000px)
      await tester.drag(bottomDividerFinder, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.drag(bottomDividerFinder, const Offset(2000.0, 0.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      // Extreme drag vertical divider up (-2000px) and down (+2000px) in Color preset
      await tester.drag(verticalDividerFinder, const Offset(0.0, -2000.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.drag(verticalDividerFinder, const Offset(0.0, 2000.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      // Double tap reset on namespaced dividers in Color mode
      await tester.tap(topDividerFinder);
      await tester.pump(const Duration(milliseconds: 50));
      await tester.tap(topDividerFinder);
      await tester.pumpAndSettle();

      final colorState = container.read(editorLayoutProvider);
      expect(colorState.topWeights[0], closeTo(0.55, 0.001));
      expect(colorState.topWeights[1], closeTo(0.45, 0.001));
    });

    testWidgets('2. Bug 2 Re-test: Extreme negative horizontal drag (-2000px) on MediaPoolView in Editing preset',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createDeepAdversarialApp());
      await tester.pumpAndSettle();

      final divider0 = find.byKey(const Key('split_divider_horizontal_0'));
      expect(divider0, findsOneWidget);

      // Extreme negative drag (-2000px)
      await tester.drag(divider0, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);

      // MediaPoolView must not crash and should remain visible with min width
      final mediaPoolSize = tester.getSize(find.byType(MediaPoolView));
      expect(mediaPoolSize.width, greaterThanOrEqualTo(299.0));

      // Test key import button is still hittable
      expect(find.byKey(const Key('import_media_button')), findsOneWidget);
    });

    testWidgets('3. Bug 3 Re-test: Extreme upward vertical drag (-2000px) on MediaPoolView & PreviewView',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createDeepAdversarialApp());
      await tester.pumpAndSettle();

      final vDivider = find.byKey(const Key('split_divider_vertical_0'));
      expect(vDivider, findsOneWidget);

      // Extreme drag upward (-2000px)
      await tester.drag(vDivider, const Offset(0.0, -2000.0));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);

      // Both MediaPoolView and PreviewView must still be rendered without overflow
      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);

      final mediaPoolHeight = tester.getSize(find.byType(MediaPoolView)).height;
      final previewHeight = tester.getSize(find.byType(PreviewView)).height;
      expect(mediaPoolHeight, greaterThan(0));
      expect(previewHeight, greaterThan(0));
    });

    testWidgets('4. Audio Preset: Extreme horizontal and vertical drag deltas',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createDeepAdversarialApp());
      await tester.pumpAndSettle();

      // Switch to Audio preset
      await tester.tap(find.byKey(const Key('preset_button_audio')));
      await tester.pumpAndSettle();

      final vDivider = find.byKey(const Key('split_divider_vertical_0'));
      final hDivider0 = find.byKey(const Key('split_divider_horizontal_0'));
      final hDivider1 = find.byKey(const Key('split_divider_horizontal_1'));

      // In Audio preset, vertical ratio defaults to 0.35
      // Extreme drags
      await tester.drag(vDivider, const Offset(0.0, -2000.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.drag(vDivider, const Offset(0.0, 2000.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.drag(hDivider0, const Offset(-2000.0, 0.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.drag(hDivider1, const Offset(2000.0, 0.0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    });

    testWidgets('5. Degradation under contract lower-bound viewport (400x300)',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(400, 300);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(createDeepAdversarialApp());
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(EditorScreen), findsOneWidget);
    });
  });
}
