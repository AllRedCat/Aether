import 'package:flutter/material.dart';

/// Unique identifiers for standard and extensible editor panels.
class EditorPanelId {
  final String value;
  const EditorPanelId(this.value);

  static const EditorPanelId mediaPool = EditorPanelId('mediaPool');
  static const EditorPanelId preview = EditorPanelId('preview');
  static const EditorPanelId inspector = EditorPanelId('inspector');
  static const EditorPanelId timeline = EditorPanelId('timeline');
  static const EditorPanelId colorGrading = EditorPanelId('colorGrading');

  const EditorPanelId.custom(String name) : value = name;

  String get name => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is EditorPanelId &&
          runtimeType == other.runtimeType &&
          value == other.value;

  @override
  int get hashCode => value.hashCode;

  @override
  String toString() => 'EditorPanelId($value)';
}

/// Content builder for an editor panel.
typedef PanelWidgetBuilder = Widget Function(BuildContext context);

/// Header action widgets builder for an editor panel.
typedef PanelActionsBuilder = List<Widget> Function(BuildContext context);

/// Self-contained descriptor defining an editor panel's identity,
/// header chrome, builder callbacks, and minimum resizing constraints.
@immutable
class EditorPanelDescriptor {
  final EditorPanelId id;
  final String title;
  final IconData icon;
  final PanelWidgetBuilder builder;
  final PanelActionsBuilder? actionsBuilder;
  final bool showHeader;
  final double minWidth;
  final double minHeight;

  const EditorPanelDescriptor({
    required this.id,
    required this.title,
    required this.icon,
    required this.builder,
    this.actionsBuilder,
    this.showHeader = true,
    this.minWidth = 180.0,
    this.minHeight = 120.0,
  });

  EditorPanelDescriptor copyWith({
    EditorPanelId? id,
    String? title,
    IconData? icon,
    PanelWidgetBuilder? builder,
    PanelActionsBuilder? actionsBuilder,
    bool? showHeader,
    double? minWidth,
    double? minHeight,
  }) {
    return EditorPanelDescriptor(
      id: id ?? this.id,
      title: title ?? this.title,
      icon: icon ?? this.icon,
      builder: builder ?? this.builder,
      actionsBuilder: actionsBuilder ?? this.actionsBuilder,
      showHeader: showHeader ?? this.showHeader,
      minWidth: minWidth ?? this.minWidth,
      minHeight: minHeight ?? this.minHeight,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is EditorPanelDescriptor &&
          runtimeType == other.runtimeType &&
          id == other.id;

  @override
  int get hashCode => id.hashCode;

  @override
  String toString() =>
      'EditorPanelDescriptor(id: $id, title: "$title", showHeader: $showHeader)';
}
