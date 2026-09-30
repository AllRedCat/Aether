import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../selected_clip_details_provider.dart';
import 'color_grading_state.dart';

export 'color_grading_state.dart';

/// StateNotifier coordinating per-clip color grading mutations and resets.
class ColorGradingNotifier extends StateNotifier<ColorGradingState> {
  ColorGradingNotifier() : super(const ColorGradingState());

  /// Sets the entire color grading model for a clip.
  void setClipGrading(UuidValue clipId, ColorGradingModel grading) {
    final updated = Map<UuidValue, ColorGradingModel>.from(state.clipGrading);
    updated[clipId] = grading;
    state = state.copyWith(clipGrading: updated);
  }

  /// Updates specified basic adjustments for a clip, preserving other settings.
  void updateBasicAdjustments(
    UuidValue clipId, {
    double? temperature,
    double? tint,
    double? exposure,
    double? contrast,
    double? saturation,
  }) {
    final current = state.gradingFor(clipId);
    final updated = current.copyWith(
      temperature: temperature,
      tint: tint,
      exposure: exposure,
      contrast: contrast,
      saturation: saturation,
    );
    setClipGrading(clipId, updated);
  }

  // --- Granular Basic Mutators ---
  void setTemperature(UuidValue clipId, double temperature) =>
      updateBasicAdjustments(clipId, temperature: temperature);

  void updateTemperature(UuidValue clipId, double temperature) =>
      setTemperature(clipId, temperature);

  void setTint(UuidValue clipId, double tint) =>
      updateBasicAdjustments(clipId, tint: tint);

  void updateTint(UuidValue clipId, double tint) =>
      setTint(clipId, tint);

  void setExposure(UuidValue clipId, double exposure) =>
      updateBasicAdjustments(clipId, exposure: exposure);

  void updateExposure(UuidValue clipId, double exposure) =>
      setExposure(clipId, exposure);

  void setContrast(UuidValue clipId, double contrast) =>
      updateBasicAdjustments(clipId, contrast: contrast);

  void updateContrast(UuidValue clipId, double contrast) =>
      setContrast(clipId, contrast);

  void setSaturation(UuidValue clipId, double saturation) =>
      updateBasicAdjustments(clipId, saturation: saturation);

  void updateSaturation(UuidValue clipId, double saturation) =>
      setSaturation(clipId, saturation);

  // --- 3-Way Color Wheels Mutators ---
  void setLift(UuidValue clipId, ColorWheelOffset lift) {
    final current = state.gradingFor(clipId);
    setClipGrading(clipId, current.copyWith(lift: lift));
  }

  void setGamma(UuidValue clipId, ColorWheelOffset gamma) {
    final current = state.gradingFor(clipId);
    setClipGrading(clipId, current.copyWith(gamma: gamma));
  }

  void setGain(UuidValue clipId, ColorWheelOffset gain) {
    final current = state.gradingFor(clipId);
    setClipGrading(clipId, current.copyWith(gain: gain));
  }

  void updateLift(UuidValue clipId, {double? x, double? y, double? luminance}) {
    final current = state.gradingFor(clipId);
    setLift(clipId, current.lift.copyWith(x: x, y: y, luminance: luminance));
  }

  void updateGamma(UuidValue clipId, {double? x, double? y, double? luminance}) {
    final current = state.gradingFor(clipId);
    setGamma(clipId, current.gamma.copyWith(x: x, y: y, luminance: luminance));
  }

  void updateGain(UuidValue clipId, {double? x, double? y, double? luminance}) {
    final current = state.gradingFor(clipId);
    setGain(clipId, current.gain.copyWith(x: x, y: y, luminance: luminance));
  }

  // --- Tone Curves Mutators ---
  void setActiveCurveChannel(UuidValue clipId, CurveChannel channel) {
    final current = state.gradingFor(clipId);
    setClipGrading(clipId, current.copyWith(activeCurveChannel: channel));
  }

  void setCurvePoints(UuidValue clipId, CurveChannel channel, List<Offset> points) {
    final current = state.gradingFor(clipId);
    final sorted = List<Offset>.from(points)..sort((a, b) => a.dx.compareTo(b.dx));
    setClipGrading(clipId, current.withCurvePoints(channel, List.unmodifiable(sorted)));
  }

  void addCurvePoint(UuidValue clipId, CurveChannel channel, Offset point) {
    final current = state.gradingFor(clipId);
    final points = List<Offset>.from(current.pointsForChannel(channel))..add(point);
    setCurvePoints(clipId, channel, points);
  }

  void removeCurvePoint(UuidValue clipId, CurveChannel channel, int index) {
    final current = state.gradingFor(clipId);
    final points = List<Offset>.from(current.pointsForChannel(channel));
    if (index >= 0 && index < points.length && points.length > 2) {
      points.removeAt(index);
      setCurvePoints(clipId, channel, points);
    }
  }

  // --- Resets ---
  void resetTemperature(UuidValue clipId) =>
      setTemperature(clipId, ColorGradingModel.defaults.temperature);

  void resetTint(UuidValue clipId) =>
      setTint(clipId, ColorGradingModel.defaults.tint);

