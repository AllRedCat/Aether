import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_view.dart';
import 'package:aether_app/src/features/preview/preview_provider.dart';
import 'package:aether_app/src/features/preview/preview_service.dart';
import 'package:aether_app/src/features/preview/preview_view.dart';
import 'package:aether_app/src/services/file_picker_service.dart';

class AdversarialFilePickerService implements FilePickerService {
  @override
  Future<List<String>> pickMediaFiles() async => [];
}

/// Advanced mock bridge service with latency simulation, call counters,
/// and session lifecycle tracking for leak detection.
class AdversarialPreviewBridgeService implements PreviewBridgeService {
  Duration createSessionDelay = Duration.zero;
  Duration closeSessionDelay = Duration.zero;
  Duration playDelay = Duration.zero;
  Duration pauseDelay = Duration.zero;
  Duration seekDelay = Duration.zero;

  int createSessionCalls = 0;
  int closeSessionCalls = 0;
  int playCalls = 0;
  int pauseCalls = 0;
  int seekSecondsCalls = 0;
  int seekPtsCalls = 0;

  final Set<String> activeSessions = {};
  final List<String> closedSessions = [];
  final List<String> createdSessionPaths = [];

  Exception? createSessionError;
  Exception? playError;
  Exception? pauseError;
  Exception? seekError;

  final StreamController<BridgeFrame> frameController =
      StreamController<BridgeFrame>.broadcast();
  final StreamController<PlaybackState> playbackStateController =
      StreamController<PlaybackState>.broadcast();

  int nextTextureId = 100;
  int nextSessionCounter = 0;

  @override
  Future<PreviewSessionInfo> createPreviewSession({
    required String filePath,
  }) async {
    createSessionCalls++;
    createdSessionPaths.add(filePath);

    if (createSessionDelay > Duration.zero) {
      await Future<void>.delayed(createSessionDelay);
    }

    if (createSessionError != null) {
      throw createSessionError!;
    }

    nextSessionCounter++;
    final sessionId = 'adv-session-$nextSessionCounter';
    final textureId = nextTextureId++;
    activeSessions.add(sessionId);

    final durationSec = filePath.contains('video_b')
        ? 20.0
        : filePath.contains('video_c')
            ? 30.0
            : 10.0;
    final durationPts = (durationSec * 60).round();

    return PreviewSessionInfo(
      sessionId: sessionId,
      textureId: textureId,
      width: 1920,
      height: 1080,
      durationPts: durationPts,
      durationSeconds: durationSec,
      fps: 60.0,
      timebase: const Rational(num: 1, den: 60),
    );
  }

  @override
  Future<void> closePreviewSession({required String sessionId}) async {
    closeSessionCalls++;
    if (closeSessionDelay > Duration.zero) {
      await Future<void>.delayed(closeSessionDelay);
    }
    activeSessions.remove(sessionId);
    closedSessions.add(sessionId);
  }

  @override
  Future<void> previewPlay({required String sessionId}) async {
    playCalls++;
    if (playDelay > Duration.zero) {
      await Future<void>.delayed(playDelay);
    }
    if (playError != null) {
      throw playError!;
    }
    playbackStateController.add(PlaybackState(
      sessionId: sessionId,
      isPlaying: true,
      currentPts: 0,
      currentSeconds: 0.0,
      durationPts: 600,
      durationSeconds: 10.0,
    ));
  }

  @override
  Future<void> previewPause({required String sessionId}) async {
    pauseCalls++;
    if (pauseDelay > Duration.zero) {
      await Future<void>.delayed(pauseDelay);
    }
    if (pauseError != null) {
      throw pauseError!;
    }
    playbackStateController.add(PlaybackState(
      sessionId: sessionId,
      isPlaying: false,
      currentPts: 0,
      currentSeconds: 0.0,
      durationPts: 600,
      durationSeconds: 10.0,
    ));
  }

