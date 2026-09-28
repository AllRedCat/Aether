import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';
import 'timeline_provider.dart';

/// TimelineView displays the non-linear editing timeline, tracks, clips,
/// duration PTS, and controls for adding clips via Riverpod state.
class TimelineView extends ConsumerWidget {
  const TimelineView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(timelineProvider);

    return Container(
      color: Colors.black87,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header / Toolbar
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            color: Colors.black54,
            child: Row(
              children: [
                const Text(
                  'Clips: ',
                  style: TextStyle(color: Colors.white70, fontSize: 14),
                ),
                Text(
                  '${state.totalClipCount}',
                  key: const Key('timeline_total_clips_count'),
                  style: const TextStyle(
                    color: Colors.white,
                    fontWeight: FontWeight.bold,
                    fontSize: 16,
                  ),
                ),
                const SizedBox(width: 24),
                Text(
                  'Duration: ${state.durationPts} PTS',
                  key: const Key('timeline_duration_pts'),
                  style: const TextStyle(color: Colors.white70, fontSize: 14),
                ),
                const Spacer(),
                ElevatedButton.icon(
                  key: const Key('add_clip_button'),
                  onPressed: state.isLoading
                      ? null
                      : () => ref.read(timelineProvider.notifier).addClip(),
                  icon: state.isLoading
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.add, size: 18),
                  label: const Text('Add Clip'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.indigoAccent,
                    foregroundColor: Colors.white,
                  ),
                ),
              ],
            ),
          ),

          // Error Banner Display
          if (state.errorMessage != null)
            Container(
              color: Colors.red.shade900,
              padding: const EdgeInsets.all(8),
              child: Row(
                children: [
                  const Icon(Icons.error_outline,
                      color: Colors.white, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      state.errorMessage!,
                      style: const TextStyle(color: Colors.white, fontSize: 12),
                    ),
                  ),
                ],
              ),
            ),

          // Track & Clip Rendering Lanes
          Expanded(
            child: state.isLoading && state.timeline == null
                ? const Center(child: CircularProgressIndicator())
                : state.tracks.isEmpty
                    ? const Center(
                        child: Text(
                          'No tracks available',
                          style: TextStyle(color: Colors.white54),
                        ),
                      )
                    : ListView.builder(
                        padding: const EdgeInsets.all(8),
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
    final trackName = 'Track ${index + 1} (${track.kind.name.toUpperCase()})';

    return Card(
      margin: const EdgeInsets.symmetric(vertical: 4),
      color: Colors.grey.shade900,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(6),
        side: BorderSide(color: Colors.grey.shade800),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  track.kind == TrackKind.video
                      ? Icons.videocam
                      : Icons.audiotrack,
                  size: 16,
                  color: Colors.indigoAccent,
                ),
                const SizedBox(width: 6),
                Text(
                  trackName,
                  style: const TextStyle(
                    color: Colors.white,
                    fontWeight: FontWeight.bold,
                    fontSize: 13,
                  ),
                ),
                const Spacer(),
                Text(
                  '${track.clips.length} clip(s)',
                  style: const TextStyle(color: Colors.white54, fontSize: 12),
                ),
              ],
            ),
            const SizedBox(height: 8),
            if (track.clips.isEmpty)
              Container(
                height: 36,
                alignment: Alignment.centerLeft,
                child: const Text(
                  'Track is empty. Click "Add Clip" to add media.',
                  style: TextStyle(
                    color: Colors.white38,
                    fontSize: 12,
                    fontStyle: FontStyle.italic,
                  ),
                ),
              )
            else
              Wrap(
                spacing: 8,
                runSpacing: 4,
                children: track.clips.map((clip) {
                  return Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 6,
                    ),
                    decoration: BoxDecoration(
                      color: Colors.indigo.shade800,
                      borderRadius: BorderRadius.circular(4),
                      border: Border.all(color: Colors.indigo.shade400),
                    ),
                    child: Text(
                      'Clip [${clip.timelineIn}..${clip.timelineOut} PTS]',
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 11,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  );
                }).toList(),
              ),
          ],
        ),
      ),
    );
  }
}
