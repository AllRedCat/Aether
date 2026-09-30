import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../../../theme/catppuccin.dart';
import '../color_grading_provider.dart';
import 'color_wheel_widget.dart';

/// 3-Way Color Wheels View (Lift, Gamma, Gain) for tonal color balancing.
/// Features:
/// - Lift (Shadows / CatppuccinMocha.sapphire)
/// - Gamma (Midtones / CatppuccinMocha.peach)
/// - Gain (Highlights / CatppuccinMocha.yellow)
/// - Responsive layout adapting between side-by-side row (wide) and wrapped/stacked cards (narrow).
/// - Zero RenderFlex overflow down to 200px viewport width.
/// - Full Riverpod state integration via [clipId] with standalone override support.
class ColorWheelsView extends ConsumerWidget {
  final UuidValue? clipId;

  // Standalone properties (optional for isolated testing)
  final ColorWheelOffset? lift;
  final ColorWheelOffset? gamma;
  final ColorWheelOffset? gain;

  final void Function(double x, double y)? onLiftOffsetChanged;
  final ValueChanged<double>? onLiftLumaChanged;
  final VoidCallback? onLiftResetOffset;
  final VoidCallback? onLiftResetLuma;
  final VoidCallback? onLiftResetAll;

  final void Function(double x, double y)? onGammaOffsetChanged;
  final ValueChanged<double>? onGammaLumaChanged;
  final VoidCallback? onGammaResetOffset;
  final VoidCallback? onGammaResetLuma;
  final VoidCallback? onGammaResetAll;

  final void Function(double x, double y)? onGainOffsetChanged;
  final ValueChanged<double>? onGainLumaChanged;
  final VoidCallback? onGainResetOffset;
  final VoidCallback? onGainResetLuma;
  final VoidCallback? onGainResetAll;

  final VoidCallback? onResetAllWheels;