  @override
  Future<BridgeFrame?> previewSeekPts({
    required String sessionId,
    required int targetPts,
  }) async {
    seekPtsCalls++;
    if (seekDelay > Duration.zero) {
      await Future<void>.delayed(seekDelay);
    }
    if (seekError != null) {
      throw seekError!;
    }
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

  @override
  Future<BridgeFrame?> previewSeekSeconds({
    required String sessionId,
    required double seconds,
  }) async {
    seekSecondsCalls++;
    if (seekDelay > Duration.zero) {
      await Future<void>.delayed(seekDelay);
    }
    if (seekError != null) {
      throw seekError!;
    }
    return BridgeFrame(
      width: 1920,
      height: 1080,
      pts: (seconds * 60).round(),
      durationPts: 1,
      rgbaBytes: Uint8List(0),
      rowStrideBytes: 1920 * 4,
      timestampSeconds: seconds,
    );
  }

  @override
  Future<PlaybackState> getPreviewState({required String sessionId}) async {
    return const PlaybackState(
      sessionId: 'adv-session-query',
      isPlaying: false,
      currentPts: 0,
      currentSeconds: 0.0,
      durationPts: 600,
      durationSeconds: 10.0,
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

  final itemA = MediaItem(
    id: uuid.v4obj(),
    filePath: '/media/video_a.mp4',
    fileName: 'video_a.mp4',
    mediaType: MediaType.video,
    metadata: const MediaMetadata(
      durationPts: 600,
      durationSeconds: 10.0,
      width: 1920,
      height: 1080,
      fileSizeBytes: 1024 * 1024 * 2,
    ),
  );

  final itemB = MediaItem(
    id: uuid.v4obj(),
    filePath: '/media/video_b.mp4',
    fileName: 'video_b.mp4',
    mediaType: MediaType.video,
    metadata: const MediaMetadata(
      durationPts: 1200,
      durationSeconds: 20.0,
      width: 1280,
      height: 720,
      fileSizeBytes: 1024 * 1024 * 4,
    ),
  );

  final itemC = MediaItem(
    id: uuid.v4obj(),
    filePath: '/media/video_c.mp4',
    fileName: 'video_c.mp4',
    mediaType: MediaType.video,
    metadata: const MediaMetadata(
      durationPts: 1800,
      durationSeconds: 30.0,
      width: 3840,
      height: 2160,
      fileSizeBytes: 1024 * 1024 * 10,
    ),
  );

  group('Group 1: Single Selection & Listener Duplication Checks', () {
    testWidgets(
        '1A: Single item selection triggers exactly ONE createPreviewSession call (no duplicate listeners)',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [itemA]);

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

      // Tap on item A
      await tester.tap(find.text('video_a.mp4'));
      await tester.pumpAndSettle();

      // CRITICAL ASSERTION: Exactly 1 session created, not 2!
      expect(
        mockBridge.createSessionCalls,
        equals(1),
        reason:
            'Selection triggered duplicate createPreviewSession calls due to duplicate ref.listen in PreviewView and previewProvider',
      );
      expect(mockBridge.activeSessions.length, equals(1));
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);

      mockBridge.dispose();
    });
  });

  group('Group 2: Rapid Selection, Deselection & Race Conditions', () {
    testWidgets(
        '2A: Rapid selection followed immediately by deselection before session resolves',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      mockBridge.createSessionDelay = const Duration(milliseconds: 50);

      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [itemA]);

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

      // Select Item A
      await tester.tap(find.text('video_a.mp4'));
      await tester.pump(); // Start creation

      // Tap Item A again to deselect while creation is still in flight
      await tester.tap(find.text('video_a.mp4'));
      await tester.pump();

      // Advance clock past creation delay
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pumpAndSettle();

      // Since item was deselected, Texture must NOT be mounted,
      // and activeSessions must not have leaked!
      expect(poolNotifier.state.selectedItemId, isNull);
      expect(
        find.byKey(const Key('preview_texture_widget')),
        findsNothing,
        reason: 'Texture remained mounted despite item being deselected before session resolved',
      );
      expect(
        find.text('Nenhuma mídia selecionada'),
        findsOneWidget,
      );
      expect(
        mockBridge.activeSessions.length,
        equals(0),
        reason: 'Session leaked in activeSessions after rapid deselection',
      );

      mockBridge.dispose();
    });

    testWidgets(
        '2B: Rapid switching across A -> B -> C without waiting does not leak native sessions',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      mockBridge.createSessionDelay = const Duration(milliseconds: 20);

      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [itemA, itemB, itemC]);

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

      // Rapidly switch selection A -> B -> C
      await tester.tap(find.text('video_a.mp4'));
      await tester.pump(const Duration(milliseconds: 5));
      await tester.tap(find.text('video_b.mp4'));
      await tester.pump(const Duration(milliseconds: 5));
      await tester.tap(find.text('video_c.mp4'));

      // Advance until all settle
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pumpAndSettle();

