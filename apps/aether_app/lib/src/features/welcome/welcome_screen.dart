import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:uuid/uuid.dart';

import '../../bridge/api.dart';
import '../../core/models/recent_project.dart';
import '../../core/providers/project_provider.dart';
import '../../core/providers/recent_projects_provider.dart';
import '../../core/utils/date_formatter.dart';
import '../../theme/catppuccin.dart';
import '../editor/editor_screen.dart';

class WelcomeScreen extends ConsumerWidget {
  const WelcomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final recentProjectsAsync = ref.watch(filteredRecentProjectsProvider);

    return Scaffold(
      backgroundColor: CatppuccinMocha.base,
      body: Row(
        children: [
          // PAINEL ESQUERDO: Projetos Recentes (Estilo IntelliJ)
          Container(
            width: 320,
            color: CatppuccinMocha.mantle,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Padding(
                  padding:
                      EdgeInsets.only(top: 40.0, left: 24.0, bottom: 20.0),
                  child: Text(
                    "Projetos",
                    style: TextStyle(
                      fontSize: 22,
                      fontWeight: FontWeight.w600,
                      color: CatppuccinMocha.text,
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16.0),
                  child: TextField(
                    onChanged: (value) {
                      ref.read(projectSearchQueryProvider.notifier).state =
                          value;
                    },
                    decoration: InputDecoration(
                      hintText: "Buscar projetos...",
                      hintStyle:
                          const TextStyle(color: CatppuccinMocha.subtext0),
                      prefixIcon: const Icon(Icons.search,
                          color: CatppuccinMocha.subtext0, size: 20),
                      filled: true,
                      fillColor: CatppuccinMocha.surface0,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: BorderSide.none,
                      ),
                      contentPadding: const EdgeInsets.symmetric(vertical: 0),
                    ),
                    style: const TextStyle(color: CatppuccinMocha.text),
                  ),
                ),
                const SizedBox(height: 16),
                Expanded(
                  child: recentProjectsAsync.when(
                    data: (projects) {
                      if (projects.isEmpty) {
                        final query = ref.watch(projectSearchQueryProvider);
                        return Center(
                          child: Padding(
                            padding: const EdgeInsets.all(16.0),
                            child: Text(
                              query.isEmpty
                                  ? "Nenhum projeto recente"
                                  : "Nenhum projeto encontrado para \"$query\"",
                              textAlign: TextAlign.center,
                              style: const TextStyle(
                                color: CatppuccinMocha.subtext0,
                                fontSize: 13,
                              ),
                            ),
                          ),
                        );
                      }

                      return ListView.builder(
                        padding: const EdgeInsets.symmetric(horizontal: 8.0),
                        itemCount: projects.length,
                        itemBuilder: (context, index) {
                          final project = projects[index];
                          return _buildRecentProjectItem(
                            context,
                            ref,
                            project,
                          );
                        },
                      );
                    },
                    loading: () => const Center(
                      child: SizedBox(
                        width: 24,
                        height: 24,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: CatppuccinMocha.mauve,
                        ),
                      ),
                    ),
                    error: (err, _) => Center(
                      child: Padding(
                        padding: const EdgeInsets.all(16.0),
                        child: Text(
                          "Erro ao carregar projetos: $err",
                          style: const TextStyle(
                              color: CatppuccinMocha.red, fontSize: 12),
                          textAlign: TextAlign.center,
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),

          // DIVISÓRIA
          Container(width: 1, color: CatppuccinMocha.crust),

          // PAINEL DIREITO: Ações Principais
          Expanded(
            child: Container(
              color: CatppuccinMocha.base,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    // Logo / Título
                    Container(
                      width: 100,
                      height: 100,
                      decoration: BoxDecoration(
                        gradient: const LinearGradient(
                          colors: [CatppuccinMocha.mauve, CatppuccinMocha.blue],
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                        ),
                        borderRadius: BorderRadius.circular(24),
                        boxShadow: [
                          BoxShadow(
                            color: CatppuccinMocha.mauve.withValues(alpha: 0.2),
                            blurRadius: 30,
                            offset: const Offset(0, 10),
                          )
                        ],
                      ),
                      child: const Icon(Icons.movie_creation_rounded,
                          size: 50, color: CatppuccinMocha.base),
                    ),
                    const SizedBox(height: 32),
                    const Text(
                      "Aether Video Editor",
                      style: TextStyle(
                        fontSize: 32,
                        fontWeight: FontWeight.w800,
                        letterSpacing: -0.5,
                        color: CatppuccinMocha.text,
                      ),
                    ),
                    const SizedBox(height: 8),
                    const Text(
                      "Versão 1.0.0-dev • Motor Híbrido FFI",
                      style: TextStyle(
                        fontSize: 14,
                        color: CatppuccinMocha.subtext0,
                      ),
                    ),
                    const SizedBox(height: 60),

                    // Botões de Ação
                    _buildActionButton(
                      icon: Icons.add_box_rounded,
                      title: "Novo Projeto",
                      subtitle: "Criar um novo pacote de projeto (.aether)",
                      color: CatppuccinMocha.mauve,
                      onTap: () => _handleNewProject(context, ref),
                    ),
                    const SizedBox(height: 16),
                    _buildActionButton(
                      icon: Icons.folder_open_rounded,
                      title: "Abrir Projeto",
                      subtitle:
                          "Abrir um pacote .aether existente no computador",
                      color: CatppuccinMocha.blue,
                      onTap: () => _handleOpenProject(context, ref),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildRecentProjectItem(
    BuildContext context,
    WidgetRef ref,
    RecentProject project,
  ) {
    final relativeDate = formatRelativeDatePtBr(project.lastOpenedAt);

    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      decoration: BoxDecoration(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Material(
        color: Colors.transparent,
        child: ListTile(
          dense: true,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
          hoverColor: CatppuccinMocha.surface0.withValues(alpha: 0.5),
          leading: Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: CatppuccinMocha.surface1,
              borderRadius: BorderRadius.circular(6),
            ),
            child: const Icon(Icons.video_library_rounded,
                color: CatppuccinMocha.mauve, size: 20),
          ),
          title: Text(
            project.name,
            style: const TextStyle(
              color: CatppuccinMocha.text,
              fontWeight: FontWeight.w500,
              fontSize: 14,
            ),
          ),
          subtitle: Text(
            project.bundlePath,
            style:
                const TextStyle(color: CatppuccinMocha.subtext0, fontSize: 11),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          trailing: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                relativeDate,
                style: const TextStyle(
                    color: CatppuccinMocha.overlay0, fontSize: 11),
              ),
              const SizedBox(width: 4),
              IconButton(
                icon: const Icon(Icons.close, size: 14),
                color: CatppuccinMocha.overlay0,
                tooltip: 'Remover da lista',
                splashRadius: 14,
                onPressed: () {
                  ref
                      .read(recentProjectsProvider.notifier)
                      .removeProject(project.bundlePath);
                },
              ),
            ],
          ),
          onTap: () => _openProjectBundle(context, ref, project.bundlePath),
        ),
      ),
    );
  }

  Widget _buildActionButton({
    required IconData icon,
    required String title,
    required String subtitle,
    required Color color,
    required VoidCallback onTap,
  }) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      hoverColor: CatppuccinMocha.surface0,
      child: Container(
        width: 400,
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          border: Border.all(color: CatppuccinMocha.surface0),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: color.withValues(alpha: 0.1),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Icon(icon, color: color, size: 28),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: const TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w600,
                      color: CatppuccinMocha.text,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    subtitle,
                    style: const TextStyle(
                      fontSize: 13,
                      color: CatppuccinMocha.subtext0,
                    ),
                  ),
                ],
              ),
            ),
            const Icon(Icons.chevron_right_rounded,
                color: CatppuccinMocha.overlay0),
          ],
        ),
      ),
    );
  }

  Future<void> _handleNewProject(BuildContext context, WidgetRef ref) async {
    // 1. Pede o nome do projeto usando um modal
    final TextEditingController nameController = TextEditingController();
    final String? projectName = await showDialog<String>(
      context: context,
      builder: (dialogContext) {
        return AlertDialog(
          backgroundColor: CatppuccinMocha.base,
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          title: const Text('Novo Projeto',
              style: TextStyle(color: CatppuccinMocha.text)),
          content: TextField(
            controller: nameController,
            autofocus: true,
            style: const TextStyle(color: CatppuccinMocha.text),
            decoration: InputDecoration(
              hintText: "Ex: Casamento na Praia",
              hintStyle: const TextStyle(color: CatppuccinMocha.subtext0),
              filled: true,
              fillColor: CatppuccinMocha.surface0,
              border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                  borderSide: BorderSide.none),
            ),
            onSubmitted: (value) {
              if (value.trim().isNotEmpty) {
                Navigator.pop(dialogContext, value.trim());
              }
            },
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(dialogContext),
              child: const Text('Cancelar',
                  style: TextStyle(color: CatppuccinMocha.subtext0)),
            ),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                  backgroundColor: CatppuccinMocha.mauve,
                  foregroundColor: CatppuccinMocha.base),
              onPressed: () {
                if (nameController.text.trim().isNotEmpty) {
                  Navigator.pop(dialogContext, nameController.text.trim());
                }
              },
              child: const Text('Continuar'),
            ),
          ],
        );
      },
    );

    if (projectName == null || projectName.trim().isEmpty || !context.mounted) {
      return;
    }

    final trimmedName = projectName.trim();
    String bundlePath;

    if (Platform.isAndroid) {
      // No Android, salva dentro do diretório de documentos do app em /Projects/
      final docDir = await getApplicationDocumentsDirectory();
      final projectsDir = Directory('${docDir.path}/Projects');
      if (!await projectsDir.exists()) {
        await projectsDir.create(recursive: true);
      }
      final safeName = trimmedName.replaceAll(RegExp(r'[^\w\s\.-]'), '_');
      bundlePath = '${projectsDir.path}/$safeName.aether';
    } else {
      // No Desktop (macOS, Windows, Linux), abre seletor de diretório nativo
      final String? selectedDirectory = await getDirectoryPath(
        confirmButtonText: 'Criar Projeto Aqui',
      );
      if (selectedDirectory == null || !context.mounted) return;

      final safeName = trimmedName.replaceAll(RegExp(r'[^\w\s\.-]'), '_');
      bundlePath = '$selectedDirectory/$safeName.aether';
    }

    try {
      // 3. Chama o motor Rust para criar o ProjectBundle
      final meta = await createProjectBundle(
        bundlePath: bundlePath,
        projectName: trimmedName,
      );

      if (!context.mounted) return;

      // 4. Registra na lista de projetos recentes
      await ref.read(recentProjectsProvider.notifier).addOrUpdateProject(
            meta.name,
            bundlePath,
          );

      if (!context.mounted) return;

      // 5. Configura o estado do projeto ativo
      ref.read(currentProjectMetadataProvider.notifier).state = meta;
      ref.read(currentProjectProvider.notifier).state = Project(
        id: meta.id,
        name: meta.name,
        projectPath: bundlePath,
        filePath: '$bundlePath/project.db',
        timeline: Timeline(
          id: UuidValue.fromString(meta.id),
          timebase: const Rational(num: 60, den: 1),
          durationPts: 0,
          tracks: [
            Track(
              id: const Uuid().v4obj(),
              kind: TrackKind.video,
              clips: const [],
            ),
            Track(
              id: const Uuid().v4obj(),
              kind: TrackKind.audio,
              clips: const [],
            ),
          ],
        ),
        mediaPool: const MediaPool(items: []),
      );

      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          backgroundColor: CatppuccinMocha.green,
          content: Text(
            'Projeto "$trimmedName" criado com sucesso!',
            style: const TextStyle(color: CatppuccinMocha.crust),
          ),
        ),
      );

      // 6. Navega para a Workspace de edição
      Navigator.push(
        context,
        MaterialPageRoute(builder: (_) => const EditorScreen()),
      );
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            backgroundColor: CatppuccinMocha.red,
            content: Text('Erro ao criar projeto: $e'),
          ),
        );
      }
    }
  }

  /// Opens a project bundle from disk, updates active state, and navigates to the editor.
  String _normalizeBundlePath(String rawPath) {
    var cleaned = rawPath.trim().replaceAll(RegExp(r'[/\\]+$'), '');
    if (cleaned.endsWith('project.db') ||
        cleaned.endsWith('project.db-wal') ||
        cleaned.endsWith('project.db-shm')) {
      cleaned = File(cleaned).parent.path;
    }
    if (!cleaned.toLowerCase().endsWith('.aether')) {
      cleaned = '$cleaned.aether';
    }
    return cleaned;
  }

  Future<void> _openProjectBundle(
    BuildContext context,
    WidgetRef ref,
    String bundlePath,
  ) async {
    try {
      var resolvedPath = _normalizeBundlePath(bundlePath);

      final meta = await openProjectBundle(bundlePath: resolvedPath);
      final activeBundlePath = await getActiveBundlePath();
      if (activeBundlePath != null && activeBundlePath.isNotEmpty) {
        resolvedPath = activeBundlePath;
      }
      if (!context.mounted) return;

      // Registra na lista de projetos recentes
      await ref.read(recentProjectsProvider.notifier).addOrUpdateProject(
            meta.name,
            resolvedPath,
          );

      if (!context.mounted) return;

      // Atualiza estado do projeto ativo carregando mídias e timeline salvas no project.db
      ref.read(currentProjectMetadataProvider.notifier).state = meta;
      final dbPath = '$resolvedPath/project.db';
      Project activeProject;
      try {
        final loaded = await loadProject(filePath: dbPath);
        final tracks = loaded.timeline.tracks.isNotEmpty
            ? loaded.timeline.tracks
            : [
                Track(
                  id: const Uuid().v4obj(),
                  kind: TrackKind.video,
                  clips: const [],
                ),
                Track(
                  id: const Uuid().v4obj(),
                  kind: TrackKind.audio,
                  clips: const [],
                ),
              ];
        activeProject = Project(
          id: loaded.id,
          name: loaded.name,
          projectPath: resolvedPath,
          filePath: dbPath,
          timeline: Timeline(
            id: loaded.timeline.id,
            timebase: loaded.timeline.timebase,
            durationPts: loaded.timeline.durationPts,
            tracks: tracks,
          ),
          mediaPool: loaded.mediaPool,
        );
      } catch (_) {
        activeProject = Project(
          id: meta.id,
          name: meta.name,
          projectPath: resolvedPath,
          filePath: dbPath,
          timeline: Timeline(
            id: UuidValue.fromString(meta.id),
            timebase: const Rational(num: 60, den: 1),
            durationPts: 0,
            tracks: [
              Track(
                id: const Uuid().v4obj(),
                kind: TrackKind.video,
                clips: const [],
              ),
              Track(
                id: const Uuid().v4obj(),
                kind: TrackKind.audio,
                clips: const [],
              ),
            ],
          ),
          mediaPool: const MediaPool(items: []),
        );
      }
      if (!context.mounted) return;
      ref.read(currentProjectProvider.notifier).state = activeProject;

      Navigator.push(
        context,
        MaterialPageRoute(builder: (_) => const EditorScreen()),
      );
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            backgroundColor: CatppuccinMocha.red,
            content: Text('Erro ao abrir projeto: $e'),
          ),
        );
      }
    }
  }

  Future<void> _handleOpenProject(BuildContext context, WidgetRef ref) async {
    const typeGroup = XTypeGroup(
      label: 'Aether Project',
      extensions: ['aether', 'db'],
      uniformTypeIdentifiers: [
        'com.aether.project',
        'com.apple.package',
        'public.database',
        'public.data',
      ],
    );
    final file = await openFile(acceptedTypeGroups: const [typeGroup]);
    if (file == null || !context.mounted) return;

    await _openProjectBundle(context, ref, file.path);
  }
}
