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
import 'package:aether_app/src/features/inspector/inspector_view.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

class MockTestFilePickerService implements FilePickerService {
  final List<String> files;
  MockTestFilePickerService({this.files = const ['/test/clip1.mp4']});

  @override
  Future<List<String>> pickMediaFiles() async => files;
}

class MockTimelineNotifierForLayout extends TimelineNotifier {
  MockTimelineNotifierForLayout()
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
    final initialTimeline = Timeline(
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
    state = TimelineState(
      timeline: initialTimeline,
      totalClipCount: 0,
      durationPts: 0,
      tracks: initialTimeline.tracks,
    );
  }

  @override
  Future<void> addClip({
    UuidValue? trackId,
    UuidValue? sourceId,
    int sourceIn = 0,
    int sourceOut = 60,
    int? timelineIn,
  }) async {
    final currentCount = state.totalClipCount + 1;
    final inPts = timelineIn ?? state.durationPts;
    final outPts = inPts + 180;
    final track = state.tracks.first;
    final newClip = Clip(
      id: UuidValue.fromString('c0000000-0000-0000-0000-${currentCount.toString().padLeft(12, '0')}'),
      sourceId: sourceId ?? UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
      timelineIn: inPts,
      timelineOut: outPts,
      sourceIn: sourceIn,
      sourceOut: sourceOut,
    );

    final updatedTrack = Track(
      id: track.id,
      kind: track.kind,
      clips: [...track.clips, newClip],
    );

    state = state.copyWith(
      timeline: Timeline(
        id: state.timeline!.id,
        timebase: state.timeline!.timebase,
        durationPts: outPts,
        tracks: [updatedTrack],
      ),
      totalClipCount: currentCount,
      durationPts: outPts,
      tracks: [updatedTrack],
    );
  }
}

MediaPoolNotifier createMockMediaPoolNotifier({void Function(MediaItem)? onAddToTimeline}) {
  return MediaPoolNotifier(
    filePickerService: MockTestFilePickerService(),
    onAddToTimeline: onAddToTimeline,
    importMediaFn: ({projectPath, required filePath}) async {
      return MediaItem(
        id: UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
        filePath: filePath,
        fileName: 'adversarial_test_clip.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationSeconds: 3.0,
          durationPts: 180,
          width: 1920,
          height: 1080,
          fileSizeBytes: 1048576,
        ),
      );
    },
  );
}

Widget buildTestableEditorScreen({
  ProviderContainer? container,
  List<Override> overrides = const [],
}) {
  const content = MaterialApp(
    home: EditorScreen(),
  );

  if (container != null) {
    return UncontrolledProviderScope(
      container: container,
      child: content,
    );
  }

  return ProviderScope(
    overrides: [
      timelineProvider.overrideWith((ref) => MockTimelineNotifierForLayout()),
      mediaPoolProvider.overrideWith((ref) => createMockMediaPoolNotifier()),
      ...overrides,
    ],
    child: content,
  );
}

