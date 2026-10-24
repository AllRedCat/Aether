import 'dart:convert';
import 'dart:io';
import 'package:path_provider/path_provider.dart';
import '../models/recent_project.dart';

/// Repository responsible for reading and persisting recent project bundles to `recent_projects.json`.
class RecentProjectsRepository {
  static const String fileName = 'recent_projects.json';
  static const int maxRecentProjects = 20;

  final Future<Directory> Function()? _getSupportDirectory;
  final File? _customFile;

  RecentProjectsRepository({
    Future<Directory> Function()? getSupportDirectory,
    File? customFile,
  })  : _getSupportDirectory = getSupportDirectory,
        _customFile = customFile;

  /// Resolves the storage file in the application support directory.
  Future<File> get _storageFile async {
    if (_customFile != null) {
      return _customFile;
    }
    final dirGetter = _getSupportDirectory ?? getApplicationSupportDirectory;
    final dir = await dirGetter();
    return File('${dir.path}/$fileName');
  }

  /// Reads raw items from `recent_projects.json`.
  Future<List<RecentProject>> _readRawList() async {
    try {
      final file = await _storageFile;
      if (!await file.exists()) {
        return [];
      }
      final content = await file.readAsString();
      if (content.trim().isEmpty) {
        return [];
      }
      final decoded = jsonDecode(content);
      if (decoded is! List) {
        return [];
      }
      return decoded
          .map((item) {
            try {
              return RecentProject.fromJson(item as Map<String, dynamic>);
            } catch (_) {
              return null;
            }
          })
          .whereType<RecentProject>()
          .toList();
    } catch (_) {
      return [];
    }
  }

  /// Writes the given project list to `recent_projects.json`.
  Future<void> _writeRawList(List<RecentProject> projects) async {
    final file = await _storageFile;
    if (!await file.parent.exists()) {
      await file.parent.create(recursive: true);
    }
    final jsonList = projects.map((p) => p.toJson()).toList();
    await file.writeAsString(jsonEncode(jsonList), flush: true);
  }

  /// Reads all stored projects, filters out entries whose `<bundlePath>/project.db`
  /// does not exist on disk, and returns them sorted by `lastOpenedAt` descending.
  Future<List<RecentProject>> getRecentProjects() async {
    final rawList = await _readRawList();

    final filtered = rawList.where((p) => p.existsOnDisk).toList();
    filtered.sort((a, b) => b.lastOpenedAt.compareTo(a.lastOpenedAt));
    return filtered;
  }

  /// Adds or updates a recent project entry:
  /// - Removes any existing entry with the same [bundlePath] (deduplication)
  /// - Inserts the new entry at the top (index 0) with `DateTime.now()`
  /// - Clamps the list to at most [maxRecentProjects] (20) items
  /// - Persists to `recent_projects.json`
  ///
  /// Returns the sorted, filtered list of available recent projects.
  Future<List<RecentProject>> addOrUpdateRecentProject(
    String name,
    String bundlePath, {
    DateTime? openedAt,
  }) async {
    final rawList = await _readRawList();

    // Remove duplicates
    rawList.removeWhere((p) => p.bundlePath == bundlePath);

    // Insert at index 0
    final newEntry = RecentProject(
      name: name,
      bundlePath: bundlePath,
      lastOpenedAt: openedAt ?? DateTime.now(),
    );
    rawList.insert(0, newEntry);

    // Limit to maxRecentProjects
    final clamped = rawList.take(maxRecentProjects).toList();

    // Persist to disk
    await _writeRawList(clamped);

    return getRecentProjects();
  }

  /// Removes a project from `recent_projects.json` by its [bundlePath]
  /// without deleting any files from the disk.
  Future<void> removeRecentProject(String bundlePath) async {
    final rawList = await _readRawList();
    rawList.removeWhere((p) => p.bundlePath == bundlePath);
    await _writeRawList(rawList);
  }
}
