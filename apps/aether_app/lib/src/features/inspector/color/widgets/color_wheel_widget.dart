import 'dart:math' as math;
import 'package:flutter/material.dart';

import '../../../../theme/catppuccin.dart';

/// Interactive circular chromatic disk painter rendering a 360-degree hue spectrum,
/// radial saturation falloff, concentric guide rings, center crosshair axes,
/// color vector guide line, and high-contrast reticle crosshair.
class ColorWheelPainter extends CustomPainter {
  final double x; // -1.0 .. +1.0
  final double y; // -1.0 .. +1.0
  final Color accentColor;

  const ColorWheelPainter({
    required this.x,
    required this.y,
    required this.accentColor,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final double radius = size.width / 2;
    final Offset center = Offset(radius, radius);
    final Rect rect = Rect.fromCircle(center: center, radius: radius);

    // 1. Clip everything cleanly to the circular disk boundary
    canvas.save();
    final Path clipPath = Path()..addOval(rect);
    canvas.clipPath(clipPath);

    // 2. Base dark foundation
    final Paint bgPaint = Paint()..color = CatppuccinMocha.crust;
    canvas.drawRect(rect, bgPaint);

    // 3. Chromatic Hue Spectrum (360-degree Sweep Gradient)
    final Paint huePaint = Paint()
      ..shader = const SweepGradient(
        colors: [
          Color(0xFFFF0000), // Red 0°
          Color(0xFFFFFF00), // Yellow 60°
          Color(0xFF00FF00), // Green 120°
          Color(0xFF00FFFF), // Cyan 180°
          Color(0xFF0000FF), // Blue 240°
          Color(0xFFFF00FF), // Magenta 300°
          Color(0xFFFF0000), // Red 360°
        ],
      ).createShader(rect);
    canvas.drawCircle(center, radius, huePaint);

    // 4. Saturation falloff: dark neutral center fading to transparent at outer rim
    final Paint satPaint = Paint()
      ..shader = RadialGradient(
        colors: [
          CatppuccinMocha.mantle.withValues(alpha: 0.95),
          CatppuccinMocha.mantle.withValues(alpha: 0.35),
          Colors.transparent,
        ],
        stops: const [0.0, 0.45, 1.0],
      ).createShader(rect);
    canvas.drawCircle(center, radius, satPaint);

    // 5. Concentric reference guide rings (33% and 66% saturation)
    final Paint ringPaint = Paint()
      ..color = CatppuccinMocha.surface1.withValues(alpha: 0.4)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 0.8;
    canvas.drawCircle(center, radius * 0.33, ringPaint);
    canvas.drawCircle(center, radius * 0.66, ringPaint);

    // 6. Center crosshair axes
    final Paint axisPaint = Paint()
      ..color = CatppuccinMocha.overlay0.withValues(alpha: 0.35)
      ..strokeWidth = 0.8;
    canvas.drawLine(
      Offset(center.dx - radius * 0.25, center.dy),
      Offset(center.dx + radius * 0.25, center.dy),
      axisPaint,
    );
    canvas.drawLine(
      Offset(center.dx, center.dy - radius * 0.25),
      Offset(center.dx, center.dy + radius * 0.25),
      axisPaint,
    );

    // 7. Outer perimeter boundary ring
    final Paint borderPaint = Paint()
      ..color = CatppuccinMocha.surface1
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    canvas.drawCircle(center, radius - 0.75, borderPaint);

    canvas.restore();

    // 8. Crosshair reticle and color vector
    final double clampedX = x.clamp(-1.0, 1.0);
    final double clampedY = y.clamp(-1.0, 1.0);
    final double magnitude = math.sqrt(clampedX * clampedX + clampedY * clampedY);

    // Keep reticle center within (radius - 5.0) to prevent edge clipping
    final double effectiveRadius = radius - 5.0;
    final Offset pointerPos = magnitude > 0
        ? Offset(
            center.dx + (clampedX / (magnitude > 1.0 ? magnitude : 1.0)) * effectiveRadius * magnitude.clamp(0.0, 1.0),
            center.dy + (clampedY / (magnitude > 1.0 ? magnitude : 1.0)) * effectiveRadius * magnitude.clamp(0.0, 1.0),
          )
        : center;

    // Vector line from neutral center to active pointer
    if (magnitude > 0.02) {
      final Paint vectorLinePaint = Paint()
        ..color = accentColor.withValues(alpha: 0.75)
        ..strokeWidth = 1.2;
      canvas.drawLine(center, pointerPos, vectorLinePaint);
    }

    // High-contrast Reticle Circle (Dark drop shadow + Text stroke)
    final Paint reticleShadow = Paint()
      ..color = CatppuccinMocha.crust.withValues(alpha: 0.9)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3.0;
    canvas.drawCircle(pointerPos, 5.0, reticleShadow);

    final Paint reticleOuter = Paint()
      ..color = CatppuccinMocha.text
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    canvas.drawCircle(pointerPos, 5.0, reticleOuter);

    // Center indicator dot
    final Paint reticleDot = Paint()
      ..color = accentColor
      ..style = PaintingStyle.fill;
    canvas.drawCircle(pointerPos, 2.0, reticleDot);
  }

  @override
  bool shouldRepaint(covariant ColorWheelPainter oldDelegate) {
    return oldDelegate.x != x ||
        oldDelegate.y != y ||
        oldDelegate.accentColor != accentColor;
  }
}

/// Standalone, reusable Color Wheel Component for 3-way color grading (Lift, Gamma, Gain).
/// Features:
/// - Circular chromatic disk with 360° hue spectrum & radial saturation gradient.
/// - Interactive dragging with automatic circle clamping and pointer crosshair reticle.
/// - Angle & magnitude (polar coordinates) real-time calculation and monospace badge readout.
/// - Master Luminance horizontal slider with individual reset and numeric readout.
/// - Double-tap gesture on chromatic disk to reset color offset to neutral (0, 0).
/// - Header with accent dot, title, sublabel, and full reset button.
class ColorWheelWidget extends StatelessWidget {
  final String label;
  final String sublabel;
  final Color accentColor;
  final double x;
  final double y;
  final double luminance;
  final void Function(double x, double y) onOffsetChanged;
  final ValueChanged<double> onLuminanceChanged;
  final VoidCallback onResetOffset;
  final VoidCallback onResetLuminance;
  final VoidCallback onResetAll;