void main() {
  setUp(() {
    TestWidgetsFlutterBinding.ensureInitialized();
  });

  group('Adversarial Challenge 1: Dynamic Panel Registration & Scalability', () {
    testWidgets('1A: 5+ Panels in top row compute valid flex factors without exceptions',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final registry = EditorPanelRegistry.standard();

      // Register 3 custom panels to reach 6 total in top row
      final customIds = <EditorPanelId>[];
      for (int i = 1; i <= 3; i++) {
        final panelId = EditorPanelId.custom('custom_panel_$i');
        customIds.add(panelId);
        registry.register(
          EditorPanelDescriptor(
            id: panelId,
            title: 'Custom Panel $i',
            icon: Icons.extension_rounded,
            builder: (context) => Center(
              key: Key('custom_panel_body_$i'),
              child: Text('Content $i'),
            ),
            showHeader: true,
            minWidth: 100.0,
          ),
        );
      }

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifierForLayout()),
          mediaPoolProvider.overrideWith((ref) => createMockMediaPoolNotifier()),
          editorPanelRegistryProvider.overrideWith((ref) => registry),
        ],
      );
      addTearDown(container.dispose);

      final topPanels = [
        EditorPanelId.mediaPool,
        EditorPanelId.preview,
        EditorPanelId.inspector,
        ...customIds,
      ];
      final topWeights = [0.15, 0.35, 0.15, 0.10, 0.15, 0.10];

      container.read(editorLayoutProvider.notifier).setPanels(
        topPanels: topPanels,
        topWeights: topWeights,
      );

      await tester.pumpWidget(buildTestableEditorScreen(container: container));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);

      // Verify all 6 panels exist and have positive widths
      expect(find.byType(MediaPoolView), findsOneWidget);
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.byType(InspectorView), findsOneWidget);
      for (int i = 1; i <= 3; i++) {
        expect(find.byKey(Key('custom_panel_body_$i')), findsOneWidget);
        final size = tester.getSize(find.byKey(Key('panel_container_custom_panel_$i')));
        expect(size.width, greaterThan(0.0));
        expect(size.height, greaterThan(0.0));
      }

      // Verify 5 dividers are rendered between 6 panels
      for (int i = 0; i < 5; i++) {
        expect(find.byKey(Key('split_divider_horizontal_$i')), findsOneWidget);
      }
    });

    testWidgets('1B: Multi-divider interactive dragging with 6 panels maintains valid layout',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final registry = EditorPanelRegistry.standard();
      final customIds = <EditorPanelId>[];
      for (int i = 1; i <= 3; i++) {
        final panelId = EditorPanelId.custom('drag_test_panel_$i');
        customIds.add(panelId);
        registry.register(
          EditorPanelDescriptor(
            id: panelId,
            title: 'Drag Panel $i',
            icon: Icons.drag_handle_rounded,
            builder: (context) => Center(child: Text('Drag Body $i')),
            minWidth: 80.0,
          ),
        );
      }

      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => MockTimelineNotifierForLayout()),
          mediaPoolProvider.overrideWith((ref) => createMockMediaPoolNotifier()),
          editorPanelRegistryProvider.overrideWith((ref) => registry),
        ],
      );
      addTearDown(container.dispose);

      container.read(editorLayoutProvider.notifier).setPanels(
        topPanels: [
          EditorPanelId.mediaPool,
          EditorPanelId.preview,
          EditorPanelId.inspector,
          ...customIds,
        ],
        topWeights: [0.20, 0.30, 0.15, 0.15, 0.10, 0.10],
      );

      await tester.pumpWidget(buildTestableEditorScreen(container: container));
      await tester.pumpAndSettle();

      // Drag divider 0 (between mediaPool and preview)
      final div0 = find.byKey(const Key('split_divider_horizontal_0'));
      await tester.drag(div0, const Offset(50, 0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      // Drag divider 3 (between custom panel 1 and 2)
      final div3 = find.byKey(const Key('split_divider_horizontal_3'));
      await tester.drag(div3, const Offset(-30, 0));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      // Verify layout state weights are all strictly positive and finite
      final currentWeights = container.read(editorLayoutProvider).topWeights;
      for (final w in currentWeights) {
        expect(w, greaterThan(0.0));
        expect(w.isFinite, isTrue);
      }
    });

    testWidgets('1C: Mathematical edge case: zero weights and extreme imbalance',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      // Direct test on ResizableSplitView.multi with extreme weights
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ResizableSplitView.multi(
              axis: Axis.horizontal,
              weights: const [0.0, 0.0, 0.0, 0.0, 0.0],
              children: List.generate(
                5,
                (i) => Container(key: Key('zero_child_$i'), color: Colors.blue),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      for (int i = 0; i < 5; i++) {
        expect(find.byKey(Key('zero_child_$i')), findsOneWidget);
        final size = tester.getSize(find.byKey(Key('zero_child_$i')));
        expect(size.width, greaterThan(0));
      }
    });
  });

  group('Adversarial Challenge 2: Extreme Small Screen Constraints & Degradation', () {
    testWidgets('2A: Pump EditorScreen at 600x400 physical size',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(600, 400);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(buildTestableEditorScreen());
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(EditorScreen), findsOneWidget);
      expect(find.byType(WorkspacePresetBar), findsOneWidget);
      expect(find.byType(TimelineView), findsOneWidget);
    });

    testWidgets('2B: Pump EditorScreen at 400x300 physical size',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(400, 300);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(buildTestableEditorScreen());
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(EditorScreen), findsOneWidget);
      expect(find.byType(WorkspacePresetBar), findsOneWidget);
    });
  });

  group('Adversarial Challenge 3: Preservation of MediaPoolView and TimelineView Keys & Interactions', () {
    testWidgets('3A: MediaPoolView test keys and import action intact in modular layout',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(buildTestableEditorScreen());
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('media_pool_view')), findsOneWidget);
      expect(find.byKey(const Key('import_media_button')), findsOneWidget);
      expect(find.byKey(const Key('media_pool_empty_state')), findsOneWidget);
      expect(find.byKey(const Key('empty_import_media_button')), findsOneWidget);

      // Trigger import via empty state button
      await tester.tap(find.byKey(const Key('empty_import_media_button')));
      await tester.pumpAndSettle();

      // Verify media list appears with item key
      expect(find.byKey(const Key('media_pool_list')), findsOneWidget);
      expect(
        find.byKey(const Key('media_item_d0000000-0000-0000-0000-000000000001')),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('add_to_timeline_d0000000-0000-0000-0000-000000000001')),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('3B: TimelineView test keys and add clip interaction intact in modular layout',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(buildTestableEditorScreen());
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('timeline_total_clips_count')), findsOneWidget);
      expect(find.byKey(const Key('timeline_duration_pts')), findsOneWidget);
      expect(find.byKey(const Key('add_clip_button')), findsOneWidget);

      expect(find.text('0'), findsOneWidget);
      expect(find.text('Duration: 0 PTS'), findsOneWidget);

      // Tap add clip button
      await tester.tap(find.byKey(const Key('add_clip_button')));
      await tester.pumpAndSettle();

      expect(find.text('1'), findsOneWidget);
      expect(find.text('Duration: 180 PTS'), findsOneWidget);
      expect(find.text('Clip [0..180 PTS]'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('3C: Cross-panel interaction: Add media from MediaPool to Timeline',
        (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final timelineNotifier = MockTimelineNotifierForLayout();
      final container = ProviderContainer(
        overrides: [
          timelineProvider.overrideWith((ref) => timelineNotifier),
          mediaPoolProvider.overrideWith(
            (ref) => createMockMediaPoolNotifier(
              onAddToTimeline: (item) => timelineNotifier.addClip(sourceId: item.id),
            ),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(buildTestableEditorScreen(container: container));
      await tester.pumpAndSettle();

      // 1. Import media
      await tester.tap(find.byKey(const Key('empty_import_media_button')));
      await tester.pumpAndSettle();

      // 2. Click Add to Timeline on imported card
      final addBtnFinder = find.byKey(
        const Key('add_to_timeline_d0000000-0000-0000-0000-000000000001'),
      );
      expect(addBtnFinder, findsOneWidget);

      await tester.tap(addBtnFinder);
      await tester.pumpAndSettle();

      // Verify Timeline updated
      expect(find.text('1'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