      // Final state must reflect Item C
      expect(poolNotifier.state.selectedItemId, itemC.id);
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);

      // Only item C's session should remain active (all prior sessions closed)
      expect(
        mockBridge.activeSessions.length,
        lessThanOrEqualTo(1),
        reason: 'Prior sessions were leaked and not closed when switching rapidly',
      );

      mockBridge.dispose();
    });

    testWidgets(
        '2D: Out-of-order session completion: slower earlier session does not overwrite newer selected session',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      // Configure variable latency: video_a takes 80ms, video_b takes 10ms
      mockBridge.createSessionDelay = Duration.zero; // default

      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [itemA, itemB]);

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

      // Tap video A with 80ms simulated delay
      mockBridge.createSessionDelay = const Duration(milliseconds: 80);
      await tester.tap(find.text('video_a.mp4'));
      await tester.pump(const Duration(milliseconds: 10));

      // Now tap video B with 10ms delay
      mockBridge.createSessionDelay = const Duration(milliseconds: 10);
      await tester.tap(find.text('video_b.mp4'));
      await tester.pump(const Duration(milliseconds: 20)); // video B finishes!

      // At this point video B should be loaded
      expect(poolNotifier.state.selectedItemId, itemB.id);

      // Now advance clock so video A's delayed creation finishes (at t = 80ms)
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pumpAndSettle();

      // CRITICAL ASSERTION: The active preview session must be video B (duration 20s),
      // NOT overwritten by the obsolete video A (duration 10s)!
      expect(
        find.text('00:00 / 00:20'),
        findsOneWidget,
        reason:
            'Obsolete session for video_a finished after video_b and overwrote the active preview state with the wrong video!',
      );

      mockBridge.dispose();
    });

    testWidgets('2C: Removing selected item from Media Pool unmounts Texture cleanly',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [itemA, itemB]);

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

      // Select Item A
      await tester.tap(find.text('video_a.mp4'));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);

      // Remove Item A from pool
      poolNotifier.removeItem(itemA.id);
      await tester.pumpAndSettle();

      // Texture must be unmounted and empty state restored
      expect(find.byKey(const Key('preview_texture_widget')), findsNothing);
      expect(find.text('Nenhuma mídia selecionada'), findsOneWidget);
      expect(mockBridge.closeSessionCalls, greaterThanOrEqualTo(1));

      mockBridge.dispose();
    });
  });

  group('Group 3: Rapid Play/Pause Button Taps', () {
    testWidgets('3A: 20 rapid consecutive taps on Play/Pause button do not crash',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(
        items: [itemA],
        selectedItemId: itemA.id,
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

      final playPauseButton = find.byKey(const Key('preview_play_pause_button'));
      expect(playPauseButton, findsOneWidget);

      // Spam 20 rapid taps
      for (int i = 0; i < 20; i++) {
        await tester.tap(playPauseButton);
        await tester.pump(const Duration(milliseconds: 10));
      }
      await tester.pumpAndSettle();

      // The widget tree must still be healthy
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);
      expect(playPauseButton, findsOneWidget);

      // At least some play and pause calls happened
      expect(mockBridge.playCalls + mockBridge.pauseCalls, greaterThan(0));

      mockBridge.dispose();
    });

    testWidgets('3B: Play/pause with network/bridge latency handles state smoothly',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      mockBridge.playDelay = const Duration(milliseconds: 30);
      mockBridge.pauseDelay = const Duration(milliseconds: 30);

      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(
        items: [itemA],
        selectedItemId: itemA.id,
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

      final playPauseButton = find.byKey(const Key('preview_play_pause_button'));

      // Tap Play
      await tester.tap(playPauseButton);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 40));
      await tester.pumpAndSettle();

      expect(mockBridge.playCalls, 1);

      // Tap Pause
      await tester.tap(playPauseButton);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 40));
      await tester.pumpAndSettle();

      expect(mockBridge.pauseCalls, 1);

      mockBridge.dispose();
    });
  });

  group('Group 4: Scrubber Boundary Conditions & Stress', () {
    testWidgets('4A: Scrubber handles min (0.0) and max (1.0) exact boundaries',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(
        items: [itemA],
        selectedItemId: itemA.id,
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

      final slider = tester.widget<Slider>(
        find.byKey(const Key('preview_progress_slider')),
      );

      // Seek to 0.0
      slider.onChanged!(0.0);
      await tester.pumpAndSettle();
      expect(find.text('00:00 / 00:10'), findsOneWidget);

      // Seek to 1.0 (end: 10s)
      slider.onChanged!(1.0);
      await tester.pumpAndSettle();
      expect(find.text('00:10 / 00:10'), findsOneWidget);

      mockBridge.dispose();
    });

    testWidgets('4B: Rapid consecutive scrubber drags across 50 positions',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(
        items: [itemA],
        selectedItemId: itemA.id,
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

      final slider = tester.widget<Slider>(
        find.byKey(const Key('preview_progress_slider')),
      );

      // Rapidly fire 50 seek events
      for (int i = 0; i < 50; i++) {
        final fraction = (i % 10) / 10.0;
        slider.onChanged!(fraction);
        await tester.pump(const Duration(milliseconds: 2));
      }
      await tester.pumpAndSettle();

      expect(mockBridge.seekSecondsCalls, greaterThanOrEqualTo(50));
      expect(find.byKey(const Key('preview_timecode_text')), findsOneWidget);

      mockBridge.dispose();
    });

    test('4C: Extreme unit values to PreviewNotifier seek methods', () async {
      final mockBridge = AdversarialPreviewBridgeService();
      final notifier = PreviewNotifier(bridgeService: mockBridge);

      await notifier.loadMedia(itemA);
      expect(notifier.state.durationSeconds, 10.0);

      // Seek negative fraction or negative seconds
      await notifier.seekSeconds(-5.0);
      expect(notifier.state.currentSeconds, 0.0);

      // Seek beyond duration
      await notifier.seekSeconds(999.0);
      expect(notifier.state.currentSeconds, 10.0);

      // Seek PTS negative
      await notifier.seekPts(-100);
      expect(notifier.state.currentPts, 0);

      // Seek PTS beyond durationPts (600)
      await notifier.seekPts(5000);
      expect(notifier.state.currentPts, 600);

      notifier.dispose();
      mockBridge.dispose();
    });

    test('4D: Zero-duration media edge cases do not trigger NaN or zero division',
        () {
      const zeroDurationState = PreviewState(
        durationSeconds: 0.0,
        currentSeconds: 0.0,
        durationPts: 0,
        currentPts: 0,
      );

      expect(zeroDurationState.progressFraction, 0.0);
      expect(zeroDurationState.progressFraction.isNaN, isFalse);
      expect(zeroDurationState.progressFraction.isInfinite, isFalse);
    });
  });

  group('Group 5: Error Handling & Stream Robustness', () {
    testWidgets('5A: Error banner displays and recovers when valid item is selected',
        (tester) async {
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final mockBridge = AdversarialPreviewBridgeService();
      mockBridge.createSessionError = Exception('Hardware decoding init failed');

      final poolNotifier = MediaPoolNotifier(
        filePickerService: AdversarialFilePickerService(),
      );
      poolNotifier.state = MediaPoolState(items: [itemA, itemB]);

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

      // Select item A -> fails
      await tester.tap(find.text('video_a.mp4'));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('preview_texture_widget')), findsNothing);
      expect(find.byIcon(Icons.error_outline_rounded), findsOneWidget);
      expect(
        find.textContaining('Hardware decoding init failed'),
        findsOneWidget,
      );

      // Now clear error condition and select item B
      mockBridge.createSessionError = null;
      await tester.tap(find.text('video_b.mp4'));
      await tester.pumpAndSettle();

      // Error banner must be gone, Texture mounted
      expect(find.byIcon(Icons.error_outline_rounded), findsNothing);
      expect(find.byKey(const Key('preview_texture_widget')), findsOneWidget);
      expect(find.text('00:00 / 00:20'), findsOneWidget);

      mockBridge.dispose();
    });

    test('5B: Stream error emission does not crash PreviewNotifier', () async {
      final mockBridge = AdversarialPreviewBridgeService();
      final notifier = PreviewNotifier(bridgeService: mockBridge);

      await notifier.loadMedia(itemA);
      expect(notifier.state.isLoaded, isTrue);

      // Emit error into playback stream
      mockBridge.playbackStateController.addError('Decoder dropped sync');
      await Future<void>.delayed(Duration.zero);

      // Emit error into frame stream
      mockBridge.frameController.addError('Frame decoding dropped');
      await Future<void>.delayed(Duration.zero);

      // Notifier state must still be valid and loaded
      expect(notifier.state.isLoaded, isTrue);

      notifier.dispose();
      mockBridge.dispose();
    });
  });
}
