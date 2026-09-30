import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/catppuccin.dart';
import '../inspector_mode_provider.dart';

/// Segmented mode switcher toggle for the Inspector panel header [Propriedades | Cor].
/// Styled strictly with the CatppuccinMocha palette:
/// - Background container: `CatppuccinMocha.surface0`
/// - Active segment background: `CatppuccinMocha.surface1`
/// - Active accent: `CatppuccinMocha.mauve`
/// - Text: `CatppuccinMocha.text`
/// - Inactive text: `CatppuccinMocha.subtext0`
class InspectorModeToggle extends ConsumerWidget {
  const InspectorModeToggle({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final currentMode = ref.watch(inspectorModeProvider);

    return Container(
      key: const Key('inspector_mode_toggle'),
      height: 24,
      padding: const EdgeInsets.all(2),
      decoration: BoxDecoration(
        color: CatppuccinMocha.surface0,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          _buildSegment(
            context: context,
            ref: ref,
            mode: InspectorMode.properties,
            label: 'Propriedades',
            icon: Icons.tune_rounded,
            isSelected: currentMode == InspectorMode.properties,
            segmentKey: const Key('inspector_mode_properties_btn'),
          ),
          const SizedBox(width: 2),
          _buildSegment(
            context: context,
            ref: ref,
            mode: InspectorMode.color,
            label: 'Cor',
            icon: Icons.palette_rounded,
            isSelected: currentMode == InspectorMode.color,
            segmentKey: const Key('inspector_mode_color_btn'),
          ),
        ],
      ),
    );
  }

  Widget _buildSegment({
    required BuildContext context,
    required WidgetRef ref,
    required InspectorMode mode,
    required String label,
    required IconData icon,
    required bool isSelected,
    required Key segmentKey,
  }) {
    return InkWell(
      key: segmentKey,
      onTap: isSelected
          ? null
          : () {
              ref.read(inspectorModeProvider.notifier).state = mode;
            },
      borderRadius: BorderRadius.circular(4),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        curve: Curves.easeOut,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        decoration: BoxDecoration(
          color: isSelected ? CatppuccinMocha.surface1 : Colors.transparent,
          borderRadius: BorderRadius.circular(4),
          border: isSelected
              ? Border.all(
                  color: CatppuccinMocha.mauve.withValues(alpha: 0.4),
                  width: 0.5,
                )
              : null,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              icon,
              size: 11,
              color: isSelected ? CatppuccinMocha.mauve : CatppuccinMocha.subtext0,
            ),
            const SizedBox(width: 4),
            Text(
              label,
              style: TextStyle(
                color: isSelected ? CatppuccinMocha.mauve : CatppuccinMocha.subtext0,
                fontSize: 10,
                fontWeight: isSelected ? FontWeight.w600 : FontWeight.normal,
                letterSpacing: 0.2,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
