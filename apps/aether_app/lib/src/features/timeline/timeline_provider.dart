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
  final int channel;
  final int lod;

  const WaveformQuery({
    required this.assetId,
    this.channel = 0,
    this.lod = 0,
  });

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is WaveformQuery &&
          runtimeType == other.runtimeType &&
          assetId == other.assetId &&
          channel == other.channel &&
          lod == other.lod;

  @override
  int get hashCode => assetId.hashCode ^ channel.hashCode ^ lod.hashCode;
}

/// Convenience alias to bridge `AudioPeakData`
typedef AudioPeak = AudioPeakData;

typedef FetchAudioWaveformFn = Future<List<AudioPeak>> Function({
  required String assetId,
  required int channel,
  required int lod,
});

/// Injectable provider for fetching waveform peaks across FFI (`getAudioWaveform`).
/// Allows overriding in headless widget tests without requiring the native dylib.
final audioWaveformFetcherProvider = Provider<FetchAudioWaveformFn>((ref) {
  return getAudioWaveform;
});

/// Timeline zoom factor represented as pixels per second of playback.
final timelineZoomProvider = StateProvider<double>((ref) => 50.0);

/// Computes the optimal audio pyramid Level-of-Detail (LOD 0..3)
/// given a timeline zoom factor in `pixels_per_second` and sample rate.
int computeAudioLodForZoom(double pixelsPerSecond, {int sampleRate = 48000}) {
  if (pixelsPerSecond <= 0) return 0;
  final samplesPerPixel = sampleRate / pixelsPerSecond;
  if (samplesPerPixel >= 16384) {
    return 3;
  } else if (samplesPerPixel >= 4096) {
    return 2;
  } else if (samplesPerPixel >= 1024) {
    return 1;
  } else {
    return 0;
  }
}

