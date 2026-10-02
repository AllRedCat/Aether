import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';
import 'timeline_provider.dart';
import 'timeline_selection_provider.dart';
import '../../theme/catppuccin.dart';

/// TimelineView displays the non-linear editing timeline, tracks, clips,
/// duration PTS, controls for adding clips, and reactive clip selection highlighting.
class TimelineView extends ConsumerWidget {
  const TimelineView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(timelineProvider);
    final selectionState = ref.watch(timelineSelectionProvider);

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
              border:
                  Border(bottom: BorderSide(color: CatppuccinMocha.surface0)),
            ),
            child: LayoutBuilder(
              builder: (context, constraints) {
                return SingleChildScrollView(
                  scrollDirection: Axis.horizontal,
                  child: ConstrainedBox(
                    constraints: BoxConstraints(minWidth: constraints.maxWidth),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const Text(
                              'Clipes: ',
                              style: TextStyle(
                                  color: CatppuccinMocha.subtext0,
                                  fontSize: 13),
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
                            Container(
                                width: 1,
                                height: 16,
                                color: CatppuccinMocha.surface1),
                            const SizedBox(width: 24),
                            Text(
                              'Duration: ${state.durationPts} PTS',
                              key: const Key('timeline_duration_pts'),
                              style: const TextStyle(
                                color: CatppuccinMocha.peach,
                                fontWeight: FontWeight.w600,
                                fontFamily: 'monospace',
                                fontSize: 14,
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(width: 16),
                        // ElevatedButton.icon(
                        //   key: const Key('add_clip_button'),
                        //   onPressed: state.isLoading
                        //       ? null
                        //       : () => ref.read(timelineProvider.notifier).addClip(),
                        //   icon: state.isLoading
                        //       ? const SizedBox(
                        //           width: 14,
                        //           height: 14,
                        //           child: CircularProgressIndicator(strokeWidth: 2, color: CatppuccinMocha.base),
                        //         )
                        //       : const Icon(Icons.add, size: 16),
                        //   label: const Text('Adicionar Clipe', style: TextStyle(fontWeight: FontWeight.w600)),
                        //   style: ElevatedButton.styleFrom(
                        //     backgroundColor: CatppuccinMocha.mauve,
                        //     foregroundColor: CatppuccinMocha.base,
                        //     elevation: 0,
                        //     shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
                        //     padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                        //   ),
                        // ),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),

          // Error Banner Display
          if (state.errorMessage != null)
            Container(
              color: CatppuccinMocha.red.withValues(alpha: 0.15),
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              child: Row(
                children: [
                  const Icon(Icons.error_outline,
                      color: CatppuccinMocha.red, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      state.errorMessage!,
                      style: const TextStyle(
                          color: CatppuccinMocha.red,
                          fontSize: 13,
                          fontWeight: FontWeight.w500),
                    ),
                  ),
                ],
              ),
            ),

          // Track & Clip Rendering Lanes with Background Deselection
          Expanded(
            child: GestureDetector(
              key: const Key('timeline_background'),
              behavior: HitTestBehavior.opaque,
              onTap: () {
                ref.read(timelineSelectionProvider.notifier).clearSelection();
              },
              child: state.isLoading && state.timeline == null
                  ? const Center(
                      child: CircularProgressIndicator(
                          color: CatppuccinMocha.mauve))
                  : state.tracks.isEmpty
                      ? const Center(
                          child: Text(
                            'No tracks available',
                            style: TextStyle(color: CatppuccinMocha.subtext0),
                          ),
                        )
                      : ListView.builder(
                          padding: const EdgeInsets.all(12),
                          itemCount: state.tracks.length,
                          itemBuilder: (context, trackIndex) {
                            final track = state.tracks[trackIndex];
                            return _buildTrackLane(
                              context,
                              ref,
                              track,
                              trackIndex,
                              selectionState,
                            );
                          },
                        ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildTrackLane(
    BuildContext context,
    WidgetRef ref,
    Track track,
    int index,
    TimelineSelectionState selectionState,
  ) {
    final isVideo = track.kind == TrackKind.video;
    final trackName = 'Track ${index + 1} (${track.kind.name.toUpperCase()})';

    final trackColor = isVideo ? CatppuccinMocha.blue : CatppuccinMocha.green;

    return Container(
      key: Key('timeline_track_${track.id}'),
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: CatppuccinMocha.base,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: CatppuccinMocha.surface0),
      ),
      child: IntrinsicHeight(
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Track Header
            GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () {},
              child: Container(
                width: 140,
                padding: const EdgeInsets.all(12),
                decoration: const BoxDecoration(
                  color: CatppuccinMocha.mantle,
                  borderRadius: BorderRadius.only(
                    topLeft: Radius.circular(8),
                    bottomLeft: Radius.circular(8),
                  ),
                  border: Border(
                      right: BorderSide(color: CatppuccinMocha.surface0)),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Row(
                      children: [
                        Icon(
                          isVideo
                              ? Icons.videocam_rounded
                              : Icons.audiotrack_rounded,
                          size: 16,
                          color: trackColor,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            trackName,
                            style: const TextStyle(
                              color: CatppuccinMocha.text,
                              fontWeight: FontWeight.bold,
                              fontSize: 13,
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 4),
                    Text(
                      '${track.clips.length} clipe(s)',
                      style: const TextStyle(
                          color: CatppuccinMocha.overlay0, fontSize: 11),
                    ),
                  ],
                ),
              ),
            ),

            // Track Body (Clips Area)
            Expanded(
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTap: () {
                  ref.read(timelineSelectionProvider.notifier).clearSelection();
                },
                child: Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  child: track.clips.isEmpty
                      ? Container(
                          alignment: Alignment.centerLeft,
                          child: const Text(
                            'Track is empty. Click "Add Clip" to add media.',
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
                            final isSelected =
                                selectionState.isClipSelected(clip.id);
                            return _buildClipItem(
                              ref: ref,
                              track: track,
                              clip: clip,
                              isVideo: isVideo,
                              trackColor: trackColor,
                              isSelected: isSelected,
                            );
                          }).toList(),
                        ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildClipItem({
    required WidgetRef ref,
    required Track track,
    required Clip clip,
    required bool isVideo,
    required Color trackColor,
    required bool isSelected,
  }) {
    return Material(
      color: Colors.transparent,
      child: Ink(
        decoration: BoxDecoration(
          color: isSelected
              ? trackColor.withValues(alpha: 0.35)
              : trackColor.withValues(alpha: 0.15),
          borderRadius: BorderRadius.circular(6),
          border: isSelected
              ? Border.all(color: CatppuccinMocha.mauve, width: 2.0)
              : Border.all(
                  color: trackColor.withValues(alpha: 0.3), width: 1.0),
          boxShadow: isSelected
              ? [
                  BoxShadow(
                    color: CatppuccinMocha.mauve.withValues(alpha: 0.3),
                    blurRadius: 4,
                    spreadRadius: 0.5,
                  ),
                ]
              : null,
        ),
        child: InkWell(
          key: Key('timeline_clip_${clip.id}'),
          borderRadius: BorderRadius.circular(6),
          onTap: () {
            ref
                .read(timelineSelectionProvider.notifier)
                .selectClip(clip.id, track.id);
          },
          mouseCursor: SystemMouseCursors.click,
          hoverColor: trackColor.withValues(alpha: 0.12),
          splashColor: CatppuccinMocha.mauve.withValues(alpha: 0.25),
          highlightColor: CatppuccinMocha.mauve.withValues(alpha: 0.15),
          child: Padding(
            padding: isSelected
                ? const EdgeInsets.symmetric(horizontal: 7, vertical: 7)
                : const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  isVideo ? Icons.movie_rounded : Icons.graphic_eq_rounded,
                  size: 12,
                  color: isSelected ? CatppuccinMocha.mauve : trackColor,
                ),
                const SizedBox(width: 6),
                Text(
                  'Clip [${clip.timelineIn}..${clip.timelineOut} PTS]',
                  style: TextStyle(
                    color: isSelected ? CatppuccinMocha.text : trackColor,
                    fontSize: 11,
                    fontWeight: isSelected ? FontWeight.bold : FontWeight.w600,
                    fontFamily: 'monospace',
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
