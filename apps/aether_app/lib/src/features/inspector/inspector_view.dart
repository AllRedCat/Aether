import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/api.dart';
import '../../theme/catppuccin.dart';
import '../media_pool/media_pool_provider.dart';
import 'color/color_grading_view.dart';
import 'inspector_mode_provider.dart';
import 'selected_clip_details_provider.dart';
import 'widgets/audio_properties_view.dart';
import 'widgets/media_item_properties_view.dart';
import 'widgets/sequence_properties_view.dart';
import 'widgets/video_properties_view.dart';

/// Context-sensitive Inspector panel for the Aether modular editor.
/// Dynamically switches between:
/// - [VideoPropertiesView]: when a video or overlay clip is selected on the timeline.
/// - [AudioPropertiesView]: when an audio clip is selected on the timeline.
/// - [MediaItemPropertiesView]: when a media item is selected in the Media Pool (but no clip on timeline).
/// - [SequencePropertiesView]: when nothing is selected.
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

    // Also watch the media pool selection for when no timeline clip is selected
    final mediaPoolState = ref.watch(mediaPoolProvider);
    final selectedMediaItem = mediaPoolState.selectedItem;

    return Container(
      key: const Key('inspector_view'),
      color: CatppuccinMocha.base,
      child: mode == InspectorMode.color
          ? ColorGradingView(clipDetails: selectedDetails)
          : _buildPropertiesContent(selectedDetails, selectedMediaItem),
    );
  }

  /// Determines which properties view to show based on current selection state.
  /// Priority: Timeline clip selection > Media Pool item selection > Sequence/empty.
  Widget _buildPropertiesContent(
    SelectedClipDetails? clipDetails,
    MediaItem? selectedMediaItem,
  ) {
    // 1. If a clip is selected in the timeline, show its specific properties
    if (clipDetails != null) {
      if (clipDetails.trackKind == TrackKind.audio) {
        return AudioPropertiesView(clipDetails: clipDetails);
      }
      return VideoPropertiesView(clipDetails: clipDetails);
    }

    // 2. If a media item is selected in the Media Pool, show its metadata
    if (selectedMediaItem != null) {
      return MediaItemPropertiesView(mediaItem: selectedMediaItem);
    }

    // 3. Nothing selected — show sequence/project overview
    return const SequencePropertiesView();
  }
}
