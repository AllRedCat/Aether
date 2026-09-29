import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';
import 'timeline_provider.dart';
import '../../theme/catppuccin.dart'; // Importando a paleta global

/// TimelineView displays the non-linear editing timeline, tracks, clips,
/// duration PTS, and controls for adding clips via Riverpod state.
class TimelineView extends ConsumerWidget {
  const TimelineView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(timelineProvider);

    return Container(
      color: CatppuccinMocha.crust, // Fundo ultra escuro para a timeline
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header / Toolbar
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            decoration: const BoxDecoration(
              color: CatppuccinMocha.mantle,
              border: Border(bottom: BorderSide(color: CatppuccinMocha.surface0)),
            ),
            child: Row(
              children: [
                const Text(
                  'Clipes: ',
                  style: TextStyle(color: CatppuccinMocha.subtext0, fontSize: 13),
                ),
                Text(
                  '${state.totalClipCount}',
                  key: const Key('timeline_total_clips_count'),
                  style: const TextStyle(
                    color: CatppuccinMocha.text,
                    fontWeight: FontWeight.bold,
                    fontSize: 14,
                  ),
                ),
                const SizedBox(width: 24),
                Container(width: 1, height: 16, color: CatppuccinMocha.surface1),
                const SizedBox(width: 24),
                const Text(
                  'Duração: ',
                  style: TextStyle(color: CatppuccinMocha.subtext0, fontSize: 13),
                ),
                Text(
                  '${state.durationPts} PTS',
                  key: const Key('timeline_duration_pts'),
                  style: const TextStyle(
                    color: CatppuccinMocha.peach, // Destaque na cor Peach para o tempo
                    fontWeight: FontWeight.w600,
                    fontFamily: 'monospace',
                    fontSize: 14,
                  ),
                ),
                const Spacer(),
                ElevatedButton.icon(
                  key: const Key('add_clip_button'),
                  onPressed: state.isLoading
                      ? null
                      : () => ref.read(timelineProvider.notifier).addClip(),
                  icon: state.isLoading
                      ? const SizedBox(
                          width: 14,
                          height: 14,
                          child: CircularProgressIndicator(strokeWidth: 2, color: CatppuccinMocha.base),
                        )
                      : const Icon(Icons.add, size: 16),
                  label: const Text('Adicionar Clipe', style: TextStyle(fontWeight: FontWeight.w600)),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: CatppuccinMocha.mauve, // Botão Mauve
                    foregroundColor: CatppuccinMocha.base,
                    elevation: 0,
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                  ),
                ),
              ],
            ),
          ),

          // Error Banner Display
          if (state.errorMessage != null)
            Container(
              color: CatppuccinMocha.red.withOpacity(0.15),
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              child: Row(
                children: [
                  const Icon(Icons.error_outline, color: CatppuccinMocha.red, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      state.errorMessage!,
                      style: const TextStyle(color: CatppuccinMocha.red, fontSize: 13, fontWeight: FontWeight.w500),
                    ),
                  ),
                ],
              ),
            ),

          // Track & Clip Rendering Lanes
          Expanded(
            child: state.isLoading && state.timeline == null
                ? const Center(child: CircularProgressIndicator(color: CatppuccinMocha.mauve))
                : state.tracks.isEmpty
                    ? const Center(
                        child: Text(
                          'Nenhuma trilha disponível',
                          style: TextStyle(color: CatppuccinMocha.subtext0),
                        ),
                      )
                    : ListView.builder(
                        padding: const EdgeInsets.all(12),
                        itemCount: state.tracks.length,
                        itemBuilder: (context, trackIndex) {
                          final track = state.tracks[trackIndex];
                          return _buildTrackLane(context, track, trackIndex);
                        },
                      ),
          ),
        ],
      ),
    );
  }

  Widget _buildTrackLane(BuildContext context, Track track, int index) {
    final isVideo = track.kind == TrackKind.video;
    final trackName = isVideo ? 'V${index + 1}' : 'A${index + 1}';
    
    // Cor condicional para vídeo (azul) e áudio (verde)
    final trackColor = isVideo ? CatppuccinMocha.blue : CatppuccinMocha.green;

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: CatppuccinMocha.base, // Fundo da trilha
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: CatppuccinMocha.surface0),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Cabeçalho da Trilha (Track Header)
          Container(
            width: 120,
            padding: const EdgeInsets.all(12),
            decoration: const BoxDecoration(
              color: CatppuccinMocha.mantle,
              borderRadius: BorderRadius.only(topLeft: Radius.circular(8), bottomLeft: Radius.circular(8)),
              border: Border(right: BorderSide(color: CatppuccinMocha.surface0)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Row(
                  children: [
                    Icon(
                      isVideo ? Icons.videocam_rounded : Icons.audiotrack_rounded,
                      size: 16,
                      color: trackColor,
                    ),
                    const SizedBox(width: 8),
                    Text(
                      trackName,
                      style: const TextStyle(
                        color: CatppuccinMocha.text,
                        fontWeight: FontWeight.bold,
                        fontSize: 13,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                Text(
                  '${track.clips.length} clipe(s)',
                  style: const TextStyle(color: CatppuccinMocha.overlay0, fontSize: 11),
                ),
              ],
            ),
          ),

          // Área dos Clipes (Track Body)
          Expanded(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              child: track.clips.isEmpty
                  ? Container(
                      alignment: Alignment.centerLeft,
                      child: const Text(
                        'Trilha vazia',
                        style: TextStyle(
                          color: CatppuccinMocha.surface2,
                          fontSize: 12,
                          fontStyle: FontStyle.italic,
                        ),
                      ),
                    )
                  : Wrap(
                      spacing: 4,
                      runSpacing: 4,
                      children: track.clips.map((clip) {
                        return Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
                          decoration: BoxDecoration(
                            color: trackColor.withOpacity(0.15),
                            borderRadius: BorderRadius.circular(6),
                            border: Border.all(color: trackColor.withOpacity(0.3)),
                          ),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Icon(isVideo ? Icons.movie_rounded : Icons.graphic_eq_rounded, size: 12, color: trackColor),
                              const SizedBox(width: 6),
                              Text(
                                '${clip.timelineIn}..${clip.timelineOut} PTS',
                                style: TextStyle(
                                  color: trackColor,
                                  fontSize: 11,
                                  fontWeight: FontWeight.w600,
                                  fontFamily: 'monospace'
                                ),
                              ),
                            ],
                          ),
                        );
                      }).toList(),
                    ),
            ),
          ),
        ],
      ),
    );
  }
}
