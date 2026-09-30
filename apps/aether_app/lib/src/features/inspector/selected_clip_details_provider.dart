import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';
import '../media_pool/media_pool_provider.dart';
import '../timeline/timeline_provider.dart';
import '../timeline/timeline_selection_provider.dart';

/// Enriched snapshot reconciling a selected clip with its parent Track,
/// TrackKind, and associated MediaItem from the media pool.
@immutable
class SelectedClipDetails {
  final Clip clip;
  final Track track;
  final TrackKind trackKind;
  final MediaItem? mediaItem;

  const SelectedClipDetails({
    required this.clip,
    required this.track,
    required this.trackKind,
    this.mediaItem,
  });

  /// Convenience helper: true if track is video or overlay (visual track).
  bool get isVideo => trackKind == TrackKind.video || trackKind == TrackKind.overlay;

  /// Convenience helper: true if track is audio.
  bool get isAudio => trackKind == TrackKind.audio;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is SelectedClipDetails &&
          runtimeType == other.runtimeType &&
          clip == other.clip &&
          track == other.track &&
          trackKind == other.trackKind &&
          mediaItem == other.mediaItem;

  @override
  int get hashCode =>
      clip.hashCode ^ track.hashCode ^ trackKind.hashCode ^ mediaItem.hashCode;

  @override
  String toString() =>
      'SelectedClipDetails(clipId: ${clip.id}, trackId: ${track.id}, kind: $trackKind, media: ${mediaItem?.fileName})';
}

/// Computed Riverpod selector reconciling the active selection with the current Timeline and Media Pool.
/// Returns [null] if no clip is selected or if the selected clip/track no longer exists in the timeline.
final selectedClipDetailsProvider = Provider<SelectedClipDetails?>((ref) {
  final selection = ref.watch(timelineSelectionProvider);
  if (!selection.hasSelection || selection.selectedClipId == null) {
    return null;
  }

  final timelineState = ref.watch(timelineProvider);
  final selectedClipId = selection.selectedClipId!;
  final selectedTrackId = selection.selectedTrackId;

  Track? targetTrack;
  Clip? targetClip;

  // 1. Search in target track first if specified
  if (selectedTrackId != null) {
    for (final track in timelineState.tracks) {
      if (track.id == selectedTrackId) {
        targetTrack = track;
        for (final clip in track.clips) {
          if (clip.id == selectedClipId) {
            targetClip = clip;
            break;
          }
        }
        break;
      }
    }
  }

  // 2. Fallback search across all tracks if not found in targetTrack
  if (targetClip == null || targetTrack == null) {
    for (final track in timelineState.tracks) {
      for (final clip in track.clips) {
        if (clip.id == selectedClipId) {
          targetClip = clip;
          targetTrack = track;
          break;
        }
      }
      if (targetClip != null) break;
    }
  }

  // If clip is not found in the current timeline, selection is stale/invalid
  if (targetClip == null || targetTrack == null) {
    return null;
  }

  // 3. Reconcile with media pool to retrieve associated media metadata
  final mediaPoolState = ref.watch(mediaPoolProvider);
  MediaItem? mediaItem;
  try {
    mediaItem = mediaPoolState.items.firstWhere(
      (item) => item.id == targetClip!.sourceId,
    );
  } catch (_) {
    mediaItem = null;
  }

  return SelectedClipDetails(
    clip: targetClip,
    track: targetTrack,
    trackKind: targetTrack.kind,
    mediaItem: mediaItem,
  );
});
