import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../models/recent_project.dart';
import '../repositories/recent_projects_repository.dart';

/// Provider for [RecentProjectsRepository].
final recentProjectsRepositoryProvider =
    Provider<RecentProjectsRepository>((ref) {
  return RecentProjectsRepository();
});

/// State notifier managing the list of recent project bundles.
class RecentProjectsNotifier extends AsyncNotifier<List<RecentProject>> {
  @override
  Future<List<RecentProject>> build() async {
    final repo = ref.watch(recentProjectsRepositoryProvider);
    return repo.getRecentProjects();
  }

  /// Adds or updates a recent project entry and updates the notifier state.
  Future<void> addOrUpdateProject(
    String name,
    String bundlePath, {
    DateTime? openedAt,
  }) async {
    final repo = ref.read(recentProjectsRepositoryProvider);
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() async {
      return await repo.addOrUpdateRecentProject(
        name,
        bundlePath,
        openedAt: openedAt,
      );
    });
  }

  /// Removes a project from recent records without deleting files on disk.
  Future<void> removeProject(String bundlePath) async {
    final repo = ref.read(recentProjectsRepositoryProvider);
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() async {
      await repo.removeRecentProject(bundlePath);
      return repo.getRecentProjects();
    });
  }

  /// Manually reloads the recent project list from disk.
  Future<void> reload() async {
    final repo = ref.read(recentProjectsRepositoryProvider);
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() => repo.getRecentProjects());
  }
}

/// Provider managing the list of recent projects.
final recentProjectsProvider =
    AsyncNotifierProvider<RecentProjectsNotifier, List<RecentProject>>(() {
  return RecentProjectsNotifier();
});

/// Provider for the search query entered in the "Buscar projetos..." input.
final projectSearchQueryProvider = StateProvider<String>((ref) => '');

/// Provider that filters the list of recent projects based on [projectSearchQueryProvider],
/// performing case-insensitive matching on both `name` and `bundlePath`.
final filteredRecentProjectsProvider =
    Provider<AsyncValue<List<RecentProject>>>((ref) {
  final projectsAsync = ref.watch(recentProjectsProvider);
  final query = ref.watch(projectSearchQueryProvider).trim().toLowerCase();

  return projectsAsync.whenData((projects) {
    if (query.isEmpty) {
      return projects;
    }
    return projects.where((p) {
      final nameMatches = p.name.toLowerCase().contains(query);
      final pathMatches = p.bundlePath.toLowerCase().contains(query);
      return nameMatches || pathMatches;
    }).toList();
  });
});

