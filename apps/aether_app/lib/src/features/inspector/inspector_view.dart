import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/api.dart';
import '../../theme/catppuccin.dart';
import 'color/color_grading_view.dart';
import 'inspector_mode_provider.dart';
import 'selected_clip_details_provider.dart';
import 'widgets/audio_properties_view.dart';
import 'widgets/sequence_properties_view.dart';
import 'widgets/video_properties_view.dart';

/// Context-sensitive Inspector panel for the Aether modular editor.
/// Dynamically switches between:
/// - [SequencePropertiesView]: when no clip is selected (in Properties mode).
/// - [VideoPropertiesView]: when a video or overlay clip is selected (in Properties mode).
/// - [AudioPropertiesView]: when an audio clip is selected (in Properties mode).
/// - [ColorGradingView]: when in Color grading mode ([InspectorMode.color]).
/// Supports [customContent] for M3 color grading panel replacement or testing.
class InspectorView extends ConsumerWidget {
  final Widget? headerActions;
  final Widget? customContent;

  const InspectorView({
    super.key,
    this.headerActions,
    this.customContent,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (customContent != null) {
      return Container(
        key: const Key('inspector_view'),
        color: CatppuccinMocha.base,
        child: SingleChildScrollView(
          key: const Key('inspector_scroll_view'),
          child: customContent!,
        ),
      );
    }

    final mode = ref.watch(inspectorModeProvider);
    final selectedDetails = ref.watch(selectedClipDetailsProvider);

    return Container(
      key: const Key('inspector_view'),
      color: CatppuccinMocha.base,
      child: mode == InspectorMode.color
          ? ColorGradingView(clipDetails: selectedDetails)
          : (selectedDetails == null
              ? const SequencePropertiesView()
              : (selectedDetails.trackKind == TrackKind.audio
                  ? AudioPropertiesView(clipDetails: selectedDetails)
                  : VideoPropertiesView(clipDetails: selectedDetails))),
    );
  }
}
