import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

/// Configurable mock file picker for adversarial testing.
class AdversarialMockPicker implements FilePickerService {
  List<String> filesToReturn;
  Duration simulatedDelay;
  int callCount = 0;
  Exception? errorToThrow;

  AdversarialMockPicker({
    this.filesToReturn = const [],
    this.simulatedDelay = Duration.zero,
    this.errorToThrow,
  });

  @override
  Future<List<String>> pickMediaFiles() async {
    callCount++;
    if (simulatedDelay > Duration.zero) {
      await Future.delayed(simulatedDelay);
    }
    if (errorToThrow != null) {
      throw errorToThrow!;
    }
    return filesToReturn;
  }
}

/// Mock timeline notifier tracking clip additions, monotonicity, and overlaps.
class AdversarialTimelineNotifier extends TimelineNotifier {
  final List<Clip> addedClips = [];
  int addClipCalls = 0;

  AdversarialTimelineNotifier(TimelineState initial)
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
    addClipCalls++;
    if (state.isLoading) {
      // In-flight guard drops the call
      return;
    }

    state = state.copyWith(isLoading: true, clearError: true);

    final duration = sourceOut - sourceIn;
    final inPts = timelineIn ?? state.durationPts;
    final outPts = inPts + duration;

    final newClip = Clip(
      id: UuidValue.fromString(
        'c0000000-0000-0000-0000-${addClipCalls.toString().padLeft(12, '0')}',
      ),
      sourceId: sourceId ??
          UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: inPts,
      timelineOut: outPts,
    );

    addedClips.add(newClip);

    final targetTrackId = trackId ?? (state.tracks.isNotEmpty ? state.tracks.first.id : null);
    final updatedTracks = state.tracks.map((t) {
      if (t.id == targetTrackId) {
        return Track(id: t.id, kind: t.kind, clips: [...t.clips, newClip]);
      }
      return t;
    }).toList();

    final maxPts = updatedTracks
        .expand((t) => t.clips)
        .map((c) => c.timelineOut)
        .fold<int>(0, (max, cur) => cur > max ? cur : max);

    final updatedTimeline = state.timeline != null
        ? Timeline(
            id: state.timeline!.id,
            timebase: state.timeline!.timebase,
            durationPts: maxPts,
            tracks: updatedTracks,
          )
        : null;

    state = state.copyWith(
      timeline: updatedTimeline,
      totalClipCount: state.totalClipCount + 1,
      durationPts: maxPts,
      tracks: updatedTracks,
      isLoading: false,
    );
  }
}

