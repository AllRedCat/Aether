import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

/// Mock file picker that returns preconfigured file paths without opening OS dialogs.
class MockFilePickerService implements FilePickerService {
  List<String> filesToReturn;
  bool pickCalled = false;

  MockFilePickerService(this.filesToReturn);

  @override
  Future<List<String>> pickMediaFiles() async {
    pickCalled = true;
    return filesToReturn;
  }
}

/// Mock timeline notifier allowing exact state tracking in headless widget tests.
class MockTimelineNotifier extends TimelineNotifier {
  bool addClipCalled = false;
  UuidValue? lastTrackId;
  UuidValue? lastSourceId;
  int? lastSourceIn;
  int? lastSourceOut;

  MockTimelineNotifier(TimelineState initial)
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
    addClipCalled = true;
    lastTrackId = trackId;
    lastSourceId = sourceId;
    lastSourceIn = sourceIn;
    lastSourceOut = sourceOut;

    final duration = sourceOut - sourceIn;
    final inPts = timelineIn ?? state.durationPts;
    final outPts = inPts + duration;

    final newClip = Clip(
      id: UuidValue.fromString('c0000000-0000-0000-0000-000000000001'),
      sourceId: sourceId ??
          UuidValue.fromString('d0000000-0000-0000-0000-000000000001'),
      sourceIn: sourceIn,
      sourceOut: sourceOut,
      timelineIn: inPts,
      timelineOut: outPts,
    );

    final updatedTracks = state.tracks.map((t) {
      if (trackId == null || t.id == trackId) {
        return Track(id: t.id, kind: t.kind, clips: [...t.clips, newClip]);
      }
      return t;
    }).toList();

    state = state.copyWith(
      totalClipCount: state.totalClipCount + 1,
      durationPts: outPts,
      tracks: updatedTracks,
    );
  }
}

