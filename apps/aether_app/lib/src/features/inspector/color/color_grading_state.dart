import 'dart:math' as math;
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:uuid/uuid.dart';

import '../../../theme/catppuccin.dart';

/// 2D chromatic offset with master luminance for a 3-way color wheel.
/// Coordinates are normalized to [-1.0 .. 1.0] with Euclidean radius clamped to 1.0.
@immutable
class ColorWheelOffset {
  final double x;
  final double y;
  final double luminance;

  const ColorWheelOffset({
    this.x = 0.0,
    this.y = 0.0,
    this.luminance = 0.0,
  });

  static const ColorWheelOffset zero = ColorWheelOffset();
  static const ColorWheelOffset defaults = ColorWheelOffset();

  /// Returns true if this offset represents neutral / no adjustment.
  bool get isZero => x == 0.0 && y == 0.0 && luminance == 0.0;

  /// Euclidean distance from center [0.0 .. 1.0] representing saturation/intensity.
  double get radius => math.sqrt(x * x + y * y).clamp(0.0, 1.0);

  /// Polar angle in radians [-pi .. pi].
  double get angle => math.atan2(y, x);

  /// Constructs a [ColorWheelOffset] from polar angle and radius.
  factory ColorWheelOffset.fromPolar(
    double angle,
    double radius, {
    double luminance = 0.0,
  }) {
    final clampedRadius = radius.clamp(0.0, 1.0);
    return ColorWheelOffset(
      x: (clampedRadius * math.cos(angle)).clamp(-1.0, 1.0),
      y: (clampedRadius * math.sin(angle)).clamp(-1.0, 1.0),
      luminance: luminance.clamp(-1.0, 1.0),
    );
  }