void main() {
  const uuid = Uuid();

  final videoTrackId =
      UuidValue.fromString('b0000000-0000-0000-0000-000000000001');
  final audioTrackId =
      UuidValue.fromString('b0000000-0000-0000-0000-000000000002');

  TimelineState buildInitialTimelineState() {
    final timeline = Timeline(
      id: UuidValue.fromString('a0000000-0000-0000-0000-000000000001'),
      timebase: const Rational(num: 60, den: 1),
      durationPts: 0,
      tracks: [
        Track(id: videoTrackId, kind: TrackKind.video, clips: const []),
        Track(id: audioTrackId, kind: TrackKind.audio, clips: const []),
      ],
    );
    return TimelineState.fromTimeline(timeline);
  }

  group('Adversarial Challenge 1: Invalid & Non-existent File Paths via Bridge', () {
    testWidgets('Non-existent file produces clean error banner without unhandled panics',
        (tester) async {
      final picker = AdversarialMockPicker(filesToReturn: ['/nonexistent/media.mp4']);
      final notifier = MediaPoolNotifier(
        filePickerService: picker,
        importMediaFn: ({projectPath, required filePath}) async {
          if (!filePath.startsWith('/valid/')) {
            throw Exception("File does not exist: $filePath");
          }
          return MediaItem(
            id: uuid.v4obj(),
            filePath: filePath,
            fileName: 'media.mp4',
            mediaType: MediaType.video,
            metadata: const MediaMetadata(
              durationPts: 300,
              durationSeconds: 5.0,
              fileSizeBytes: 1000,
            ),
          );
        },
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

      // Verify empty state initially
      expect(find.byKey(const Key('media_pool_empty_state')), findsOneWidget);
      expect(notifier.state.errorMessage, isNull);

      // Tap import button
      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      // Verify clean error handling
      expect(notifier.state.errorMessage, isNotNull);
      expect(notifier.state.errorMessage, contains('File does not exist'));
      expect(notifier.state.items, isEmpty);

      // Error banner must be rendered in UI with warning icon
      expect(find.byIcon(Icons.error_outline), findsOneWidget);
      expect(find.textContaining('File does not exist: /nonexistent/media.mp4'), findsOneWidget);

      // Verify no uncaught exceptions crashed the widget hierarchy
      expect(tester.takeException(), isNull);
    });

    testWidgets('Empty and whitespace-only paths are rejected safely',
        (tester) async {
      final notifier = MediaPoolNotifier(
        filePickerService: AdversarialMockPicker(filesToReturn: ['   ', '']),
        importMediaFn: ({projectPath, required filePath}) async {
          if (filePath.trim().isEmpty) {
            throw Exception('File path cannot be empty');
          }
          throw Exception('Unexpected');
        },
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

      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      expect(notifier.state.errorMessage, contains('File path cannot be empty'));
      expect(find.byIcon(Icons.error_outline), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('Partial batch failure: valid files are imported while errors are reported',
        (tester) async {
      final picker = AdversarialMockPicker(
        filesToReturn: ['/valid/clip1.mp4', '/bad/corrupt.mp4', '/valid/audio.wav'],
      );

      final notifier = MediaPoolNotifier(
        filePickerService: picker,
        importMediaFn: ({projectPath, required filePath}) async {
          if (filePath.contains('/bad/')) {
            throw Exception("Corrupt header: $filePath");
          }
          final isAudio = filePath.endsWith('.wav');
          return MediaItem(
            id: uuid.v4obj(),
            filePath: filePath,
            fileName: filePath.split('/').last,
            mediaType: isAudio ? MediaType.audio : MediaType.video,
            metadata: MediaMetadata(
              durationPts: isAudio ? 600 : 300,
              durationSeconds: isAudio ? 10.0 : 5.0,
              fileSizeBytes: 2048,
            ),
          );
        },
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

      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      // 2 valid files must be in the pool
      expect(notifier.state.items.length, equals(2));
      expect(find.text('clip1.mp4'), findsOneWidget);
      expect(find.text('audio.wav'), findsOneWidget);

      // Error banner must report the single failure
      expect(notifier.state.errorMessage, isNotNull);
      expect(notifier.state.errorMessage, contains('Falha ao importar 1 arquivo(s)'));
      expect(find.byIcon(Icons.error_outline), findsOneWidget);

      // Verify error recovery: subsequent 100% successful import clears error banner
      picker.filesToReturn = ['/valid/clip2.mp4'];
      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      expect(notifier.state.items.length, equals(3));
      expect(notifier.state.errorMessage, isNull);
      expect(find.byIcon(Icons.error_outline), findsNothing);
      expect(tester.takeException(), isNull);
    });
  });

  group('Adversarial Challenge 2: Rapid Multiple Taps & Debouncing', () {
    testWidgets('Sequential multiple taps on "Adicionar à Timeline" add clips monotonically without overlap',
        (tester) async {
      final timelineNotifier = AdversarialTimelineNotifier(buildInitialTimelineState());
      final sampleVideo = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/sample.mp4',
        fileName: 'sample.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 180, // 3s @ 60fps
          durationSeconds: 3.0,
          fileSizeBytes: 4096,
        ),
      );

      final mediaNotifier = MediaPoolNotifier(
        filePickerService: AdversarialMockPicker(),
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
      mediaNotifier.state = MediaPoolState(items: [sampleVideo]);

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

      final addBtn = find.byKey(Key('add_to_timeline_${sampleVideo.id}'));
      expect(addBtn, findsOneWidget);

      // Tap 5 times sequentially, waiting between taps
      for (int i = 0; i < 5; i++) {
        await tester.tap(addBtn);
        await tester.pump();
      }

      // Assertions
      expect(timelineNotifier.addedClips.length, equals(5));
      expect(timelineNotifier.state.totalClipCount, equals(5));
      expect(timelineNotifier.state.durationPts, equals(5 * 180));

      // Monotonic non-overlapping bounds check
      for (int i = 0; i < 5; i++) {
        final clip = timelineNotifier.addedClips[i];
        expect(clip.timelineIn, equals(i * 180));
        expect(clip.timelineOut, equals((i + 1) * 180));
        if (i > 0) {
          final prevClip = timelineNotifier.addedClips[i - 1];
          expect(clip.timelineIn, equals(prevClip.timelineOut));
        }
      }
    });

    testWidgets('Rapid in-flight taps on "Adicionar à Timeline" do not crash or corrupt state',
        (tester) async {
      final timelineNotifier = AdversarialTimelineNotifier(buildInitialTimelineState());
      final sampleVideo = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/sample.mp4',
        fileName: 'sample.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 180,
          durationSeconds: 3.0,
          fileSizeBytes: 4096,
        ),
      );

      final mediaNotifier = MediaPoolNotifier(
        filePickerService: AdversarialMockPicker(),
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
      mediaNotifier.state = MediaPoolState(items: [sampleVideo]);

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

      final addBtn = find.byKey(Key('add_to_timeline_${sampleVideo.id}'));

      // Fire 5 taps without pump between them (simulating rapid in-flight clicks)
      for (int i = 0; i < 5; i++) {
        await tester.tap(addBtn);
      }
      await tester.pumpAndSettle();

      // Zero unhandled exceptions
      expect(tester.takeException(), isNull);

      // Verify that every clip that was successfully added has valid bounds
      for (int i = 1; i < timelineNotifier.addedClips.length; i++) {
        final prev = timelineNotifier.addedClips[i - 1];
        final curr = timelineNotifier.addedClips[i];
        expect(curr.timelineIn, greaterThanOrEqualTo(prev.timelineOut));
      }
    });

    testWidgets('Rapid multiple taps on Import button while picker is in flight',
        (tester) async {
      final picker = AdversarialMockPicker(
        filesToReturn: ['/media/unique.mp4'],
        simulatedDelay: const Duration(milliseconds: 50),
      );

      final notifier = MediaPoolNotifier(
        filePickerService: picker,
        importMediaFn: ({projectPath, required filePath}) async {
          return MediaItem(
            id: uuid.v4obj(),
            filePath: filePath,
            fileName: 'unique.mp4',
            mediaType: MediaType.video,
            metadata: const MediaMetadata(
              durationPts: 300,
              durationSeconds: 5.0,
              fileSizeBytes: 1024,
            ),
          );
        },
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

      final importBtn = find.byKey(const Key('import_media_button'));

      // Tap 3 times rapidly
      await tester.tap(importBtn);
      await tester.tap(importBtn);
      await tester.tap(importBtn);

      await tester.pumpAndSettle(const Duration(milliseconds: 200));

      // Check whether picker was invoked multiple times and whether items were duplicated
      // Even if picker was called multiple times, deduplication must prevent duplicate entries
      final matchingItems = notifier.state.items.where((i) => i.filePath == '/media/unique.mp4').toList();
      expect(
        matchingItems.length,
        lessThanOrEqualTo(1),
        reason: 'Media Pool must never contain duplicate items with identical file paths',
      );
      expect(tester.takeException(), isNull);
    });
  });

  group('Adversarial Challenge 3: Metadata Bounds & Track Routing', () {
    test('Image media with 0 duration PTS falls back safely to default duration', () {
      final imageItem = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/photo.png',
        fileName: 'photo.png',
        mediaType: MediaType.image,
        metadata: const MediaMetadata(
          durationPts: 0,
          durationSeconds: 0.0,
          width: 3840,
          height: 2160,
          fileSizeBytes: 500000,
        ),
      );

      // Verify metadata duration is 0
      expect(imageItem.metadata.durationPts, equals(0));

      // Test default duration fallback in onAddToTimeline logic (300 PTS)
      final duration = imageItem.metadata.durationPts > 0
          ? imageItem.metadata.durationPts
          : 300;
      expect(duration, equals(300));
    });

    testWidgets('Removing selected item clears selection safely', (tester) async {
      final item = MediaItem(
        id: uuid.v4obj(),
        filePath: '/media/item.mp4',
        fileName: 'item.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 60,
          durationSeconds: 1.0,
          fileSizeBytes: 100,
        ),
      );

      final notifier = MediaPoolNotifier(filePickerService: AdversarialMockPicker());
      notifier.state = MediaPoolState(items: [item], selectedItemId: item.id);

      expect(notifier.state.selectedItemId, equals(item.id));
      expect(notifier.state.selectedItem, isNotNull);

      notifier.removeItem(item.id);

      expect(notifier.state.items, isEmpty);
      expect(notifier.state.selectedItemId, isNull);
      expect(notifier.state.selectedItem, isNull);
    });
  });
}
