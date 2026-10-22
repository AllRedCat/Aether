import 'dart:io';

/// Represents a recently opened or created Aether project bundle.
class RecentProject {
  final String name;
  final String bundlePath;
  final DateTime lastOpenedAt;

  const RecentProject({
    required this.name,
    required this.bundlePath,
    required this.lastOpenedAt,
  });

  /// Returns true if `<bundlePath>/project.db` exists on disk.
  bool get existsOnDisk => File('$bundlePath/project.db').existsSync();

  Map<String, dynamic> toJson() => {
        'name': name,
        'bundlePath': bundlePath,
        'lastOpenedAt': lastOpenedAt.toIso8601String(),
      };

  factory RecentProject.fromJson(Map<String, dynamic> json) {
    return RecentProject(
      name: json['name'] as String,
      bundlePath: json['bundlePath'] as String,
      lastOpenedAt: DateTime.parse(json['lastOpenedAt'] as String),
    );
  }

  RecentProject copyWith({
    String? name,
    String? bundlePath,
    DateTime? lastOpenedAt,
  }) {
    return RecentProject(
      name: name ?? this.name,
      bundlePath: bundlePath ?? this.bundlePath,
      lastOpenedAt: lastOpenedAt ?? this.lastOpenedAt,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RecentProject &&
          runtimeType == other.runtimeType &&
          name == other.name &&
          bundlePath == other.bundlePath &&
          lastOpenedAt == other.lastOpenedAt;

  @override
  int get hashCode =>
      name.hashCode ^ bundlePath.hashCode ^ lastOpenedAt.hashCode;

  @override
  String toString() =>
      'RecentProject(name: $name, bundlePath: $bundlePath, lastOpenedAt: $lastOpenedAt)';
}

