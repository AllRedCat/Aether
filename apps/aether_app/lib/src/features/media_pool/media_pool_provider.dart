import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../bridge/api.dart';
import '../../services/file_picker_service.dart';
import '../timeline/active_timeline_provider.dart';
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
  final Map<String, MediaAnalysisProgress> analysisProgress;

  const MediaPoolState({
    this.items = const [],
    this.selectedItemId,
    this.isLoading = false,
    this.errorMessage,
    this.indexingProgress = const {},
    this.analysisProgress = const {},
  });

  const MediaPoolState.initial()
      : items = const [],
        selectedItemId = null,
        isLoading = false,
        errorMessage = null,
        indexingProgress = const {},
        analysisProgress = const {};

  MediaItem? get selectedItem {
    if (selectedItemId == null) return null;
    try {
      return items.firstWhere((i) => i.id == selectedItemId);
    } catch (_) {
      return null;
    }
  }

  /// Returns `true` when one or more imported assets are actively being indexed,
  /// analyzed, or having their `.peak` / `.atlas` caches generated in the background.
  bool get isIndexing =>
      analysisProgress.values.any((p) => !(p.step == 'thumbnails' && p.progress >= 1.0)) ||
      indexingProgress.values.any((progress) => !progress.isDone);

  /// List of currently active (non-completed) background indexing tasks.
  List<ImportProgress> get activeIndexingTasks =>
      indexingProgress.values.where((progress) => !progress.isDone).toList();

  /// Aggregate `[0.0, 1.0]` progress across all active background indexing and analysis tasks,
  /// or `null` when no tasks are active.
  double? get overallIndexingProgress {
    final activeAnalysis = analysisProgress.values
        .where((p) => !(p.step == 'thumbnails' && p.progress >= 1.0))
        .toList();
    if (activeAnalysis.isNotEmpty) {
      final sum = activeAnalysis.fold<double>(0.0, (acc, p) {
        double stepBase = 0.0;
        if (p.step == 'audio_peaks') stepBase = 0.33;
        if (p.step == 'thumbnails') stepBase = 0.66;
        final total = (stepBase + (p.progress * 0.33)).clamp(0.0, 1.0);
        return acc + total;
      });
      return (sum / activeAnalysis.length).clamp(0.0, 1.0);
    }

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

  /// Looks up the latest analysis progress for a given media item UUID.
  MediaAnalysisProgress? analysisProgressForItem(UuidValue id) =>
      analysisProgress[id.uuid];

  /// Returns whether a media item is currently being analyzed or indexed.
  bool isItemIndexing(UuidValue id) {
    final ap = analysisProgress[id.uuid];
    if (ap != null && !(ap.step == 'thumbnails' && ap.progress >= 1.0)) {
      return true;
    }
    final ip = indexingProgress[id.uuid];
    if (ip != null && !ip.isDone) {
      return true;
    }
    return false;
  }

  MediaPoolState copyWith({
    List<MediaItem>? items,
    UuidValue? selectedItemId,
    bool clearSelection = false,
    bool? isLoading,
    String? errorMessage,
    bool clearError = false,
    Map<String, ImportProgress>? indexingProgress,
    Map<String, MediaAnalysisProgress>? analysisProgress,
  }) {
    return MediaPoolState(
      items: items ?? this.items,
      selectedItemId:
          clearSelection ? null : (selectedItemId ?? this.selectedItemId),
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
      indexingProgress: indexingProgress ?? this.indexingProgress,
      analysisProgress: analysisProgress ?? this.analysisProgress,
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

typedef StartMediaAnalysisStreamFn = Stream<MediaAnalysisProgress> Function({
  required String assetId,
  required String path,
});

/// StateNotifier responsible for managing media pool assets, importing files,
/// tracking non-blocking background indexing progress, and coordinating clip
/// insertions onto the timeline.
class MediaPoolNotifier extends StateNotifier<MediaPoolState> {
  final FilePickerService _filePickerService;
  final ImportMediaFn? _importMediaFn;
  final GetMediaItemsFn? _getMediaItemsFn;
  final SubscribeImportProgressFn? _subscribeProgressFn;
  final StartMediaAnalysisStreamFn? _startAnalysisStreamFn;
  final void Function(MediaItem item)? _onAddToTimeline;
  final String? projectPath;
  StreamSubscription<ImportProgress>? _progressSubscription;
  final Map<String, StreamSubscription<MediaAnalysisProgress>>
      _analysisSubscriptions = {};
  final Map<String, DateTime> _lastProgressUpdate = {};

  MediaPoolNotifier({
    FilePickerService? filePickerService,
    ImportMediaFn? importMediaFn,
    GetMediaItemsFn? getMediaItemsFn,
    SubscribeImportProgressFn? subscribeProgressFn,
    StartMediaAnalysisStreamFn? startAnalysisStreamFn,
    void Function(MediaItem item)? onAddToTimeline,
    this.projectPath,
    bool autoLoad = false,
    List<MediaItem>? initialItems,
  })  : _filePickerService =
            filePickerService ?? const FileSelectorPickerService(),
        _importMediaFn = importMediaFn,
        _getMediaItemsFn = getMediaItemsFn,
        _subscribeProgressFn = subscribeProgressFn,
        _startAnalysisStreamFn = startAnalysisStreamFn,
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

  /// Starts non-blocking analysis and caching (.index, .peak, .atlas) for an asset.
  void startMediaAnalysis({required String assetId, required String filePath}) {
    final analysisFn = _startAnalysisStreamFn;
    if (analysisFn == null) return;

    _analysisSubscriptions[assetId]?.cancel();
    try {
      final stream = analysisFn(assetId: assetId, path: filePath);
      _analysisSubscriptions[assetId] = stream.listen(
        (progress) => updateAnalysisProgress(progress),
        onError: (Object err) {
          debugPrint(
              '[MediaPool] Error in media analysis stream for $assetId: $err');
        },
        onDone: () {
          _analysisSubscriptions.remove(assetId);
        },
      );
    } catch (_) {
      // Safe fallback when running without FFI dylib
    }
  }

  /// Updates analysis progress with throttling to prevent excessive UI rebuilds.
  void updateAnalysisProgress(MediaAnalysisProgress progress) {
    if (!mounted) return;

    final now = DateTime.now();
    final key = 'ana_${progress.assetId}';
    final lastTime = _lastProgressUpdate[key];
    final previous = state.analysisProgress[progress.assetId];
    final stepChanged = previous == null || previous.step != progress.step;
    final isDone = progress.step == 'thumbnails' && progress.progress >= 1.0;
    final delta = previous != null
        ? (progress.progress - previous.progress).abs()
        : 1.0;

    // Only skip updates if it's within the same step, not done, change < 2%, and within 50ms window
    if (!stepChanged &&
        !isDone &&
        delta < 0.02 &&
        lastTime != null &&
        now.difference(lastTime).inMilliseconds < 50) {
      return;
    }
    _lastProgressUpdate[key] = now;

    final updated =
        Map<String, MediaAnalysisProgress>.from(state.analysisProgress);
    updated[progress.assetId] = progress;
    state = state.copyWith(analysisProgress: updated);
  }

  /// Updates the non-blocking indexing progress state for an asset with throttling.
  void updateIndexingProgress(ImportProgress progress) {
    if (!mounted) return;

    final now = DateTime.now();
    final key = 'idx_${progress.assetId}';
    final lastTime = _lastProgressUpdate[key];
    final previous = state.indexingProgress[progress.assetId];
    final stageChanged = previous == null || previous.stage != progress.stage;
    final isDone = progress.isDone;
    final delta = previous != null
        ? (progress.progress - previous.progress).abs()
        : 1.0;

    if (!stageChanged &&
        !isDone &&
        delta < 0.02 &&
        lastTime != null &&
        now.difference(lastTime).inMilliseconds < 50) {
      return;
    }
    _lastProgressUpdate[key] = now;

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
        startMediaAnalysis(
          assetId: mediaItem.id.uuid,
          filePath: mediaItem.filePath,
        );
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
    _analysisSubscriptions[id.uuid]?.cancel();
    _analysisSubscriptions.remove(id.uuid);
    _lastProgressUpdate.remove('ana_${id.uuid}');
    _lastProgressUpdate.remove('idx_${id.uuid}');

    final updated = state.items.where((i) => i.id != id).toList();
    final updatedProgress =
        Map<String, ImportProgress>.from(state.indexingProgress)
          ..remove(id.uuid);
    final updatedAnalysis =
        Map<String, MediaAnalysisProgress>.from(state.analysisProgress)
          ..remove(id.uuid);
    state = state.copyWith(
      items: updated,
      clearSelection: state.selectedItemId == id,
      indexingProgress: updatedProgress,
      analysisProgress: updatedAnalysis,
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
    for (final sub in _analysisSubscriptions.values) {
      sub.cancel();
    }
    _analysisSubscriptions.clear();
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
    startAnalysisStreamFn: startMediaAnalysisStream,
    // Rust's Project::load expects the .aether FILE path, not the folder.
    projectPath: project?.filePath,
    initialItems: project?.mediaPool.items,
    onAddToTimeline: (mediaItem) async {
      final timelineNotifier = ref.read(timelineProvider.notifier);

      // Match media type to track kind:
      final targetKind = mediaItem.mediaType == MediaType.audio
          ? TrackKind.audio
          : TrackKind.video;

      // Check if activeTimelineProvider has a track of matching type to keep IDs aligned
      UuidValue? preferredTrackId;
      final activeTimeline = ref.read(activeTimelineProvider).valueOrNull;
      if (activeTimeline != null) {
        final wantVideo = targetKind != TrackKind.audio;
        try {
          final dtoTrack = activeTimeline.tracks.firstWhere(
            (t) => t.isVideo == wantVideo,
          );
          preferredTrackId = UuidValue.fromString(dtoTrack.id);
        } catch (_) {}
      }

      final targetTrack = await timelineNotifier.ensureTrackForKind(
        targetKind,
        preferredTrackId: preferredTrackId,
      );

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
