import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../core/providers/project_provider.dart';

import 'package:flutter/material.dart';
import 'package:file_selector/file_selector.dart';
import '../../theme/catppuccin.dart';
import '../../bridge/api.dart';
import '../editor/editor_screen.dart';

class WelcomeScreen extends ConsumerWidget {
  const WelcomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
                  padding: EdgeInsets.only(top: 40.0, left: 24.0, bottom: 20.0),
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
                    decoration: InputDecoration(
                      hintText: "Buscar projetos...",
                      hintStyle: const TextStyle(color: CatppuccinMocha.subtext0),
                      prefixIcon: const Icon(Icons.search, color: CatppuccinMocha.subtext0, size: 20),
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
                  child: ListView(
                    padding: const EdgeInsets.symmetric(horizontal: 8.0),
                    children: [
                      _buildRecentProjectItem(
                        context,
                        "Edição Casamento.aether",
                        "/Users/gabriel/Videos/Casamento",
                        "Ontem",
                        isActive: true,
                      ),
                      _buildRecentProjectItem(
                        context,
                        "Vlog Viagem Japão.aether",
                        "/Users/gabriel/Videos/Vlogs",
                        "Semana Passada",
                      ),
                      _buildRecentProjectItem(
                        context,
                        "Clipe Banda.aether",
                        "/Users/gabriel/Videos/Banda",
                        "Há 2 semanas",
                      ),
                    ],
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
                    // Logo Falsa / Título
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
                      child: const Icon(Icons.movie_creation_rounded, size: 50, color: CatppuccinMocha.base),
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
                      subtitle: "Criar uma nova timeline de edição em branco",
                      color: CatppuccinMocha.mauve,
                      onTap: () => _handleNewProject(context, ref),
                    ),
                    const SizedBox(height: 16),
                    _buildActionButton(
                      icon: Icons.folder_open_rounded,
                      title: "Abrir Projeto",
                      subtitle: "Abrir um arquivo .aether existente no computador",
                      color: CatppuccinMocha.blue,
                      onTap: () {},
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

  Widget _buildRecentProjectItem(BuildContext context, String title, String path, String date, {bool isActive = false}) {
    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      decoration: BoxDecoration(
        color: isActive ? CatppuccinMocha.surface0 : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Material(
        color: Colors.transparent,
        child: ListTile(
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
          hoverColor: CatppuccinMocha.surface0.withValues(alpha: 0.5),
          leading: Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: CatppuccinMocha.surface1,
              borderRadius: BorderRadius.circular(6),
            ),
            child: const Icon(Icons.video_library_rounded, color: CatppuccinMocha.mauve, size: 20),
          ),
          title: Text(
            title,
            style: const TextStyle(
              color: CatppuccinMocha.text,
              fontWeight: FontWeight.w500,
              fontSize: 14,
            ),
          ),
          subtitle: Text(
            path,
            style: const TextStyle(color: CatppuccinMocha.subtext0, fontSize: 11),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          trailing: Text(
            date,
            style: const TextStyle(color: CatppuccinMocha.overlay0, fontSize: 11),
          ),
          onTap: () {
            // Navega para o workspace de edição
            Navigator.push(context, MaterialPageRoute(builder: (_) => const EditorScreen()));
          },
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
            const Icon(Icons.chevron_right_rounded, color: CatppuccinMocha.overlay0),
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
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          title: const Text('Novo Projeto', style: TextStyle(color: CatppuccinMocha.text)),
          content: TextField(
            controller: nameController,
            autofocus: true,
            style: const TextStyle(color: CatppuccinMocha.text),
            decoration: InputDecoration(
              hintText: "Ex: Casamento na Praia",
              hintStyle: const TextStyle(color: CatppuccinMocha.subtext0),
              filled: true,
              fillColor: CatppuccinMocha.surface0,
              border: OutlineInputBorder(borderRadius: BorderRadius.circular(8), borderSide: BorderSide.none),
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
              child: const Text('Cancelar', style: TextStyle(color: CatppuccinMocha.subtext0)),
            ),
            ElevatedButton(
              style: ElevatedButton.styleFrom(backgroundColor: CatppuccinMocha.mauve, foregroundColor: CatppuccinMocha.base),
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

    if (projectName == null || !context.mounted) return;

    // 2. Abre o seletor de pasta NATIVO do sistema operacional
    final String? selectedDirectory = await getDirectoryPath(
      confirmButtonText: 'Criar Projeto Aqui',
    );

    if (selectedDirectory == null || !context.mounted) return; // Cancelou

    try {
      // 3. Chama o motor Rust para criar o projeto no destino escolhido
      final project = await createProject(
        name: projectName,
        baseDir: selectedDirectory,
      );

      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            backgroundColor: CatppuccinMocha.green,
            content: Text(
              'Projeto "$projectName" criado em: ${project.projectPath}',
              style: const TextStyle(color: CatppuccinMocha.crust),
            ),
          ),
        );
        // 4. Salva o projeto no provider global e vai para a Workspace de edição
        ref.read(currentProjectProvider.notifier).state = project;
        Navigator.push(context, MaterialPageRoute(builder: (_) => const EditorScreen()));
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(backgroundColor: CatppuccinMocha.red, content: Text('Erro ao criar projeto: $e')),
        );
      }
    }
  }
}
