import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../theme/catppuccin.dart';
import 'layout/editor_layout_state.dart';
import 'layout/panel_container.dart';
import 'layout/panel_registry.dart';
import 'layout/widgets/resizable_split_view.dart';
import 'layout/widgets/workspace_preset_bar.dart';

class EditorScreen extends ConsumerWidget {
  const EditorScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final layoutState = ref.watch(editorLayoutProvider);
    final layoutNotifier = ref.read(editorLayoutProvider.notifier);
    final registry = ref.watch(editorPanelRegistryProvider);

    // Build top panels
    final topWidgets = layoutState.topPanels.map((id) {
      final descriptor = registry.maybeGet(id);
      if (descriptor == null) {
        return Center(child: Text('Panel $id not found'));
      }
      return EditorPanelContainer(descriptor: descriptor);
    }).toList();

    // Build bottom panels
    final bottomWidgets = layoutState.bottomPanels.map((id) {
      final descriptor = registry.maybeGet(id);
      if (descriptor == null) {
        return Center(child: Text('Panel $id not found'));
      }
      return EditorPanelContainer(descriptor: descriptor);
    }).toList();

    final topMinSizes = layoutState.topPanels
        .map((id) => registry.maybeGet(id)?.minWidth ?? 120.0)
        .toList();
    final bottomMinSizes = layoutState.bottomPanels
        .map((id) => registry.maybeGet(id)?.minWidth ?? 120.0)
        .toList();

    return Scaffold(
      backgroundColor: CatppuccinMocha.crust,
      body: LayoutBuilder(
        builder: (context, constraints) {
          final barWidth =
              constraints.maxWidth >= 520 ? constraints.maxWidth : 520.0;
          return Column(
            children: [
              // 1. Workspace Presets Bar [AETHER | Edição | Cores | Áudio | Reset]
              Container(
                height: 38,
                width: constraints.maxWidth,
                margin: const EdgeInsets.only(bottom: 4),
                child: FittedBox(
                  fit: BoxFit.scaleDown,
                  alignment: Alignment.centerLeft,
                  child: SizedBox(
                    width: barWidth,
                    height: 38,
                    child: const WorkspacePresetBar(),
                  ),
                ),
              ),

              // 2. 2D Resizable Modular Workspace
              Expanded( 
                child: ResizableSplitView(
                  axis: Axis.vertical,
                  ratio: layoutState.verticalRatio,
                  minFirstSize: 240.0,
                  minSecondSize: 120.0,
                  onRatioChanged: (ratio) =>
                      layoutNotifier.updateVerticalRatio(ratio),
                  onReset: () => layoutNotifier.resetToPresetDefaults(),
                  first: ResizableSplitView.multi(
                    dividerKeyPrefix: layoutState.preset == LayoutPreset.color
                        ? 'top_'
                        : null,
                    axis: Axis.horizontal,
                    weights: layoutState.topWeights,
                    minSizes: topMinSizes,
                    onWeightsChanged: (weights) =>
                        layoutNotifier.updateTopWeights(weights),
                    onReset: () => layoutNotifier.resetToPresetDefaults(),
                    children: topWidgets,
                  ),
                  second: ResizableSplitView.multi(
                    dividerKeyPrefix: layoutState.preset == LayoutPreset.color
                        ? 'bottom_'
                        : null,
                    axis: Axis.horizontal,
                    weights: layoutState.bottomWeights,
                    minSizes: bottomMinSizes,
                    onWeightsChanged: (weights) =>
                        layoutNotifier.updateBottomWeights(weights),
                    onReset: () => layoutNotifier.resetToPresetDefaults(),
                    children: bottomWidgets,
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}
