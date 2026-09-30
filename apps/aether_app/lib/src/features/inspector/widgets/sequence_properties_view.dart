import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../bridge/api.dart';
import '../../../theme/catppuccin.dart';
import '../../timeline/timeline_provider.dart';

/// Contextual inspector view displayed when no clip is selected.
/// Presents sequence-level metadata (PTS duration, timebase, tracks, clips)
/// and a friendly prompt guiding the user.
class SequencePropertiesView extends ConsumerWidget {
  const SequencePropertiesView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final timelineState = ref.watch(timelineProvider);
    final timeline = timelineState.timeline;

    final videoTracks = timelineState.tracks
        .where((t) => t.kind == TrackKind.video || t.kind == TrackKind.overlay)
        .length;
    final audioTracks = timelineState.tracks
        .where((t) => t.kind == TrackKind.audio)
        .length;

    final timebaseNum = timeline?.timebase.num ?? 60;
    final timebaseDen = timeline?.timebase.den ?? 1;
    final fpsString = timebaseDen == 1
        ? '$timebaseNum fps'
        : '${(timebaseNum / timebaseDen).toStringAsFixed(2)} fps';

    return SingleChildScrollView(
      key: const Key('sequence_properties_view'),
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Friendly Guidance Banner
          Container(
            key: const Key('sequence_prompt_banner'),
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: CatppuccinMocha.surface0.withValues(alpha: 0.5),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: CatppuccinMocha.surface1, width: 1),
            ),
            child: const Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(
                  Icons.touch_app_rounded,
                  color: CatppuccinMocha.blue,
                  size: 20,
                ),
                SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Sequence Properties',
                        key: Key('sequence_title'),
                        style: TextStyle(
                          color: CatppuccinMocha.text,
                          fontWeight: FontWeight.bold,
                          fontSize: 13,
                        ),
                      ),
                      SizedBox(height: 4),
                      Text(
                        'Select a clip in the Timeline to inspect and edit its video transform or audio parameters.',
                        key: Key('sequence_hint'),
                        style: TextStyle(
                          color: CatppuccinMocha.subtext0,
                          fontSize: 11,
                          height: 1.3,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 20),

          // Section Header: Timeline Info
          _buildSectionHeader('TIMELINE INFO', CatppuccinMocha.blue),
          const SizedBox(height: 12),

          // Info Cards / Rows
          Container(
            decoration: BoxDecoration(
              color: CatppuccinMocha.mantle,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: CatppuccinMocha.surface0),
            ),
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
            child: Column(
              children: [
                _buildMetadataRow(
                  label: 'Duration PTS',
                  value: '${timelineState.durationPts} PTS',
                  valueKey: const Key('seq_duration_pts'),
                  icon: Icons.timer_outlined,
                ),
                const Divider(color: CatppuccinMocha.surface0, height: 16),
                _buildMetadataRow(
                  label: 'Timebase',
                  value: '$timebaseNum / $timebaseDen ($fpsString)',
                  valueKey: const Key('seq_timebase'),
                  icon: Icons.speed_rounded,
                ),
                const Divider(color: CatppuccinMocha.surface0, height: 16),
                _buildMetadataRow(
                  label: 'Active Tracks',
                  value: '${timelineState.tracks.length} ($videoTracks Video, $audioTracks Audio)',
                  valueKey: const Key('seq_track_count'),
                  icon: Icons.layers_outlined,
                ),
                const Divider(color: CatppuccinMocha.surface0, height: 16),
                _buildMetadataRow(
                  label: 'Total Clips',
                  value: '${timelineState.totalClipCount} clips',
                  valueKey: const Key('seq_clip_count'),
                  icon: Icons.movie_creation_outlined,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSectionHeader(String title, Color accentColor) {
    return Row(
      children: [
        Container(
          width: 3,
          height: 12,
          decoration: BoxDecoration(
            color: accentColor,
            borderRadius: BorderRadius.circular(2),
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            title,
            overflow: TextOverflow.ellipsis,
            maxLines: 1,
            style: TextStyle(
              color: accentColor,
              fontWeight: FontWeight.bold,
              fontSize: 12,
              letterSpacing: 0.5,
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildMetadataRow({
    required String label,
    required String value,
    required Key valueKey,
    required IconData icon,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: LayoutBuilder(
        builder: (context, constraints) {
          if (constraints.maxWidth < 140) {
            // Compact vertically-stacked layout for extreme narrow/adversarial constraints
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: const TextStyle(
                    color: CatppuccinMocha.subtext0,
                    fontSize: 10,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 2),
                Text(
                  value,
                  key: valueKey,
                  style: const TextStyle(
                    color: CatppuccinMocha.text,
                    fontFamily: 'monospace',
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            );
          }
          return Row(
            children: [
              Icon(icon, size: 14, color: CatppuccinMocha.overlay0),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  label,
                  style: const TextStyle(
                    color: CatppuccinMocha.subtext0,
                    fontSize: 12,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  value,
                  key: valueKey,
                  style: const TextStyle(
                    color: CatppuccinMocha.text,
                    fontFamily: 'monospace',
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.end,
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}
