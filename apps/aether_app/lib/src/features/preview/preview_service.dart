import 'dart:async';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/api.dart' as bridge;

/// Abstract bridge service contract for preview session lifecycle, playback controls,
/// and video frame / playback state streaming.
abstract class PreviewBridgeService {
  /// Opens a media file, initializes a native decoder session, and returns session info
  /// including the GPU texture ID.
  Future<bridge.PreviewSessionInfo> createPreviewSession({
    required String filePath,
  });

  /// Closes an active preview session and frees allocated native resources.
  Future<void> closePreviewSession({required String sessionId});

  /// Starts sequential playback at native frame rate.
  Future<void> previewPlay({required String sessionId});

  /// Pauses active playback.
  Future<void> previewPause({required String sessionId});

  /// Seeks to a specific presentation timestamp (PTS).
  Future<bridge.BridgeFrame?> previewSeekPts({
    required String sessionId,
    required int targetPts,
  });

  /// Seeks to a specific fractional timestamp in seconds.
  Future<bridge.BridgeFrame?> previewSeekSeconds({
    required String sessionId,
    required double seconds,
  });

  /// Retrieves the instantaneous playback state.
  Future<bridge.PlaybackState> getPreviewState({required String sessionId});

  /// Subscribes to decoded video frame stream.
  Stream<bridge.BridgeFrame> subscribePreviewFrames({
    required String sessionId,
  });

  /// Subscribes to playback state ticks (play status, current PTS, seconds).
  Stream<bridge.PlaybackState> subscribePlaybackState({
    required String sessionId,
  });

  /// Decodes and extracts a single frame without persistent session state.
  Future<bridge.BridgeFrame> extractSingleFrame({
    required String filePath,
    required int targetPts,
  });
}

/// Default implementation delegating directly to `aether_bridge` FFI calls.
class DefaultPreviewBridgeService implements PreviewBridgeService {
  const DefaultPreviewBridgeService();

  @override
  Future<bridge.PreviewSessionInfo> createPreviewSession({
    required String filePath,
  }) {
    return bridge.createPreviewSession(filePath: filePath);
  }

  @override
  Future<void> closePreviewSession({required String sessionId}) {
    return bridge.closePreviewSession(sessionId: sessionId);
  }

  @override
  Future<void> previewPlay({required String sessionId}) {
    return bridge.previewPlay(sessionId: sessionId);
  }

  @override
  Future<void> previewPause({required String sessionId}) {
    return bridge.previewPause(sessionId: sessionId);
  }

  @override
  Future<bridge.BridgeFrame?> previewSeekPts({
    required String sessionId,
    required int targetPts,
  }) {
    return bridge.previewSeekPts(sessionId: sessionId, targetPts: targetPts);
  }

  @override
  Future<bridge.BridgeFrame?> previewSeekSeconds({
    required String sessionId,
    required double seconds,
  }) {
    return bridge.previewSeekSeconds(
      sessionId: sessionId,
      seconds: seconds,
    );
  }

  @override
  Future<bridge.PlaybackState> getPreviewState({required String sessionId}) {
    return bridge.getPreviewState(sessionId: sessionId);
  }

  @override
  Stream<bridge.BridgeFrame> subscribePreviewFrames({
    required String sessionId,
  }) {
    return bridge.subscribePreviewFrames(sessionId: sessionId);
  }

  @override
  Stream<bridge.PlaybackState> subscribePlaybackState({
    required String sessionId,
  }) {
    return bridge.subscribePlaybackState(sessionId: sessionId);
  }

  @override
  Future<bridge.BridgeFrame> extractSingleFrame({
    required String filePath,
    required int targetPts,
  }) {
    return bridge.extractSingleFrame(
      filePath: filePath,
      targetPts: targetPts,
    );
  }
}

/// Riverpod provider for [PreviewBridgeService].
final previewBridgeServiceProvider = Provider<PreviewBridgeService>((ref) {
  return const DefaultPreviewBridgeService();
});
