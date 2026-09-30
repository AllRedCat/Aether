import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../theme/catppuccin.dart';
import '../clip_properties_provider.dart';
import '../selected_clip_details_provider.dart';
import 'inspector_slider_row.dart';

/// Contextual inspector view displayed when an audio clip is selected.
/// Exposes volume (dB), stereo pan, and mute toggle controls.
class AudioPropertiesView extends ConsumerWidget {
  final SelectedClipDetails clipDetails;

  const AudioPropertiesView({
    super.key,
    required this.clipDetails,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final clipId = clipDetails.clip.id;
    final clipProps = ref.watch(clipPropertiesProvider);
    final audioProps = clipProps.audioPropertiesFor(clipId);
    final notifier = ref.read(clipPropertiesProvider.notifier);

    final clipDuration = clipDetails.clip.timelineOut - clipDetails.clip.timelineIn;
    final fileName = clipDetails.mediaItem?.fileName ?? 'Audio Clip';

    final volumeText = audioProps.volumeDb >= 0
        ? '+${audioProps.volumeDb.toStringAsFixed(1)} dB'
        : '${audioProps.volumeDb.toStringAsFixed(1)} dB';

    final panText = audioProps.pan == 0.0
        ? 'Center'
        : (audioProps.pan < 0
            ? 'L ${(audioProps.pan.abs() * 100).toStringAsFixed(0)}%'
            : 'R ${(audioProps.pan * 100).toStringAsFixed(0)}%');

    return SingleChildScrollView(
      key: const Key('audio_properties_view'),
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
                    const Icon(
                      Icons.graphic_eq_rounded,
                      size: 18,
                      color: CatppuccinMocha.green,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        fileName,
                        key: const Key('audio_clip_name'),
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
                        color: CatppuccinMocha.green.withValues(alpha: 0.2),
                        borderRadius: BorderRadius.circular(4),
                        border: Border.all(
                          color: CatppuccinMocha.green.withValues(alpha: 0.4),
                        ),
                      ),
                      child: const Text(
                        'AUDIO',
                        style: TextStyle(
                          color: CatppuccinMocha.green,
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

          // Audio Controls Section Header with Reset All
          Row(
            children: [
              Container(
                width: 3,
                height: 12,
                decoration: BoxDecoration(
                  color: CatppuccinMocha.green,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  'AUDIO CONTROLS',
                  style: TextStyle(
                    color: CatppuccinMocha.green,
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                    letterSpacing: 0.5,
                  ),
                ),
              ),
              InkWell(
                key: const Key('reset_all_audio'),
                onTap: () => notifier.resetAudioProperties(clipId),
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

          // Volume dB Slider
          InspectorSliderRow(
            label: 'Volume (dB)',
            value: audioProps.volumeDb,
            min: AudioClipProperties.minVolumeDb,
            max: AudioClipProperties.maxVolumeDb,
            formattedValue: volumeText,
            sliderKey: const Key('slider_volume_db'),
            badgeKey: const Key('badge_volume_db'),
            resetKey: const Key('reset_volume_db'),
            activeColor: CatppuccinMocha.green,
            onChanged: (val) => notifier.updateAudioProperties(clipId, volumeDb: val),
            onReset: () => notifier.resetVolume(clipId),
          ),

          // Pan Slider
          InspectorSliderRow(
            label: 'Pan',
            value: audioProps.pan,
            min: AudioClipProperties.minPan,
            max: AudioClipProperties.maxPan,
            formattedValue: panText,
            sliderKey: const Key('slider_pan'),
            badgeKey: const Key('badge_pan'),
            resetKey: const Key('reset_pan'),
            activeColor: CatppuccinMocha.green,
            onChanged: (val) => notifier.updateAudioProperties(clipId, pan: val),
            onReset: () => notifier.resetPan(clipId),
          ),

          const SizedBox(height: 6),

          // Mute Toggle Row
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: BoxDecoration(
              color: CatppuccinMocha.mantle,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(
                color: audioProps.isMuted
                    ? CatppuccinMocha.red.withValues(alpha: 0.5)
                    : CatppuccinMocha.surface0,
              ),
            ),
            child: Row(
              children: [
                Icon(
                  audioProps.isMuted ? Icons.volume_off_rounded : Icons.volume_up_rounded,
                  size: 18,
                  color: audioProps.isMuted ? CatppuccinMocha.red : CatppuccinMocha.green,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        audioProps.isMuted ? 'Muted' : 'Track Audio Enabled',
                        style: TextStyle(
                          color: audioProps.isMuted ? CatppuccinMocha.red : CatppuccinMocha.text,
                          fontWeight: FontWeight.w600,
                          fontSize: 12,
                        ),
                      ),
                      Text(
                        audioProps.isMuted
                            ? 'Audio output is silenced for this clip'
                            : 'Audio playback active',
                        style: const TextStyle(
                          color: CatppuccinMocha.subtext0,
                          fontSize: 10,
                        ),
                      ),
                    ],
                  ),
                ),
                Switch(
                  key: const Key('switch_mute'),
                  value: audioProps.isMuted,
                  activeThumbColor: CatppuccinMocha.red,
                  activeTrackColor: CatppuccinMocha.red.withValues(alpha: 0.3),
                  inactiveThumbColor: CatppuccinMocha.overlay0,
                  inactiveTrackColor: CatppuccinMocha.surface0,
                  onChanged: (_) => notifier.toggleMute(clipId),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
