import 'package:flutter/material.dart';
import '../../../theme/catppuccin.dart';
import 'panel_descriptor.dart';

class EditorPanelContainer extends StatelessWidget {
  final EditorPanelDescriptor descriptor;
  final Widget? child;

  const EditorPanelContainer({
    super.key,
    required this.descriptor,
    this.child,
  });

  @override
  Widget build(BuildContext context) {
    final panelContent = child ?? descriptor.builder(context);

    return Container(
      key: Key('panel_container_${descriptor.id.name}'),
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        color: CatppuccinMocha.base,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: CatppuccinMocha.surface0, width: 1),
      ),
      child: descriptor.showHeader
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _buildHeader(context),
                Expanded(
                  child: ClipRect(
                    key: Key('panel_body_${descriptor.id.name}'),
                    child: panelContent,
                  ),
                ),
              ],
            )
          : ClipRect(
              key: Key('panel_body_${descriptor.id.name}'),
              child: panelContent,
            ),
    );
  }

  Widget _buildHeader(BuildContext context) {
    final actions = descriptor.actionsBuilder?.call(context) ?? const [];

    return Container(
      key: Key('panel_header_${descriptor.id.name}'),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
      decoration: const BoxDecoration(
        color: CatppuccinMocha.mantle,
        border: Border(
          bottom: BorderSide(color: CatppuccinMocha.surface0, width: 1),
        ),
      ),
      child: Row(
        children: [
          Icon(
            descriptor.icon,
            size: 14,
            color: CatppuccinMocha.overlay0,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              descriptor.title.toUpperCase(),
              key: Key('panel_title_${descriptor.id.name}'),
              overflow: TextOverflow.ellipsis,
              maxLines: 1,
              style: const TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.bold,
                color: CatppuccinMocha.overlay0,
                letterSpacing: 0.5,
              ),
            ),
          ),
          if (actions.isNotEmpty) ...[
            const SizedBox(width: 8),
            ...actions,
          ],
        ],
      ),
    );
  }
}
