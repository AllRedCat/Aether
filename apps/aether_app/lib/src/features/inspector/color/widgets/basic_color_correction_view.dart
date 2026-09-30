import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../../../theme/catppuccin.dart';
import '../../widgets/gradient_slider_row.dart';
import '../color_grading_provider.dart';

/// Basic Color Correction panel featuring custom gradient and accent sliders
/// for Temperature (White Balance), Tint, Exposure (EV), Contrast, and Saturation.
/// Supports both direct Riverpod state integration via [clipId] and standalone
/// parameter overrides for decoupled unit and widget testing.
class BasicColorCorrectionView extends ConsumerWidget {
  final UuidValue? clipId;

  // Standalone property overrides (optional for testing or stateless usage)
  final double? temperature;
  final double? tint;
  final double? exposure;
  final double? contrast;
  final double? saturation;

  final ValueChanged<double>? onTemperatureChanged;
  final ValueChanged<double>? onTintChanged;
  final ValueChanged<double>? onExposureChanged;
  final ValueChanged<double>? onContrastChanged;
  final ValueChanged<double>? onSaturationChanged;

  final VoidCallback? onResetTemperature;
  final VoidCallback? onResetTint;
  final VoidCallback? onResetExposure;
  final VoidCallback? onResetContrast;
  final VoidCallback? onResetSaturation;
  final VoidCallback? onResetAll;

