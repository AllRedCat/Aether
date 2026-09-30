import 'dart:math' as math;
import 'package:flutter/material.dart';

import '../../../../theme/catppuccin.dart';
import '../color_grading_state.dart';

/// Mathematically rigorous Monotone Cubic Spline (Fritsch-Carlson algorithm)
/// that computes Hermite tangents and translates each segment into an exact
/// hardware-accelerated Cubic Bezier segment for Flutter's [Path.cubicTo].
///
/// Properties:
/// - Guarantees the curve passes strictly through all control points.
/// - Preserves monotonicity between adjacent points (eliminates unnatural overshoot/ringing).
/// - Provides silky smooth C1 continuous tone curves identical to DaVinci Resolve & Lightroom.
class MonotoneCubicSpline {
  final List<Offset> points;
  late final List<double> tangents;

  MonotoneCubicSpline(List<Offset> inputPoints)
      : points = _sanitizePoints(inputPoints) {
    tangents = _computeTangents(points);
  }

  /// Sorts points by X ascending, clamps to [0.0 .. 1.0], and ensures unique strictly increasing X values.
  static List<Offset> _sanitizePoints(List<Offset> pts) {
    if (pts.isEmpty) {
      return const [Offset(0.0, 0.0), Offset(1.0, 1.0)];
    }

    final sorted = List<Offset>.from(pts)
      ..sort((a, b) => a.dx.compareTo(b.dx));

    final result = <Offset>[];
    for (int i = 0; i < sorted.length; i++) {
      final p = Offset(
        sorted[i].dx.clamp(0.0, 1.0),
        sorted[i].dy.clamp(0.0, 1.0),
      );

      if (result.isEmpty) {
        result.add(p);
      } else {
        final last = result.last;
        if (p.dx <= last.dx) {
          final nextX = (last.dx + 0.005).clamp(0.0, 1.0);
          result.add(Offset(nextX, p.dy));
        } else {
          result.add(p);
        }
      }
    }

    if (result.length == 1) {
      result.add(const Offset(1.0, 1.0));
    }

    return result;
  }

  /// Computes Fritsch-Carlson monotone tangents.
  static List<double> _computeTangents(List<Offset> p) {
    final n = p.length;
    final m = List<double>.filled(n, 0.0);
    if (n < 2) return m;

    final deltas = List<double>.filled(n - 1, 0.0);
    for (int i = 0; i < n - 1; i++) {
      final h = p[i + 1].dx - p[i].dx;
      deltas[i] = h <= 1e-6 ? 0.0 : (p[i + 1].dy - p[i].dy) / h;
    }

    m[0] = deltas[0];
    m[n - 1] = deltas[n - 2];

    for (int i = 1; i < n - 1; i++) {
      if (deltas[i - 1] * deltas[i] <= 0) {
        m[i] = 0.0;
      } else {
        m[i] = (deltas[i - 1] + deltas[i]) / 2.0;
      }
    }

    // Monotonicity filter (Fritsch & Carlson 1980)
    for (int i = 0; i < n - 1; i++) {
      if (deltas[i] == 0) {
        m[i] = 0.0;
        m[i + 1] = 0.0;
      } else {
        final alpha = m[i] / deltas[i];
        final beta = m[i + 1] / deltas[i];
        final dist = alpha * alpha + beta * beta;
        if (dist > 9.0) {
          final tau = 3.0 / math.sqrt(dist);
          m[i] = tau * alpha * deltas[i];
          m[i + 1] = tau * beta * deltas[i];
        }
      }
    }
    return m;
  }

  /// Evaluates curve Y at any normalized X in [0.0 .. 1.0].
  double evaluate(double x) {
    if (points.isEmpty) return 0.0;
    if (x <= points.first.dx) return points.first.dy;
    if (x >= points.last.dx) return points.last.dy;

    int idx = 0;
    for (int i = 0; i < points.length - 1; i++) {
      if (x >= points[i].dx && x <= points[i + 1].dx) {
        idx = i;
        break;
      }
    }

    final pA = points[idx];
    final pB = points[idx + 1];
    final h = pB.dx - pA.dx;
    if (h <= 1e-6) return pA.dy;

    final t = (x - pA.dx) / h;
    final t2 = t * t;
    final t3 = t2 * t;

    final h00 = 2 * t3 - 3 * t2 + 1;
    final h10 = t3 - 2 * t2 + t;
    final h01 = -2 * t3 + 3 * t2;
    final h11 = t3 - t2;

    return (h00 * pA.dy +
            h10 * h * tangents[idx] +
            h01 * pB.dy +
            h11 * h * tangents[idx + 1])
        .clamp(0.0, 1.0);
  }