  final Key wheelKey;
  final Key diskKey;
  final Key lumaSliderKey;
  final Key lumaBadgeKey;
  final Key resetLumaKey;
  final Key resetWheelKey;
  final double wheelDiameter;

  const ColorWheelWidget({
    super.key,
    required this.label,
    required this.sublabel,
    required this.accentColor,
    required this.x,
    required this.y,
    required this.luminance,
    required this.onOffsetChanged,
    required this.onLuminanceChanged,
    required this.onResetOffset,
    required this.onResetLuminance,
    required this.onResetAll,
    required this.wheelKey,
    required this.diskKey,
    required this.lumaSliderKey,
    required this.lumaBadgeKey,
    required this.resetLumaKey,
    required this.resetWheelKey,
    this.wheelDiameter = 110.0,
  });

  void _handlePan(Offset localPosition, double diameter) {
    final double radius = diameter / 2;
    final Offset center = Offset(radius, radius);
    final double dx = localPosition.dx - center.dx;
    final double dy = localPosition.dy - center.dy;
    final double distance = math.sqrt(dx * dx + dy * dy);

    double normalizedX;
    double normalizedY;

    if (distance <= radius) {
      normalizedX = radius > 0 ? dx / radius : 0.0;
      normalizedY = radius > 0 ? dy / radius : 0.0;
    } else {
      normalizedX = distance > 0 ? dx / distance : 0.0;
      normalizedY = distance > 0 ? dy / distance : 0.0;
    }

    onOffsetChanged(
      normalizedX.clamp(-1.0, 1.0),
      normalizedY.clamp(-1.0, 1.0),
    );
  }

