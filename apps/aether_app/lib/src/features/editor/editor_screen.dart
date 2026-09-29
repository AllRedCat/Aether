import 'package:flutter/material.dart';
import '../../theme/catppuccin.dart';
import '../timeline/timeline_view.dart';

class EditorScreen extends StatelessWidget {
  const EditorScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CatppuccinMocha.crust, // Fundo mais escuro entre os painéis
      body: Column(
        children: [
          // TOPO: Media Pool | Preview | Inspector
          Expanded(
            flex: 6, // 60% da tela para o topo
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // 1. Painel Esquerdo: Media Pool
                Expanded(
                  flex: 2,
                  child: _buildPanel(
                    title: "Media Pool",
                    icon: Icons.video_library_rounded,
                    child: _buildDummyMediaPool(),
                  ),
                ),
                // Divisória sutil
                Container(width: 2, color: CatppuccinMocha.crust),
                
                // 2. Painel Central: Video Preview
                Expanded(
                  flex: 5,
                  child: _buildPanel(
                    title: "Preview",
                    icon: Icons.monitor_rounded,
                    child: _buildDummyPreview(),
                  ),
                ),
                // Divisória sutil
                Container(width: 2, color: CatppuccinMocha.crust),

                // 3. Painel Direito: Inspector / Propriedades
                Expanded(
                  flex: 2,
                  child: _buildPanel(
                    title: "Inspector",
                    icon: Icons.tune_rounded,
                    child: _buildDummyInspector(),
                  ),
                ),
              ],
            ),
          ),
          
          // DIVISÓRIA HORIZONTAL
          Container(height: 2, color: CatppuccinMocha.crust),

          // BASE: Timeline
          const Expanded(
            flex: 4, // 40% da tela para a timeline
            child: TimelineView(), // Nosso widget existente do motor Rust!
          ),
        ],
      ),
    );
  }

  // Wrapper para criar um visual de "Janela" padrão para cada painel
  Widget _buildPanel({required String title, required IconData icon, required Widget child}) {
    return Container(
      color: CatppuccinMocha.base, // Fundo de cada janela individual
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Cabeçalho do painel
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: const BoxDecoration(
              color: CatppuccinMocha.mantle,
              border: Border(bottom: BorderSide(color: CatppuccinMocha.surface0)),
            ),
            child: Row(
              children: [
                Icon(icon, size: 14, color: CatppuccinMocha.overlay0),
                const SizedBox(width: 8),
                Text(
                  title.toUpperCase(),
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.bold,
                    color: CatppuccinMocha.overlay0,
                    letterSpacing: 0.5,
                  ),
                ),
              ],
            ),
          ),
          // Corpo do painel
          Expanded(child: child),
        ],
      ),
    );
  }

  // --- WIDGETS FAKES DE PRÉ-VISUALIZAÇÃO ---

  Widget _buildDummyMediaPool() {
    return ListView(
      padding: const EdgeInsets.all(8),
      children: [
        _buildMediaItem("IMG_8342.MOV", "00:15", CatppuccinMocha.blue),
        _buildMediaItem("entrevista_audio.wav", "12:04", CatppuccinMocha.green),
        _buildMediaItem("B-Roll_Drone.mp4", "01:23", CatppuccinMocha.blue),
      ],
    );
  }

  Widget _buildMediaItem(String name, String duration, Color color) {
    return Container(
      margin: const EdgeInsets.only(bottom: 4),
      decoration: BoxDecoration(
        color: CatppuccinMocha.surface0.withOpacity(0.5),
        borderRadius: BorderRadius.circular(6),
      ),
      child: ListTile(
        dense: true,
        leading: Icon(
          color == CatppuccinMocha.green ? Icons.audiotrack : Icons.videocam,
          color: color,
          size: 18,
        ),
        title: Text(name, style: const TextStyle(color: CatppuccinMocha.text, fontSize: 13)),
        trailing: Text(duration, style: const TextStyle(color: CatppuccinMocha.subtext0, fontSize: 11)),
      ),
    );
  }

  Widget _buildDummyPreview() {
    return Padding(
      padding: const EdgeInsets.all(16.0),
      child: Column(
        children: [
          Expanded(
            child: Container(
              decoration: BoxDecoration(
                color: Colors.black, // O preview do vídeo deve ser preto absoluto
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: CatppuccinMocha.surface0),
              ),
              child: const Center(
                child: Text(
                  "Frame Visualizado da GPU",
                  style: TextStyle(color: CatppuccinMocha.surface2),
                ),
              ),
            ),
          ),
          const SizedBox(height: 16),
          // Controles de Playback Fakes
          Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              IconButton(onPressed: () {}, icon: const Icon(Icons.skip_previous_rounded, color: CatppuccinMocha.text)),
              Container(
                margin: const EdgeInsets.symmetric(horizontal: 8),
                decoration: const BoxDecoration(
                  color: CatppuccinMocha.surface0,
                  shape: BoxShape.circle,
                ),
                child: IconButton(onPressed: () {}, icon: const Icon(Icons.play_arrow_rounded, color: CatppuccinMocha.text)),
              ),
              IconButton(onPressed: () {}, icon: const Icon(Icons.skip_next_rounded, color: CatppuccinMocha.text)),
            ],
          )
        ],
      ),
    );
  }

  Widget _buildDummyInspector() {
    return Padding(
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text("Transform", style: TextStyle(color: CatppuccinMocha.mauve, fontWeight: FontWeight.bold, fontSize: 13)),
          const SizedBox(height: 12),
          _buildSliderProp("Zoom"),
          _buildSliderProp("Position X"),
          _buildSliderProp("Position Y"),
          const SizedBox(height: 24),
          const Text("Audio", style: TextStyle(color: CatppuccinMocha.green, fontWeight: FontWeight.bold, fontSize: 13)),
          const SizedBox(height: 12),
          _buildSliderProp("Volume (dB)"),
        ],
      ),
    );
  }

  Widget _buildSliderProp(String label) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: const TextStyle(color: CatppuccinMocha.subtext0, fontSize: 11)),
          const SizedBox(height: 4),
          Container(
            height: 24,
            decoration: BoxDecoration(
              color: CatppuccinMocha.surface0,
              borderRadius: BorderRadius.circular(4),
            ),
            child: Row(
              children: [
                Expanded(
                  child: Container(
                    margin: const EdgeInsets.all(2),
                    decoration: BoxDecoration(
                      color: CatppuccinMocha.surface1,
                      borderRadius: BorderRadius.circular(2),
                    ),
                  ),
                ),
                const SizedBox(width: 48), // Espaço vazio para simular um slider não preenchido
              ],
            ),
          ),
        ],
      ),
    );
  }
}
