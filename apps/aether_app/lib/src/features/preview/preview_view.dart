import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../theme/catppuccin.dart';
import '../media_pool/media_pool_view.dart';
import 'preview_provider.dart';

/// Preview panel widget rendering hardware-accelerated video frames via Flutter's
/// [Texture] widget, along with playback controls, timeline scrubber, and timecode.
class PreviewView extends ConsumerWidget {
  const PreviewView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(previewProvider);
    final notifier = ref.read(previewProvider.notifier);

    return Padding(
      padding: const EdgeInsets.all(16.0),
      child: Column(
        children: [
          // 1. Video Canvas Viewport
          Expanded(
            child: Container(
              width: double.infinity,
              decoration: BoxDecoration(
                color: Colors.black,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: CatppuccinMocha.surface0),
              ),
              clipBehavior: Clip.antiAlias,
              child: _buildViewport(context, state),
            ),
          ),
          const SizedBox(height: 12),
          // 2. Scrubber Bar with Timecode
          _buildScrubberBar(context, state, notifier),
          const SizedBox(height: 8),
          // 3. Playback Controls
          _buildControlBar(context, state, notifier),
        ],
      ),
    );
  }

  Widget _buildViewport(BuildContext context, PreviewState state) {
    // 1. Loading State
    if (state.isLoading) {
      return const Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            CircularProgressIndicator(color: CatppuccinMocha.mauve),
            SizedBox(height: 12),
            Text(
              'Inicializando decodificador...',
              style: TextStyle(
                color: CatppuccinMocha.subtext0,
                fontSize: 13,
              ),
            ),
          ],
        ),
      );
    }

    // 2. Error State
    if (state.errorMessage != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(
                Icons.error_outline_rounded,
                color: CatppuccinMocha.red,
                size: 40,
              ),
              const SizedBox(height: 8),
              Text(
                state.errorMessage!,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  color: CatppuccinMocha.red,
                  fontSize: 13,
                ),
              ),
            ],
          ),
        ),
      );
    }

    // 3. Active Session / Video Frame Display
    if (state.session != null) {
      return Center(
        child: AspectRatio(
          aspectRatio: state.aspectRatio,
          child: Texture(
            key: const Key('preview_texture_widget'),
            textureId: state.textureId,
            filterQuality: FilterQuality.medium,
          ),
        ),
      );
    }

    // 4. Empty Placeholder State
    return const Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.movie_filter_outlined,
            color: CatppuccinMocha.surface2,
            size: 48,
          ),
          SizedBox(height: 8),
          Text(
            'Nenhuma mídia selecionada',
            style: TextStyle(
              color: CatppuccinMocha.subtext0,
              fontSize: 14,
              fontWeight: FontWeight.w500,
            ),
          ),
          SizedBox(height: 4),
          Text(
            'Selecione um arquivo no Media Pool para iniciar a pré-visualização',
            style: TextStyle(
              color: CatppuccinMocha.overlay0,
              fontSize: 12,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildScrubberBar(
    BuildContext context,
    PreviewState state,
    PreviewNotifier notifier,
  ) {
    final isEnabled = state.isLoaded && state.durationSeconds > 0;
    final progressFraction = state.progressFraction;

    return Row(
      children: [
        // Scrubber progress slider
        Expanded(
          child: SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 4.0,
              thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6.0),
              overlayShape: const RoundSliderOverlayShape(overlayRadius: 14.0),
              activeTrackColor: CatppuccinMocha.mauve,
              inactiveTrackColor: CatppuccinMocha.surface0,
              thumbColor: CatppuccinMocha.mauve,
              overlayColor: CatppuccinMocha.mauve.withValues(alpha: 0.2),
            ),
            child: Slider(
              key: const Key('preview_progress_slider'),
              value: progressFraction.clamp(0.0, 1.0),
              min: 0.0,
              max: 1.0,
              onChanged: isEnabled
                  ? (value) {
                      notifier.seek(value);
                    }
                  : null,
            ),
          ),
        ),
        const SizedBox(width: 8),
        // Timecode display
        Text(
          '${MediaFormatter.formatDuration(state.currentSeconds)} / ${MediaFormatter.formatDuration(state.durationSeconds)}',
          key: const Key('preview_timecode_text'),
          style: const TextStyle(
            fontFamily: 'monospace',
            color: CatppuccinMocha.subtext0,
            fontSize: 12,
          ),
        ),
      ],
    );
  }

  Widget _buildControlBar(
    BuildContext context,
    PreviewState state,
    PreviewNotifier notifier,
  ) {
    final isEnabled = state.isLoaded;

    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        // Skip previous / rewind to start
        IconButton(
          onPressed: isEnabled ? () => notifier.seekSeconds(0.0) : null,
          icon: const Icon(Icons.skip_previous_rounded),
          color: CatppuccinMocha.text,
          disabledColor: CatppuccinMocha.surface2,
          tooltip: 'Início',
        ),
        const SizedBox(width: 8),
        // Play / Pause toggle button
        Container(
          decoration: BoxDecoration(
            color: isEnabled
                ? CatppuccinMocha.mauve
                : CatppuccinMocha.surface0,
            shape: BoxShape.circle,
          ),
          child: IconButton(
            key: const Key('preview_play_pause_button'),
            onPressed: isEnabled ? () => notifier.togglePlayPause() : null,
            icon: Icon(
              state.isPlaying
                  ? Icons.pause_rounded
                  : Icons.play_arrow_rounded,
            ),
            color:
                isEnabled ? CatppuccinMocha.crust : CatppuccinMocha.surface2,
            iconSize: 28,
            tooltip: state.isPlaying ? 'Pausar' : 'Reproduzir',
          ),
        ),
        const SizedBox(width: 8),
        // Skip next / jump to end
        IconButton(
          onPressed: isEnabled
              ? () => notifier.seekSeconds(state.durationSeconds)
              : null,
          icon: const Icon(Icons.skip_next_rounded),
          color: CatppuccinMocha.text,
          disabledColor: CatppuccinMocha.surface2,
          tooltip: 'Fim',
        ),
      ],
    );
  }
}
