import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../theme/catppuccin.dart';
import '../editor_layout_state.dart';

/// Workspace preset switcher toolbar displayed at the top of the editor.
/// Allows instant switching between [LayoutPreset.editing], [LayoutPreset.color],
/// and [LayoutPreset.audio], plus quick layout reset.
class WorkspacePresetBar extends ConsumerWidget {
  const WorkspacePresetBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final layoutState = ref.watch(editorLayoutProvider);
    final notifier = ref.read(editorLayoutProvider.notifier);

    return Container(
      height: 38,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: const BoxDecoration(
        color: CatppuccinMocha.crust,
        border: Border(
          bottom: BorderSide(color: CatppuccinMocha.surface0, width: 1),
        ),
      ),
      child: Row(
        children: [
          // Branding badge
          const Icon(
            Icons.movie_creation_rounded,
            size: 16,
            color: CatppuccinMocha.mauve,
          ),
          const SizedBox(width: 8),
          const Text(
            'AETHER',
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              letterSpacing: 1.2,
              color: CatppuccinMocha.text,
            ),
          ),
          const SizedBox(width: 14),
          Container(width: 1, height: 16, color: CatppuccinMocha.surface0),
          const SizedBox(width: 14),

          // Preset toggle buttons
          for (final preset in LayoutPreset.values) ...[
            _PresetButton(
              preset: preset,
              isSelected: layoutState.preset == preset,
              onTap: () => notifier.setPreset(preset),
            ),
            const SizedBox(width: 6),
          ],

          const Spacer(),

          // Quick reset button
          Tooltip(
            message: 'Redefinir proporções do layout para o padrão',
            child: InkWell(
              key: const Key('reset_layout_button'),
              borderRadius: BorderRadius.circular(4),
              onTap: () => notifier.resetToPresetDefaults(),
              child: const Padding(
                padding: EdgeInsets.all(6.0),
                child: Icon(
                  Icons.refresh_rounded,
                  size: 15,
                  color: CatppuccinMocha.overlay0,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _PresetButton extends StatelessWidget {
  final LayoutPreset preset;
  final bool isSelected;
  final VoidCallback onTap;

  const _PresetButton({
    required this.preset,
    required this.isSelected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      key: Key('preset_button_${preset.name}'),
      onTap: onTap,
      borderRadius: BorderRadius.circular(6),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 140),
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
          color: isSelected ? CatppuccinMocha.surface0 : Colors.transparent,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
            color: isSelected
                ? CatppuccinMocha.mauve.withValues(alpha: 0.5)
                : Colors.transparent,
            width: 1,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              preset.icon,
              size: 13,
              color: isSelected
                  ? CatppuccinMocha.mauve
                  : CatppuccinMocha.overlay0,
            ),
            const SizedBox(width: 6),
            Text(
              preset.label,
              style: TextStyle(
                fontSize: 11,
                fontWeight: isSelected ? FontWeight.w600 : FontWeight.normal,
                color: isSelected
                    ? CatppuccinMocha.text
                    : CatppuccinMocha.subtext0,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
