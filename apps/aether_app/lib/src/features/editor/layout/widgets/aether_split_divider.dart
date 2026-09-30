import 'package:flutter/material.dart';
import '../../../../theme/catppuccin.dart';

/// Interactive draggable divider supporting horizontal and vertical split orientations.
/// Features hover highlight (`surface1`), active drag glow (`mauve`), double-tap reset,
/// and native desktop resize cursors.
class AetherSplitDivider extends StatefulWidget {
  /// Orientation of the split.
  /// - [Axis.horizontal]: Separates side-by-side panes. Divider runs vertically. Dragging moves along X axis.
  /// - [Axis.vertical]: Separates stacked panes. Divider runs horizontally. Dragging moves along Y axis.
  final Axis axis;

  /// Callback delivering incremental drag delta in logical pixels.
  final ValueChanged<double> onDragDelta;

  /// Optional callback invoked when the divider is double-tapped.
  final VoidCallback? onDoubleTap;

  /// Width or height of the touch/grab hit region in logical pixels.
  final double hitThickness;

  /// Visible thickness of the divider line when idle.
  final double visualThickness;

  /// Visible thickness of the divider line when hovered or actively dragged.
  final double activeThickness;

  const AetherSplitDivider({
    super.key,
    required this.axis,
    required this.onDragDelta,
    this.onDoubleTap,
    this.hitThickness = 8.0,
    this.visualThickness = 2.0,
    this.activeThickness = 3.5,
  });

  @override
  State<AetherSplitDivider> createState() => _AetherSplitDividerState();
}

class _AetherSplitDividerState extends State<AetherSplitDivider> {
  bool _isHovered = false;
  bool _isDragging = false;

  @override
  Widget build(BuildContext context) {
    final isHorizontal = widget.axis == Axis.horizontal;
    final cursor = isHorizontal
        ? SystemMouseCursors.resizeColumn
        : SystemMouseCursors.resizeRow;

    Color dividerColor = CatppuccinMocha.crust;
    if (_isDragging) {
      dividerColor = CatppuccinMocha.mauve;
    } else if (_isHovered) {
      dividerColor = CatppuccinMocha.surface1;
    }

    final currentThickness = (_isHovered || _isDragging)
        ? widget.activeThickness
        : widget.visualThickness;

    return MouseRegion(
      cursor: cursor,
      onEnter: (_) => setState(() => _isHovered = true),
      onExit: (_) => setState(() => _isHovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.translucent,
        onDoubleTap: widget.onDoubleTap,
        onHorizontalDragStart: isHorizontal
            ? (_) => setState(() => _isDragging = true)
            : null,
        onHorizontalDragUpdate: isHorizontal
            ? (details) => widget.onDragDelta(details.delta.dx)
            : null,
        onHorizontalDragEnd: isHorizontal
            ? (_) => setState(() => _isDragging = false)
            : null,
        onVerticalDragStart: !isHorizontal
            ? (_) => setState(() => _isDragging = true)
            : null,
        onVerticalDragUpdate: !isHorizontal
            ? (details) => widget.onDragDelta(details.delta.dy)
            : null,
        onVerticalDragEnd: !isHorizontal
            ? (_) => setState(() => _isDragging = false)
            : null,
        child: SizedBox(
          width: isHorizontal ? widget.hitThickness : double.infinity,
          height: !isHorizontal ? widget.hitThickness : double.infinity,
          child: Center(
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              curve: Curves.easeOut,
              width: isHorizontal ? currentThickness : double.infinity,
              height: !isHorizontal ? currentThickness : double.infinity,
              decoration: BoxDecoration(
                color: dividerColor,
                boxShadow: _isDragging
                    ? [
                        BoxShadow(
                          color: CatppuccinMocha.mauve.withValues(alpha: 0.4),
                          blurRadius: 4.0,
                          spreadRadius: 1.0,
                        ),
                      ]
                    : null,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
