import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

/// Configurable mock file picker for adversarial testing.
class AdversarialMockFilePickerService implements FilePickerService {
  List<String> filesToReturn;
  Exception? exceptionToThrow;
  int callCount = 0;

  AdversarialMockFilePickerService({
    this.filesToReturn = const [],
    this.exceptionToThrow,
  });

  @override
  Future<List<String>> pickMediaFiles() async {
    callCount++;
    if (exceptionToThrow != null) {
      throw exceptionToThrow!;
    }
    return filesToReturn;
  }
}

/// Mock timeline notifier tracking clip placements, bounds, and monotonicity.
class AdversarialMockTimelineNotifier extends TimelineNotifier {
  final List<Clip> addedClips = [];
  int addClipCallCount = 0;

  AdversarialMockTimelineNotifier(TimelineState initial)
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
    // Mimic the in-flight guard present in TimelineNotifier
    if (state.isLoading) return;

    state = state.copyWith(isLoading: true, clearError: true);
    addClipCallCount++;

    final duration = sourceOut - sourceIn;
    final inPts = timelineIn ?? state.durationPts;
    final outPts = inPts + duration;

    final clip = Clip(
      id: UuidValue.fromString(
        'c0000000-0000-0000-0000-${addClipCallCount.toString().padLeft(12, '0')}',
      ),
      sourceId: sourceId ??
          UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: inPts,
      timelineOut: outPts,
    );

    addedClips.add(clip);

    final targetTrackId = trackId ?? state.tracks.first.id;
    final updatedTracks = state.tracks.map((t) {
      if (t.id == targetTrackId) {
        return Track(id: t.id, kind: t.kind, clips: [...t.clips, clip]);
      }
      return t;
    }).toList();

    // The timeline duration is the max timelineOut across all tracks
    final newDurationPts = updatedTracks
        .expand((t) => t.clips)
        .map((c) => c.timelineOut)
        .fold<int>(0, (max, cur) => cur > max ? cur : max);

    final updatedTimeline = state.timeline != null
        ? Timeline(
            id: state.timeline!.id,
            timebase: state.timeline!.timebase,
            durationPts: newDurationPts,
            tracks: updatedTracks,
          )
        : null;

    state = state.copyWith(
      timeline: updatedTimeline,
      totalClipCount: state.totalClipCount + 1,
      durationPts: newDurationPts,
      tracks: updatedTracks,
      isLoading: false,
    );
  }
}

/// Helper to generate N realistic MediaItems.
List<MediaItem> generateMediaItems(int count) {
  const uuid = Uuid();
  return List.generate(count, (i) {
    final type = i % 3 == 0
        ? MediaType.video
        : (i % 3 == 1 ? MediaType.audio : MediaType.image);
    final ext = type == MediaType.video
        ? 'mp4'
        : (type == MediaType.audio ? 'wav' : 'png');
    final durationPts = type == MediaType.image ? 300 : (i + 1) * 60;

    return MediaItem(
      id: uuid.v4obj(),
      filePath: '/media/stress_test/file_$i.$ext',
      fileName: 'file_$i.$ext',
      mediaType: type,
      metadata: MediaMetadata(
        durationPts: durationPts,
        durationSeconds: durationPts / 60.0,
        width: type != MediaType.audio ? 1920 : null,
        height: type != MediaType.audio ? 1080 : null,
        sampleRate: type != MediaType.image ? 48000 : null,
        audioChannels: type != MediaType.image ? 2 : null,
        fileSizeBytes: 1024 * (i + 1),
      ),
    );
  });
}