  /// Generates a smooth Flutter Path mapped to canvas screen [size].
  Path computePath(Size size) {
    final path = Path();
    if (points.isEmpty) return path;

    Offset toScreen(Offset norm) {
      return Offset(
        norm.dx * size.width,
        (1.0 - norm.dy) * size.height,
      );
    }

    final p0 = toScreen(points[0]);
    path.moveTo(p0.dx, p0.dy);

    for (int i = 0; i < points.length - 1; i++) {
      final pA = points[i];
      final pB = points[i + 1];
      final h = pB.dx - pA.dx;

      // Translate Hermite tangents to Cubic Bezier control points
      final cp1Norm = Offset(
        pA.dx + h / 3.0,
        pA.dy + (h * tangents[i]) / 3.0,
      );
      final cp2Norm = Offset(
        pB.dx - h / 3.0,
        pB.dy - (h * tangents[i + 1]) / 3.0,
      );

      final cp1 = toScreen(cp1Norm);
      final cp2 = toScreen(cp2Norm);
      final pBScreen = toScreen(pB);

      path.cubicTo(cp1.dx, cp1.dy, cp2.dx, cp2.dy, pBScreen.dx, pBScreen.dy);
    }

    return path;
  }
}

/// CustomPainter rendering the professional tone curve canvas:
/// - Rounded base frame with surface0 fill & surface1 border.
/// - 4x4 grid lines (25%, 50%, 75% subdivisions).
/// - Diagonal linear identity reference line (from (0,0) to (1,1)).
/// - Ghost curves for inactive channels (Red, Green, Blue, Master) at 20% opacity.
/// - Under-curve subtle gradient fill.
/// - Smooth active spline curve line.
/// - Control points with center fill, channel stroke, and active drag glow ring.
class ColorCurvesPainter extends CustomPainter {
  final CurveChannel activeChannel;
  final List<Offset> points;
  final int? selectedPointIndex;
  final Map<CurveChannel, List<Offset>>? allCurves;

  const ColorCurvesPainter({
    required this.activeChannel,
    required this.points,
    this.selectedPointIndex,
    this.allCurves,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final Rect rect = Offset.zero & size;
    final RRect rrect = RRect.fromRectAndRadius(rect, const Radius.circular(8.0));

    // 1. Clip to canvas rounded bounds
    canvas.save();
    canvas.clipRRect(rrect);

    // 2. Background base fill
    final Paint bgPaint = Paint()..color = CatppuccinMocha.surface0.withValues(alpha: 0.6);
    canvas.drawRect(rect, bgPaint);

    // 3. Grid Lines (4x4 subdivisions: 25%, 50%, 75%)
    final Paint gridPaint = Paint()
      ..color = CatppuccinMocha.surface1.withValues(alpha: 0.5)
      ..strokeWidth = 0.8
      ..style = PaintingStyle.stroke;

    for (final fraction in [0.25, 0.50, 0.75]) {
      final x = fraction * size.width;
      canvas.drawLine(Offset(x, 0), Offset(x, size.height), gridPaint);

      final y = fraction * size.height;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), gridPaint);
    }

    // 4. Diagonal Linear Identity Reference Line (Bottom-Left to Top-Right)
    final Paint identityPaint = Paint()
      ..color = CatppuccinMocha.overlay0.withValues(alpha: 0.35)
      ..strokeWidth = 1.0
      ..style = PaintingStyle.stroke;
    canvas.drawLine(Offset(0, size.height), Offset(size.width, 0), identityPaint);

