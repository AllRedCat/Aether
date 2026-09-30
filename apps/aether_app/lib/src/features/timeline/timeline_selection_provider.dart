import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

/// Immutable state capturing the active clip and track selection on the timeline.
@immutable
class TimelineSelectionState {
  /// Unique identifier of the selected clip, or null if no clip is selected.
  final UuidValue? selectedClipId;

  /// Unique identifier of the track containing the selected clip, or null.
  final UuidValue? selectedTrackId;

  const TimelineSelectionState({
    this.selectedClipId,
    this.selectedTrackId,
  });

  /// Factory constructor for empty selection.
  const TimelineSelectionState.unselected()
      : selectedClipId = null,
        selectedTrackId = null;

  /// Returns true if a clip is currently selected.
  bool get hasSelection => selectedClipId != null;

  /// Checks if a specific clip is selected.
  bool isClipSelected(UuidValue clipId) => selectedClipId == clipId;

  /// Checks if a specific track contains the selected clip.
  bool isTrackSelected(UuidValue trackId) => selectedTrackId == trackId;

  TimelineSelectionState copyWith({
    UuidValue? selectedClipId,
    UuidValue? selectedTrackId,
    bool clearSelection = false,
  }) {
    if (clearSelection) {
      return const TimelineSelectionState.unselected();
    }
    return TimelineSelectionState(
      selectedClipId: selectedClipId ?? this.selectedClipId,
      selectedTrackId: selectedTrackId ?? this.selectedTrackId,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TimelineSelectionState &&
          runtimeType == other.runtimeType &&
          selectedClipId == other.selectedClipId &&
          selectedTrackId == other.selectedTrackId;

  @override
  int get hashCode => selectedClipId.hashCode ^ selectedTrackId.hashCode;

  @override
  String toString() =>
      'TimelineSelectionState(clipId: $selectedClipId, trackId: $selectedTrackId)';
}

/// StateNotifier managing timeline clip selection.
class TimelineSelectionNotifier extends StateNotifier<TimelineSelectionState> {
  TimelineSelectionNotifier() : super(const TimelineSelectionState.unselected());

  /// Selects a specific clip within a track.
  void selectClip(UuidValue clipId, UuidValue trackId) {
    state = TimelineSelectionState(
      selectedClipId: clipId,
      selectedTrackId: trackId,
    );
  }

  /// Toggles clip selection: deselects if already selected, otherwise selects it.
  void toggleClip(UuidValue clipId, UuidValue trackId) {
    if (state.selectedClipId == clipId && state.selectedTrackId == trackId) {
      clearSelection();
    } else {
      selectClip(clipId, trackId);
    }
  }

  /// Clears active selection.
  void clearSelection() {
    if (state.hasSelection) {
      state = const TimelineSelectionState.unselected();
    }
  }
}

/// Riverpod StateNotifierProvider for ephemeral timeline clip selection.
final timelineSelectionProvider =
    StateNotifierProvider<TimelineSelectionNotifier, TimelineSelectionState>((ref) {
  return TimelineSelectionNotifier();
});