  const ColorWheelsView({
    super.key,
    this.clipId,
    this.lift,
    this.gamma,
    this.gain,
    this.onLiftOffsetChanged,
    this.onLiftLumaChanged,
    this.onLiftResetOffset,
    this.onLiftResetLuma,
    this.onLiftResetAll,
    this.onGammaOffsetChanged,
    this.onGammaLumaChanged,
    this.onGammaResetOffset,
    this.onGammaResetLuma,
    this.onGammaResetAll,
    this.onGainOffsetChanged,
    this.onGainLumaChanged,
    this.onGainResetOffset,
    this.onGainResetLuma,
    this.onGainResetAll,
    this.onResetAllWheels,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    ColorWheelOffset currentLift = lift ?? ColorWheelOffset.defaults;
    ColorWheelOffset currentGamma = gamma ?? ColorWheelOffset.defaults;
    ColorWheelOffset currentGain = gain ?? ColorWheelOffset.defaults;

    ColorGradingNotifier? notifier;
    if (clipId != null) {
      try {
        final gradingState = ref.watch(colorGradingProvider);
        final clipGrading = gradingState.gradingFor(clipId!);
        currentLift = lift ?? clipGrading.lift;
        currentGamma = gamma ?? clipGrading.gamma;
        currentGain = gain ?? clipGrading.gain;
        notifier = ref.read(colorGradingProvider.notifier);
      } catch (_) {
        // Fallback gracefully in standalone headless testing
      }
    }

    final liftWidget = ColorWheelWidget(
      label: 'LIFT',
      sublabel: 'SOMBRAS',
      accentColor: CatppuccinMocha.sapphire,
      x: currentLift.x,
      y: currentLift.y,
      luminance: currentLift.luminance,
      wheelKey: const Key('color_wheel_lift'),
      diskKey: const Key('chromatic_disk_lift'),
      lumaSliderKey: const Key('slider_luma_lift'),
      lumaBadgeKey: const Key('badge_luma_lift'),
      resetLumaKey: const Key('reset_luma_lift'),
      resetWheelKey: const Key('reset_wheel_lift'),
      onOffsetChanged: (x, y) {
        if (onLiftOffsetChanged != null) {
          onLiftOffsetChanged!(x, y);
        } else if (clipId != null && notifier != null) {
          notifier.updateLift(clipId!, x: x, y: y);
        }
      },
      onLuminanceChanged: (luma) {
        if (onLiftLumaChanged != null) {
          onLiftLumaChanged!(luma);
        } else if (clipId != null && notifier != null) {
          notifier.updateLift(clipId!, luminance: luma);
        }
      },
      onResetOffset: () {
        if (onLiftResetOffset != null) {
          onLiftResetOffset!();
        } else if (clipId != null && notifier != null) {
          notifier.updateLift(clipId!, x: 0.0, y: 0.0);
        }
      },
      onResetLuminance: () {
        if (onLiftResetLuma != null) {
          onLiftResetLuma!();
        } else if (clipId != null && notifier != null) {
          notifier.updateLift(clipId!, luminance: 0.0);
        }
      },
      onResetAll: () {
        if (onLiftResetAll != null) {
          onLiftResetAll!();
        } else if (clipId != null && notifier != null) {
          notifier.resetLift(clipId!);
        }
      },
    );

    final gammaWidget = ColorWheelWidget(
      label: 'GAMMA',
      sublabel: 'MÉDIOS',
      accentColor: CatppuccinMocha.peach,
      x: currentGamma.x,
      y: currentGamma.y,
      luminance: currentGamma.luminance,
      wheelKey: const Key('color_wheel_gamma'),
      diskKey: const Key('chromatic_disk_gamma'),
      lumaSliderKey: const Key('slider_luma_gamma'),
      lumaBadgeKey: const Key('badge_luma_gamma'),
      resetLumaKey: const Key('reset_luma_gamma'),
      resetWheelKey: const Key('reset_wheel_gamma'),
      onOffsetChanged: (x, y) {
        if (onGammaOffsetChanged != null) {
          onGammaOffsetChanged!(x, y);
        } else if (clipId != null && notifier != null) {
          notifier.updateGamma(clipId!, x: x, y: y);
        }
      },
      onLuminanceChanged: (luma) {
        if (onGammaLumaChanged != null) {
          onGammaLumaChanged!(luma);
        } else if (clipId != null && notifier != null) {
          notifier.updateGamma(clipId!, luminance: luma);
        }
      },
      onResetOffset: () {
        if (onGammaResetOffset != null) {
          onGammaResetOffset!();
        } else if (clipId != null && notifier != null) {
          notifier.updateGamma(clipId!, x: 0.0, y: 0.0);
        }
      },
      onResetLuminance: () {
        if (onGammaResetLuma != null) {
          onGammaResetLuma!();
        } else if (clipId != null && notifier != null) {
          notifier.updateGamma(clipId!, luminance: 0.0);
        }
      },
      onResetAll: () {
        if (onGammaResetAll != null) {
          onGammaResetAll!();
        } else if (clipId != null && notifier != null) {
          notifier.resetGamma(clipId!);
        }
      },
    );

    final gainWidget = ColorWheelWidget(
      label: 'GAIN',
      sublabel: 'ALTOS',
      accentColor: CatppuccinMocha.yellow,
      x: currentGain.x,
      y: currentGain.y,
      luminance: currentGain.luminance,
      wheelKey: const Key('color_wheel_gain'),
      diskKey: const Key('chromatic_disk_gain'),
      lumaSliderKey: const Key('slider_luma_gain'),
      lumaBadgeKey: const Key('badge_luma_gain'),
      resetLumaKey: const Key('reset_luma_gain'),
      resetWheelKey: const Key('reset_wheel_gain'),
      onOffsetChanged: (x, y) {
        if (onGainOffsetChanged != null) {
          onGainOffsetChanged!(x, y);
        } else if (clipId != null && notifier != null) {
          notifier.updateGain(clipId!, x: x, y: y);
        }
      },
      onLuminanceChanged: (luma) {
        if (onGainLumaChanged != null) {
          onGainLumaChanged!(luma);
        } else if (clipId != null && notifier != null) {
          notifier.updateGain(clipId!, luminance: luma);
        }
      },
      onResetOffset: () {
        if (onGainResetOffset != null) {
          onGainResetOffset!();
        } else if (clipId != null && notifier != null) {
          notifier.updateGain(clipId!, x: 0.0, y: 0.0);
        }
      },
      onResetLuminance: () {
        if (onGainResetLuma != null) {
          onGainResetLuma!();
        } else if (clipId != null && notifier != null) {
          notifier.updateGain(clipId!, luminance: 0.0);
        }
      },
      onResetAll: () {
        if (onGainResetAll != null) {
          onGainResetAll!();
        } else if (clipId != null && notifier != null) {
          notifier.resetGain(clipId!);
        }
      },
    );

    return Column(
      key: const Key('color_wheels_view'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Section Header with Reset All Wheels
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
                'RODAS DE CORES 3-WAY',
                style: TextStyle(
                  color: CatppuccinMocha.mauve,
                  fontWeight: FontWeight.bold,
                  fontSize: 12,
                  letterSpacing: 0.5,
                ),
              ),
            ),
            InkWell(
              key: const Key('reset_all_wheels'),
              onTap: () {
                if (onResetAllWheels != null) {
                  onResetAllWheels!();
                } else if (clipId != null && notifier != null) {
                  notifier.resetAllWheels(clipId!);
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

        // Responsive Wheels Layout: Row on wide viewport, Wrap on medium/narrow
        LayoutBuilder(
          builder: (context, constraints) {
            final double availableWidth = constraints.maxWidth;

            if (availableWidth >= 520) {
              // Wide: 3 wheels side-by-side
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: liftWidget),
                  const SizedBox(width: 10),
                  Expanded(child: gammaWidget),
                  const SizedBox(width: 10),
                  Expanded(child: gainWidget),
                ],
              );
            } else {
              // Narrow or Medium: wrap gracefully with centered alignment
              final double cardWidth = availableWidth > 340
                  ? (availableWidth - 12) / 2
                  : availableWidth;

              return Center(
                child: Wrap(
                  alignment: WrapAlignment.center,
                  spacing: 10,
                  runSpacing: 10,
                  children: [
                    SizedBox(width: cardWidth, child: liftWidget),
                    SizedBox(width: cardWidth, child: gammaWidget),
                    SizedBox(width: cardWidth, child: gainWidget),
                  ],
                ),
              );
            }
          },
        ),
      ],
    );
  }
}