/// Reactive provider that loads `List<AudioPeak>` (`min`, `max`, `rms`) for a
/// clip's `assetId` and `lod` directly from the binary `.peak` cache via FFI.
/// Automatically re-evaluates when background indexing or analysis for `assetId` completes.
final clipWaveformProvider =
    FutureProvider.family<List<AudioPeak>, WaveformQuery>((ref, query) async {
  if (query.assetId.isEmpty) return const <AudioPeak>[];

  // Re-fetch automatically when background indexing transitions stage (e.g. to Completed)
  ref.watch(
    mediaPoolProvider.select(
      (s) => s.indexingProgress[query.assetId]?.stage,
    ),
  );
  ref.watch(
    mediaPoolProvider.select(
      (s) => s.analysisProgress[query.assetId]?.step,
    ),
  );
  ref.watch(
    mediaPoolProvider.select(
      (s) => s.analysisProgress[query.assetId]?.progress == 1.0,
    ),
  );

  final fetchFn = ref.watch(audioWaveformFetcherProvider);
  try {
    return await fetchFn(
      assetId: query.assetId,
      channel: query.channel,
      lod: query.lod,
    );
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
  final double pixelsPerSecond;

  const TimelineState({
    this.timeline,
    this.totalClipCount = 0,
    this.durationPts = 0,
    this.tracks = const [],
    this.isLoading = false,
    this.errorMessage,
    this.pixelsPerSecond = 50.0,
  });

  const TimelineState.initial()
      : timeline = null,
        totalClipCount = 0,
        durationPts = 0,
        tracks = const [],
        isLoading = false,
        errorMessage = null,
        pixelsPerSecond = 50.0;

  factory TimelineState.fromTimeline(
    Timeline timeline, {
    bool isLoading = false,
    double pixelsPerSecond = 50.0,
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
      pixelsPerSecond: pixelsPerSecond,
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
    double? pixelsPerSecond,
  }) {
    return TimelineState(
      timeline: timeline ?? this.timeline,
      totalClipCount: totalClipCount ?? this.totalClipCount,
      durationPts: durationPts ?? this.durationPts,
      tracks: tracks ?? this.tracks,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
      pixelsPerSecond: pixelsPerSecond ?? this.pixelsPerSecond,
    );
  }

  /// Returns the number of clips across all tracks referencing [sourceId].
  int countClipsForSource(UuidValue sourceId) {
    return tracks.fold<int>(
      0,
      (acc, track) =>
          acc + track.clips.where((c) => c.sourceId == sourceId).length,
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
          errorMessage == other.errorMessage &&
          pixelsPerSecond == other.pixelsPerSecond;

  @override
  int get hashCode =>
      timeline.hashCode ^
      totalClipCount.hashCode ^
      durationPts.hashCode ^
      tracks.hashCode ^
      isLoading.hashCode ^
      errorMessage.hashCode ^
      pixelsPerSecond.hashCode;
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
  final Future<Timeline> Function({
    required Timeline timeline,
    required UuidValue sourceId,
  })? _removeClipsBySourceFn;

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
    Future<Timeline> Function({
      required Timeline timeline,
      required UuidValue sourceId,
    })? removeClipsBySourceFn,
    bool autoInit = true,
    Timeline? initialTimeline,
  })  : _createTimelineFn = createTimelineFn,
        _addClipToTrackFn = addClipToTrackFn,
        _removeClipsBySourceFn = removeClipsBySourceFn,
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

  /// Ensures a track of [kind] (optionally matching [preferredTrackId]) exists in the timeline.
  Future<Track?> ensureTrackForKind(
    TrackKind kind, {
    UuidValue? preferredTrackId,
  }) async {
    var currentTimeline = state.timeline;
    if (currentTimeline == null) {
      await initTimeline();
      currentTimeline = state.timeline;
    }
    if (currentTimeline == null) {
      if (state.tracks.isNotEmpty) {
        try {
          return state.tracks.firstWhere((t) => t.kind == kind);
        } catch (_) {
          return state.tracks.first;
        }
      }
      return null;
    }

    if (preferredTrackId != null) {
      for (final t in currentTimeline.tracks) {
        if (t.id == preferredTrackId) return t;
      }
      // If an empty track of the same kind exists, adopt preferredTrackId so activeTimeline & timelineProvider match
      final emptyIdx = currentTimeline.tracks.indexWhere(
        (t) => t.kind == kind && t.clips.isEmpty,
      );
      if (emptyIdx >= 0) {
        final updatedTracks = List<Track>.from(currentTimeline.tracks);
        updatedTracks[emptyIdx] = Track(
          id: preferredTrackId,
          kind: kind,
          clips: const [],
        );
        final updatedTl = Timeline(
          id: currentTimeline.id,
          timebase: currentTimeline.timebase,
          durationPts: currentTimeline.durationPts,
          tracks: updatedTracks,
        );
        state = TimelineState.fromTimeline(updatedTl);
        return updatedTracks[emptyIdx];
      }
      final newTrack = Track(
        id: preferredTrackId,
        kind: kind,
        clips: const [],
      );
      final updatedTracks = [...currentTimeline.tracks, newTrack];
      final updatedTl = Timeline(
        id: currentTimeline.id,
        timebase: currentTimeline.timebase,
        durationPts: currentTimeline.durationPts,
        tracks: updatedTracks,
      );
      state = TimelineState.fromTimeline(updatedTl);
      return newTrack;
    }

    for (final t in currentTimeline.tracks) {
      if (t.kind == kind) return t;
    }

    final newTrack = Track(
      id: const Uuid().v4obj(),
      kind: kind,
      clips: const [],
    );
    final updatedTracks = [...currentTimeline.tracks, newTrack];
    final updatedTl = Timeline(
      id: currentTimeline.id,
      timebase: currentTimeline.timebase,
      durationPts: currentTimeline.durationPts,
      tracks: updatedTracks,
    );
    state = TimelineState.fromTimeline(updatedTl);
    return newTrack;
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

  /// Removes all clips referencing [sourceId] across all tracks and recalculates duration.
  /// Returns the number of removed clips.
  Future<int> removeClipsBySource(UuidValue sourceId) async {
    final removedCount = state.countClipsForSource(sourceId);
    if (removedCount == 0) return 0;

    final currentTimeline = state.timeline;
    if (currentTimeline != null && _removeClipsBySourceFn != null) {
      try {
        final updated = await _removeClipsBySourceFn(
          timeline: currentTimeline,
          sourceId: sourceId,
        );
        state = TimelineState.fromTimeline(
          updated,
          pixelsPerSecond: state.pixelsPerSecond,
        );
        return removedCount;
      } catch (_) {
        // Fallback to pure Dart calculation below if FFI dylib is not initialized
      }
    }

    final updatedTracks = state.tracks.map((track) {
      final remaining =
          track.clips.where((c) => c.sourceId != sourceId).toList();
      return Track(id: track.id, kind: track.kind, clips: remaining);
    }).toList();

    final newDurationPts = updatedTracks
        .expand((t) => t.clips)
        .fold<int>(0, (max, c) => c.timelineOut > max ? c.timelineOut : max);
    final newClipCount =
        updatedTracks.fold<int>(0, (acc, t) => acc + t.clips.length);

    final updatedTimeline = currentTimeline != null
        ? Timeline(
            id: currentTimeline.id,
            timebase: currentTimeline.timebase,
            durationPts: newDurationPts,
            tracks: updatedTracks,
          )
        : null;

    state = state.copyWith(
      timeline: updatedTimeline,
      tracks: updatedTracks,
      durationPts: newDurationPts,
      totalClipCount: newClipCount,
    );
    return removedCount;
  }
}

/// Riverpod StateNotifierProvider for TimelineState.
final timelineProvider =
    StateNotifierProvider<TimelineNotifier, TimelineState>((ref) {
  final project = ref.watch(currentProjectProvider);
  return TimelineNotifier(initialTimeline: project?.timeline);
});
