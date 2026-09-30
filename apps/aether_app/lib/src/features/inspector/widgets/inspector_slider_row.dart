import 'package:flutter/material.dart';
import '../../../theme/catppuccin.dart';

/// Reusable NLE property slider row adhering to Catppuccin Mocha styling.
/// Combines a parameter label, formatted monospace value badge, individual reset button,
/// and interactive slider with custom track/thumb styling.
class InspectorSliderRow extends StatelessWidget {
  final String label;
  final double value;
  final double min;
  final double max;
  final ValueChanged<double> onChanged;
  final VoidCallback onReset;
  final String formattedValue;
  final Key sliderKey;
  final Key badgeKey;
  final Key resetKey;
  final Color activeColor;
  final int? divisions;

  const InspectorSliderRow({
    super.key,
    required this.label,
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
    required this.onReset,
    required this.formattedValue,
    required this.sliderKey,
    required this.badgeKey,
    required this.resetKey,
    this.activeColor = CatppuccinMocha.mauve,
    this.divisions,
  });

  @override
  Widget build(BuildContext context) {
    final clampedValue = value.clamp(min, max);

    return Padding(
      padding: const EdgeInsets.only(bottom: 12.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  label,
                  style: const TextStyle(
                    color: CatppuccinMocha.subtext0,
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              Container(
                key: badgeKey,
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: CatppuccinMocha.surface0,
                  borderRadius: BorderRadius.circular(4),
                  border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
                ),
                child: Text(
                  formattedValue,
                  style: const TextStyle(
                    color: CatppuccinMocha.text,
                    fontFamily: 'monospace',
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              const SizedBox(width: 4),
              InkWell(
                key: resetKey,
                onTap: onReset,
                borderRadius: BorderRadius.circular(4),
                child: const Padding(
                  padding: EdgeInsets.all(3.0),
                  child: Icon(
                    Icons.restart_alt_rounded,
                    size: 14,
                    color: CatppuccinMocha.overlay0,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 2),
          SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 3.5,
              activeTrackColor: activeColor,
              inactiveTrackColor: CatppuccinMocha.surface0,
              thumbColor: CatppuccinMocha.text,
              overlayColor: activeColor.withValues(alpha: 0.15),
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6.0),
              overlayShape: const RoundSliderOverlayShape(overlayRadius: 12.0),
            ),
            child: Slider(
              key: sliderKey,
              value: clampedValue,
              min: min,
              max: max,
              divisions: divisions,
              onChanged: onChanged,
            ),
          ),
        ],
      ),
    );
  }
}
