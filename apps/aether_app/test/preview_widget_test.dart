import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/editor/editor_screen.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_provider.dart';
import 'package:aether_app/src/features/preview/preview_service.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

/// Test mock implementation of [FilePickerService].
class MockFilePickerService implements FilePickerService {
  @override
  Future<List<String>> pickMediaFiles() async => [];
}

/// Headless mock implementation of [PreviewBridgeService] for unit and widget testing.
class MockPreviewBridgeService implements PreviewBridgeService {
  bool createSessionCalled = false;
  String? lastCreatedFilePath;
  bool closeSessionCalled = false;
  String? lastClosedSessionId;
  bool playCalled = false;
  bool pauseCalled = false;
  double? lastSeekSeconds;
  int? lastSeekPts;
  int textureIdToReturn = 42;
  String sessionIdToReturn = 'test-session-42';
  int widthToReturn = 1920;
  int heightToReturn = 1080;
  double durationSecondsToReturn = 10.0;
  int durationPtsToReturn = 600;
  Exception? errorToThrow;

  final StreamController<BridgeFrame> frameController =
      StreamController<BridgeFrame>.broadcast();
  final StreamController<PlaybackState> playbackStateController =
      StreamController<PlaybackState>.broadcast();

  @override
  Future<PreviewSessionInfo> createPreviewSession({
    required String filePath,
  }) async {
    createSessionCalled = true;
    lastCreatedFilePath = filePath;
    if (errorToThrow != null) {
      throw errorToThrow!;
    }
    return PreviewSessionInfo(
      sessionId: sessionIdToReturn,
      textureId: textureIdToReturn,
      width: widthToReturn,
      height: heightToReturn,
      durationPts: durationPtsToReturn,
      durationSeconds: durationSecondsToReturn,
      fps: 60.0,
      timebase: const Rational(num: 1, den: 60),
    );
  }

  @override
  Future<void> closePreviewSession({required String sessionId}) async {
    closeSessionCalled = true;
    lastClosedSessionId = sessionId;
  }

  @override
  Future<void> previewPlay({required String sessionId}) async {
    playCalled = true;
    playbackStateController.add(PlaybackState(
      sessionId: sessionId,
      isPlaying: true,
      currentPts: 0,
      currentSeconds: 0.0,
      durationPts: durationPtsToReturn,
      durationSeconds: durationSecondsToReturn,
    ));
  }

  @override
  Future<void> previewPause({required String sessionId}) async {
    pauseCalled = true;
    playbackStateController.add(PlaybackState(
      sessionId: sessionId,
      isPlaying: false,
      currentPts: 0,
      currentSeconds: 0.0,
      durationPts: durationPtsToReturn,
      durationSeconds: durationSecondsToReturn,
    ));
  }

  @override
  Future<BridgeFrame?> previewSeekPts({
    required String sessionId,
    required int targetPts,
  }) async {
    lastSeekPts = targetPts;
    final seconds = targetPts / 60.0;
    return BridgeFrame(
      width: widthToReturn,
      height: heightToReturn,
      pts: targetPts,
      durationPts: 1,
      rgbaBytes: Uint8List(0),
      rowStrideBytes: widthToReturn * 4,
      timestampSeconds: seconds,
    );
  }

  @override
  Future<BridgeFrame?> previewSeekSeconds({
    required String sessionId,
    required double seconds,
  }) async {
    lastSeekSeconds = seconds;
    final pts = (seconds * 60).round();
    return BridgeFrame(
      width: widthToReturn,
      height: heightToReturn,
      pts: pts,
      durationPts: 1,
      rgbaBytes: Uint8List(0),
      rowStrideBytes: widthToReturn * 4,
      timestampSeconds: seconds,
    );
  }

  @override
  Future<PlaybackState> getPreviewState({required String sessionId}) async {
    return PlaybackState(
      sessionId: sessionId,
      isPlaying: playCalled && !pauseCalled,
      currentPts:
          (lastSeekSeconds != null ? (lastSeekSeconds! * 60).round() : 0),
      currentSeconds: lastSeekSeconds ?? 0.0,
      durationPts: durationPtsToReturn,
      durationSeconds: durationSecondsToReturn,
    );
  }

  @override
  Stream<BridgeFrame> subscribePreviewFrames({required String sessionId}) {
    return frameController.stream;
  }

  @override
  Stream<PlaybackState> subscribePlaybackState({required String sessionId}) {
    return playbackStateController.stream;
  }

  @override
  Future<BridgeFrame> extractSingleFrame({
    required String filePath,
    required int targetPts,
  }) async {
    return BridgeFrame(
      width: 1920,
      height: 1080,
      pts: targetPts,
      durationPts: 1,
      rgbaBytes: Uint8List(0),
      rowStrideBytes: 1920 * 4,
      timestampSeconds: targetPts / 60.0,
    );
  }

