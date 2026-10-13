import 'dart:async';

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
  final Map<String, ImportProgress> indexingProgress;

  const MediaPoolState({
    this.items = const [],
    this.selectedItemId,
    this.isLoading = false,
    this.errorMessage,
    this.indexingProgress = const {},
  });

  const MediaPoolState.initial()
      : items = const [],
        selectedItemId = null,
        isLoading = false,
        errorMessage = null,
        indexingProgress = const {};

  MediaItem? get selectedItem {
    if (selectedItemId == null) return null;
    try {
      return items.firstWhere((i) => i.id == selectedItemId);
    } catch (_) {
      return null;
    }
  }

  /// Returns `true` when one or more imported assets are actively being indexed
  /// or having their `.peak` / `.atlas` caches generated in the background.
  bool get isIndexing =>
      indexingProgress.values.any((progress) => !progress.isDone);

  /// List of currently active (non-completed) background indexing tasks.
  List<ImportProgress> get activeIndexingTasks =>
      indexingProgress.values.where((progress) => !progress.isDone).toList();

  /// Aggregate `[0.0, 1.0]` progress across all active background indexing tasks,
  /// or `null` when no indexing tasks are active.
  double? get overallIndexingProgress {
    final active = activeIndexingTasks;
    if (active.isEmpty) return null;
    final sum = active.fold<double>(
      0.0,
      (acc, item) => acc + item.progress.clamp(0.0, 1.0),
    );
    return (sum / active.length).clamp(0.0, 1.0);
  }

  /// Looks up the latest indexing progress for a given media item UUID.
  ImportProgress? progressForItem(UuidValue id) => indexingProgress[id.uuid];

  MediaPoolState copyWith({
    List<MediaItem>? items,
    UuidValue? selectedItemId,
    bool clearSelection = false,
    bool? isLoading,
    String? errorMessage,
    bool clearError = false,
    Map<String, ImportProgress>? indexingProgress,
  }) {
    return MediaPoolState(
      items: items ?? this.items,
      selectedItemId:
          clearSelection ? null : (selectedItemId ?? this.selectedItemId),
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
      indexingProgress: indexingProgress ?? this.indexingProgress,
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

typedef SubscribeImportProgressFn = Stream<ImportProgress> Function();

/// StateNotifier responsible for managing media pool assets, importing files,
/// tracking non-blocking background indexing progress, and coordinating clip
/// insertions onto the timeline.
class MediaPoolNotifier extends StateNotifier<MediaPoolState> {
  final FilePickerService _filePickerService;
  final ImportMediaFn? _importMediaFn;
  final GetMediaItemsFn? _getMediaItemsFn;
  final SubscribeImportProgressFn? _subscribeProgressFn;
  final void Function(MediaItem item)? _onAddToTimeline;
  final String? projectPath;
  StreamSubscription<ImportProgress>? _progressSubscription;

  MediaPoolNotifier({
    FilePickerService? filePickerService,
    ImportMediaFn? importMediaFn,
    GetMediaItemsFn? getMediaItemsFn,
    SubscribeImportProgressFn? subscribeProgressFn,
    void Function(MediaItem item)? onAddToTimeline,
    this.projectPath,
    bool autoLoad = false,
    List<MediaItem>? initialItems,
  })  : _filePickerService =
            filePickerService ?? const FileSelectorPickerService(),
        _importMediaFn = importMediaFn,
        _getMediaItemsFn = getMediaItemsFn,
        _subscribeProgressFn = subscribeProgressFn,
        _onAddToTimeline = onAddToTimeline,
        super(initialItems != null
            ? MediaPoolState(items: initialItems)
            : const MediaPoolState.initial()) {
    _initProgressStream();
    if (autoLoad &&
        initialItems == null &&
        projectPath != null &&
        projectPath!.isNotEmpty) {
      loadMediaPool();
    }
  }

  void _initProgressStream() {
    final subFn = _subscribeProgressFn;
    if (subFn == null) return;
    try {
      _progressSubscription = subFn().listen(
        updateIndexingProgress,
        onError: (Object err) {
          debugPrint('[MediaPool] Import progress stream error: $err');
        },
      );
    } catch (_) {
      // Safe fallback when running in headless widget tests without Rust dylib initialized.
    }
  }

  /// Updates the non-blocking indexing progress state for an asset.
  void updateIndexingProgress(ImportProgress progress) {
    if (!mounted) return;
    final updated = Map<String, ImportProgress>.from(state.indexingProgress);
    updated[progress.assetId] = progress;
    state = state.copyWith(indexingProgress: updated);
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
        if (projectPath == null) {
          debugPrint(
              '[MediaPool] Nenhum projeto ativo: "$path" não será salvo no .aether');
        }
        final mediaItem = await importFn(
          projectPath: projectPath,
          filePath: path,
        );
        updatedItems.add(mediaItem);
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
    final updatedProgress =
        Map<String, ImportProgress>.from(state.indexingProgress)
          ..remove(id.uuid);
    state = state.copyWith(
      items: updated,
      clearSelection: state.selectedItemId == id,
      indexingProgress: updatedProgress,
    );
  }

  void addMediaToTimeline(MediaItem item) {
    final callback = _onAddToTimeline;
    if (callback != null) {
      callback(item);
    }
  }

  @override
  void dispose() {
    _progressSubscription?.cancel();
    super.dispose();
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
    subscribeProgressFn: subscribeImportProgress,
    // Rust's Project::load expects the .aether FILE path, not the folder.
    projectPath: project?.filePath,
    initialItems: project?.mediaPool.items,
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
