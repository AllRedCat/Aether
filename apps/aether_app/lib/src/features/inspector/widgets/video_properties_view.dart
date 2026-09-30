import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../bridge/api.dart';
import '../../../theme/catppuccin.dart';
import '../clip_properties_provider.dart';
import '../selected_clip_details_provider.dart';
import 'inspector_slider_row.dart';

/// Contextual inspector view displayed when a video or overlay clip is selected.
/// Exposes transform controls (Position X/Y, Scale/Zoom, Rotation, Opacity)
/// with real-time numeric badges and reset actions.
class VideoPropertiesView extends ConsumerWidget {
  final SelectedClipDetails clipDetails;

  const VideoPropertiesView({
    super.key,
    required this.clipDetails,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final clipId = clipDetails.clip.id;
    final clipProps = ref.watch(clipPropertiesProvider);
    final videoProps = clipProps.videoPropertiesFor(clipId);
    final notifier = ref.read(clipPropertiesProvider.notifier);

    final clipDuration = clipDetails.clip.timelineOut - clipDetails.clip.timelineIn;
    final fileName = clipDetails.mediaItem?.fileName ?? 'Video Clip';
    final isOverlay = clipDetails.trackKind == TrackKind.overlay;

    return SingleChildScrollView(
      key: const Key('video_properties_view'),
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Clip Header Card
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: CatppuccinMocha.mantle,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: CatppuccinMocha.surface0),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(
                      isOverlay ? Icons.layers_rounded : Icons.movie_filter_rounded,
                      size: 18,
                      color: CatppuccinMocha.mauve,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        fileName,
                        key: const Key('video_clip_name'),
                        style: const TextStyle(
                          color: CatppuccinMocha.text,
                          fontWeight: FontWeight.bold,
                          fontSize: 13,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                      decoration: BoxDecoration(
                        color: CatppuccinMocha.mauve.withValues(alpha: 0.2),
                        borderRadius: BorderRadius.circular(4),
                        border: Border.all(
                          color: CatppuccinMocha.mauve.withValues(alpha: 0.4),
                        ),
                      ),
                      child: Text(
                        isOverlay ? 'OVERLAY' : 'VIDEO',
                        style: const TextStyle(
                          color: CatppuccinMocha.mauve,
                          fontSize: 10,
                          fontWeight: FontWeight.bold,
                          letterSpacing: 0.5,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                Text(
                  'Range: [${clipDetails.clip.timelineIn} .. ${clipDetails.clip.timelineOut} PTS]  •  Duration: $clipDuration PTS',
                  style: const TextStyle(
                    color: CatppuccinMocha.overlay0,
                    fontSize: 11,
                    fontFamily: 'monospace',
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 20),

          // Transform Section Header with Reset All
          Row(
            children: [
              Container(
                width: 3,
                height: 12,
                decoration: BoxDecoration(
                  color: CatppuccinMocha.mauve,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  'TRANSFORM',
                  style: TextStyle(
                    color: CatppuccinMocha.mauve,
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                    letterSpacing: 0.5,
                  ),
                ),
              ),
              InkWell(
                key: const Key('reset_all_transform'),
                onTap: () => notifier.resetVideoProperties(clipId),
                borderRadius: BorderRadius.circular(4),
                child: const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  child: Row(
                    children: [
                      Icon(Icons.restart_alt_rounded, size: 13, color: CatppuccinMocha.overlay0),
                      SizedBox(width: 4),
                      Text(
                        'Reset All',
                        style: TextStyle(color: CatppuccinMocha.overlay0, fontSize: 11),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 14),

          // Position X Slider
          InspectorSliderRow(
            label: 'Position X',
            value: videoProps.positionX,
            min: VideoTransformProperties.minPositionX,
            max: VideoTransformProperties.maxPositionX,
            formattedValue: '${videoProps.positionX.toStringAsFixed(1)} px',
            sliderKey: const Key('slider_position_x'),
            badgeKey: const Key('badge_position_x'),
            resetKey: const Key('reset_position_x'),
            activeColor: CatppuccinMocha.mauve,
            onChanged: (val) => notifier.updateVideoProperties(clipId, positionX: val),
            onReset: () => notifier.resetVideoPositionX(clipId),
          ),

          // Position Y Slider
          InspectorSliderRow(
            label: 'Position Y',
            value: videoProps.positionY,
            min: VideoTransformProperties.minPositionY,
            max: VideoTransformProperties.maxPositionY,
            formattedValue: '${videoProps.positionY.toStringAsFixed(1)} px',
            sliderKey: const Key('slider_position_y'),
            badgeKey: const Key('badge_position_y'),
            resetKey: const Key('reset_position_y'),
            activeColor: CatppuccinMocha.mauve,
            onChanged: (val) => notifier.updateVideoProperties(clipId, positionY: val),
            onReset: () => notifier.resetVideoPositionY(clipId),
          ),

          // Scale / Zoom Slider
          InspectorSliderRow(
            label: 'Scale / Zoom',
            value: videoProps.scale,
            min: VideoTransformProperties.minScale,
            max: VideoTransformProperties.maxScale,
            formattedValue: '${(videoProps.scale * 100).toStringAsFixed(0)}%',
            sliderKey: const Key('slider_scale'),
            badgeKey: const Key('badge_scale'),
            resetKey: const Key('reset_scale'),
            activeColor: CatppuccinMocha.mauve,
            onChanged: (val) => notifier.updateVideoProperties(clipId, scale: val),
            onReset: () => notifier.resetVideoScale(clipId),
          ),

          // Rotation Slider
          InspectorSliderRow(
            label: 'Rotation',
            value: videoProps.rotation,
            min: VideoTransformProperties.minRotation,
            max: VideoTransformProperties.maxRotation,
            formattedValue: '${videoProps.rotation.toStringAsFixed(1)}°',
            sliderKey: const Key('slider_rotation'),
            badgeKey: const Key('badge_rotation'),
            resetKey: const Key('reset_rotation'),
            activeColor: CatppuccinMocha.mauve,
            onChanged: (val) => notifier.updateVideoProperties(clipId, rotation: val),
            onReset: () => notifier.resetVideoRotation(clipId),
          ),

          // Opacity Slider
          InspectorSliderRow(
            label: 'Opacity',
            value: videoProps.opacity,
            min: VideoTransformProperties.minOpacity,
            max: VideoTransformProperties.maxOpacity,
            formattedValue: '${(videoProps.opacity * 100).toStringAsFixed(0)}%',
            sliderKey: const Key('slider_opacity'),
            badgeKey: const Key('badge_opacity'),
            resetKey: const Key('reset_opacity'),
            activeColor: CatppuccinMocha.mauve,
            onChanged: (val) => notifier.updateVideoProperties(clipId, opacity: val),
            onReset: () => notifier.resetVideoOpacity(clipId),
          ),
        ],
      ),
    );
  }
}
