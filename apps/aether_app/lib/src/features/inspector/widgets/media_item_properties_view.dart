import 'package:flutter/material.dart';

import '../../../bridge/api.dart';
import '../../../theme/catppuccin.dart';
import '../../media_pool/media_pool_view.dart';

/// Inspector view displayed when a media item is selected in the Media Pool
/// but no clip is selected in the Timeline. Shows detailed metadata about
/// the source media file (resolution, codec, duration, sample rate, etc.).
class MediaItemPropertiesView extends StatelessWidget {
  final MediaItem mediaItem;

  const MediaItemPropertiesView({
    super.key,
    required this.mediaItem,
  });

  @override
  Widget build(BuildContext context) {
    final meta = mediaItem.metadata;
    final typeColor = MediaFormatter.typeColor(mediaItem.mediaType);
    final typeIcon = MediaFormatter.typeIcon(mediaItem.mediaType);
    final typeLabel = MediaFormatter.typeLabel(mediaItem.mediaType);
    final durationStr = MediaFormatter.formatItemDuration(mediaItem);
    final fileSizeStr = _formatFileSize(meta.fileSizeBytes);

    return SingleChildScrollView(
      key: const Key('media_item_properties_view'),
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Media Item Header Card
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
                    Container(
                      width: 36,
                      height: 36,
                      decoration: BoxDecoration(
                        color: typeColor.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(6),
                      ),
                      child: Icon(typeIcon, color: typeColor, size: 20),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            mediaItem.fileName,
                            key: const Key('media_item_name'),
                            style: const TextStyle(
                              color: CatppuccinMocha.text,
                              fontWeight: FontWeight.bold,
                              fontSize: 13,
                            ),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                          const SizedBox(height: 4),
                          Container(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 6,
                              vertical: 2,
                            ),
                            decoration: BoxDecoration(
                              color: typeColor.withValues(alpha: 0.2),
                              borderRadius: BorderRadius.circular(4),
                              border: Border.all(
                                color: typeColor.withValues(alpha: 0.4),
                              ),
                            ),
                            child: Text(
                              typeLabel,
                              style: TextStyle(
                                color: typeColor,
                                fontSize: 10,
                                fontWeight: FontWeight.bold,
                                letterSpacing: 0.5,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
          const SizedBox(height: 20),

          // Section Header: Media Info
          _buildSectionHeader('INFORMAÇÕES DA MÍDIA', typeColor),
          const SizedBox(height: 12),

          // Info Rows
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
                  label: 'Duração',
                  value: durationStr,
                  valueKey: const Key('media_duration'),
                  icon: Icons.timer_outlined,
                ),
                if (meta.durationPts > 0) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Duração (PTS)',
                    value: '${meta.durationPts} PTS',
                    valueKey: const Key('media_duration_pts'),
                    icon: Icons.schedule_rounded,
                  ),
                ],
                const Divider(color: CatppuccinMocha.surface0, height: 16),
                _buildMetadataRow(
                  label: 'Tamanho',
                  value: fileSizeStr,
                  valueKey: const Key('media_file_size'),
                  icon: Icons.storage_rounded,
                ),

                // Video-specific metadata
                if (meta.width != null && meta.height != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Resolução',
                    value: '${meta.width} × ${meta.height}',
                    valueKey: const Key('media_resolution'),
                    icon: Icons.aspect_ratio_rounded,
                  ),
                ],
                if (meta.videoCodec != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Codec de Vídeo',
                    value: meta.videoCodec!,
                    valueKey: const Key('media_video_codec'),
                    icon: Icons.videocam_rounded,
                  ),
                ],
                if (meta.pixelFormat != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Pixel Format',
                    value: meta.pixelFormat!,
                    valueKey: const Key('media_pixel_format'),
                    icon: Icons.palette_rounded,
                  ),
                ],
                if (meta.timebase != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Timebase',
                    value: '${meta.timebase!.num} / ${meta.timebase!.den}',
                    valueKey: const Key('media_timebase'),
                    icon: Icons.speed_rounded,
                  ),
                ],

                // Audio-specific metadata
                if (meta.audioCodec != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Codec de Áudio',
                    value: meta.audioCodec!,
                    valueKey: const Key('media_audio_codec'),
                    icon: Icons.audiotrack_rounded,
                  ),
                ],
                if (meta.sampleRate != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Sample Rate',
                    value: '${(meta.sampleRate! / 1000).toStringAsFixed(1)} kHz',
                    valueKey: const Key('media_sample_rate'),
                    icon: Icons.graphic_eq_rounded,
                  ),
                ],
                if (meta.audioChannels != null) ...[
                  const Divider(color: CatppuccinMocha.surface0, height: 16),
                  _buildMetadataRow(
                    label: 'Canais',
                    value: meta.audioChannels == 1
                        ? 'Mono'
                        : meta.audioChannels == 2
                            ? 'Estéreo'
                            : '${meta.audioChannels} canais',
                    valueKey: const Key('media_channels'),
                    icon: Icons.surround_sound_rounded,
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 20),

          // File Path Section
          _buildSectionHeader('CAMINHO DO ARQUIVO', CatppuccinMocha.overlay0),
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(10),
            decoration: BoxDecoration(
              color: CatppuccinMocha.mantle,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: CatppuccinMocha.surface0),
            ),
            child: Text(
              mediaItem.filePath,
              key: const Key('media_file_path'),
              style: const TextStyle(
                color: CatppuccinMocha.subtext0,
                fontFamily: 'monospace',
                fontSize: 10,
                height: 1.4,
              ),
              maxLines: 3,
              overflow: TextOverflow.ellipsis,
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
      child: Row(
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
      ),
    );
  }

  String _formatFileSize(int bytes) {
    if (bytes <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    int unitIndex = 0;
    double size = bytes.toDouble();
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    return '${size.toStringAsFixed(unitIndex == 0 ? 0 : 1)} ${units[unitIndex]}';
  }
}