  ColorWheelOffset copyWith({
    double? x,
    double? y,
    double? luminance,
  }) {
    return ColorWheelOffset(
      x: (x ?? this.x).clamp(-1.0, 1.0),
      y: (y ?? this.y).clamp(-1.0, 1.0),
      luminance: (luminance ?? this.luminance).clamp(-1.0, 1.0),
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ColorWheelOffset &&
          runtimeType == other.runtimeType &&
          (x - other.x).abs() < 1e-6 &&
          (y - other.y).abs() < 1e-6 &&
          (luminance - other.luminance).abs() < 1e-6;

  @override
  int get hashCode => Object.hash(x, y, luminance);

  @override
  String toString() =>
      'ColorWheelOffset(x: ${x.toStringAsFixed(3)}, y: ${y.toStringAsFixed(3)}, lum: ${luminance.toStringAsFixed(3)})';
}

/// Tonal curve channels for multi-channel RGB curve editing.
enum CurveChannel {
  rgb('RGB', CatppuccinMocha.text),
  red('Vermelho', CatppuccinMocha.red),
  green('Verde', CatppuccinMocha.green),
  blue('Azul', CatppuccinMocha.blue);

  final String label;
  final Color color;

  const CurveChannel(this.label, this.color);
}

/// Immutable color grading parameters for an individual video clip.
@immutable
class ColorGradingModel {
  // --- Basic Color Adjustments ---
  final double temperature;
  final double tint;
  final double exposure;
  final double contrast;
  final double saturation;

  // --- 3-Way Color Wheels ---
  final ColorWheelOffset lift;
  final ColorWheelOffset gamma;
  final ColorWheelOffset gain;

  // --- Tone Curves ---
  final CurveChannel activeCurveChannel;
  final List<Offset> rgbCurvePoints;
  final List<Offset> redCurvePoints;
  final List<Offset> greenCurvePoints;
  final List<Offset> blueCurvePoints;

  const ColorGradingModel({
    this.temperature = 0.0,
    this.tint = 0.0,
    this.exposure = 0.0,
    this.contrast = 0.0,
    this.saturation = 100.0,
    this.lift = ColorWheelOffset.zero,
    this.gamma = ColorWheelOffset.zero,
    this.gain = ColorWheelOffset.zero,
    this.activeCurveChannel = CurveChannel.rgb,
    this.rgbCurvePoints = defaultCurvePoints,
    this.redCurvePoints = defaultCurvePoints,
    this.greenCurvePoints = defaultCurvePoints,
    this.blueCurvePoints = defaultCurvePoints,
  });

  /// Factory default configuration.
  static const ColorGradingModel defaults = ColorGradingModel();

  /// Standard identity tone curve points `[(0,0), (1,1)]`.
  static const List<Offset> defaultCurvePoints = [
    Offset(0.0, 0.0),
    Offset(1.0, 1.0),
  ];

  // Parameter Constraints & Bounds
  static const double minTemperature = -100.0;
  static const double maxTemperature = 100.0;
  static const double minTint = -100.0;
  static const double maxTint = 100.0;
  static const double minExposure = -5.0;
  static const double maxExposure = 5.0;
  static const double minContrast = -100.0;
  static const double maxContrast = 100.0;
  static const double minSaturation = 0.0;
  static const double maxSaturation = 200.0;

  /// Convenience getter returning the control points for the currently active curve channel.
  List<Offset> get curvePoints => pointsForChannel(activeCurveChannel);

  /// Returns the control points for a specific curve channel.
  List<Offset> pointsForChannel(CurveChannel channel) {
    switch (channel) {
      case CurveChannel.rgb:
        return rgbCurvePoints;
      case CurveChannel.red:
        return redCurvePoints;
      case CurveChannel.green:
        return greenCurvePoints;
      case CurveChannel.blue:
        return blueCurvePoints;
    }
  }

  /// Returns true if basic adjustments (temperature, tint, exposure, contrast, saturation) differ from defaults.
  bool get hasBasicModifications =>
      temperature != 0.0 ||
      tint != 0.0 ||
      exposure != 0.0 ||
      contrast != 0.0 ||
      saturation != 100.0;

  /// Returns true if any 3-way color wheel differs from neutral.
  bool get hasWheelModifications =>
      !lift.isZero || !gamma.isZero || !gain.isZero;

  /// Returns true if any curve channel differs from the default identity curve.
  bool get hasCurveModifications =>
      !listEquals(rgbCurvePoints, defaultCurvePoints) ||
      !listEquals(redCurvePoints, defaultCurvePoints) ||
      !listEquals(greenCurvePoints, defaultCurvePoints) ||
      !listEquals(blueCurvePoints, defaultCurvePoints);

  /// Returns true if all color grading parameters are at their default values.
  bool get isDefault =>
      !hasBasicModifications &&
      !hasWheelModifications &&
      !hasCurveModifications;

  /// Creates a copy with specified modifications.
  ColorGradingModel copyWith({
    double? temperature,
    double? tint,
    double? exposure,
    double? contrast,
    double? saturation,
    ColorWheelOffset? lift,
    ColorWheelOffset? gamma,
    ColorWheelOffset? gain,
    CurveChannel? activeCurveChannel,
    List<Offset>? rgbCurvePoints,
    List<Offset>? redCurvePoints,
    List<Offset>? greenCurvePoints,
    List<Offset>? blueCurvePoints,
  }) {
    return ColorGradingModel(
      temperature: (temperature ?? this.temperature).clamp(minTemperature, maxTemperature),
      tint: (tint ?? this.tint).clamp(minTint, maxTint),
      exposure: (exposure ?? this.exposure).clamp(minExposure, maxExposure),
      contrast: (contrast ?? this.contrast).clamp(minContrast, maxContrast),
      saturation: (saturation ?? this.saturation).clamp(minSaturation, maxSaturation),
      lift: lift ?? this.lift,
      gamma: gamma ?? this.gamma,
      gain: gain ?? this.gain,
      activeCurveChannel: activeCurveChannel ?? this.activeCurveChannel,
      rgbCurvePoints: rgbCurvePoints ?? this.rgbCurvePoints,
      redCurvePoints: redCurvePoints ?? this.redCurvePoints,
      greenCurvePoints: greenCurvePoints ?? this.greenCurvePoints,
      blueCurvePoints: blueCurvePoints ?? this.blueCurvePoints,
    );
  }

  /// Convenience helper returning a new model with updated points for the specified channel.
  ColorGradingModel withCurvePoints(CurveChannel channel, List<Offset> points) {
    switch (channel) {
      case CurveChannel.rgb:
        return copyWith(rgbCurvePoints: points);
      case CurveChannel.red:
        return copyWith(redCurvePoints: points);
      case CurveChannel.green:
        return copyWith(greenCurvePoints: points);
      case CurveChannel.blue:
        return copyWith(blueCurvePoints: points);
    }
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ColorGradingModel &&
          runtimeType == other.runtimeType &&
          temperature == other.temperature &&
          tint == other.tint &&
          exposure == other.exposure &&
          contrast == other.contrast &&
          saturation == other.saturation &&
          lift == other.lift &&
          gamma == other.gamma &&
          gain == other.gain &&
          activeCurveChannel == other.activeCurveChannel &&
          listEquals(rgbCurvePoints, other.rgbCurvePoints) &&
          listEquals(redCurvePoints, other.redCurvePoints) &&
          listEquals(greenCurvePoints, other.greenCurvePoints) &&
          listEquals(blueCurvePoints, other.blueCurvePoints);

  @override
  int get hashCode => Object.hash(
        temperature,
        tint,
        exposure,
        contrast,
        saturation,
        lift,
        gamma,
        gain,
        activeCurveChannel,
        Object.hashAll(rgbCurvePoints),
        Object.hashAll(redCurvePoints),
        Object.hashAll(greenCurvePoints),
        Object.hashAll(blueCurvePoints),
      );

  @override
  String toString() =>
      'ColorGradingModel(temp: $temperature, tint: $tint, exp: $exposure, cont: $contrast, sat: $saturation)';
}

/// Type aliases for flexible interoperability with PROJECT.md and M3 contracts.
typedef ColorGradingValues = ColorGradingModel;
typedef ClipColorGrading = ColorGradingModel;

/// Immutable state containing per-clip color grading mappings indexed by clip ID.
@immutable
class ColorGradingState {
  final Map<UuidValue, ColorGradingModel> clipGrading;

  const ColorGradingState({
    this.clipGrading = const {},
  });

  /// Retrieves the grading model for a clip, returning [ColorGradingModel.defaults] if unassigned.
  ColorGradingModel gradingFor(UuidValue clipId) {
    return clipGrading[clipId] ?? ColorGradingModel.defaults;
  }

  /// True if explicit custom grading has been saved for this clip.
  bool hasGrading(UuidValue clipId) => clipGrading.containsKey(clipId);

  ColorGradingState copyWith({
    Map<UuidValue, ColorGradingModel>? clipGrading,
  }) {
    return ColorGradingState(
      clipGrading: clipGrading ?? this.clipGrading,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ColorGradingState &&
          runtimeType == other.runtimeType &&
          mapEquals(clipGrading, other.clipGrading);

  @override
  int get hashCode => Object.hashAll(clipGrading.entries);
}
