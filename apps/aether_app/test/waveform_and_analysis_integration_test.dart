import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_view.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

class MockFilePickerService implements FilePickerService {
  @override
  Future<List<String>> pickMediaFiles() async => [];
}

void main() {
  const uuid = Uuid();

  final testAudioItem = MediaItem(
    id: uuid.v4obj(),
    filePath: '/audio/sample_voice.wav',
    fileName: 'sample_voice.wav',
    mediaType: MediaType.audio,
    metadata: const MediaMetadata(
      durationPts: 240,
      durationSeconds: 4.0,
      fileSizeBytes: 10240,
      audioChannels: 2,
      sampleRate: 48000,
    ),
  );

  group('Waveform LOD Computation Tests', () {
    test('computeAudioLodForZoom adapts LOD based on timeline zoom (pps)', () {
      // Zoomed in (100 px/sec): samplesPerPixel = 48000 / 100 = 480 -> LOD 0 (< 1024)
      expect(computeAudioLodForZoom(100.0, sampleRate: 48000), equals(0));

      // Standard zoom (50 px/sec): samplesPerPixel = 48000 / 50 = 960 -> LOD 0 (< 1024)
      expect(computeAudioLodForZoom(50.0, sampleRate: 48000), equals(0));

      // Zoomed out (30 px/sec): samplesPerPixel = 48000 / 30 = 1600 -> LOD 1 (>= 1024, < 4096)
      expect(computeAudioLodForZoom(30.0, sampleRate: 48000), equals(1));

      // Moderately zoomed out (8 px/sec): samplesPerPixel = 48000 / 8 = 6000 -> LOD 2 (>= 4096, < 16384)
      expect(computeAudioLodForZoom(8.0, sampleRate: 48000), equals(2));

      // Heavily zoomed out (1.5 px/sec): samplesPerPixel = 48000 / 1.5 = 32000 -> LOD 3 (>= 16384)
      expect(computeAudioLodForZoom(1.5, sampleRate: 48000), equals(3));

      // Boundary: zero or negative pixelsPerSecond clamps to LOD 0
      expect(computeAudioLodForZoom(0.0), equals(0));
      expect(computeAudioLodForZoom(-10.0), equals(0));
    });
  });

  group('Media Analysis Stream & MediaPoolNotifier Integration', () {
    testWidgets(
        'Streams analysis steps (indexing -> audio_peaks -> thumbnails) and displays discrete progress in card',
        (tester) async {
      final analysisController =
          StreamController<MediaAnalysisProgress>.broadcast();

      final notifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
        startAnalysisStreamFn: ({required assetId, required path}) {
          return analysisController.stream;
        },
        initialItems: [testAudioItem],
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

      // Initially, no analysis in flight
      expect(notifier.state.isIndexing, isFalse);
      expect(find.byType(CircularProgressIndicator), findsNothing);

      // Trigger analysis for the item
      notifier.startMediaAnalysis(
        assetId: testAudioItem.id.uuid,
        filePath: testAudioItem.filePath,
      );

      // 1. Emit Step 1: indexing (50%)
      analysisController.add(
        MediaAnalysisProgress(
          assetId: testAudioItem.id.uuid,
          step: 'indexing',
          progress: 0.50,
        ),
      );
      await tester.pump();

      expect(notifier.state.isIndexing, isTrue);
      expect(find.text('Indexando 50%'), findsOneWidget);
      expect(
        find.byKey(Key('media_item_indexing_spinner_${testAudioItem.id}')),
        findsOneWidget,
      );
      expect(
        find.byKey(Key('media_item_progress_bar_${testAudioItem.id}')),
        findsOneWidget,
      );

      // 2. Emit Step 2: audio_peaks (75%)
      analysisController.add(
        MediaAnalysisProgress(
          assetId: testAudioItem.id.uuid,
          step: 'audio_peaks',
          progress: 0.75,
        ),
      );
      await tester.pump();

      expect(find.text('Forma de onda 75%'), findsOneWidget);

      // 3. Emit Step 3: thumbnails (100% - completed)
      analysisController.add(
        MediaAnalysisProgress(
          assetId: testAudioItem.id.uuid,
          step: 'thumbnails',
          progress: 1.0,
        ),
      );
      await tester.pump();

      // Analysis completed: indicator and spinner vanish
      expect(notifier.state.isIndexing, isFalse);
      expect(find.text('Indexando 50%'), findsNothing);
      expect(find.text('Forma de onda 75%'), findsNothing);
      expect(
        find.byKey(Key('media_item_progress_bar_${testAudioItem.id}')),
        findsNothing,
      );

      await analysisController.close();
    });

    test('Throttling suppresses micro-tick state floods (< 2% within 50ms)', () {
      final notifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
        initialItems: [testAudioItem],
      );

      int stateChangeCount = 0;
      notifier.addListener((_) {
        stateChangeCount++;
      });

      // Initial state emit when listener attached
      expect(stateChangeCount, equals(1));

      // Emit initial progress (10%)
      notifier.updateAnalysisProgress(
        MediaAnalysisProgress(
          assetId: testAudioItem.id.uuid,
          step: 'audio_peaks',
          progress: 0.10,
        ),
      );
      expect(stateChangeCount, equals(2));

      // Rapid micro-ticks (+0.5% each, within same millisecond)
      for (int i = 1; i <= 10; i++) {
        notifier.updateAnalysisProgress(
          MediaAnalysisProgress(
            assetId: testAudioItem.id.uuid,
            step: 'audio_peaks',
            progress: 0.10 + (i * 0.001), // +0.1% each
          ),
        );
      }

      // Micro-ticks should be throttled, avoiding 10 excess state emissions
      expect(stateChangeCount, equals(2));

      // A significant step change or >= 2% advance updates state
      notifier.updateAnalysisProgress(
        MediaAnalysisProgress(
          assetId: testAudioItem.id.uuid,
          step: 'audio_peaks',
          progress: 0.20, // +10% advance
        ),
      );
      expect(stateChangeCount, equals(3));
    });
  });

  group('AudioWaveformPainter Path Batching Tests', () {
    testWidgets('AudioWaveformPainter paints via batched Path without exceptions',
        (tester) async {
      const peaks = [
        AudioPeak(min: -0.9, max: 0.85, rms: 0.60),
        AudioPeak(min: -0.5, max: 0.40, rms: 0.25),
        AudioPeak(min: -0.95, max: 0.95, rms: 0.70),
        AudioPeak(min: -0.2, max: 0.20, rms: 0.15),
      ];

      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: 200,
                height: 40,
                child: CustomPaint(
                  painter: AudioWaveformPainter(
                    peaks: peaks,
                    waveColor: Colors.blue,
                    rmsColor: Colors.teal,
                  ),
                ),
              ),
            ),
          ),
        ),
      );

      final waveformFinder = find.byWidgetPredicate(
        (w) => w is CustomPaint && w.painter is AudioWaveformPainter,
      );
      expect(waveformFinder, findsOneWidget);

      final customPaint = tester.widget<CustomPaint>(waveformFinder);
      final painter = customPaint.painter as AudioWaveformPainter;
      expect(painter.peaks.length, equals(4));

      // shouldRepaint returns false when delegate parameters are identical
      const identicalPainter = AudioWaveformPainter(
        peaks: peaks,
        waveColor: Colors.blue,
        rmsColor: Colors.teal,
      );
      expect(painter.shouldRepaint(identicalPainter), isFalse);

      // shouldRepaint returns true when peaks or color changes
      const differentPainter = AudioWaveformPainter(
        peaks: [AudioPeak(min: -0.1, max: 0.1, rms: 0.05)],
        waveColor: Colors.blue,
        rmsColor: Colors.teal,
      );
      expect(painter.shouldRepaint(differentPainter), isTrue);
    });

    testWidgets('TimelineView requests LOD based on timelineZoomProvider',
        (tester) async {
      final trackId =
          UuidValue.fromString('b0000000-0000-0000-0000-000000000001');
      final clipId =
          UuidValue.fromString('c0000000-0000-0000-0000-000000000001');
      final sourceId =
          UuidValue.fromString('d0000000-0000-0000-0000-000000000001');

      final clip = Clip(
        id: clipId,
        sourceId: sourceId,
        sourceIn: 0,
        sourceOut: 240,
        timelineIn: 0,
        timelineOut: 240,
      );

      final timelineState = TimelineState(
        totalClipCount: 1,
        durationPts: 240,
        tracks: [
          Track(id: trackId, kind: TrackKind.audio, clips: [clip]),
        ],
        isLoading: false,
      );

      int? requestedLod;
      String? requestedAssetId;

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            timelineProvider.overrideWith((ref) => MockTimelineNotifier(timelineState)),
            timelineZoomProvider.overrideWith((ref) => 10.0), // Zoomed out -> LOD 2
            audioWaveformFetcherProvider.overrideWithValue(
                ({required String assetId, required int channel, required int lod}) async {
              requestedAssetId = assetId;
              requestedLod = lod;
              return const [
                AudioPeak(min: -0.7, max: 0.7, rms: 0.4),
              ];
            }),
          ],
          child: const MaterialApp(
            home: Scaffold(body: TimelineView()),
          ),
        ),
      );

      await tester.pumpAndSettle();

      expect(requestedAssetId, equals(sourceId.toString()));
      // 48000 / 10.0 = 4800 samples/pixel -> LOD 2 (>= 4096)
      expect(requestedLod, equals(2));

      expect(find.byKey(Key('clip_waveform_$clipId')), findsOneWidget);
    });
  });
}

class MockTimelineNotifier extends TimelineNotifier {
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
}
