import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/src/core/providers/recent_projects_provider.dart';
import 'package:aether_app/src/core/repositories/recent_projects_repository.dart';

void main() {
  group('RecentProjects Riverpod Providers', () {
    late Directory tempDir;
    late File jsonStorageFile;
    late RecentProjectsRepository repository;
    late ProviderContainer container;

    setUp(() {
      tempDir = Directory.systemTemp.createTempSync('aether_test_riverpod_');
      jsonStorageFile = File('${tempDir.path}/recent_projects.json');
      repository = RecentProjectsRepository(customFile: jsonStorageFile);

      container = ProviderContainer(
        overrides: [
          recentProjectsRepositoryProvider.overrideWithValue(repository),
        ],
      );
    });

    tearDown(() {
      container.dispose();
      if (tempDir.existsSync()) {
        tempDir.deleteSync(recursive: true);
      }
    });

    void createFakeBundle(String bundlePath) {
      final dir = Directory(bundlePath);
      dir.createSync(recursive: true);
      File('${dir.path}/project.db').writeAsStringSync('sqlite data');
    }

    test('recentProjectsProvider loads empty on start', () async {
      final projects = await container.read(recentProjectsProvider.future);
      expect(projects, isEmpty);
    });

    test('addOrUpdateProject updates provider state', () async {
      final bundle = '${tempDir.path}/Clipe.aether';
      createFakeBundle(bundle);

      await container
          .read(recentProjectsProvider.notifier)
          .addOrUpdateProject('Clipe Banda', bundle);

      final state = await container.read(recentProjectsProvider.future);
      expect(state.length, 1);
      expect(state.first.name, 'Clipe Banda');
      expect(state.first.bundlePath, bundle);
    });

    test('removeProject updates provider state', () async {
      final bundle = '${tempDir.path}/Vlog.aether';
      createFakeBundle(bundle);

      await container
          .read(recentProjectsProvider.notifier)
          .addOrUpdateProject('Vlog Viagem', bundle);

      expect((await container.read(recentProjectsProvider.future)).length, 1);

      await container
          .read(recentProjectsProvider.notifier)
          .removeProject(bundle);

      expect((await container.read(recentProjectsProvider.future)), isEmpty);
    });

    test('filteredRecentProjectsProvider filters case-insensitively on name and bundlePath', () async {
      final bundle1 = '${tempDir.path}/Casamento_Praia.aether';
      final bundle2 = '${tempDir.path}/Vlog_Japao.aether';
      final bundle3 = '${tempDir.path}/Comercial_Carro.aether';
      createFakeBundle(bundle1);
      createFakeBundle(bundle2);
      createFakeBundle(bundle3);

      final notifier = container.read(recentProjectsProvider.notifier);
      await notifier.addOrUpdateProject('Casamento na Praia', bundle1);
      await notifier.addOrUpdateProject('Viagem Japão 2026', bundle2);
      await notifier.addOrUpdateProject('Comercial Automotivo', bundle3);

      // 1. Empty query returns all 3
      var filtered = container.read(filteredRecentProjectsProvider).value;
      expect(filtered?.length, 3);

      // 2. Query matching name case-insensitively
      container.read(projectSearchQueryProvider.notifier).state = 'japão';
      filtered = container.read(filteredRecentProjectsProvider).value;
      expect(filtered?.length, 1);
      expect(filtered?.first.name, 'Viagem Japão 2026');

      // 3. Query matching partial bundle path
      container.read(projectSearchQueryProvider.notifier).state = 'praia';
      filtered = container.read(filteredRecentProjectsProvider).value;
      expect(filtered?.length, 1);
      expect(filtered?.first.name, 'Casamento na Praia');

      // 4. Query matching nothing
      container.read(projectSearchQueryProvider.notifier).state = 'inexistente';
      filtered = container.read(filteredRecentProjectsProvider).value;
      expect(filtered, isEmpty);
    });
  });
}

