import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api/timeline_api.dart';
import '../../core/providers/project_provider.dart';

typedef CreateTimelineFn = Future<FullTimelineStateDto> Function({
  required String name,
  required int width,
  required int height,
  required int fpsNum,
  required int fpsDen,
});

typedef GetProjectTimelinesFn = Future<List<TimelineDto>> Function();
typedef LoadTimelineStateFn = Future<FullTimelineStateDto> Function({required String timelineId});
typedef CreateTrackFn = Future<TrackDto> Function({
  required String timelineId,
  required bool isVideo,
  required String name,
});

/// Injectable providers for FFI operations to enable headless unit testing.
final createTimelineFnProvider =
    Provider<CreateTimelineFn>((ref) => createNewTimeline);
final getProjectTimelinesFnProvider =
    Provider<GetProjectTimelinesFn>((ref) => getProjectTimelines);
final loadTimelineStateFnProvider =
    Provider<LoadTimelineStateFn>((ref) => loadTimelineState);
final createTrackFnProvider =
    Provider<CreateTrackFn>((ref) => createTrack);

/// AsyncNotifier managing the active project's timeline state.
class ActiveTimelineNotifier extends AsyncNotifier<FullTimelineStateDto?> {
  @override
  Future<FullTimelineStateDto?> build() async {
    // Watch the current project so timeline state re-evaluates when project opens/changes
    final currentProject = ref.watch(currentProjectProvider);
    if (currentProject == null) {
      return null;
    }

    try {
      final getTimelines = ref.read(getProjectTimelinesFnProvider);
      final timelines = await getTimelines();
      if (timelines.isNotEmpty) {
        final loadState = ref.read(loadTimelineStateFnProvider);
        return await loadState(timelineId: timelines.first.id);
      } else {
        // Automatically create a default timeline if the project does not have one yet
        return await createDefaultTimeline();
      }
    } catch (_) {
      // Best effort fallback
      return null;
    }
  }

  /// Creates a default timeline: "Timeline 1" (1920x1080, 60fps).
  Future<FullTimelineStateDto> createDefaultTimeline() async {
    state = const AsyncValue.loading();
    try {
      final createFn = ref.read(createTimelineFnProvider);
      final created = await createFn(
        name: 'Timeline 1',
        width: 1920,
        height: 1080,
        fpsNum: 60,
        fpsDen: 1,
      );
      state = AsyncValue.data(created);
      return created;
    } catch (e, st) {
      state = AsyncValue.error(e, st);
      rethrow;
    }
  }

  /// Creates a custom timeline with specified name, dimensions, and FPS.
  Future<FullTimelineStateDto> createCustomTimeline({
    required String name,
    required int width,
    required int height,
    required int fps,
  }) async {
    state = const AsyncValue.loading();
    try {
      final createFn = ref.read(createTimelineFnProvider);
      final created = await createFn(
        name: name,
        width: width,
        height: height,
        fpsNum: fps,
        fpsDen: 1,
      );
      state = AsyncValue.data(created);
      return created;
    } catch (e, st) {
      state = AsyncValue.error(e, st);
      rethrow;
    }
  }

  /// Loads an existing timeline and all its tracks/clips by timeline ID.
  Future<FullTimelineStateDto> loadTimeline(String timelineId) async {
    state = const AsyncValue.loading();
    try {
      final loadFn = ref.read(loadTimelineStateFnProvider);
      final loaded = await loadFn(timelineId: timelineId);
      state = AsyncValue.data(loaded);
      return loaded;
    } catch (e, st) {
      state = AsyncValue.error(e, st);
      rethrow;
    }
  }

  /// Adds a new track (video or audio) to the active timeline and refreshes state.
  Future<TrackDto> addTrack({
    required bool isVideo,
    String? name,
  }) async {
    final current = state.value;
    if (current == null) {
      throw StateError('Cannot add track: No active timeline loaded');
    }

    final timelineId = current.timeline.id;
    final existingCount = current.tracks.where((t) => t.isVideo == isVideo).length;
    final trackName = name ?? (isVideo ? 'Video ${existingCount + 1}' : 'Audio ${existingCount + 1}');

    final addFn = ref.read(createTrackFnProvider);
    final track = await addFn(
      timelineId: timelineId,
      isVideo: isVideo,
      name: trackName,
    );

    // Refresh active timeline state from SQLite
    await loadTimeline(timelineId);
    return track;
  }
}

/// Provider for the active Timeline state and operations.
final activeTimelineProvider =
    AsyncNotifierProvider<ActiveTimelineNotifier, FullTimelineStateDto?>(() {
  return ActiveTimelineNotifier();
});

