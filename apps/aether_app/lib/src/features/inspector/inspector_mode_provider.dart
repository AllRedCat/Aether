import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Available viewing modes for the Inspector panel.
enum InspectorMode {
  properties('Propriedades', Icons.tune_rounded),
  color('Cor', Icons.palette_rounded);

  final String label;
  final IconData icon;

  const InspectorMode(this.label, this.icon);

  /// Helper: true if current mode is properties.
  bool get isProperties => this == InspectorMode.properties;

  /// Helper: true if current mode is color grading.
  bool get isColor => this == InspectorMode.color;
}

/// Riverpod state provider managing the active Inspector mode.
/// Defaults to [InspectorMode.properties] for standard transform & audio editing.
final inspectorModeProvider = StateProvider<InspectorMode>((ref) {
  return InspectorMode.properties;
});