  Widget _buildReadout(double x, double y) {
    final double clampedX = x.clamp(-1.0, 1.0);
    final double clampedY = y.clamp(-1.0, 1.0);
    final double magnitude = math.sqrt(clampedX * clampedX + clampedY * clampedY).clamp(0.0, 1.0);
    final double angleRad = math.atan2(clampedY, clampedX);
    double angleDeg = angleRad * 180 / math.pi;
    if (angleDeg < 0) angleDeg += 360;

    final String text = magnitude < 0.005
        ? 'Neutral'
        : 'H: ${angleDeg.toStringAsFixed(0)}°  S: ${(magnitude * 100).toStringAsFixed(0)}%';

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: CatppuccinMocha.surface0,
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
      ),
      child: Text(
        text,
        style: const TextStyle(
          color: CatppuccinMocha.text,
          fontFamily: 'monospace',
          fontSize: 10,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }

  Widget _buildLuminanceRow(BuildContext context) {
    final clampedLuma = luminance.clamp(-1.0, 1.0);
    final formattedLuma = '${clampedLuma >= 0 ? '+' : ''}${clampedLuma.toStringAsFixed(2)}';

    return Row(
      children: [
        Text(
          'Y',
          style: TextStyle(
            color: accentColor,
            fontWeight: FontWeight.bold,
            fontSize: 11,
            fontFamily: 'monospace',
          ),
        ),
        const SizedBox(width: 4),
        Expanded(
          child: SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 3.0,
              activeTrackColor: accentColor,
              inactiveTrackColor: CatppuccinMocha.surface0,
              thumbColor: CatppuccinMocha.text,
              overlayColor: accentColor.withValues(alpha: 0.15),
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 5.0),
              overlayShape: const RoundSliderOverlayShape(overlayRadius: 10.0),
            ),
            child: Slider(
              key: lumaSliderKey,
              value: clampedLuma,
              min: -1.0,
              max: 1.0,
              onChanged: onLuminanceChanged,
            ),
          ),
        ),
        const SizedBox(width: 4),
        Container(
          key: lumaBadgeKey,
          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
          decoration: BoxDecoration(
            color: CatppuccinMocha.surface0,
            borderRadius: BorderRadius.circular(4),
            border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
          ),
          child: Text(
            formattedLuma,
            style: const TextStyle(
              color: CatppuccinMocha.text,
              fontFamily: 'monospace',
              fontSize: 10,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
        const SizedBox(width: 2),
        InkWell(
          key: resetLumaKey,
          onTap: onResetLuminance,
          borderRadius: BorderRadius.circular(4),
          child: const Padding(
            padding: EdgeInsets.all(2.0),
            child: Icon(
              Icons.restart_alt_rounded,
              size: 13,
              color: CatppuccinMocha.overlay0,
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      key: wheelKey,
      padding: const EdgeInsets.all(10.0),
      decoration: BoxDecoration(
        color: CatppuccinMocha.mantle,
        borderRadius: BorderRadius.circular(8.0),
        border: Border.all(color: CatppuccinMocha.surface0),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          // Header: Accent dot + Title + Sublabel + Reset All Wheel button
          Row(
            children: [
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(
                  color: accentColor,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 6),
              Expanded(
                child: RichText(
                  overflow: TextOverflow.ellipsis,
                  text: TextSpan(
                    children: [
                      TextSpan(
                        text: label,
                        style: TextStyle(
                          color: accentColor,
                          fontWeight: FontWeight.bold,
                          fontSize: 11,
                          letterSpacing: 0.5,
                        ),
                      ),
                      TextSpan(
                        text: ' ($sublabel)',
                        style: const TextStyle(
                          color: CatppuccinMocha.subtext0,
                          fontSize: 10,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              InkWell(
                key: resetWheelKey,
                onTap: onResetAll,
                borderRadius: BorderRadius.circular(4),
                child: const Padding(
                  padding: EdgeInsets.all(3.0),
                  child: Icon(
                    Icons.restart_alt_rounded,
                    size: 13,
                    color: CatppuccinMocha.overlay0,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),

          // Chromatic Disk with Drag gestures & double-tap reset
          GestureDetector(
            key: diskKey,
            onPanStart: (details) => _handlePan(details.localPosition, wheelDiameter),
            onPanUpdate: (details) => _handlePan(details.localPosition, wheelDiameter),
            onDoubleTap: onResetOffset,
            child: SizedBox(
              width: wheelDiameter,
              height: wheelDiameter,
              child: CustomPaint(
                painter: ColorWheelPainter(
                  x: x,
                  y: y,
                  accentColor: accentColor,
                ),
              ),
            ),
          ),
          const SizedBox(height: 8),

          // Readout: Hue Angle & Saturation Magnitude
          _buildReadout(x, y),
          const SizedBox(height: 8),

          // Master Luminance (Y) Slider
          _buildLuminanceRow(context),
        ],
      ),
    );
  }
}
