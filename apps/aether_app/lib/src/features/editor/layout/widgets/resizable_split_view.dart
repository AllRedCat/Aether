import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'aether_split_divider.dart';

/// Pure Flutter 2D dynamic layout engine computing flex/fractional dimensions
/// and enforcing boundary clamping against minimum panel sizes.
class ResizableSplitView extends StatelessWidget {
  /// Split direction: [Axis.vertical] for stacked panes, [Axis.horizontal] for side-by-side panes.
  final Axis axis;

  /// Widgets to arrange in the split view.
  final List<Widget> children;

  /// Proportional weights for each child.
  final List<double> weights;

  /// Minimum allowed size in logical pixels for each corresponding child.
  final List<double> minSizes;

  /// Callback returning updated weights when any divider is dragged.
  final ValueChanged<List<double>>? onWeightsChanged;

  /// Optional callback invoked when any divider is double-tapped to reset layout.
  final VoidCallback? onReset;

  /// Hit thickness of dividers.
  final double dividerThickness;

  /// Optional prefix prepended to divider Keys to prevent collisions in nested/multi-row layouts.
  final String? dividerKeyPrefix;

  const ResizableSplitView.multi({
    super.key,
    required this.axis,
    required this.children,
    required this.weights,
    this.minSizes = const [],
    this.onWeightsChanged,
    this.onReset,
    this.dividerThickness = 8.0,
    this.dividerKeyPrefix,
  }) : assert(children.length == weights.length, 'Children and weights must have identical length.');

  /// Convenience constructor for a 2-pane split view (e.g. Top vs Bottom workspace).
  factory ResizableSplitView({
    Key? key,
    required Axis axis,
    required double ratio,
    required ValueChanged<double> onRatioChanged,
    VoidCallback? onReset,
    required Widget first,
    required Widget second,
    double minFirstSize = 120.0,
    double minSecondSize = 100.0,
    double dividerThickness = 8.0,
  }) {
    return ResizableSplitView.multi(
      key: key,
      axis: axis,
      weights: [ratio, (1.0 - ratio).clamp(0.01, 1.0)],
      minSizes: [minFirstSize, minSecondSize],
      onWeightsChanged: (newWeights) {
        if (newWeights.isNotEmpty) {
          onRatioChanged(newWeights[0]);
        }
      },
      onReset: onReset,
      dividerThickness: dividerThickness,
      children: [first, second],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (children.isEmpty) {
      return const SizedBox.shrink();
    }
    if (children.length == 1) {
      return SizedBox.expand(child: children.first);
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final isHorizontal = axis == Axis.horizontal;
        final totalExtent = isHorizontal ? constraints.maxWidth : constraints.maxHeight;

        // If constraints are unconstrained or zero, render children proportionally without dividers
        if (totalExtent <= 0 || !totalExtent.isFinite) {
          return Flex(
            direction: axis,
            children: children.map((c) => Expanded(child: c)).toList(),
          );
        }

        final numDividers = children.length - 1;
        final totalAvailableExtent = math.max(0.0, totalExtent - (numDividers * dividerThickness));

        // Normalize weights
        final weightSum = weights.fold<double>(0.0, (acc, w) => acc + w);
        final safeWeightSum = weightSum > 0 ? weightSum : 1.0;
        final normalizedWeights = weights.map((w) => w / safeWeightSum).toList();

        // Resolve minimum sizes with fallback
        final effectiveMinSizes = List<double>.generate(
          children.length,
          (i) => i < minSizes.length ? minSizes[i] : 80.0,
        );

        // Compute current pixel sizes
        final currentPixelSizes = normalizedWeights
            .map((w) => w * totalAvailableExtent)
            .toList();

        void handleDividerDrag(int dividerIndex, double deltaPx) {
          if (onWeightsChanged == null || totalAvailableExtent <= 0) return;

          final i = dividerIndex;
          final next = dividerIndex + 1;

          final currentSizeI = currentPixelSizes[i];
          final currentSizeNext = currentPixelSizes[next];

          final minSizeI = effectiveMinSizes[i];
          final minSizeNext = effectiveMinSizes[next];

          // Clamp delta so neither child shrinks below its minimum size
          final maxNegativeDelta = -(currentSizeI - minSizeI);
          final maxPositiveDelta = currentSizeNext - minSizeNext;

          if (maxNegativeDelta > maxPositiveDelta) {
            // Cannot resize further due to strict minimum bounds
            return;
          }

          final clampedDelta = deltaPx.clamp(maxNegativeDelta, maxPositiveDelta);
          if (clampedDelta == 0.0) return;

          final newPixelSizes = List<double>.from(currentPixelSizes);
          newPixelSizes[i] = currentSizeI + clampedDelta;
          newPixelSizes[next] = currentSizeNext - clampedDelta;

          // Convert back to normalized weights
          final newWeights = newPixelSizes
              .map((px) => px / totalAvailableExtent)
              .toList();

          onWeightsChanged!(newWeights);
        }

        // Build Flex layout with Expanded children and AetherSplitDivider elements
        final flexItems = <Widget>[];
        for (int i = 0; i < children.length; i++) {
          if (i > 0) {
            final dividerIndex = i - 1;
            flexItems.add(
              AetherSplitDivider(
                key: Key(
                  '${dividerKeyPrefix ?? ""}${isHorizontal ? "split_divider_horizontal" : "split_divider_vertical"}_$dividerIndex',
                ),
                axis: axis,
                hitThickness: dividerThickness,
                onDragDelta: (delta) => handleDividerDrag(dividerIndex, delta),
                onDoubleTap: onReset,
              ),
            );
          }

          final flexFactor = math.max(1, (normalizedWeights[i] * 10000).toInt());
          flexItems.add(
            Expanded(
              flex: flexFactor,
              child: children[i],
            ),
          );
        }

        return Flex(
          direction: axis,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: flexItems,
        );
      },
    );
  }
}
