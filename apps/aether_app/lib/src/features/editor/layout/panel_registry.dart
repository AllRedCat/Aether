import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../inspector/inspector_view.dart';
import '../../media_pool/media_pool_view.dart';
import '../../preview/preview_view.dart';
import '../../timeline/timeline_view.dart';
import 'panel_descriptor.dart';

class EditorPanelRegistry extends ChangeNotifier {
  final Map<EditorPanelId, EditorPanelDescriptor> _descriptors = {};

  EditorPanelRegistry([Map<EditorPanelId, EditorPanelDescriptor>? initial]) {
    if (initial != null) {
      _descriptors.addAll(initial);
    }
  }

  factory EditorPanelRegistry.standard() {
    final registry = EditorPanelRegistry();

    registry.register(
      const EditorPanelDescriptor(
        id: EditorPanelId.mediaPool,
        title: 'Media Pool',
        icon: Icons.video_library_rounded,
        builder: _buildMediaPool,
        showHeader: false,
        minWidth: 300.0,
        minHeight: 140.0,
      ),
    );

    registry.register(
      const EditorPanelDescriptor(
        id: EditorPanelId.preview,
        title: 'Preview',
        icon: Icons.monitor_rounded,
        builder: _buildPreview,
        showHeader: true,
        minWidth: 240.0,
        minHeight: 160.0,
      ),
    );

    registry.register(
      const EditorPanelDescriptor(
        id: EditorPanelId.inspector,
        title: 'Inspector',
        icon: Icons.tune_rounded,
        builder: _buildInspector,
        showHeader: true,
        minWidth: 200.0,
        minHeight: 140.0,
      ),
    );

    registry.register(
      const EditorPanelDescriptor(
        id: EditorPanelId.timeline,
        title: 'Timeline',
        icon: Icons.view_timeline_rounded,
        builder: _buildTimeline,
        showHeader: false,
        minWidth: 300.0,
        minHeight: 120.0,
      ),
    );

    registry.register(
      const EditorPanelDescriptor(
        id: EditorPanelId.colorGrading,
        title: 'Color Grading',
        icon: Icons.palette_rounded,
        builder: _buildColorGradingPlaceholder,
        showHeader: true,
        minWidth: 240.0,
        minHeight: 160.0,
      ),
    );

    return registry;
  }

  static Widget _buildMediaPool(BuildContext context) => const MediaPoolView();
  static Widget _buildPreview(BuildContext context) => const PreviewView();
  static Widget _buildInspector(BuildContext context) => const InspectorView();
  static Widget _buildTimeline(BuildContext context) => const TimelineView();
  static Widget _buildColorGradingPlaceholder(BuildContext context) =>
      const Center(
        key: Key('color_grading_placeholder'),
        child: Text(
          'Painel de Correção de Cores (M3)',
          style: TextStyle(color: Colors.white54, fontSize: 13),
        ),
      );

  void register(EditorPanelDescriptor descriptor) {
    _descriptors[descriptor.id] = descriptor;
    notifyListeners();
  }

  bool unregister(EditorPanelId id) {
    final removed = _descriptors.remove(id) != null;
    if (removed) {
      notifyListeners();
    }
    return removed;
  }

  EditorPanelDescriptor get(EditorPanelId id) {
    final descriptor = _descriptors[id];
    if (descriptor == null) {
      throw StateError('Panel descriptor for $id not found in EditorPanelRegistry.');
    }
    return descriptor;
  }

  EditorPanelDescriptor? maybeGet(EditorPanelId id) => _descriptors[id];
  bool has(EditorPanelId id) => _descriptors.containsKey(id);
  List<EditorPanelDescriptor> getAll() => List.unmodifiable(_descriptors.values);
  void clear() {
    _descriptors.clear();
    notifyListeners();
  }
}

final editorPanelRegistryProvider =
    ChangeNotifierProvider<EditorPanelRegistry>((ref) {
  return EditorPanelRegistry.standard();
});
