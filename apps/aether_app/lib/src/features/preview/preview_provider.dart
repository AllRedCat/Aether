import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/api.dart';
import '../media_pool/media_pool_provider.dart';
import 'preview_service.dart';

/// Immutable state representation for the video preview and playback engine.
@immutable
class PreviewState {
  final PreviewSessionInfo? session;
  final BridgeFrame? currentFrame;
  final MediaItem? currentMedia;
  final bool isPlaying;
  final int currentPts;
  final double currentSeconds;
  final int durationPts;
  final double durationSeconds;
  final bool isLoading;
  final String? errorMessage;
  final int? _customTextureId;

  const PreviewState({
    this.session,
    this.currentFrame,
    this.currentMedia,
    this.isPlaying = false,
    this.currentPts = 0,
    this.currentSeconds = 0.0,
    this.durationPts = 0,
    this.durationSeconds = 0.0,
    this.isLoading = false,
    this.errorMessage,
    int? textureId,
  }) : _customTextureId = textureId;

  const PreviewState.initial()
      : session = null,
        currentFrame = null,
        currentMedia = null,
        isPlaying = false,
        currentPts = 0,
        currentSeconds = 0.0,
        durationPts = 0,
        durationSeconds = 0.0,
        isLoading = false,
        errorMessage = null,
        _customTextureId = null;

  /// The active GPU texture ID registered with Flutter's Texture widget.
  int get textureId => _customTextureId ?? session?.textureId ?? 0;

  /// Progress fraction in range [0.0, 1.0] for the scrubber slider.
  double get progressFraction {
    if (durationSeconds <= 0.0) return 0.0;
    return (currentSeconds / durationSeconds).clamp(0.0, 1.0);
  }

  /// Aspect ratio (width / height) defaulting to 16:9 if unknown.
  double get aspectRatio {
    final w = session?.width ?? 0;
    final h = session?.height ?? 0;
    if (w > 0 && h > 0) return w / h;
    return 16.0 / 9.0;
  }

  /// Whether a valid session exists and is ready for display.
  bool get isLoaded =>
      session != null && !isLoading && errorMessage == null;

  /// Whether media is currently selected/associated.
  bool get hasMedia => currentMedia != null || session != null;