  void dispose() {
    frameController.close();
    playbackStateController.close();
  }
}

void main() {
  const uuid = Uuid();

  final sampleVideo = MediaItem(
    id: uuid.v4obj(),
    filePath: '/videos/test_clip.mp4',
    fileName: 'test_clip.mp4',
    mediaType: MediaType.video,
    metadata: const MediaMetadata(
      durationPts: 600,
      durationSeconds: 10.0,
      width: 1920,
      height: 1080,
      fileSizeBytes: 1024 * 1024 * 4,
    ),
  );

  group('Preview State and Unit Tests', () {
    test('Initial PreviewState default values', () {
      const state = PreviewState.initial();
      expect(state.session, isNull);
      expect(state.currentFrame, isNull);
      expect(state.currentMedia, isNull);
      expect(state.isPlaying, isFalse);
      expect(state.currentPts, 0);
      expect(state.currentSeconds, 0.0);
      expect(state.durationPts, 0);
      expect(state.durationSeconds, 0.0);
      expect(state.isLoading, isFalse);
      expect(state.errorMessage, isNull);
      expect(state.textureId, 0);
      expect(state.progressFraction, 0.0);
      expect(state.aspectRatio, closeTo(16 / 9, 0.001));
      expect(state.isLoaded, isFalse);
      expect(state.hasMedia, isFalse);
    });

    test('PreviewState copyWith and custom textureId', () {
      const state = PreviewState(
        textureId: 99,
        durationSeconds: 20.0,
        currentSeconds: 10.0,
      );
      expect(state.textureId, 99);
      expect(state.progressFraction, 0.5);

      final updated = state.copyWith(currentSeconds: 15.0);
      expect(updated.currentSeconds, 15.0);
      expect(updated.progressFraction, 0.75);
    });

    test('PreviewNotifier loads media and controls playback', () async {
      final mockBridge = MockPreviewBridgeService();
      final notifier = PreviewNotifier(bridgeService: mockBridge);

      // Load media
      await notifier.loadMedia(sampleVideo);
      expect(mockBridge.createSessionCalled, isTrue);
      expect(mockBridge.lastCreatedFilePath, sampleVideo.filePath);
      expect(notifier.state.session?.textureId, 42);
      expect(notifier.state.textureId, 42);
      expect(notifier.state.durationSeconds, 10.0);
      expect(notifier.state.isLoaded, isTrue);

      // Play
      await notifier.play();
      expect(mockBridge.playCalled, isTrue);
      expect(notifier.state.isPlaying, isTrue);

      // Pause
      await notifier.pause();
      expect(mockBridge.pauseCalled, isTrue);
      expect(notifier.state.isPlaying, isFalse);

      // Toggle Play/Pause
      await notifier.togglePlayPause();
      expect(notifier.state.isPlaying, isTrue);
      await notifier.togglePlayPause();
      expect(notifier.state.isPlaying, isFalse);

      // Seek
      await notifier.seek(0.5);
      expect(mockBridge.lastSeekSeconds, closeTo(5.0, 0.01));
      expect(notifier.state.currentSeconds, closeTo(5.0, 0.01));

      // Close session
      await notifier.closeSession();
      expect(mockBridge.closeSessionCalled, isTrue);
      expect(notifier.state.session, isNull);
      expect(notifier.state.isLoaded, isFalse);

      notifier.dispose();
      mockBridge.dispose();
    });
  });

  group('PreviewView Widget Tests', () {
    testWidgets('1. Initial Empty State renders placeholder and disabled controls',
        (tester) async {
      final mockBridge = MockPreviewBridgeService();

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => MediaPoolNotifier(
                  filePickerService: MockFilePickerService(),
                )),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: PreviewView(),
            ),
          ),
        ),
      );

      // Texture widget should not be present
      expect(find.byKey(const Key('preview_texture_widget')), findsNothing);

      // Placeholder text
      expect(find.text('Nenhuma mídia selecionada'), findsOneWidget);
      expect(
        find.text(
            'Selecione um arquivo no Media Pool para iniciar a pré-visualização'),
        findsOneWidget,
      );

      // Play/Pause button exists but is disabled
      final playPauseButton = tester.widget<IconButton>(
        find.byKey(const Key('preview_play_pause_button')),
      );
      expect(playPauseButton.onPressed, isNull);

      // Timecode is 00:00 / 00:00
      expect(find.byKey(const Key('preview_timecode_text')), findsOneWidget);
      expect(find.text('00:00 / 00:00'), findsOneWidget);

      // Scrubber slider exists but is disabled
      final slider = tester.widget<Slider>(
        find.byKey(const Key('preview_progress_slider')),
      );
      expect(slider.onChanged, isNull);

      mockBridge.dispose();
    });

    testWidgets(
        '2. Selecting a media item in Media Pool initializes preview and mounts Texture',
        (tester) async {
      final mockBridge = MockPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [sampleVideo]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => poolNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  Expanded(child: MediaPoolView()),
                  Expanded(child: PreviewView()),
                ],
              ),
            ),
          ),
        ),
      );

      // Before selection: empty preview
      expect(find.byKey(const Key('preview_texture_widget')), findsNothing);

      // Tap on the media item card to select it
      await tester.tap(find.text('test_clip.mp4'));
      await tester.pumpAndSettle();

      // Verify bridge service was called
      expect(mockBridge.createSessionCalled, isTrue);
      expect(mockBridge.lastCreatedFilePath, sampleVideo.filePath);

      // Texture widget should now be mounted with textureId == 42
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);
      final textureWidget = tester.widget<Texture>(
        find.byKey(const Key('preview_texture_widget')),
      );
      expect(textureWidget.textureId, 42);

      // Timecode display updated
      expect(find.text('00:00 / 00:10'), findsOneWidget);

      mockBridge.dispose();
    });

    testWidgets('3. Play/Pause button toggles playback state', (tester) async {
      final mockBridge = MockPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(
        items: [sampleVideo],
        selectedItemId: sampleVideo.id,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => poolNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: PreviewView(),
            ),
          ),
        ),
      );

      await tester.pumpAndSettle();

      // Play/Pause button is enabled and shows play icon
      expect(find.byIcon(Icons.play_arrow_rounded), findsOneWidget);
      expect(find.byIcon(Icons.pause_rounded), findsNothing);

      // Tap Play
      await tester.tap(find.byKey(const Key('preview_play_pause_button')));
      await tester.pumpAndSettle();

      expect(mockBridge.playCalled, isTrue);
      expect(find.byIcon(Icons.pause_rounded), findsOneWidget);

      // Tap Pause
      await tester.tap(find.byKey(const Key('preview_play_pause_button')));
      await tester.pumpAndSettle();

      expect(mockBridge.pauseCalled, isTrue);
      expect(find.byIcon(Icons.play_arrow_rounded), findsOneWidget);

      mockBridge.dispose();
    });

    testWidgets('4. Scrubber slider triggers seeking', (tester) async {
      final mockBridge = MockPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(
        items: [sampleVideo],
        selectedItemId: sampleVideo.id,
      );

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => poolNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: PreviewView(),
            ),
          ),
        ),
      );

      await tester.pumpAndSettle();

      // Locate scrubber slider
      final sliderFinder = find.byKey(const Key('preview_progress_slider'));
      expect(sliderFinder, findsOneWidget);

      final slider = tester.widget<Slider>(sliderFinder);
      expect(slider.onChanged, isNotNull);

      // Seek to 50% (5 seconds)
      slider.onChanged!(0.5);
      await tester.pumpAndSettle();

      expect(mockBridge.lastSeekSeconds, closeTo(5.0, 0.01));
      expect(find.text('00:05 / 00:10'), findsOneWidget);

      mockBridge.dispose();
    });

    testWidgets('5. Deselecting media closes preview session', (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = MockPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [sampleVideo]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => poolNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  Expanded(child: MediaPoolView()),
                  Expanded(child: PreviewView()),
                ],
              ),
            ),
          ),
        ),
      );

      // 1. Select item
      await tester.tap(find.text('test_clip.mp4'));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);
      expect(poolNotifier.state.selectedItemId, sampleVideo.id);

      // 2. Tap item again to deselect
      await tester.tap(find.text('test_clip.mp4'));
      await tester.pumpAndSettle();
      expect(poolNotifier.state.selectedItemId, isNull);

      // Session should be closed and Texture removed
      expect(mockBridge.closeSessionCalled, isTrue);
      expect(find.byKey(const Key('preview_texture_widget')), findsNothing);
      expect(find.text('Nenhuma mídia selecionada'), findsOneWidget);

      mockBridge.dispose();
    });

    testWidgets('6. Error state displays error banner on decoder failure',
        (tester) async {
      final mockBridge = MockPreviewBridgeService();
      mockBridge.errorToThrow = Exception('Corrupted video header');

      final poolNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [sampleVideo]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => poolNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  Expanded(child: MediaPoolView()),
                  Expanded(child: PreviewView()),
                ],
              ),
            ),
          ),
        ),
      );

      // Select item to trigger error
      await tester.tap(find.text('test_clip.mp4'));
      await tester.pumpAndSettle();

      // Texture should not be mounted
      expect(find.byKey(const Key('preview_texture_widget')), findsNothing);

      // Error banner with message
      expect(find.byIcon(Icons.error_outline_rounded), findsOneWidget);
      expect(
        find.textContaining('Falha ao inicializar pré-visualização'),
        findsOneWidget,
      );

      mockBridge.dispose();
    });

    testWidgets('7. EditorScreen mounts PreviewView in center panel',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = MockPreviewBridgeService();

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => MediaPoolNotifier(
                  filePickerService: MockFilePickerService(),
                )),
          ],
          child: const MaterialApp(
            home: EditorScreen(),
          ),
        ),
      );

      // Verify PreviewView is rendered inside EditorScreen
      expect(find.byType(PreviewView), findsOneWidget);
      expect(find.text('PREVIEW'), findsOneWidget);
      expect(find.text('Nenhuma mídia selecionada'), findsOneWidget);

      mockBridge.dispose();
    });

    testWidgets('8. Switching selection directly between items closes previous session and loads new one',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final sampleVideo2 = MediaItem(
        id: uuid.v4obj(),
        filePath: '/videos/clip_two.mp4',
        fileName: 'clip_two.mp4',
        mediaType: MediaType.video,
        metadata: const MediaMetadata(
          durationPts: 1200,
          durationSeconds: 20.0,
          width: 1280,
          height: 720,
          fileSizeBytes: 1024 * 1024 * 8,
        ),
      );

      final mockBridge = MockPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: MockFilePickerService(),
      );
      poolNotifier.state =
          MediaPoolState(items: [sampleVideo, sampleVideo2]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            previewBridgeServiceProvider.overrideWithValue(mockBridge),
            mediaPoolProvider.overrideWith((ref) => poolNotifier),
          ],
          child: const MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  Expanded(child: MediaPoolView()),
                  Expanded(child: PreviewView()),
                ],
              ),
            ),
          ),
        ),
      );

      // Select first item
      await tester.tap(find.text('test_clip.mp4'));
      await tester.pumpAndSettle();
      expect(mockBridge.lastCreatedFilePath, sampleVideo.filePath);

      // Reset tracker and select second item directly
      mockBridge.closeSessionCalled = false;
      mockBridge.createSessionCalled = false;
      await tester.tap(find.text('clip_two.mp4'));
      await tester.pumpAndSettle();

      expect(mockBridge.closeSessionCalled, isTrue);
      expect(mockBridge.createSessionCalled, isTrue);
      expect(mockBridge.lastCreatedFilePath, sampleVideo2.filePath);

      mockBridge.dispose();
    });

    test('9. Reactive streams update frame PTS and playback state in PreviewNotifier',
        () async {
      final mockBridge = MockPreviewBridgeService();
      final notifier = PreviewNotifier(bridgeService: mockBridge);

      await notifier.loadMedia(sampleVideo);
      expect(notifier.state.isLoaded, isTrue);
      expect(notifier.state.isPlaying, isFalse);

      // Emit playback state update
      mockBridge.playbackStateController.add(const PlaybackState(
        sessionId: 'test-session-42',
        isPlaying: true,
        currentPts: 120,
        currentSeconds: 2.0,
        durationPts: 600,
        durationSeconds: 10.0,
      ));
      await Future<void>.delayed(Duration.zero);

      expect(notifier.state.isPlaying, isTrue);
      expect(notifier.state.currentPts, 120);
      expect(notifier.state.currentSeconds, 2.0);

      // Emit frame update
      final newFrame = BridgeFrame(
        width: 1920,
        height: 1080,
        pts: 180,
        durationPts: 1,
        rgbaBytes: Uint8List(0),
        rowStrideBytes: 1920 * 4,
        timestampSeconds: 3.0,
      );
      mockBridge.frameController.add(newFrame);
      await Future<void>.delayed(Duration.zero);

      expect(notifier.state.currentFrame, newFrame);
      expect(notifier.state.currentPts, 180);
      expect(notifier.state.currentSeconds, 3.0);

      notifier.dispose();
      mockBridge.dispose();
    });

    test('10. seekPts clamps PTS and updates position', () async {
      final mockBridge = MockPreviewBridgeService();
      final notifier = PreviewNotifier(bridgeService: mockBridge);

      await notifier.loadMedia(sampleVideo);
      expect(notifier.state.durationPts, 600);

      await notifier.seekPts(300);
      expect(mockBridge.lastSeekPts, 300);
      expect(notifier.state.currentPts, 300);
      expect(notifier.state.currentSeconds, 5.0);

      // Out of bounds clamped to durationPts
      await notifier.seekPts(9999);
      expect(mockBridge.lastSeekPts, 600);
      expect(notifier.state.currentPts, 600);

      notifier.dispose();
      mockBridge.dispose();
    });
  });
}