void main() {
  const uuid = Uuid();
  final videoTrackId =
      UuidValue.fromString('11111111-1111-1111-1111-111111111111');
  final audioTrackId =
      UuidValue.fromString('22222222-2222-2222-2222-222222222222');

  TimelineState createInitialTimelineState() {
    return TimelineState(
      timeline: Timeline(
        id: uuid.v4obj(),
        timebase: const Rational(num: 60, den: 1),
        durationPts: 0,
        tracks: [
          Track(id: videoTrackId, kind: TrackKind.video, clips: const []),
          Track(id: audioTrackId, kind: TrackKind.audio, clips: const []),
        ],
      ),
      totalClipCount: 0,
      durationPts: 0,
      tracks: [
        Track(id: videoTrackId, kind: TrackKind.video, clips: const []),
        Track(id: audioTrackId, kind: TrackKind.audio, clips: const []),
      ],
    );
  }

  group('Group 1: Rapid & Concurrent Imports (50 files)', () {
    test('1A: Single batch import of 50 media files through MediaPoolNotifier',
        () async {
      final items50 = generateMediaItems(50);
      final paths = items50.map((i) => i.filePath).toList();
      final itemMap = {for (var item in items50) item.filePath: item};

      final mockPicker =
          AdversarialMockFilePickerService(filesToReturn: paths);
      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
        importMediaFn: ({projectPath, required filePath}) async {
          return itemMap[filePath]!;
        },
      );

      await notifier.importMedia();

      expect(notifier.state.items.length, equals(50));
      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.errorMessage, isNull);

      // Verify all items are properly stored
      for (int i = 0; i < 50; i++) {
        expect(notifier.state.items[i].fileName, equals('file_$i.${notifier.state.items[i].fileName.split('.').last}'));
      }
    });

    test('1B: Deduplication stress — importing 50 files with 25 pre-existing duplicates',
        () async {
      final items50 = generateMediaItems(50);
      final itemMap = {for (var item in items50) item.filePath: item};

      // Pre-seed notifier with first 25 items
      final preSeeded = items50.take(25).toList();
      final mockPicker = AdversarialMockFilePickerService(
        filesToReturn: items50.map((i) => i.filePath).toList(),
      );

      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
        importMediaFn: ({projectPath, required filePath}) async {
          return itemMap[filePath]!;
        },
      );
      notifier.state = MediaPoolState(items: preSeeded);

      await notifier.importMedia();

      // Should deduplicate existing 25 and only add the remaining 25
      expect(notifier.state.items.length, equals(50));
      final uniquePaths = notifier.state.items.map((i) => i.filePath).toSet();
      expect(uniquePaths.length, equals(50));
    });

    test('1C: Concurrent importMedia calls via Future.wait trigger reentrancy guard',
        () async {
      final items = generateMediaItems(50);
      final itemMap = {for (var item in items) item.filePath: item};

      int importedCount = 0;
      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
        importMediaFn: ({projectPath, required filePath}) async {
          await Future.delayed(const Duration(milliseconds: 10));
          importedCount++;
          return itemMap[filePath]!;
        },
      );

      // Fire 50 concurrent import requests with distinct single-file paths
      final futures = List.generate(
        50,
        (i) => notifier.importMedia(explicitPaths: [items[i].filePath]),
      );

      await Future.wait(futures);

      // Because Dart executes synchronously until the first await in importMedia,
      // the first call sets isLoading = true.
      // Subsequent calls find isLoading == true and return immediately.
      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.items.length, equals(1),
          reason:
              'Concurrent importMedia calls are guarded by isLoading check, dropping concurrent calls');
      expect(importedCount, equals(1));
    });

    test('1D: Rapid sequential importMedia in loop processes all 50 items',
        () async {
      final items = generateMediaItems(50);
      final itemMap = {for (var item in items) item.filePath: item};

      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
        importMediaFn: ({projectPath, required filePath}) async {
          return itemMap[filePath]!;
        },
      );

      for (int i = 0; i < 50; i++) {
        await notifier.importMedia(explicitPaths: [items[i].filePath]);
      }

      expect(notifier.state.items.length, equals(50));
      expect(notifier.state.isLoading, isFalse);
      expect(notifier.state.errorMessage, isNull);
    });

    testWidgets('1E: Headless UI renders 50 media cards with correct header count',
        (tester) async {
      final items50 = generateMediaItems(50);
      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
      );
      notifier.state = MediaPoolState(items: items50);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            mediaPoolProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      // Header should say "Mídias (50)"
      expect(find.text('Mídias (50)'), findsOneWidget);
      expect(find.byKey(const Key('media_pool_list')), findsOneWidget);

      // Verify first visible items render
      expect(find.text('file_0.mp4'), findsOneWidget);

      // Scroll down the list without exception
      await tester.drag(
        find.byKey(const Key('media_pool_list')),
        const Offset(0, -1000),
      );
      await tester.pump();

      // UI is stable and does not throw
      expect(tester.takeException(), isNull);
    });
  });

  group('Group 2: Rapid Multiple Taps on "Adicionar à Timeline" (Monotonic & Overlap Checks)', () {
    testWidgets('2A: 10 consecutive taps place clips monotonically without overlap',
        (tester) async {
      final timelineNotifier =
          AdversarialMockTimelineNotifier(createInitialTimelineState());

      final videoItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/clip.mp4',
        fileName: 'clip.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 180, // 3s @ 60fps
          durationSeconds: 3.0,
          width: 1920,
          height: 1080,
          fileSizeBytes: 1024 * 1024,
        ),
      );

      final mediaNotifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
        onAddToTimeline: (item) {
          // The production onAddToTimeline computes insertionPts = currentTimeline.durationPts
          final currentTimeline = timelineNotifier.state.timeline!;
          final duration = item.metadata.durationPts > 0
              ? item.metadata.durationPts
              : 300;
          timelineNotifier.addClip(
            trackId: videoTrackId,
            sourceId: item.id,
            sourceIn: 0,
            sourceOut: duration,
            timelineIn: currentTimeline.durationPts,
          );
        },
      );
      mediaNotifier.state = MediaPoolState(items: [videoItem]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => timelineNotifier),
            mediaPoolProvider.overrideWith((ref) => mediaNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      final addBtn = find.byKey(Key('add_to_timeline_${videoItem.id}'));
      expect(addBtn, findsOneWidget);

      // Tap 10 times consecutively, waiting for each tap to complete
      for (int i = 0; i < 10; i++) {
        await tester.tap(addBtn);
        await tester.pump();
      }

      // Assertions
      expect(timelineNotifier.addedClips.length, equals(10));
      expect(timelineNotifier.state.totalClipCount, equals(10));
      expect(timelineNotifier.state.durationPts, equals(10 * 180));

      // Empirical Monotonic & Overlap Check:
      // Clip bounds: [timelineIn, timelineOut]
      for (int i = 0; i < 10; i++) {
        final clip = timelineNotifier.addedClips[i];
        final expectedIn = i * 180;
        final expectedOut = (i + 1) * 180;

        expect(clip.timelineIn, equals(expectedIn),
            reason: 'Clip $i must start at $expectedIn');
        expect(clip.timelineOut, equals(expectedOut),
            reason: 'Clip $i must end at $expectedOut');
        expect(clip.sourceId, equals(videoItem.id),
            reason: 'Clip $i must link to original MediaItem id');

        // Check non-overlapping monotonicity with previous clip
        if (i > 0) {
          final prevClip = timelineNotifier.addedClips[i - 1];
          expect(clip.timelineIn, equals(prevClip.timelineOut),
              reason: 'Clip $i must start exactly where Clip ${i - 1} ends (contiguous non-overlapping)');
          expect(clip.timelineIn, greaterThan(prevClip.timelineIn),
              reason: 'Timeline positions must be strictly monotonic');
        }
      }
    });

    testWidgets('2B: Rapid button taps in-flight do not corrupt timeline bounds',
        (tester) async {
      final timelineNotifier =
          AdversarialMockTimelineNotifier(createInitialTimelineState());

      final videoItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/action.mp4',
        fileName: 'action.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 120,
          durationSeconds: 2.0,
          fileSizeBytes: 2048,
        ),
      );

      final mediaNotifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
        onAddToTimeline: (item) {
          final currentTimeline = timelineNotifier.state.timeline!;
          timelineNotifier.addClip(
            trackId: videoTrackId,
            sourceId: item.id,
            sourceIn: 0,
            sourceOut: item.metadata.durationPts,
            timelineIn: currentTimeline.durationPts,
          );
        },
      );
      mediaNotifier.state = MediaPoolState(items: [videoItem]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => timelineNotifier),
            mediaPoolProvider.overrideWith((ref) => mediaNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      final addBtn = find.byKey(Key('add_to_timeline_${videoItem.id}'));

      // Fire 5 taps without waiting for pump between them
      await tester.tap(addBtn);
      await tester.tap(addBtn);
      await tester.tap(addBtn);
      await tester.tap(addBtn);
      await tester.tap(addBtn);
      await tester.pumpAndSettle();

      // Verify no exceptions were thrown
      expect(tester.takeException(), isNull);

      // Verify that all placed clips are strictly monotonic and non-overlapping
      for (int i = 1; i < timelineNotifier.addedClips.length; i++) {
        final prev = timelineNotifier.addedClips[i - 1];
        final curr = timelineNotifier.addedClips[i];
        expect(curr.timelineIn, greaterThanOrEqualTo(prev.timelineOut));
      }
    });

    test('2C: Cross-track PTS placement behavior: Video followed by Audio clip',
        () {
      // Challenge the design where timelineIn defaults to global timeline.durationPts
      final timelineNotifier =
          AdversarialMockTimelineNotifier(createInitialTimelineState());

      final videoItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/video.mp4',
        fileName: 'video.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 300,
          durationSeconds: 5.0,
          fileSizeBytes: 1024,
        ),
      );

      final audioItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/soundtrack.wav',
        fileName: 'soundtrack.wav',
        mediaType: MediaType.audio,
        metadata: const MediaMetadata(
          durationPts: 600,
          durationSeconds: 10.0,
          fileSizeBytes: 1024,
        ),
      );

      // Step 1: Add video clip to video track
      timelineNotifier.addClip(
        trackId: videoTrackId,
        sourceId: videoItem.id,
        sourceIn: 0,
        sourceOut: videoItem.metadata.durationPts,
        timelineIn: timelineNotifier.state.durationPts, // 0
      );

      expect(timelineNotifier.state.durationPts, equals(300));
      expect(timelineNotifier.addedClips[0].timelineIn, equals(0));
      expect(timelineNotifier.addedClips[0].timelineOut, equals(300));

      // Step 2: Add audio clip to audio track using currentTimeline.durationPts
      timelineNotifier.addClip(
        trackId: audioTrackId,
        sourceId: audioItem.id,
        sourceIn: 0,
        sourceOut: audioItem.metadata.durationPts,
        timelineIn: timelineNotifier.state.durationPts, // 300
      );

      // Observation on design: Audio clip on track A1 starts at 300 PTS instead of 0 PTS
      // because mediaPoolProvider references global durationPts rather than track durationPts.
      expect(timelineNotifier.addedClips[1].timelineIn, equals(300));
      expect(timelineNotifier.addedClips[1].timelineOut, equals(900));
      expect(timelineNotifier.state.durationPts, equals(900));
    });
  });

  group('Group 3: Error Handling & UI Resilience', () {
    testWidgets('3A: FilePicker throwing PlatformException renders red error banner',
        (tester) async {
      final mockPicker = AdversarialMockFilePickerService(
        exceptionToThrow: PlatformException(
          code: 'PERMISSION_DENIED',
          message: 'Access to file system was denied by the user.',
        ),
      );

      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            mediaPoolProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      // Initial state: no error
      expect(find.byIcon(Icons.error_outline), findsNothing);

      // Tap import button to trigger exception
      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      // Verify error banner is rendered in UI
      expect(notifier.state.errorMessage, isNotNull);
      expect(
        notifier.state.errorMessage,
        contains('Erro ao abrir seletor de arquivos'),
      );
      expect(find.byIcon(Icons.error_outline), findsOneWidget);
      expect(
        find.textContaining('Erro ao abrir seletor de arquivos'),
        findsOneWidget,
      );

      // Widget tree does not crash and import button remains clickable
      expect(find.byKey(const Key('import_media_button')), findsOneWidget);
    });

    testWidgets('3B: FFI import throwing an error displays error banner and preserves pool',
        (tester) async {
      final existingItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/good.mp4',
        fileName: 'good.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 100,
          durationSeconds: 1.6,
          fileSizeBytes: 1024,
        ),
      );

      final mockPicker =
          AdversarialMockFilePickerService(filesToReturn: ['/media/corrupt.mp4']);

      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
        importMediaFn: ({projectPath, required filePath}) async {
          throw Exception('FFI: DemuxerError(CorruptedContainerHeader)');
        },
      );
      notifier.state = MediaPoolState(items: [existingItem]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            mediaPoolProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      // Trigger import which fails at FFI level
      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      // Error banner is visible
      expect(find.byIcon(Icons.error_outline), findsOneWidget);
      expect(
        find.textContaining('Falha ao importar 1 arquivo(s)'),
        findsOneWidget,
      );

      // Existing item was preserved
      expect(find.text('good.mp4'), findsOneWidget);
      expect(notifier.state.items.length, equals(1));
      expect(notifier.state.isLoading, isFalse);
    });

    test('3C: Partial import failure — 3 succeed, 2 fail', () async {
      final paths = [
        '/media/valid1.mp4',
        '/media/bad1.dat',
        '/media/valid2.mp4',
        '/media/bad2.dat',
        '/media/valid3.mp4',
      ];

      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
        importMediaFn: ({projectPath, required filePath}) async {
          if (filePath.contains('bad')) {
            throw Exception('UnsupportedFormat');
          }
          return MediaItem(
            id: uuid.v4obj(),
            filePath: filePath,
            fileName: filePath.split('/').last,
            mediaType: MediaType.video,
            metadata: const MediaMetadata(
              durationPts: 60,
              durationSeconds: 1.0,
              fileSizeBytes: 1024,
            ),
          );
        },
      );

      await notifier.importMedia(explicitPaths: paths);

      expect(notifier.state.items.length, equals(3));
      expect(notifier.state.errorMessage, contains('Falha ao importar 2 arquivo(s)'));
      expect(notifier.state.errorMessage, contains('/media/bad1.dat'));
      expect(notifier.state.errorMessage, contains('/media/bad2.dat'));
      expect(notifier.state.isLoading, isFalse);
    });

    testWidgets('3D: Error state is cleared on next successful import attempt',
        (tester) async {
      final mockPicker =
          AdversarialMockFilePickerService(filesToReturn: ['/media/good.mp4']);

      final validItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/good.mp4',
        fileName: 'good.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 100,
          durationSeconds: 1.6,
          fileSizeBytes: 1024,
        ),
      );

      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
        importMediaFn: ({projectPath, required filePath}) async => validItem,
      );
      // Pre-set an error state
      notifier.state = const MediaPoolState(
        items: [],
        errorMessage: 'Erro pré-existente no seletor',
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            mediaPoolProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      expect(find.byIcon(Icons.error_outline), findsOneWidget);

      // Perform a successful import
      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      // Error banner should be cleared
      expect(notifier.state.errorMessage, isNull);
      expect(find.byIcon(Icons.error_outline), findsNothing);
      expect(find.text('good.mp4'), findsOneWidget);
    });
  });

  group('Group 4: Empty State Lifecycle Transitions', () {
    testWidgets('4A: Transition empty -> imported -> removed -> empty',
        (tester) async {
      final item = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/intro.mp4',
        fileName: 'intro.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 120,
          durationSeconds: 2.0,
          fileSizeBytes: 1024,
        ),
      );

      final notifier = MediaPoolNotifier(
        filePickerService:
            AdversarialMockFilePickerService(filesToReturn: ['/media/intro.mp4']),
        importMediaFn: ({projectPath, required filePath}) async => item,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            mediaPoolProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      // Phase 1: EMPTY
      expect(find.byKey(const Key('media_pool_empty_state')), findsOneWidget);
      expect(find.text('Nenhuma mídia importada'), findsOneWidget);
      expect(find.text('Mídias (0)'), findsOneWidget);
      expect(find.byKey(const Key('media_pool_list')), findsNothing);

      // Phase 2: IMPORTED
      await tester.tap(find.byKey(const Key('empty_import_media_button')));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('media_pool_empty_state')), findsNothing);
      expect(find.byKey(const Key('media_pool_list')), findsOneWidget);
      expect(find.text('intro.mp4'), findsOneWidget);
      expect(find.text('Mídias (1)'), findsOneWidget);

      // Phase 3: REMOVED -> EMPTY AGAIN
      notifier.removeItem(item.id);
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('media_pool_empty_state')), findsOneWidget);
      expect(find.text('Nenhuma mídia importada'), findsOneWidget);
      expect(find.text('Mídias (0)'), findsOneWidget);
      expect(find.byKey(const Key('media_pool_list')), findsNothing);
    });

    test('4B: Removing selected item clears selection safely', () {
      final item1 = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/1.mp4',
        fileName: '1.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 60,
          durationSeconds: 1.0,
          fileSizeBytes: 1024,
        ),
      );
      final item2 = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/2.mp4',
        fileName: '2.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 60,
          durationSeconds: 1.0,
          fileSizeBytes: 1024,
        ),
      );

      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
      );
      notifier.state = MediaPoolState(items: [item1, item2]);

      // Select item 1
      notifier.selectItem(item1.id);
      expect(notifier.state.selectedItemId, equals(item1.id));
      expect(notifier.state.selectedItem, equals(item1));

      // Remove item 1
      notifier.removeItem(item1.id);
      expect(notifier.state.selectedItemId, isNull);
      expect(notifier.state.selectedItem, isNull);
      expect(notifier.state.items.length, equals(1));
      expect(notifier.state.items.first, equals(item2));

      // Remove item 2
      notifier.removeItem(item2.id);
      expect(notifier.state.items, isEmpty);
      expect(notifier.state.selectedItemId, isNull);
    });

    test('4C: Removing non-existent item is a safe no-op', () {
      final item = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/1.mp4',
        fileName: '1.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 60,
          durationSeconds: 1.0,
          fileSizeBytes: 1024,
        ),
      );

      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockFilePickerService(),
      );
      notifier.state = MediaPoolState(items: [item]);

      final randomUuid = uuid.v4obj();
      notifier.removeItem(randomUuid);

      expect(notifier.state.items.length, equals(1));
      expect(notifier.state.items.first, equals(item));
    });
  });
}