  const BasicColorCorrectionView({
    super.key,
    this.clipId,
    this.temperature,
    this.tint,
    this.exposure,
    this.contrast,
    this.saturation,
    this.onTemperatureChanged,
    this.onTintChanged,
    this.onExposureChanged,
    this.onContrastChanged,
    this.onSaturationChanged,
    this.onResetTemperature,
    this.onResetTint,
    this.onResetExposure,
    this.onResetContrast,
    this.onResetSaturation,
    this.onResetAll,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Resolve values: explicit props have priority, otherwise query Riverpod if clipId is present
    double currentTemp = temperature ?? 0.0;
    double currentTint = tint ?? 0.0;
    double currentExposure = exposure ?? 0.0;
    double currentContrast = contrast ?? 0.0;
    double currentSaturation = saturation ?? 100.0;

    ColorGradingNotifier? notifier;
    if (clipId != null) {
      try {
        final gradingState = ref.watch(colorGradingProvider);
        final clipGrading = gradingState.gradingFor(clipId!);
        currentTemp = temperature ?? clipGrading.temperature;
        currentTint = tint ?? clipGrading.tint;
        currentExposure = exposure ?? clipGrading.exposure;
        currentContrast = contrast ?? clipGrading.contrast;
        currentSaturation = saturation ?? clipGrading.saturation;
        notifier = ref.read(colorGradingProvider.notifier);
      } catch (_) {
        // Fallback gracefully if provider is not registered in mock tree
      }
    }

    final tempText = '${currentTemp >= 0 ? '+' : ''}${currentTemp.toStringAsFixed(1)}';
    final tintText = '${currentTint >= 0 ? '+' : ''}${currentTint.toStringAsFixed(1)}';
    final exposureText = '${currentExposure >= 0 ? '+' : ''}${currentExposure.toStringAsFixed(2)} EV';
    final contrastText = '${currentContrast >= 0 ? '+' : ''}${currentContrast.toStringAsFixed(1)}';
    final saturationText = '${currentSaturation.toStringAsFixed(0)}%';

    return Column(
      key: const Key('basic_color_correction_view'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Section Header with Reset All
        Row(
          children: [
            Container(
              width: 3,
              height: 12,
              decoration: BoxDecoration(
                color: CatppuccinMocha.mauve,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
            const SizedBox(width: 8),
            const Expanded(
              child: Text(
                'AJUSTES BÁSICOS',
                style: TextStyle(
                  color: CatppuccinMocha.mauve,
                  fontWeight: FontWeight.bold,
                  fontSize: 12,
                  letterSpacing: 0.5,
                ),
              ),
            ),
            InkWell(
              key: const Key('reset_all_basic_color'),
              onTap: () {
                if (onResetAll != null) {
                  onResetAll!();
                } else if (clipId != null && notifier != null) {
                  notifier.resetBasicColor(clipId!);
                }
              },
              borderRadius: BorderRadius.circular(4),
              child: const Padding(
                padding: EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                child: Row(
                  children: [
                    Icon(Icons.restart_alt_rounded, size: 13, color: CatppuccinMocha.overlay0),
                    SizedBox(width: 4),
                    Text(
                      'Reset All',
                      style: TextStyle(color: CatppuccinMocha.overlay0, fontSize: 11),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 14),

        // Temperature (White Balance) Slider: Blue -> Peach Gradient
        GradientSliderRow(
          label: 'Temperatura (Kelvin)',
          value: currentTemp,
          min: -100.0,
          max: 100.0,
          formattedValue: tempText,
          gradient: const LinearGradient(
            colors: [CatppuccinMocha.blue, CatppuccinMocha.peach],
          ),
          sliderKey: const Key('slider_temperature'),
          badgeKey: const Key('badge_temperature'),
          resetKey: const Key('reset_temperature'),
          onChanged: (val) {
            if (onTemperatureChanged != null) {
              onTemperatureChanged!(val);
            } else if (clipId != null && notifier != null) {
              notifier.updateTemperature(clipId!, val);
            }
          },
          onReset: () {
            if (onResetTemperature != null) {
              onResetTemperature!();
            } else if (clipId != null && notifier != null) {
              notifier.resetTemperature(clipId!);
            }
          },
        ),

        // Tint Slider: Green -> Pink/Magenta Gradient
        GradientSliderRow(
          label: 'Matiz (Tint)',
          value: currentTint,
          min: -100.0,
          max: 100.0,
          formattedValue: tintText,
          gradient: const LinearGradient(
            colors: [CatppuccinMocha.green, CatppuccinMocha.pink],
          ),
          sliderKey: const Key('slider_tint'),
          badgeKey: const Key('badge_tint'),
          resetKey: const Key('reset_tint'),
          onChanged: (val) {
            if (onTintChanged != null) {
              onTintChanged!(val);
            } else if (clipId != null && notifier != null) {
              notifier.updateTint(clipId!, val);
            }
          },
          onReset: () {
            if (onResetTint != null) {
              onResetTint!();
            } else if (clipId != null && notifier != null) {
              notifier.resetTint(clipId!);
            }
          },
        ),

        // Exposure Slider: Yellow Accent (-5.0..+5.0 EV)
        GradientSliderRow(
          label: 'Exposição (EV)',
          value: currentExposure,
          min: -5.0,
          max: 5.0,
          formattedValue: exposureText,
          activeColor: CatppuccinMocha.yellow,
          sliderKey: const Key('slider_exposure'),
          badgeKey: const Key('badge_exposure'),
          resetKey: const Key('reset_exposure'),
          onChanged: (val) {
            if (onExposureChanged != null) {
              onExposureChanged!(val);
            } else if (clipId != null && notifier != null) {
              notifier.updateExposure(clipId!, val);
            }
          },
          onReset: () {
            if (onResetExposure != null) {
              onResetExposure!();
            } else if (clipId != null && notifier != null) {
              notifier.resetExposure(clipId!);
            }
          },
        ),

        // Contrast Slider: Maroon/Text Accent (-100..+100)
        GradientSliderRow(
          label: 'Contraste',
          value: currentContrast,
          min: -100.0,
          max: 100.0,
          formattedValue: contrastText,
          activeColor: CatppuccinMocha.maroon,
          sliderKey: const Key('slider_contrast'),
          badgeKey: const Key('badge_contrast'),
          resetKey: const Key('reset_contrast'),
          onChanged: (val) {
            if (onContrastChanged != null) {
              onContrastChanged!(val);
            } else if (clipId != null && notifier != null) {
              notifier.updateContrast(clipId!, val);
            }
          },
          onReset: () {
            if (onResetContrast != null) {
              onResetContrast!();
            } else if (clipId != null && notifier != null) {
              notifier.resetContrast(clipId!);
            }
          },
        ),

        // Saturation Slider: Teal Accent (0..200%, default 100%)
        GradientSliderRow(
          label: 'Saturação',
          value: currentSaturation,
          min: 0.0,
          max: 200.0,
          formattedValue: saturationText,
          activeColor: CatppuccinMocha.teal,
          sliderKey: const Key('slider_saturation'),
          badgeKey: const Key('badge_saturation'),
          resetKey: const Key('reset_saturation'),
          onChanged: (val) {
            if (onSaturationChanged != null) {
              onSaturationChanged!(val);
            } else if (clipId != null && notifier != null) {
              notifier.updateSaturation(clipId!, val);
            }
          },
          onReset: () {
            if (onResetSaturation != null) {
              onResetSaturation!();
            } else if (clipId != null && notifier != null) {
              notifier.resetSaturation(clipId!);
            }
          },
        ),
      ],
    );
  }
}
