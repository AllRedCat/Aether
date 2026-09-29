import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/api.dart';
import '../../theme/catppuccin.dart';
import 'media_pool_provider.dart';

/// Formatter and styling helpers for media assets.
class MediaFormatter {
  static String formatDuration(double seconds) {
    if (seconds.isNaN || seconds.isInfinite || seconds <= 0) return '00:00';
    final totalSeconds = seconds.round();
    final hours = totalSeconds ~/ 3600;
    final minutes = (totalSeconds % 3600) ~/ 60;
    final remainingSecs = totalSeconds % 60;

    if (hours > 0) {
      return '${hours.toString().padLeft(2, '0')}:${minutes.toString().padLeft(2, '0')}:${remainingSecs.toString().padLeft(2, '0')}';
    } else {
      return '${minutes.toString().padLeft(2, '0')}:${remainingSecs.toString().padLeft(2, '0')}';
    }
  }

  static String formatPts(int pts, {int fps = 60}) {
    if (pts <= 0) return '00:00';
    return formatDuration(pts / fps);
  }

  static Color typeColor(MediaType type) {
    switch (type) {
      case MediaType.video:
        return CatppuccinMocha.blue;
      case MediaType.audio:
        return CatppuccinMocha.green;
      case MediaType.image:
        return CatppuccinMocha.mauve;
    }
  }

  static IconData typeIcon(MediaType type) {
    switch (type) {
      case MediaType.video:
        return Icons.videocam_rounded;
      case MediaType.audio:
        return Icons.audiotrack_rounded;
      case MediaType.image:
        return Icons.image_rounded;
    }
  }

  static String typeLabel(MediaType type) {
    switch (type) {
      case MediaType.video:
        return 'VÍDEO';
      case MediaType.audio:
        return 'ÁUDIO';
      case MediaType.image:
        return 'IMAGEM';
    }
  }

  static String formatItemDuration(MediaItem item) {
    if (item.metadata.durationSeconds > 0) {
      return formatDuration(item.metadata.durationSeconds);
    }
    if (item.metadata.durationPts > 0) {
      return formatPts(item.metadata.durationPts);
    }
    return '00:00';
  }
}

/// MediaPoolView displays the project's media library, import button,
/// empty state placeholder, and media asset cards with one-tap timeline insertion.
class MediaPoolView extends ConsumerWidget {
  const MediaPoolView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(mediaPoolProvider);
    final notifier = ref.read(mediaPoolProvider.notifier);