  void resetExposure(UuidValue clipId) =>
      setExposure(clipId, ColorGradingModel.defaults.exposure);

  void resetContrast(UuidValue clipId) =>
      setContrast(clipId, ColorGradingModel.defaults.contrast);

  void resetSaturation(UuidValue clipId) =>
      setSaturation(clipId, ColorGradingModel.defaults.saturation);

  void resetBasicAdjustments(UuidValue clipId) {
    final current = state.gradingFor(clipId);
    setClipGrading(
      clipId,
      current.copyWith(
        temperature: ColorGradingModel.defaults.temperature,
        tint: ColorGradingModel.defaults.tint,
        exposure: ColorGradingModel.defaults.exposure,
        contrast: ColorGradingModel.defaults.contrast,
        saturation: ColorGradingModel.defaults.saturation,
      ),
    );
  }

  void resetBasicColor(UuidValue clipId) => resetBasicAdjustments(clipId);

  void resetLift(UuidValue clipId) => setLift(clipId, ColorWheelOffset.zero);
  void resetGamma(UuidValue clipId) => setGamma(clipId, ColorWheelOffset.zero);
  void resetGain(UuidValue clipId) => setGain(clipId, ColorWheelOffset.zero);

  void resetWheels(UuidValue clipId) {
    final current = state.gradingFor(clipId);
    setClipGrading(
      clipId,
      current.copyWith(
        lift: ColorWheelOffset.zero,
        gamma: ColorWheelOffset.zero,
        gain: ColorWheelOffset.zero,
      ),
    );
  }

  void resetAllWheels(UuidValue clipId) => resetWheels(clipId);

  void resetCurveChannel(UuidValue clipId, CurveChannel channel) {
    setCurvePoints(clipId, channel, ColorGradingModel.defaultCurvePoints);
  }

  void resetAllCurves(UuidValue clipId) {
    final current = state.gradingFor(clipId);
    setClipGrading(
      clipId,
      current.copyWith(
        rgbCurvePoints: ColorGradingModel.defaultCurvePoints,
        redCurvePoints: ColorGradingModel.defaultCurvePoints,
        greenCurvePoints: ColorGradingModel.defaultCurvePoints,
        blueCurvePoints: ColorGradingModel.defaultCurvePoints,
      ),
    );
  }

  /// Resets all grading parameters for a clip to default (removes from map).
  void resetClipGrading(UuidValue clipId) {
    final updated = Map<UuidValue, ColorGradingModel>.from(state.clipGrading);
    updated.remove(clipId);
    state = state.copyWith(clipGrading: updated);
  }

  /// Clears stored grading parameters when a clip is permanently removed from timeline.
  void removeClip(UuidValue clipId) => resetClipGrading(clipId);
}

/// Riverpod StateNotifierProvider managing per-clip color grading.
final colorGradingProvider =
    StateNotifierProvider<ColorGradingNotifier, ColorGradingState>((ref) {
  return ColorGradingNotifier();
});

/// Convenience computed provider returning the active clip's [ColorGradingModel].
final activeColorGradingProvider = Provider<ColorGradingModel>((ref) {
  final details = ref.watch(selectedClipDetailsProvider);
  if (details == null) return ColorGradingModel.defaults;
  final gradingState = ref.watch(colorGradingProvider);
  return gradingState.gradingFor(details.clip.id);
});

/// Alias provider for semantic consistency.
final activeClipColorGradingProvider = activeColorGradingProvider;

/// Granular selectors to prevent unnecessary widget rebuilds:
final activeClipTemperatureProvider = Provider<double>((ref) {
  return ref.watch(activeColorGradingProvider).temperature;
});

final activeClipTintProvider = Provider<double>((ref) {
  return ref.watch(activeColorGradingProvider).tint;
});

final activeClipExposureProvider = Provider<double>((ref) {
  return ref.watch(activeColorGradingProvider).exposure;
});

final activeClipContrastProvider = Provider<double>((ref) {
  return ref.watch(activeColorGradingProvider).contrast;
});

final activeClipSaturationProvider = Provider<double>((ref) {
  return ref.watch(activeColorGradingProvider).saturation;
});

final activeClipLiftProvider = Provider<ColorWheelOffset>((ref) {
  return ref.watch(activeColorGradingProvider).lift;
});

final activeClipGammaProvider = Provider<ColorWheelOffset>((ref) {
  return ref.watch(activeColorGradingProvider).gamma;
});

final activeClipGainProvider = Provider<ColorWheelOffset>((ref) {
  return ref.watch(activeColorGradingProvider).gain;
});

final activeClipCurvePointsProvider = Provider<List<Offset>>((ref) {
  return ref.watch(activeColorGradingProvider).curvePoints;
});

final activeClipActiveCurveChannelProvider = Provider<CurveChannel>((ref) {
  return ref.watch(activeColorGradingProvider).activeCurveChannel;
});

final hasActiveClipGradingModificationsProvider = Provider<bool>((ref) {
  return !ref.watch(activeColorGradingProvider).isDefault;
});
