import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/src/core/repositories/recent_projects_repository.dart';

void main() {
  group('RecentProjectsRepository', () {
    late Directory tempDir;
    late File jsonStorageFile;
    late RecentProjectsRepository repository;

    setUp(() {
      tempDir = Directory.systemTemp.createTempSync('aether_test_repo_');
      jsonStorageFile = File('${tempDir.path}/recent_projects.json');
      repository = RecentProjectsRepository(
        customFile: jsonStorageFile,
      );
    });

    tearDown(() {
      if (tempDir.existsSync()) {
        tempDir.deleteSync(recursive: true);
      }
    });

    void createFakeBundle(String bundlePath) {
      final dir = Directory(bundlePath);
      dir.createSync(recursive: true);
      File('${dir.path}/project.db').writeAsStringSync('sqlite data');
    }

    test('getRecentProjects returns empty list when file does not exist', () async {
      final list = await repository.getRecentProjects();
      expect(list, isEmpty);
    });

    test('addOrUpdateRecentProject inserts at index 0 and persists to disk', () async {
      final bundle1 = '${tempDir.path}/Project1.aether';
      createFakeBundle(bundle1);

      final list1 = await repository.addOrUpdateRecentProject('Project 1', bundle1);
      expect(list1.length, 1);
      expect(list1.first.name, 'Project 1');
      expect(list1.first.bundlePath, bundle1);
      expect(jsonStorageFile.existsSync(), isTrue);

      final bundle2 = '${tempDir.path}/Project2.aether';
      createFakeBundle(bundle2);

      final list2 = await repository.addOrUpdateRecentProject('Project 2', bundle2);
      expect(list2.length, 2);
      expect(list2.first.name, 'Project 2');
      expect(list2[1].name, 'Project 1');
    });

    test('addOrUpdateRecentProject deduplicates by bundlePath and updates timestamp', () async {
      final bundle1 = '${tempDir.path}/Project1.aether';
      createFakeBundle(bundle1);

      final t1 = DateTime(2026, 1, 1);
      await repository.addOrUpdateRecentProject('Old Name', bundle1, openedAt: t1);

      final bundle2 = '${tempDir.path}/Project2.aether';
      createFakeBundle(bundle2);
      final t2 = DateTime(2026, 1, 2);
      await repository.addOrUpdateRecentProject('Project 2', bundle2, openedAt: t2);

      // Re-add Project 1 with a newer timestamp and new name
      final t3 = DateTime(2026, 1, 3);
      final list = await repository.addOrUpdateRecentProject('Updated Name', bundle1, openedAt: t3);

      expect(list.length, 2);
      expect(list[0].name, 'Updated Name');
      expect(list[0].bundlePath, bundle1);
      expect(list[0].lastOpenedAt, t3);
      expect(list[1].name, 'Project 2');
    });

    test('limits to maximum 20 recent projects', () async {
      for (int i = 1; i <= 25; i++) {
        final path = '${tempDir.path}/Project$i.aether';
        createFakeBundle(path);
        await repository.addOrUpdateRecentProject(
          'Project $i',
          path,
          openedAt: DateTime(2026, 1, i),
        );
      }

      final list = await repository.getRecentProjects();
      expect(list.length, 20);
      expect(list.first.name, 'Project 25');
      expect(list.last.name, 'Project 6');
    });

    test('getRecentProjects filters out projects deleted from disk', () async {
      final bundle1 = '${tempDir.path}/Existing.aether';
      createFakeBundle(bundle1);

      final bundle2 = '${tempDir.path}/Deleted.aether';
      // Do not create project.db for bundle2

      await repository.addOrUpdateRecentProject('Existing', bundle1);
      await repository.addOrUpdateRecentProject('Deleted', bundle2);

      final list = await repository.getRecentProjects();
      expect(list.length, 1);
      expect(list.first.name, 'Existing');
    });

    test('removeRecentProject removes from json without deleting from disk', () async {
      final bundle = '${tempDir.path}/Preserved.aether';
      createFakeBundle(bundle);

      await repository.addOrUpdateRecentProject('Preserved', bundle);
      expect((await repository.getRecentProjects()).length, 1);

      await repository.removeRecentProject(bundle);
      expect((await repository.getRecentProjects()).length, 0);

      // Verify folder and project.db still exist on disk!
      expect(File('$bundle/project.db').existsSync(), isTrue);
    });
  });
}
