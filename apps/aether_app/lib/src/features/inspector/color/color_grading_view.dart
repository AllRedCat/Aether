import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../bridge/api.dart';
import '../../../theme/catppuccin.dart';
import '../inspector_mode_provider.dart';
import '../selected_clip_details_provider.dart';
import 'color_grading_provider.dart';
import 'widgets/basic_color_correction_view.dart';
import 'widgets/color_curves_view.dart';
import 'widgets/color_wheels_view.dart';

/// Context-sensitive Color Grading panel for the Aether editor.
/// Reacts to the current timeline selection:
/// - Video/Overlay clip: displays full color correction controls (Basics, Wheels, Curves).
/// - Audio clip: displays an informative banner and guidance button.
/// - No selection: displays a friendly sequence guidance prompt.
/// Also preserves Key('color_grading_placeholder') for backward compatibility with M1 layout tests.
class ColorGradingView extends ConsumerWidget {
  final SelectedClipDetails? clipDetails;

  const ColorGradingView({
    super.key,
    this.clipDetails,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final details = clipDetails ?? ref.watch(selectedClipDetailsProvider);

    if (details == null) {
      return const ColorEmptySequencePrompt();
    }

    if (details.trackKind == TrackKind.audio) {
      return ColorAudioClipBanner(clipDetails: details);
    }

    return ColorGradingControlsView(clipDetails: details);
  }
}

/// Prompt displayed when no clip is selected.
class ColorEmptySequencePrompt extends StatelessWidget {
  const ColorEmptySequencePrompt({super.key});

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      key: const Key('color_empty_sequence_prompt'),
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            key: const Key('color_grading_placeholder'),
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: CatppuccinMocha.surface0.withValues(alpha: 0.5),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: CatppuccinMocha.surface1, width: 1),
            ),
            child: const Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(
                  Icons.palette_outlined,
                  color: CatppuccinMocha.mauve,
                  size: 22,
                ),
                SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Correção de Cores',
                        key: Key('color_prompt_title'),
                        style: TextStyle(
                          color: CatppuccinMocha.text,
                          fontWeight: FontWeight.bold,
                          fontSize: 13,
                        ),
                      ),
                      SizedBox(height: 6),
                      Text(
                        'Nenhum clipe de vídeo selecionado. Selecione um clipe na Timeline para aplicar balanço de branco, exposição, saturação, rodas cromáticas e curvas de tom.',
                        key: Key('color_prompt_hint'),
                        style: TextStyle(
                          color: CatppuccinMocha.subtext0,
                          fontSize: 11,
                          height: 1.35,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Informative banner displayed when an audio clip is active.
class ColorAudioClipBanner extends ConsumerWidget {
  final SelectedClipDetails clipDetails;

  const ColorAudioClipBanner({
    super.key,
    required this.clipDetails,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final fileName = clipDetails.mediaItem?.fileName ?? 'Audio Clip';

    return SingleChildScrollView(
      key: const Key('color_audio_clip_banner'),
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.all(16),
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
                      size: 20,
                      color: CatppuccinMocha.green,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        fileName,
                        key: const Key('color_audio_clip_name'),
                        style: const TextStyle(
                          color: CatppuccinMocha.text,
                          fontWeight: FontWeight.bold,
                          fontSize: 13,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: CatppuccinMocha.surface0.withValues(alpha: 0.5),
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
                  ),
                  child: const Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(
                        Icons.info_outline_rounded,
                        color: CatppuccinMocha.green,
                        size: 16,
                      ),
                      SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          'Ajustes de correção de cores aplicam-se exclusivamente a faixas visuais (Vídeo e Sobreposição). Para configurar volume, pan estéreo e silenciamento deste áudio, alterne para a aba "Propriedades".',
                          key: Key('color_audio_explanation'),
                          style: TextStyle(
                            color: CatppuccinMocha.subtext0,
                            fontSize: 11,
                            height: 1.35,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 14),
                OutlinedButton.icon(
                  key: const Key('color_switch_to_properties_btn'),
                  onPressed: () {
                    ref.read(inspectorModeProvider.notifier).state =
                        InspectorMode.properties;
                  },
                  icon: const Icon(Icons.tune_rounded, size: 14),
                  label: const Text('Ir para Propriedades de Áudio'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: CatppuccinMocha.green,
                    side: const BorderSide(color: CatppuccinMocha.green),
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                    textStyle: const TextStyle(fontSize: 11, fontWeight: FontWeight.bold),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Full color grading controls view for a selected video/overlay clip.
class ColorGradingControlsView extends ConsumerWidget {
  final SelectedClipDetails clipDetails;

  const ColorGradingControlsView({
    super.key,
    required this.clipDetails,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final clipId = clipDetails.clip.id;
    final notifier = ref.read(colorGradingProvider.notifier);

    final fileName = clipDetails.mediaItem?.fileName ?? 'Video Clip';
    final isOverlay = clipDetails.trackKind == TrackKind.overlay;
    final clipDuration = clipDetails.clip.timelineOut - clipDetails.clip.timelineIn;

    return SingleChildScrollView(
      key: const Key('color_grading_controls'),
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
                      isOverlay ? Icons.layers_rounded : Icons.palette_rounded,
                      size: 18,
                      color: CatppuccinMocha.mauve,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        fileName,
                        key: const Key('color_clip_name'),
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
          const SizedBox(height: 18),

          // Section 1: Basic Color Correction
          BasicColorCorrectionView(clipId: clipId),
          const SizedBox(height: 18),

          // Section 2: 3-Way Color Wheels
          ColorWheelsView(clipId: clipId),
          const SizedBox(height: 18),

          // Section 3: Tone Curves
          ColorCurvesView(clipId: clipId),
          const SizedBox(height: 20),

          // Global Reset Button
          Center(
            child: TextButton.icon(
              key: const Key('reset_all_color'),
              onPressed: () => notifier.resetClipGrading(clipId),
              icon: const Icon(Icons.restart_alt_rounded, size: 15, color: CatppuccinMocha.red),
              label: const Text(
                'Redefinir Todas as Cores do Clipe',
                style: TextStyle(color: CatppuccinMocha.red, fontSize: 12),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
