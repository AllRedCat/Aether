import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/src/core/models/recent_project.dart';
import 'package:aether_app/src/core/providers/recent_projects_provider.dart';
import 'package:aether_app/src/core/repositories/recent_projects_repository.dart';
import 'package:aether_app/src/features/welcome/welcome_screen.dart';

class InMemoryRecentProjectsRepository extends RecentProjectsRepository {
  final List<RecentProject> _projects = [];

  InMemoryRecentProjectsRepository([List<RecentProject>? initial]) {
    if (initial != null) _projects.addAll(initial);
  }

  @override
  Future<List<RecentProject>> getRecentProjects() async {
    final filtered = List<RecentProject>.from(_projects);
    filtered.sort((a, b) => b.lastOpenedAt.compareTo(a.lastOpenedAt));
    return filtered;
  }

  @override
  Future<List<RecentProject>> addOrUpdateRecentProject(
    String name,
    String bundlePath, {
    DateTime? openedAt,
  }) async {
    _projects.removeWhere((p) => p.bundlePath == bundlePath);
    _projects.insert(
      0,
      RecentProject(
        name: name,
        bundlePath: bundlePath,
        lastOpenedAt: openedAt ?? DateTime.now(),
      ),
    );
    if (_projects.length > RecentProjectsRepository.maxRecentProjects) {
      _projects.removeRange(
        RecentProjectsRepository.maxRecentProjects,
        _projects.length,
      );
    }
    return getRecentProjects();
  }

  @override
  Future<void> removeRecentProject(String bundlePath) async {
    _projects.removeWhere((p) => p.bundlePath == bundlePath);
  }
}

void main() {
  group('WelcomeScreen Widget Tests', () {
    testWidgets('renders welcome screen with empty state when no recent projects',
        (tester) async {
      tester.view.physicalSize = const Size(1280, 720);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final repo = InMemoryRecentProjectsRepository();

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            recentProjectsRepositoryProvider.overrideWithValue(repo),
          ],
          child: const MaterialApp(
            home: WelcomeScreen(),
          ),
        ),
      );

      // Settle Riverpod build()
      await tester.pump();
      await tester.pump();

      expect(find.text('Projetos'), findsOneWidget);
      expect(find.text('Buscar projetos...'), findsOneWidget);
      expect(find.text('Novo Projeto'), findsOneWidget);
      expect(find.text('Abrir Projeto'), findsOneWidget);
      expect(find.text('Nenhum projeto recente'), findsOneWidget);
    });

    testWidgets('displays recent projects list and filters via search text field',
        (tester) async {
      tester.view.physicalSize = const Size(1280, 720);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final now = DateTime.now();
      final repo = InMemoryRecentProjectsRepository([
        RecentProject(
          name: 'Documentário Pantanal',
          bundlePath: '/path/to/Pantanal.aether',
          lastOpenedAt: now,
        ),
        RecentProject(
          name: 'Videoclipe Rock',
          bundlePath: '/path/to/Rock.aether',
          lastOpenedAt: now.subtract(const Duration(days: 1)),
        ),
      ]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            recentProjectsRepositoryProvider.overrideWithValue(repo),
          ],
          child: const MaterialApp(
            home: WelcomeScreen(),
          ),
        ),
      );

      await tester.pump();
      await tester.pump();

      // Check both projects are visible
      expect(find.text('Documentário Pantanal'), findsOneWidget);
      expect(find.text('Videoclipe Rock'), findsOneWidget);
      expect(find.text('Hoje'), findsOneWidget);
      expect(find.text('Ontem'), findsOneWidget);

      // Search for "Pantanal"
      await tester.enterText(find.byType(TextField), 'Pantanal');
      await tester.pump();

      expect(find.text('Documentário Pantanal'), findsOneWidget);
      expect(find.text('Videoclipe Rock'), findsNothing);

      // Search for non-existent item
      await tester.enterText(find.byType(TextField), 'Inexistente');
      await tester.pump();

      expect(find.text('Nenhum projeto encontrado para "Inexistente"'), findsOneWidget);
    });

    testWidgets('removing a project from list updates UI', (tester) async {
      tester.view.physicalSize = const Size(1280, 720);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final repo = InMemoryRecentProjectsRepository([
        RecentProject(
          name: 'Projeto Removível',
          bundlePath: '/path/to/Removivel.aether',
          lastOpenedAt: DateTime.now(),
        ),
      ]);

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            recentProjectsRepositoryProvider.overrideWithValue(repo),
          ],
          child: const MaterialApp(
            home: WelcomeScreen(),
          ),
        ),
      );

      await tester.pump();
      await tester.pump();
      expect(find.text('Projeto Removível'), findsOneWidget);

      // Click remove icon button
      final closeButton = find.byTooltip('Remover da lista');
      expect(closeButton, findsOneWidget);
      await tester.tap(closeButton);
      await tester.pump();
      await tester.pump();

      expect(find.text('Projeto Removível'), findsNothing);
      expect(find.text('Nenhum projeto recente'), findsOneWidget);
    });
  });
}