void main() {
  const uuid = Uuid();

  final sampleVideo = MediaItem(
    id: uuid.v4obj(),
    filePath: '/videos/sample.mp4',
    fileName: 'sample.mp4',
    mediaType: MediaType.video,
    metadata: const MediaMetadata(
      durationPts: 900, // 15s @ 60fps
      durationSeconds: 15.0,
      width: 1920,
      height: 1080,
      fileSizeBytes: 1024 * 1024 * 10,
    ),
  );

  final sampleAudio = MediaItem(
    id: uuid.v4obj(),
    filePath: '/audio/music.wav',
    fileName: 'music.wav',
    mediaType: MediaType.audio,
    metadata: const MediaMetadata(
      durationPts: 7200, // 120s @ 60fps
      durationSeconds: 120.0,
      sampleRate: 48000,
      audioChannels: 2,
      fileSizeBytes: 1024 * 1024 * 5,
    ),
  );

  final sampleImage = MediaItem(
    id: uuid.v4obj(),
    filePath: '/images/photo.png',
    fileName: 'photo.png',
    mediaType: MediaType.image,
    metadata: const MediaMetadata(
      durationPts: 300, // 5s @ 60fps
      durationSeconds: 5.0,
      width: 3840,
      height: 2160,
      fileSizeBytes: 1024 * 1024 * 2,
    ),
  );

  group('MediaFormatter Unit Tests', () {
    test('formats durations correctly', () {
      expect(MediaFormatter.formatDuration(0), '00:00');
      expect(MediaFormatter.formatDuration(15), '00:15');
      expect(MediaFormatter.formatDuration(120), '02:00');
      expect(MediaFormatter.formatDuration(3665), '01:01:05');
    });

    test('formats pts with default 60 fps', () {
      expect(MediaFormatter.formatPts(900), '00:15');
      expect(MediaFormatter.formatPts(7200), '02:00');
      expect(MediaFormatter.formatPts(0), '00:00');
    });

    test('maps media types to icons and labels', () {
      expect(MediaFormatter.typeIcon(MediaType.video), Icons.videocam_rounded);
      expect(MediaFormatter.typeIcon(MediaType.audio), Icons.audiotrack_rounded);
      expect(MediaFormatter.typeIcon(MediaType.image), Icons.image_rounded);
      expect(MediaFormatter.typeLabel(MediaType.video), 'VÍDEO');
      expect(MediaFormatter.typeLabel(MediaType.audio), 'ÁUDIO');
      expect(MediaFormatter.typeLabel(MediaType.image), 'IMAGEM');
    });
  });

  group('MediaPoolNotifier Unit Tests', () {
    test('selectItem and removeItem update state correctly', () {
      final notifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService([]),
      );
      notifier.state = MediaPoolState(items: [sampleVideo, sampleAudio]);

      // Select item
      notifier.selectItem(sampleVideo.id);
      expect(notifier.state.selectedItemId, sampleVideo.id);
      expect(notifier.state.selectedItem, sampleVideo);

      // Deselect by toggling same item
      notifier.selectItem(sampleVideo.id);
      expect(notifier.state.selectedItemId, isNull);

      // Select again then remove
      notifier.selectItem(sampleVideo.id);
      notifier.removeItem(sampleVideo.id);
      expect(notifier.state.items.length, 1);
      expect(notifier.state.selectedItemId, isNull);
    });

    test('deduplicates existing media by file path on import', () async {
      final mockPicker = MockFilePickerService(['/videos/sample.mp4']);
      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
        importMediaFn: ({projectPath, required filePath}) async => sampleVideo,
      );
      notifier.state = MediaPoolState(items: [sampleVideo]);

      await notifier.importMedia();
      expect(notifier.state.items.length, 1);
    });
  });

  group('MediaPoolView Headless Widget Tests', () {
    testWidgets('Test 1: Empty state renders when pool is empty',
        (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            mediaPoolProvider.overrideWith((ref) => MediaPoolNotifier(
                  filePickerService: MockFilePickerService([]),
                )),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      expect(find.byKey(const Key('media_pool_empty_state')), findsOneWidget);
      expect(find.text('Nenhuma mídia importada'), findsOneWidget);
      expect(find.byKey(const Key('import_media_button')), findsOneWidget);
      expect(find.byKey(const Key('empty_import_media_button')), findsOneWidget);
    });

    testWidgets(
        'Test 2: Renders list of items with type badges, filename and formatted duration',
        (tester) async {
      final notifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService([]),
      );
      notifier.state =
          MediaPoolState(items: [sampleVideo, sampleAudio, sampleImage]);

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

      expect(find.text('sample.mp4'), findsOneWidget);
      expect(find.text('music.wav'), findsOneWidget);
      expect(find.text('photo.png'), findsOneWidget);
      expect(find.text('00:15'), findsOneWidget);
      expect(find.text('02:00'), findsOneWidget);
      expect(find.text('00:05'), findsOneWidget);
      expect(find.byIcon(Icons.videocam_rounded), findsOneWidget);
      expect(find.byIcon(Icons.audiotrack_rounded), findsOneWidget);
      expect(find.byIcon(Icons.image_rounded), findsOneWidget);
      expect(find.text('VÍDEO'), findsOneWidget);
      expect(find.text('ÁUDIO'), findsOneWidget);
      expect(find.text('IMAGEM'), findsOneWidget);
    });

    testWidgets(
        'Test 3: Import button triggers file picker and updates state',
        (tester) async {
      final mockPicker = MockFilePickerService(['/videos/sample.mp4']);
      final notifier = MediaPoolNotifier(
        filePickerService: mockPicker,
        importMediaFn: ({projectPath, required filePath}) async => sampleVideo,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            filePickerServiceProvider.overrideWithValue(mockPicker),
            mediaPoolProvider.overrideWith((ref) => notifier),
          ],
          child: const MaterialApp(
            home: Scaffold(body: MediaPoolView()),
          ),
        ),
      );

      await tester.tap(find.byKey(const Key('import_media_button')));
      await tester.pumpAndSettle();

      expect(mockPicker.pickCalled, isTrue);
      expect(find.text('sample.mp4'), findsOneWidget);
      expect(find.text('00:15'), findsOneWidget);
    });

    testWidgets(
        'Test 4: Tapping Adicionar à Timeline appends clip to timeline with matching sourceId and updates timeline clip count',
        (tester) async {
      final trackId = uuid.v4obj();
      final timelineState = TimelineState(
        timeline: Timeline(
          id: uuid.v4obj(),
          timebase: const Rational(num: 60, den: 1),
          durationPts: 0,
          tracks: [
            Track(id: trackId, kind: TrackKind.video, clips: const []),
          ],
        ),
        totalClipCount: 0,
        durationPts: 0,
        tracks: [
          Track(id: trackId, kind: TrackKind.video, clips: const []),
        ],
      );

      final timelineNotifier = MockTimelineNotifier(timelineState);
      final mediaNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService([]),
        onAddToTimeline: (item) {
          timelineNotifier.addClip(
            trackId: trackId,
            sourceId: item.id,
            sourceIn: 0,
            sourceOut: item.metadata.durationPts,
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
          child: MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  const Expanded(child: MediaPoolView()),
                  Consumer(
                    builder: (context, ref, _) {
                      final tState = ref.watch(timelineProvider);
                      return Column(
                        children: [
                          Text(
                            '${tState.totalClipCount}',
                            key: const Key('timeline_total_clips_count'),
                          ),
                          Text(
                            '${tState.durationPts} PTS',
                            key: const Key('timeline_duration_pts'),
                          ),
                        ],
                      );
                    },
                  ),
                ],
              ),
            ),
          ),
        ),
      );

      expect(
        find.byKey(const Key('timeline_total_clips_count')),
        findsOneWidget,
      );
      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('0'),
      );

      // Tap "Adicionar à Timeline" on the sample video
      final addBtn = find.byKey(Key('add_to_timeline_${sampleVideo.id}'));
      expect(addBtn, findsOneWidget);
      await tester.tap(addBtn);
      await tester.pump();

      // Verify Timeline updated
      expect(timelineNotifier.addClipCalled, isTrue);
      expect(timelineNotifier.lastSourceId, equals(sampleVideo.id));
      expect(timelineNotifier.lastSourceOut, equals(sampleVideo.metadata.durationPts));
      expect(
        tester
            .widget<Text>(find.byKey(const Key('timeline_total_clips_count')))
            .data,
        equals('1'),
      );
      expect(find.text('900 PTS'), findsOneWidget);
    });
  });
}