  PreviewState copyWith({
    PreviewSessionInfo? session,
    bool clearSession = false,
    BridgeFrame? currentFrame,
    bool clearFrame = false,
    MediaItem? currentMedia,
    bool clearMedia = false,
    bool? isPlaying,
    int? currentPts,
    double? currentSeconds,
    int? durationPts,
    double? durationSeconds,
    bool? isLoading,
    String? errorMessage,
    bool clearError = false,
    int? textureId,
    bool clearTextureId = false,
  }) {
    return PreviewState(
      session: clearSession ? null : (session ?? this.session),
      currentFrame: clearFrame ? null : (currentFrame ?? this.currentFrame),
      currentMedia: clearMedia ? null : (currentMedia ?? this.currentMedia),
      isPlaying: isPlaying ?? this.isPlaying,
      currentPts: currentPts ?? this.currentPts,
      currentSeconds: currentSeconds ?? this.currentSeconds,
      durationPts: durationPts ?? this.durationPts,
      durationSeconds: durationSeconds ?? this.durationSeconds,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
      textureId: clearTextureId
          ? null
          : (textureId ?? _customTextureId),
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PreviewState &&
          runtimeType == other.runtimeType &&
          session == other.session &&
          currentFrame == other.currentFrame &&
          currentMedia == other.currentMedia &&
          isPlaying == other.isPlaying &&
          currentPts == other.currentPts &&
          currentSeconds == other.currentSeconds &&
          durationPts == other.durationPts &&
          durationSeconds == other.durationSeconds &&
          isLoading == other.isLoading &&
          errorMessage == other.errorMessage &&
          textureId == other.textureId;

  @override
  int get hashCode =>
      session.hashCode ^
      currentFrame.hashCode ^
      currentMedia.hashCode ^
      isPlaying.hashCode ^
      currentPts.hashCode ^
      currentSeconds.hashCode ^
      durationPts.hashCode ^
      durationSeconds.hashCode ^
      isLoading.hashCode ^
      errorMessage.hashCode ^
      textureId.hashCode;
}

/// StateNotifier managing preview lifecycle, frame updates, and playback controls.
class PreviewNotifier extends StateNotifier<PreviewState> {
  final PreviewBridgeService _bridgeService;
  StreamSubscription<BridgeFrame>? _frameSub;
  StreamSubscription<PlaybackState>? _playbackSub;
  int _sessionGeneration = 0;

  PreviewNotifier({required PreviewBridgeService bridgeService})
      : _bridgeService = bridgeService,
        super(const PreviewState.initial());

  /// Automatically loads a media item selected from Media Pool.
  Future<void> loadMedia(MediaItem media) async {
    // If the same media is already loaded or loading, no-op
    if (state.currentMedia?.id == media.id &&
        (state.session != null || state.isLoading)) {
      return;
    }
    await createSession(filePath: media.filePath, media: media);
  }

  /// Opens a playback session for the given media file path.
  Future<void> createSession({
    required String filePath,
    MediaItem? media,
  }) async {
    final generation = ++_sessionGeneration;
    final previousSessionId = state.session?.sessionId;

    state = state.copyWith(
      isLoading: true,
      currentMedia: media,
      clearError: true,
      clearSession: true,
      currentPts: 0,
      currentSeconds: 0.0,
      isPlaying: false,
    );

    // Teardown previous session before initializing new one
    await _cancelSubscriptionsAndCloseSession(sessionId: previousSessionId);

    if (!mounted || _sessionGeneration != generation) return;

    try {
      final sessionInfo =
          await _bridgeService.createPreviewSession(filePath: filePath);

      if (!mounted || _sessionGeneration != generation) {
        try {
          await _bridgeService.closePreviewSession(
            sessionId: sessionInfo.sessionId,
          );
        } catch (_) {
          // Safe to ignore during teardown
        }
        return;
      }

      _subscribeToSessionStreams(sessionInfo.sessionId);

      state = state.copyWith(
        session: sessionInfo,
        isLoading: false,
        currentPts: 0,
        currentSeconds: 0.0,
        durationPts: sessionInfo.durationPts,
        durationSeconds: sessionInfo.durationSeconds,
        isPlaying: false,
        clearError: true,
      );
    } catch (e) {
      if (!mounted || _sessionGeneration != generation) return;
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Falha ao inicializar pré-visualização: $e',
        clearSession: true,
      );
    }
  }

  void _subscribeToSessionStreams(String sessionId) {
    _frameSub?.cancel();
    _frameSub = _bridgeService
        .subscribePreviewFrames(sessionId: sessionId)
        .listen(
      (frame) {
        if (!mounted) return;
        state = state.copyWith(
          currentFrame: frame,
          currentPts: frame.pts,
          currentSeconds: frame.timestampSeconds,
        );
      },
      onError: (e) {
        // Stream errors do not crash UI
      },
    );

    _playbackSub?.cancel();
    _playbackSub = _bridgeService
        .subscribePlaybackState(sessionId: sessionId)
        .listen(
      (pbState) {
        if (!mounted) return;
        state = state.copyWith(
          isPlaying: pbState.isPlaying,
          currentPts: pbState.currentPts,
          currentSeconds: pbState.currentSeconds,
          durationPts: pbState.durationPts > 0
              ? pbState.durationPts
              : state.durationPts,
          durationSeconds: pbState.durationSeconds > 0
              ? pbState.durationSeconds
              : state.durationSeconds,
        );
      },
      onError: (e) {
        // Stream errors do not crash UI
      },
    );
  }

  /// Starts playback.
  Future<void> play() async {
    final sessionId = state.session?.sessionId;
    if (sessionId == null) return;

    try {
      await _bridgeService.previewPlay(sessionId: sessionId);
      state = state.copyWith(isPlaying: true);
    } catch (e) {
      state = state.copyWith(errorMessage: 'Erro ao iniciar reprodução: $e');
    }
  }

  /// Pauses playback.
  Future<void> pause() async {
    final sessionId = state.session?.sessionId;
    if (sessionId == null) return;

    try {
      await _bridgeService.previewPause(sessionId: sessionId);
      state = state.copyWith(isPlaying: false);
    } catch (e) {
      state = state.copyWith(errorMessage: 'Erro ao pausar reprodução: $e');
    }
  }

  /// Toggles between play and pause.
  Future<void> togglePlayPause() async {
    if (state.isPlaying) {
      await pause();
    } else {
      await play();
    }
  }

  /// Seeks to a position. If `value` is in range [0.0, 1.0], it is treated as
  /// a fraction of total duration. If `value > 1.0`, it is treated directly as seconds.
  Future<void> seek(double value) async {
    final duration = state.durationSeconds;
    final double targetSeconds;
    if (duration > 0.0 && value <= 1.0 && value >= 0.0) {
      targetSeconds = duration * value;
    } else {
      targetSeconds = value;
    }
    await seekSeconds(targetSeconds);
  }

  /// Seeks directly to a specified timestamp in seconds.
  Future<void> seekSeconds(double seconds) async {
    final sessionId = state.session?.sessionId;
    if (sessionId == null) return;

    final clamped = state.durationSeconds > 0.0
        ? seconds.clamp(0.0, state.durationSeconds)
        : seconds;

    state = state.copyWith(currentSeconds: clamped);

    try {
      final frame = await _bridgeService.previewSeekSeconds(
        sessionId: sessionId,
        seconds: clamped,
      );
      if (frame != null) {
        state = state.copyWith(
          currentFrame: frame,
          currentPts: frame.pts,
          currentSeconds: frame.timestampSeconds,
        );
      }
    } catch (e) {
      state = state.copyWith(errorMessage: 'Erro ao buscar posição: $e');
    }
  }

  /// Seeks to a specific PTS.
  Future<void> seekPts(int targetPts) async {
    final sessionId = state.session?.sessionId;
    if (sessionId == null) return;

    final clamped = state.durationPts > 0
        ? targetPts.clamp(0, state.durationPts)
        : targetPts;

    state = state.copyWith(currentPts: clamped);

    try {
      final frame = await _bridgeService.previewSeekPts(
        sessionId: sessionId,
        targetPts: clamped,
      );
      if (frame != null) {
        state = state.copyWith(
          currentFrame: frame,
          currentPts: frame.pts,
          currentSeconds: frame.timestampSeconds,
        );
      }
    } catch (e) {
      state = state.copyWith(errorMessage: 'Erro ao buscar PTS: $e');
    }
  }

  /// Closes the active session and unloads media.
  Future<void> closeSession() async {
    ++_sessionGeneration;
    await _cancelSubscriptionsAndCloseSession();
    state = const PreviewState.initial();
  }

  Future<void> _cancelSubscriptionsAndCloseSession({String? sessionId}) async {
    final targetSessionId = sessionId ?? state.session?.sessionId;
    _frameSub?.cancel();
    _frameSub = null;
    _playbackSub?.cancel();
    _playbackSub = null;

    if (!mounted) return;
    if (targetSessionId != null) {
      try {
        await _bridgeService.closePreviewSession(sessionId: targetSessionId);
      } catch (_) {
        // Safe to ignore during teardown
      }
    }
  }

  @override
  void dispose() {
    ++_sessionGeneration;
    final sessionId = state.session?.sessionId;
    _frameSub?.cancel();
    _frameSub = null;
    _playbackSub?.cancel();
    _playbackSub = null;
    if (sessionId != null) {
      _bridgeService.closePreviewSession(sessionId: sessionId).ignore();
    }
    super.dispose();
  }
}

/// Riverpod Provider exposing [PreviewNotifier] and observing [mediaPoolProvider].
final previewProvider =
    StateNotifierProvider<PreviewNotifier, PreviewState>((ref) {
  final bridgeService = ref.watch(previewBridgeServiceProvider);
  final notifier = PreviewNotifier(bridgeService: bridgeService);

  // Automatically react to MediaPool selection changes:
  ref.listen<MediaItem?>(
    mediaPoolProvider.select((state) => state.selectedItem),
    (previous, current) {
      if (current != null) {
        notifier.loadMedia(current);
      } else {
        notifier.closeSession();
      }
    },
  );

  // Check if an item is already selected at provider initialization time:
  final currentSelection = ref.read(mediaPoolProvider).selectedItem;
  if (currentSelection != null) {
    notifier.loadMedia(currentSelection);
  }

  return notifier;
});
