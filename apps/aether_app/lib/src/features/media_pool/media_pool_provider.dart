import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../bridge/api.dart';
import '../../services/file_picker_service.dart';
import '../timeline/timeline_provider.dart';
import '../../core/providers/project_provider.dart';


/// Immutable state representation for the Media Pool catalog.
@immutable
class MediaPoolState {
  final List<MediaItem> items;
  final UuidValue? selectedItemId;
  final bool isLoading;
  final String? errorMessage;

  const MediaPoolState({
    this.items = const [],
    this.selectedItemId,
    this.isLoading = false,
    this.errorMessage,
  });

  const MediaPoolState.initial()
      : items = const [],
        selectedItemId = null,
        isLoading = false,
        errorMessage = null;

  MediaItem? get selectedItem {
    if (selectedItemId == null) return null;
    try {
      return items.firstWhere((i) => i.id == selectedItemId);
    } catch (_) {
      return null;
    }
  }

  MediaPoolState copyWith({
    List<MediaItem>? items,
    UuidValue? selectedItemId,
    bool clearSelection = false,
    bool? isLoading,
    String? errorMessage,
    bool clearError = false,
  }) {
    return MediaPoolState(
      items: items ?? this.items,
      selectedItemId:
          clearSelection ? null : (selectedItemId ?? this.selectedItemId),
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
    );
  }
}

typedef ImportMediaFn = Future<MediaItem> Function({
  String? projectPath,
  required String filePath,
});

typedef GetMediaItemsFn = Future<List<MediaItem>> Function({
  required String projectPath,
});

/// StateNotifier responsible for managing media pool assets, importing files,
/// and coordinating clip insertions onto the timeline.
class MediaPoolNotifier extends StateNotifier<MediaPoolState> {
  final FilePickerService _filePickerService;
  final ImportMediaFn? _importMediaFn;
  final GetMediaItemsFn? _getMediaItemsFn;
  final void Function(MediaItem item)? _onAddToTimeline;
  final String? projectPath;

  MediaPoolNotifier({
    FilePickerService? filePickerService,
    ImportMediaFn? importMediaFn,
    GetMediaItemsFn? getMediaItemsFn,
    void Function(MediaItem item)? onAddToTimeline,
    this.projectPath,
    bool autoLoad = false,
  })  : _filePickerService =
            filePickerService ?? const FileSelectorPickerService(),
        _importMediaFn = importMediaFn,
        _getMediaItemsFn = getMediaItemsFn,
        _onAddToTimeline = onAddToTimeline,
        super(const MediaPoolState.initial()) {
    if (autoLoad && projectPath != null && projectPath!.isNotEmpty) {
      loadMediaPool();
    }
  }

  Future<void> loadMediaPool() async {
    final path = projectPath;
    if (path == null || path.isEmpty) return;

    state = state.copyWith(isLoading: true, clearError: true);
    try {
      final getFn = _getMediaItemsFn ?? getMediaItems;
      final items = await getFn(projectPath: path);
      state = state.copyWith(items: items, isLoading: false);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Falha ao carregar mídias: $e',
      );
    }
  }

  Future<void> importMedia({List<String>? explicitPaths}) async {
    if (state.isLoading) return;

    state = state.copyWith(isLoading: true, clearError: true);

    List<String> paths;
    if (explicitPaths != null) {
      paths = explicitPaths;
    } else {
      try {
        paths = await _filePickerService.pickMediaFiles();
      } catch (e) {
        state = state.copyWith(
          isLoading: false,
          errorMessage: 'Erro ao abrir seletor de arquivos: $e',
        );
        return;
      }
    }

    if (paths.isEmpty) {
      state = state.copyWith(isLoading: false);
      return;
    }

    final List<MediaItem> updatedItems = List.from(state.items);
    final List<String> errors = [];

    for (final path in paths) {
      if (updatedItems.any((i) => i.filePath == path)) continue;

      try {
        final importFn = _importMediaFn ?? importMediaFile;
        debugPrint('Importing media with projectPath: $projectPath');
        final mediaItem = await importFn(
          projectPath: projectPath,
          filePath: path,
        );
        updatedItems.add(mediaItem);

        // Asynchronously generate audio peaks for waveform visualization
        // if the media has audio channels and we have a valid project.
        if (projectPath != null &&
            (mediaItem.metadata.audioChannels ?? 0) > 0) {
          generateAudioPeaks(
            projectPath: projectPath!,
            mediaId: mediaItem.id.uuid,
            samplesPerPeak: 1000,
          ).catchError((e) {
            debugPrint(
                'Failed to generate audio peaks for \${mediaItem.id.uuid}: \$e');
            return '';
          });
        }
      } catch (e) {
        errors.add('$path: $e');
      }
    }

    state = state.copyWith(
      items: updatedItems,
      isLoading: false,
      errorMessage: errors.isNotEmpty
          ? 'Falha ao importar ${errors.length} arquivo(s):\n${errors.join('\n')}'
          : null,
    );
  }

  void selectItem(UuidValue? id) {
    if (state.selectedItemId == id) {
      state = state.copyWith(clearSelection: true);
    } else {
      state = state.copyWith(selectedItemId: id);
    }
  }

  void removeItem(UuidValue id) {
    final updated = state.items.where((i) => i.id != id).toList();
    state = state.copyWith(
      items: updated,
      clearSelection: state.selectedItemId == id,
    );
  }

  void addMediaToTimeline(MediaItem item) {
    final callback = _onAddToTimeline;
    if (callback != null) {
      callback(item);
    }
  }
}

/// Riverpod StateNotifierProvider for [MediaPoolState].
final mediaPoolProvider =
    StateNotifierProvider<MediaPoolNotifier, MediaPoolState>((ref) {
  final filePicker = ref.watch(filePickerServiceProvider);
  final project = ref.watch(currentProjectProvider);

  return MediaPoolNotifier(
    filePickerService: filePicker,
    importMediaFn: importMediaFile,
    getMediaItemsFn: getMediaItems,
    projectPath: project?.filePath,
    onAddToTimeline: (mediaItem) async {
      final timelineNotifier = ref.read(timelineProvider.notifier);
      var currentTimeline = ref.read(timelineProvider).timeline;
      if (currentTimeline == null) {
        await timelineNotifier.initTimeline();
        currentTimeline = ref.read(timelineProvider).timeline;
      }
      if (currentTimeline == null) return;

      // Match media type to track kind:
      final targetKind = mediaItem.mediaType == MediaType.audio
          ? TrackKind.audio
          : TrackKind.video;

      // Find first track of matching kind, fallback to first track
      Track? targetTrack;
      try {
        targetTrack =
            currentTimeline.tracks.firstWhere((t) => t.kind == targetKind);
      } catch (_) {
        if (currentTimeline.tracks.isNotEmpty) {
          targetTrack = currentTimeline.tracks.first;
        }
      }

      if (targetTrack != null) {
        final duration = mediaItem.metadata.durationPts > 0
            ? mediaItem.metadata.durationPts
            : 300;
        final trackDuration = targetTrack.clips.fold<int>(
          0,
          (max, c) => c.timelineOut > max ? c.timelineOut : max,
        );
        await timelineNotifier.addClip(
          trackId: targetTrack.id,
          sourceId: mediaItem.id,
          sourceIn: 0,
          sourceOut: duration,
          timelineIn: trackDuration,
        );
      }
    },
  );
});
