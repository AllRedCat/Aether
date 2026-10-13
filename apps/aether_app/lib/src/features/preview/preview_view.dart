import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../theme/catppuccin.dart';
import '../media_pool/media_pool_view.dart';
import 'frame_renderer.dart';
import 'preview_provider.dart';

/// Preview panel widget rendering hardware-accelerated video frames via Flutter's
/// [Texture] / [FrameRenderer] widget, along with playback controls, timeline
/// scrubber, and timecode.
class PreviewView extends ConsumerStatefulWidget {
  final Widget? customViewport;

  const PreviewView({
    super.key,
    this.customViewport,
  });

  @override
  ConsumerState<PreviewView> createState() => _PreviewViewState();
}

class _PreviewViewState extends ConsumerState<PreviewView> {
  bool _localIsPlaying = false;

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(previewProvider);
    final notifier = ref.read(previewProvider.notifier);

    return Container(
      key: const Key('preview_view'),
      color: CatppuccinMocha.base,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompactHeight = constraints.maxHeight < 200;
          final outerPad = isCompactHeight ? 6.0 : 16.0;
          final gap = isCompactHeight ? 2.0 : 10.0;

          return Padding(
            padding: EdgeInsets.all(outerPad),
            child: Column(
              children: [
                // 1. Video Canvas Viewport
                Expanded(
                  child: widget.customViewport ??
                      Container(
                        key: const Key('preview_gpu_viewport'),
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
                SizedBox(height: gap),
                // 2. Scrubber Bar with Timecode
                _buildScrubberBar(context, state, notifier, isCompactHeight),
                SizedBox(height: isCompactHeight ? 2.0 : 6.0),
                // 3. Playback Controls
                _buildControlBar(context, state, notifier, isCompactHeight),
              ],
            ),
          );
        },
      ),
    );
  }

  Widget _buildViewport(BuildContext context, PreviewState state) {
    // 1. Loading State
    if (state.isLoading) {
      return const Center(
        child: SingleChildScrollView(
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
        ),
      );
    }

    // 2. Error State
    if (state.errorMessage != null) {
      return Center(
        child: SingleChildScrollView(
          child: Padding(
            padding: const EdgeInsets.all(12.0),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(
                  Icons.error_outline_rounded,
                  color: CatppuccinMocha.red,
                  size: 36,
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
        ),
      );
    }

    // 3. Active Session / Video Frame Display
    if (state.session != null) {
      return Center(
        child: AspectRatio(
          aspectRatio: state.aspectRatio,
          child: FrameRenderer(
            frame: state.currentFrame,
            textureId: state.textureId,
          ),
        ),
      );
    }

    // 4. Empty Placeholder State
    return const Center(
      child: SingleChildScrollView(
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: 8.0, vertical: 4.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.movie_filter_outlined,
                color: CatppuccinMocha.surface2,
                size: 40,
              ),
              SizedBox(height: 6),
              Text(
                'Nenhuma mídia selecionada',
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: CatppuccinMocha.subtext0,
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
              SizedBox(height: 4),
              Text(
                'Selecione um arquivo no Media Pool para iniciar a pré-visualização',
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: CatppuccinMocha.overlay0,
                  fontSize: 11,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildScrubberBar(
    BuildContext context,
    PreviewState state,
    PreviewNotifier notifier,
    bool isCompact,
  ) {
    final isEnabled = state.isLoaded && state.durationSeconds > 0;
    final progressFraction = state.progressFraction;

    return SizedBox(
      height: isCompact ? 22.0 : 28.0,
      child: Row(
        children: [
          // Scrubber progress slider
          Expanded(
            child: SliderTheme(
              data: SliderTheme.of(context).copyWith(
                trackHeight: 4.0,
                thumbShape:
                    const RoundSliderThumbShape(enabledThumbRadius: 6.0),
                overlayShape:
                    const RoundSliderOverlayShape(overlayRadius: 10.0),
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
              fontSize: 11,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildControlBar(
    BuildContext context,
    PreviewState state,
    PreviewNotifier notifier,
    bool isCompact,
  ) {
    final isEnabled = state.isLoaded;
    final effectivePlaying = isEnabled ? state.isPlaying : _localIsPlaying;
    final btnSize = isCompact ? 28.0 : 36.0;
    final iconSz = isCompact ? 18.0 : 24.0;

    return SizedBox(
      height: btnSize,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          // Skip previous / rewind to start
          IconButton(
            key: const Key('preview_prev_button'),
            onPressed: isEnabled ? () => notifier.seekSeconds(0.0) : null,
            icon: const Icon(Icons.skip_previous_rounded, size: 20),
            color: CatppuccinMocha.text,
            disabledColor: CatppuccinMocha.surface2,
            tooltip: 'Início',
            padding: EdgeInsets.zero,
            constraints: BoxConstraints(minWidth: btnSize, minHeight: btnSize),
          ),
          const SizedBox(width: 8),
          // Play / Pause toggle button
          GestureDetector(
            key: const Key('preview_play_button'),
            behavior: HitTestBehavior.opaque,
            onTap: isEnabled
                ? null
                : () {
                    setState(() {
                      _localIsPlaying = !_localIsPlaying;
                    });
                  },
            child: Container(
              width: btnSize,
              height: btnSize,
              decoration: BoxDecoration(
                color: isEnabled
                    ? CatppuccinMocha.mauve
                    : CatppuccinMocha.surface0,
                shape: BoxShape.circle,
              ),
              child: IconButton(
                key: const Key('preview_play_pause_button'),
                onPressed: isEnabled
                    ? () {
                        notifier.togglePlayPause();
                      }
                    : null,
                icon: Icon(
                  effectivePlaying
                      ? Icons.pause_rounded
                      : Icons.play_arrow_rounded,
                ),
                color: isEnabled ? CatppuccinMocha.crust : CatppuccinMocha.text,
                disabledColor: CatppuccinMocha.surface2,
                iconSize: iconSz,
                padding: EdgeInsets.zero,
                constraints:
                    BoxConstraints(minWidth: btnSize, minHeight: btnSize),
                tooltip: effectivePlaying ? 'Pausar' : 'Reproduzir',
              ),
            ),
          ),
          const SizedBox(width: 8),
          // Skip next / jump to end
          IconButton(
            key: const Key('preview_next_button'),
            onPressed: isEnabled
                ? () => notifier.seekSeconds(state.durationSeconds)
                : null,
            icon: const Icon(Icons.skip_next_rounded, size: 20),
            color: CatppuccinMocha.text,
            disabledColor: CatppuccinMocha.surface2,
            tooltip: 'Fim',
            padding: EdgeInsets.zero,
            constraints: BoxConstraints(minWidth: btnSize, minHeight: btnSize),
          ),
        ],
      ),
    );
  }
}
