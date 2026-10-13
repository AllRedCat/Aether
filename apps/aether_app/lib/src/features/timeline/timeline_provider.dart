import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../bridge/api.dart';
import '../../core/providers/project_provider.dart';
import '../media_pool/media_pool_provider.dart';

/// Immutable query key for requesting pre-computed audio waveform peaks
/// at a specific Level of Detail (`lod`) from the Rust `.peak` sidecar cache.
@immutable
class WaveformQuery {
  final String assetId;
  final int lod;

  const WaveformQuery({
    required this.assetId,
    this.lod = 0,
  });

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is WaveformQuery &&
          runtimeType == other.runtimeType &&
          assetId == other.assetId &&
          lod == other.lod;

  @override
  int get hashCode => assetId.hashCode ^ lod.hashCode;
}

typedef FetchAudioWaveformFn = Future<List<AudioPeak>> Function({
  required String assetId,
  required int lod,
});

/// Injectable provider for fetching waveform peaks across FFI (`getAudioWaveform`).
/// Allows overriding in headless widget tests without requiring the native dylib.
final audioWaveformFetcherProvider = Provider<FetchAudioWaveformFn>((ref) {
  return getAudioWaveform;
});

/// Reactive provider that loads `List<AudioPeak>` (`min`, `max`, `rms`) for a
/// clip's `assetId` and `lod` directly from the binary `.peak` cache via FFI.
/// Automatically re-evaluates when background indexing for `assetId` completes.
final clipWaveformProvider =
    FutureProvider.family<List<AudioPeak>, WaveformQuery>((ref, query) async {
  if (query.assetId.isEmpty) return const <AudioPeak>[];

  // Re-fetch automatically when background indexing transitions stage (e.g. to Completed)
  ref.watch(
    mediaPoolProvider.select(
      (s) => s.indexingProgress[query.assetId]?.stage,
    ),
  );

  final fetchFn = ref.watch(audioWaveformFetcherProvider);
  try {
    return await fetchFn(assetId: query.assetId, lod: query.lod);
  } catch (_) {
    return const <AudioPeak>[];
  }
});

/// Immutable state representation for the Aether Timeline.
@immutable
class TimelineState {
  final Timeline? timeline;
  final int totalClipCount;
  final int durationPts;
  final List<Track> tracks;
  final bool isLoading;
  final String? errorMessage;

  const TimelineState({
    this.timeline,
    this.totalClipCount = 0,
    this.durationPts = 0,
    this.tracks = const [],
    this.isLoading = false,
    this.errorMessage,
  });

  const TimelineState.initial()
      : timeline = null,
        totalClipCount = 0,
        durationPts = 0,
        tracks = const [],
        isLoading = false,
        errorMessage = null;

  factory TimelineState.fromTimeline(
    Timeline timeline, {
    bool isLoading = false,
  }) {
    final count = timeline.tracks.fold<int>(
      0,
      (acc, track) => acc + track.clips.length,
    );
    return TimelineState(
      timeline: timeline,
      totalClipCount: count,
      durationPts: timeline.durationPts,
      tracks: timeline.tracks,
      isLoading: isLoading,
      errorMessage: null,
    );
  }

  TimelineState copyWith({
    Timeline? timeline,
    int? totalClipCount,
    int? durationPts,
    List<Track>? tracks,
    bool? isLoading,
    String? errorMessage,
    bool clearError = false,
  }) {
    return TimelineState(
      timeline: timeline ?? this.timeline,
      totalClipCount: totalClipCount ?? this.totalClipCount,
      durationPts: durationPts ?? this.durationPts,
      tracks: tracks ?? this.tracks,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TimelineState &&
          runtimeType == other.runtimeType &&
          timeline == other.timeline &&
          totalClipCount == other.totalClipCount &&
          durationPts == other.durationPts &&
          listEquals(tracks, other.tracks) &&
          isLoading == other.isLoading &&
          errorMessage == other.errorMessage;

  @override
  int get hashCode =>
      timeline.hashCode ^
      totalClipCount.hashCode ^
      durationPts.hashCode ^
      tracks.hashCode ^
      isLoading.hashCode ^
      errorMessage.hashCode;
}

/// StateNotifier coordinating Timeline operations between UI and Rust FFI.
class TimelineNotifier extends StateNotifier<TimelineState> {
  final Future<Timeline> Function()? _createTimelineFn;
  final Future<Timeline> Function({
    required Timeline timeline,
    required UuidValue trackId,
    required UuidValue sourceId,
    required int sourceIn,
    required int sourceOut,
    required int timelineIn,
  })? _addClipToTrackFn;

  TimelineNotifier({
    Future<Timeline> Function()? createTimelineFn,
    Future<Timeline> Function({
      required Timeline timeline,
      required UuidValue trackId,
      required UuidValue sourceId,
      required int sourceIn,
      required int sourceOut,
      required int timelineIn,
    })? addClipToTrackFn,
    bool autoInit = true,
    Timeline? initialTimeline,
  })  : _createTimelineFn = createTimelineFn,
        _addClipToTrackFn = addClipToTrackFn,
        super(initialTimeline != null
            ? TimelineState.fromTimeline(initialTimeline)
            : const TimelineState.initial()) {
    if (autoInit && initialTimeline == null) {
      initTimeline();
    }
  }

  /// Initializes the default timeline from the native Rust core.
  Future<void> initTimeline() async {
    state = state.copyWith(isLoading: true, clearError: true);
    try {
      final createFn = _createTimelineFn ?? createTimeline;
      final timeline = await createFn();
      state = TimelineState.fromTimeline(timeline);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Failed to initialize timeline: $e',
      );
    }
  }

  /// Adds a new clip to a track and recalculates duration in Rust.
  Future<void> addClip({
    UuidValue? trackId,
    UuidValue? sourceId,
    int sourceIn = 0,
    int sourceOut = 60,
    int? timelineIn,
  }) async {
    if (state.isLoading) return;

    var currentTimeline = state.timeline;
    if (currentTimeline == null) {
      await initTimeline();
      currentTimeline = state.timeline;
      if (currentTimeline == null) {
        state = state.copyWith(
          isLoading: false,
          errorMessage: 'No active timeline available',
        );
        return;
      }
    }

    if (currentTimeline.tracks.isEmpty) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'No track available to add clip',
      );
      return;
    }

    state = state.copyWith(isLoading: true, clearError: true);

    try {
      final targetTrack = trackId != null
          ? currentTimeline.tracks.firstWhere(
              (t) => t.id == trackId,
              orElse: () => currentTimeline!.tracks.first,
            )
          : currentTimeline.tracks.first;

      final clipSourceId = sourceId ?? const Uuid().v4obj();
      final insertionPts = timelineIn ?? currentTimeline.durationPts;

      final addFn = _addClipToTrackFn ?? addClipToTrack;
      final updatedTimeline = await addFn(
        timeline: currentTimeline,
        trackId: targetTrack.id,
        sourceId: clipSourceId,
        sourceIn: sourceIn,
        sourceOut: sourceOut,
        timelineIn: insertionPts,
      );

      state = TimelineState.fromTimeline(updatedTimeline);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Failed to add clip: $e',
      );
    }
  }
}

/// Riverpod StateNotifierProvider for TimelineState.
final timelineProvider =
    StateNotifierProvider<TimelineNotifier, TimelineState>((ref) {
  final project = ref.watch(currentProjectProvider);
  return TimelineNotifier(initialTimeline: project?.timeline);
});
