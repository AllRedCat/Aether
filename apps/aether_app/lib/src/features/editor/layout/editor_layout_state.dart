import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'panel_descriptor.dart';

/// Available workspace layout presets tailored for specific NLE workflows.
enum LayoutPreset {
  editing('Edição', Icons.dashboard_customize_rounded),
  color('Cores', Icons.palette_rounded),
  audio('Áudio', Icons.graphic_eq_rounded);

  final String label;
  final IconData icon;
  const LayoutPreset(this.label, this.icon);
}

/// Immutable state describing the active layout preset, vertical split ratio,
/// horizontal panel weights, and panel ordering.
@immutable
class EditorLayoutState {
  /// Active workflow preset.
  final LayoutPreset preset;

  /// Vertical split ratio (fraction of available height allocated to the top region).
  /// Clamped between 0.15 and 0.85.
  final double verticalRatio;

  /// Proportional horizontal weights for panels in the top region.
  final List<double> topWeights;

  /// Proportional horizontal weights for panels in the bottom region.
  final List<double> bottomWeights;

  /// Ordered panel identifiers mounted in the top region.
  final List<EditorPanelId> topPanels;

  /// Ordered panel identifiers mounted in the bottom region.
  final List<EditorPanelId> bottomPanels;

  const EditorLayoutState({
    required this.preset,
    required this.verticalRatio,
    required this.topWeights,
    required this.bottomWeights,
    required this.topPanels,
    required this.bottomPanels,
  });

  /// Factory creating factory default configurations for each preset.
  factory EditorLayoutState.fromPreset(LayoutPreset preset) {
    switch (preset) {
      case LayoutPreset.editing:
        return const EditorLayoutState(
          preset: LayoutPreset.editing,
          verticalRatio: 0.60,
          topWeights: [0.22, 0.53, 0.25],
          bottomWeights: [1.0],
          topPanels: [
            EditorPanelId.mediaPool,
            EditorPanelId.preview,
            EditorPanelId.inspector,
          ],
          bottomPanels: [EditorPanelId.timeline],
        );

      case LayoutPreset.color:
        return const EditorLayoutState(
          preset: LayoutPreset.color,
          verticalRatio: 0.55,
          topWeights: [0.55, 0.45],
          bottomWeights: [0.60, 0.40],
          topPanels: [
            EditorPanelId.preview,
            EditorPanelId.inspector,
          ],
          bottomPanels: [
            EditorPanelId.colorGrading,
            EditorPanelId.timeline,
          ],
        );

      case LayoutPreset.audio:
        return const EditorLayoutState(
          preset: LayoutPreset.audio,
          verticalRatio: 0.35,
          topWeights: [0.25, 0.45, 0.30],
          bottomWeights: [1.0],
          topPanels: [
            EditorPanelId.mediaPool,
            EditorPanelId.preview,
            EditorPanelId.inspector,
          ],
          bottomPanels: [EditorPanelId.timeline],
        );
    }
  }

  /// Creates a copy of this state with specified modifications.
  EditorLayoutState copyWith({
    LayoutPreset? preset,
    double? verticalRatio,
    List<double>? topWeights,
    List<double>? bottomWeights,
    List<EditorPanelId>? topPanels,
    List<EditorPanelId>? bottomPanels,
  }) {
    return EditorLayoutState(
      preset: preset ?? this.preset,
      verticalRatio: verticalRatio ?? this.verticalRatio,
      topWeights: topWeights ?? this.topWeights,
      bottomWeights: bottomWeights ?? this.bottomWeights,
      topPanels: topPanels ?? this.topPanels,
      bottomPanels: bottomPanels ?? this.bottomPanels,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is EditorLayoutState &&
          runtimeType == other.runtimeType &&
          preset == other.preset &&
          verticalRatio == other.verticalRatio &&
          listEquals(topWeights, other.topWeights) &&
          listEquals(bottomWeights, other.bottomWeights) &&
          listEquals(topPanels, other.topPanels) &&
          listEquals(bottomPanels, other.bottomPanels);

  @override
  int get hashCode =>
      preset.hashCode ^
      verticalRatio.hashCode ^
      topWeights.hashCode ^
      bottomWeights.hashCode ^
      topPanels.hashCode ^
      bottomPanels.hashCode;
}

/// StateNotifier coordinating dynamic layout resizing and preset transitions.
class EditorLayoutNotifier extends StateNotifier<EditorLayoutState> {
  EditorLayoutNotifier()
      : super(EditorLayoutState.fromPreset(LayoutPreset.editing));

  /// Switches active workflow preset and initializes its default proportions.
  void setPreset(LayoutPreset preset) {
    state = EditorLayoutState.fromPreset(preset);
  }

  /// Updates the vertical top/bottom split ratio clamped between 15% and 85%.
  void updateVerticalRatio(double ratio) {
    state = state.copyWith(verticalRatio: ratio.clamp(0.15, 0.85));
  }

  /// Updates the top panel horizontal weights.
  void updateTopWeights(List<double> weights) {
    state = state.copyWith(topWeights: List.unmodifiable(weights));
  }

  /// Updates the bottom panel horizontal weights.
  void updateBottomWeights(List<double> weights) {
    state = state.copyWith(bottomWeights: List.unmodifiable(weights));
  }

  /// Resets the active preset to factory default proportions.
  void resetToPresetDefaults() {
    state = EditorLayoutState.fromPreset(state.preset);
  }

  /// Extensible hook allowing dynamic registration and mounting of panels.
  void setPanels({
    List<EditorPanelId>? topPanels,
    List<EditorPanelId>? bottomPanels,
    List<double>? topWeights,
    List<double>? bottomWeights,
  }) {
    state = state.copyWith(
      topPanels: topPanels,
      bottomPanels: bottomPanels,
      topWeights: topWeights,
      bottomWeights: bottomWeights,
    );
  }
}

/// Riverpod provider exposing [EditorLayoutState].
final editorLayoutProvider =
    StateNotifierProvider<EditorLayoutNotifier, EditorLayoutState>((ref) {
  return EditorLayoutNotifier();
});