    // 5. Inactive Ghost Curves (render other channels with subtle opacity)
    if (allCurves != null) {
      for (final entry in allCurves!.entries) {
        if (entry.key == activeChannel) continue;
        final ghostSpline = MonotoneCubicSpline(entry.value);
        final ghostPath = ghostSpline.computePath(size);

        final Paint ghostPaint = Paint()
          ..color = entry.key.color.withValues(alpha: 0.25)
          ..strokeWidth = 1.2
          ..style = PaintingStyle.stroke;
        canvas.drawPath(ghostPath, ghostPaint);
      }
    }

    // 6. Active Curve Spline & Gradient Fill
    final spline = MonotoneCubicSpline(points);
    final curvePath = spline.computePath(size);

    // Subtle Under-curve Gradient Fill
    final Path fillPath = Path.from(curvePath)
      ..lineTo(size.width, size.height)
      ..lineTo(0, size.height)
      ..close();

    final Paint fillPaint = Paint()
      ..shader = LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [
          activeChannel.color.withValues(alpha: 0.16),
          activeChannel.color.withValues(alpha: 0.01),
        ],
      ).createShader(rect)
      ..style = PaintingStyle.fill;
    canvas.drawPath(fillPath, fillPaint);

    // Active Curve Line Stroke
    final Paint curvePaint = Paint()
      ..color = activeChannel.color
      ..strokeWidth = 2.0
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;
    canvas.drawPath(curvePath, curvePaint);

    // 7. Outer Border
    final Paint borderPaint = Paint()
      ..color = CatppuccinMocha.surface1
      ..strokeWidth = 1.0
      ..style = PaintingStyle.stroke;
    canvas.drawRRect(rrect, borderPaint);

    canvas.restore();

    // 8. Control Points (rendered unclipped so handles remain fully visible at edges)
    for (int i = 0; i < points.length; i++) {
      final p = points[i];
      final screenPos = Offset(
        p.dx * size.width,
        (1.0 - p.dy) * size.height,
      );

      final bool isSelected = i == selectedPointIndex;

      // Glow halo on selected point
      if (isSelected) {
        final Paint haloPaint = Paint()
          ..color = activeChannel.color.withValues(alpha: 0.3)
          ..style = PaintingStyle.fill;
        canvas.drawCircle(screenPos, 11.0, haloPaint);
      }

      // Outer stroke circle
      final Paint pointStroke = Paint()
        ..color = isSelected ? CatppuccinMocha.text : activeChannel.color
        ..strokeWidth = isSelected ? 2.5 : 1.8
        ..style = PaintingStyle.stroke;
      canvas.drawCircle(screenPos, isSelected ? 6.5 : 5.0, pointStroke);

      // Inner center fill
      final Paint pointFill = Paint()
        ..color = isSelected ? activeChannel.color : CatppuccinMocha.base
        ..style = PaintingStyle.fill;
      canvas.drawCircle(screenPos, isSelected ? 4.5 : 3.2, pointFill);
    }
  }

  @override
  bool shouldRepaint(covariant ColorCurvesPainter oldDelegate) {
    return oldDelegate.activeChannel != activeChannel ||
        oldDelegate.points != points ||
        oldDelegate.selectedPointIndex != selectedPointIndex ||
        oldDelegate.allCurves != allCurves;
  }
}

/// Interactive Tone Curves Canvas Component.
class ColorCurvesWidget extends StatefulWidget {
  final CurveChannel activeChannel;
  final List<Offset> points;
  final ValueChanged<List<Offset>> onPointsChanged;
  final VoidCallback? onReset;
  final ValueChanged<int?>? onSelectedPointChanged;
  final Map<CurveChannel, List<Offset>>? allCurves;
  final bool enabled;

  const ColorCurvesWidget({
    super.key,
    required this.activeChannel,
    required this.points,
    required this.onPointsChanged,
    this.onReset,
    this.onSelectedPointChanged,
    this.allCurves,
    this.enabled = true,
  });

  @override
  State<ColorCurvesWidget> createState() => _ColorCurvesWidgetState();
}

class _ColorCurvesWidgetState extends State<ColorCurvesWidget> {
  int? _selectedPointIndex;