    return Container(
      key: const Key('media_pool_view'),
      color: CatppuccinMocha.base,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header / Toolbar
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            decoration: const BoxDecoration(
              color: CatppuccinMocha.mantle,
              border: Border(
                bottom: BorderSide(color: CatppuccinMocha.surface0),
              ),
            ),
            child: Row(
              children: [
                Text(
                  'Mídias (${state.items.length})',
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.bold,
                    color: CatppuccinMocha.text,
                    letterSpacing: 0.3,
                  ),
                ),
                const Spacer(),
                ElevatedButton.icon(
                  key: const Key('import_media_button'),
                  onPressed: state.isLoading ? null : () => notifier.importMedia(),
                  icon: state.isLoading
                      ? const SizedBox(
                          width: 12,
                          height: 12,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: CatppuccinMocha.base,
                          ),
                        )
                      : const Icon(Icons.file_upload_outlined, size: 14),
                  label: const Text(
                    'Importar',
                    style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600),
                  ),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: CatppuccinMocha.mauve,
                    foregroundColor: CatppuccinMocha.base,
                    elevation: 0,
                    visualDensity: VisualDensity.compact,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 8,
                    ),
                  ),
                ),
              ],
            ),
          ),

          // Loading Progress Bar
          if (state.isLoading)
            const LinearProgressIndicator(
              minHeight: 2,
              backgroundColor: CatppuccinMocha.surface0,
              valueColor: AlwaysStoppedAnimation<Color>(CatppuccinMocha.mauve),
            ),

          // Error Banner
          if (state.errorMessage != null)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              color: CatppuccinMocha.red.withValues(alpha: 0.15),
              child: Row(
                children: [
                  const Icon(
                    Icons.error_outline,
                    size: 16,
                    color: CatppuccinMocha.red,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      state.errorMessage!,
                      style: const TextStyle(
                        fontSize: 11,
                        color: CatppuccinMocha.red,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
            ),

          // Body: Empty State or Media List
          Expanded(
            child: state.items.isEmpty && !state.isLoading
                ? _buildEmptyState(context, notifier)
                : _buildMediaList(context, state, notifier),
          ),
        ],
      ),
    );
  }

  Widget _buildEmptyState(BuildContext context, MediaPoolNotifier notifier) {
    return Center(
      key: const Key('media_pool_empty_state'),
      child: Padding(
        padding: const EdgeInsets.all(24.0),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Container(
              width: 56,
              height: 56,
              decoration: BoxDecoration(
                color: CatppuccinMocha.surface0.withValues(alpha: 0.5),
                shape: BoxShape.circle,
              ),
              child: const Icon(
                Icons.video_library_outlined,
                size: 28,
                color: CatppuccinMocha.overlay0,
              ),
            ),
            const SizedBox(height: 12),
            const Text(
              'Nenhuma mídia importada',
              style: TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: CatppuccinMocha.text,
              ),
            ),
            const SizedBox(height: 4),
            const Text(
              'Importe vídeos, áudios e imagens para o projeto',
              textAlign: TextAlign.center,
              style: TextStyle(
                fontSize: 11,
                color: CatppuccinMocha.subtext0,
              ),
            ),
            const SizedBox(height: 16),
            OutlinedButton.icon(
              key: const Key('empty_import_media_button'),
              onPressed: () => notifier.importMedia(),
              icon: const Icon(Icons.add, size: 14),
              label: const Text(
                'Importar Arquivos',
                style: TextStyle(fontSize: 12),
              ),
              style: OutlinedButton.styleFrom(
                foregroundColor: CatppuccinMocha.mauve,
                side: const BorderSide(color: CatppuccinMocha.mauve),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(6),
                ),
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 8,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMediaList(
    BuildContext context,
    MediaPoolState state,
    MediaPoolNotifier notifier,
  ) {
    return ListView.builder(
      key: const Key('media_pool_list'),
      padding: const EdgeInsets.all(8),
      itemCount: state.items.length,
      itemBuilder: (context, index) {
        final item = state.items[index];
        final isSelected = state.selectedItemId == item.id;

        return _buildMediaItemCard(context, item, isSelected, notifier);
      },
    );
  }

  Widget _buildMediaItemCard(
    BuildContext context,
    MediaItem item,
    bool isSelected,
    MediaPoolNotifier notifier,
  ) {
    final typeColor = MediaFormatter.typeColor(item.mediaType);
    final typeIcon = MediaFormatter.typeIcon(item.mediaType);
    final typeLabel = MediaFormatter.typeLabel(item.mediaType);
    final durationStr = MediaFormatter.formatItemDuration(item);

    return Container(
      key: Key('media_item_${item.id}'),
      margin: const EdgeInsets.only(bottom: 6),
      decoration: BoxDecoration(
        color: isSelected
            ? CatppuccinMocha.surface1
            : CatppuccinMocha.surface0.withValues(alpha: 0.6),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(
          color: isSelected
              ? CatppuccinMocha.mauve
              : CatppuccinMocha.surface0,
          width: 1,
        ),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(6),
        onTap: () => notifier.selectItem(item.id),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: Row(
            children: [
              // Type Icon thumbnail
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

              // Title and metadata badge
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      item.fileName,
                      style: const TextStyle(
                        color: CatppuccinMocha.text,
                        fontSize: 13,
                        fontWeight: FontWeight.w500,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 3),
                    Row(
                      children: [
                        Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 5,
                            vertical: 1,
                          ),
                          decoration: BoxDecoration(
                            color: typeColor.withValues(alpha: 0.2),
                            borderRadius: BorderRadius.circular(3),
                          ),
                          child: Text(
                            typeLabel,
                            style: TextStyle(
                              color: typeColor,
                              fontSize: 9,
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                        ),
                        if (item.metadata.width != null &&
                            item.metadata.height != null) ...[
                          const SizedBox(width: 6),
                          Text(
                            '${item.metadata.width}x${item.metadata.height}',
                            style: const TextStyle(
                              color: CatppuccinMocha.subtext0,
                              fontSize: 10,
                            ),
                          ),
                        ],
                        if (item.metadata.sampleRate != null) ...[
                          const SizedBox(width: 6),
                          Text(
                            '${(item.metadata.sampleRate! / 1000).toStringAsFixed(1)} kHz',
                            style: const TextStyle(
                              color: CatppuccinMocha.subtext0,
                              fontSize: 10,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),

              // Formatted duration
              Text(
                durationStr,
                style: const TextStyle(
                  color: CatppuccinMocha.subtext1,
                  fontFamily: 'monospace',
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                ),
              ),
              const SizedBox(width: 4),

              // Add to Timeline Button
              IconButton(
                key: Key('add_to_timeline_${item.id}'),
                tooltip: 'Adicionar à Timeline',
                icon: const Icon(
                  Icons.playlist_add_rounded,
                  size: 20,
                  color: CatppuccinMocha.mauve,
                ),
                onPressed: () => notifier.addMediaToTimeline(item),
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