  void _setSelected(int? index) {
    setState(() => _selectedPointIndex = index);
    widget.onSelectedPointChanged?.call(index);
  }

  Offset _toNorm(Offset localPos, Size size) {
    return Offset(
      (localPos.dx / size.width).clamp(0.0, 1.0),
      (1.0 - (localPos.dy / size.height)).clamp(0.0, 1.0),
    );
  }

  Offset _toScreen(Offset norm, Size size) {
    return Offset(
      norm.dx * size.width,
      (1.0 - norm.dy) * size.height,
    );
  }

  int? _findNearestPoint(Offset localPos, Size size, {double hitRadius = 22.0}) {
    int? closestIndex;
    double minDistance = double.infinity;

    for (int i = 0; i < widget.points.length; i++) {
      final pScreen = _toScreen(widget.points[i], size);
      final dist = (localPos - pScreen).distance;
      if (dist <= hitRadius && dist < minDistance) {
        minDistance = dist;
        closestIndex = i;
      }
    }
    return closestIndex;
  }

  void _handleTapDown(TapDownDetails details, Size size) {
    if (!widget.enabled) return;

    final hitIndex = _findNearestPoint(details.localPosition, size);

    if (hitIndex != null) {
      _setSelected(hitIndex);
    } else {
      // Tap on empty space: Add a new point at tapped coordinate
      final newPoint = _toNorm(details.localPosition, size);
      final updated = List<Offset>.from(widget.points)..add(newPoint);
      updated.sort((a, b) => a.dx.compareTo(b.dx));
      final newIndex = updated.indexOf(newPoint);
      _setSelected(newIndex);
      widget.onPointsChanged(List.unmodifiable(updated));
    }
  }

  void _handlePanStart(DragStartDetails details, Size size) {
    if (!widget.enabled) return;

    final hitIndex = _findNearestPoint(details.localPosition, size);
    if (hitIndex != null) {
      _setSelected(hitIndex);
    }
  }

  void _handlePanUpdate(DragUpdateDetails details, Size size) {
    if (!widget.enabled || _selectedPointIndex == null) return;
    final idx = _selectedPointIndex!;
    if (idx < 0 || idx >= widget.points.length) return;

    final norm = _toNorm(details.localPosition, size);
    final pts = List<Offset>.from(widget.points);

    if (idx == 0) {
      // Pinned Black/Shadow endpoint: X locked to 0.0, Y free
      pts[idx] = Offset(0.0, norm.dy);
    } else if (idx == pts.length - 1) {
      // Pinned White/Highlight endpoint: X locked to 1.0, Y free
      pts[idx] = Offset(1.0, norm.dy);
    } else {
      // Interior point: X bounded between adjacent points, Y free
      final minX = (pts[idx - 1].dx + 0.02).clamp(0.0, 1.0);
      final maxX = (pts[idx + 1].dx - 0.02).clamp(0.0, 1.0);
      final clampedX = minX < maxX ? norm.dx.clamp(minX, maxX) : norm.dx;
      pts[idx] = Offset(clampedX, norm.dy);
    }

    widget.onPointsChanged(List.unmodifiable(pts));
  }

  void _handleDoubleTap() {
    if (!widget.enabled) return;
    if (widget.onReset != null) {
      widget.onReset!();
    } else {
      widget.onPointsChanged(const [Offset(0.0, 0.0), Offset(1.0, 1.0)]);
    }
    _setSelected(null);
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final double width = constraints.maxWidth;
        final double height = width.clamp(140.0, 220.0);
        final Size canvasSize = Size(width, height);

        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (details) => _handleTapDown(details, canvasSize),
          onPanStart: (details) => _handlePanStart(details, canvasSize),
          onPanUpdate: (details) => _handlePanUpdate(details, canvasSize),
          onDoubleTap: _handleDoubleTap,
          child: SizedBox(
            width: width,
            height: height,
            child: CustomPaint(
              size: canvasSize,
              painter: ColorCurvesPainter(
                activeChannel: widget.activeChannel,
                points: widget.points,
                selectedPointIndex: _selectedPointIndex,
                allCurves: widget.allCurves,
              ),
            ),
          ),
        );
      },
    );
  }
}
