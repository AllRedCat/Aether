# Investigation Report: Flutter Architecture & Riverpod State Management in `apps/aether_app`

**Author**: `explorer_m2_1`  
**Date**: 2026-09-28  
**Scope**: Milestone M2 — Flutter Application & State (R2)  
**Target Codebase**: `apps/aether_app`

---

## Executive Summary

This investigation analyzed the Flutter client application located at `apps/aether_app`, focusing on existing dependencies, application bootstrapping, the state management model (Flutter Riverpod), and the integration with the Rust native core via the FFI bridge (`aether_bridge`).

### Key Findings
1. **Dependencies (`pubspec.yaml`)**:
   - `flutter_riverpod: ^2.5.1`, `riverpod: ^2.5.1`, and `uuid: ^4.4.0` are already declared and resolved cleanly (`flutter pub get` succeeded with Flutter 3.47.5 / Dart 3.13.4). No missing dependencies exist in `pubspec.yaml`.
   - `apps/aether_app/analysis_options.yaml` is currently **missing**, which causes `tests/test_dart_ui_contract.py` (`test_analysis_options_config`) to fail. Creating this file configured with `package:flutter_lints/flutter.yaml` is required.
2. **Bootstrapping (`main.dart`)**:
   - `main.dart` already wraps the root application with `ProviderScope(child: AetherApp())`.
   - However, `main()` is currently synchronous (`void main()`), lacks `WidgetsFlutterBinding.ensureInitialized()`, and leaves `// TODO: await RustLib.init();` unimplemented.
   - For robust cross-platform execution and headless widget testing, `RustLib.init()` and `initEngine()` should be called asynchronously with defensive handling.
3. **State Management Contract**:
   - Strict adherence to `PROJECT.md § Interface Contracts` requires `TimelineState` with 6 explicit properties (`timeline`, `totalClipCount`, `durationPts`, `tracks`, `isLoading`, `errorMessage`) and `TimelineNotifier` providing `init()` and `addClip()`.
   - `tests/test_dart_ui_contract.py` specifies exact AST regexes:
     - `class TimelineState`
     - `totalClipCount`, `durationPts`
     - `class TimelineNotifier extends StateNotifier<TimelineState>`
     - `final timelineProvider =`
     - `Future<void> addClip`
4. **UI Integration Contract (`timeline_view.dart`)**:
   - Must consume `ref.watch(timelineProvider)`.
   - Must expose `Key('timeline_total_clips_count')` presenting the total clip count integer.
   - Must expose `Key('add_clip_button')` triggering `ref.read(timelineProvider.notifier).addClip()`.

---

## 1. Dependency Analysis (`pubspec.yaml`)

### 1.1 Existing Contents
The file `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app/pubspec.yaml` contains:
```yaml
name: aether_app
description: "A cross-platform NLE video editor using Flutter and Rust."
publish_to: 'none'
version: 1.0.0+1

environment:
  sdk: '>=3.3.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter
  flutter_rust_bridge: 2.3.0
  flutter_riverpod: ^2.5.1
  riverpod: ^2.5.1
  uuid: ^4.4.0

dev_dependencies:
  flutter_test:
    sdk: flutter
  flutter_lints: ^3.0.0

flutter:
  uses-material-design: true
```

### 1.2 Verification Results
- Command `flutter pub get` executed with exit code 0:
  - `flutter_riverpod` resolved to `2.6.1`.
  - `riverpod` resolved to `2.6.1`.
  - `flutter_rust_bridge` resolved to `2.3.0`.
  - `uuid` resolved to `4.5.3`.
  - `flutter_lints` resolved to `3.0.2`.
- `tests/test_dart_ui_contract.py` executed:
  - `test_pubspec_dependencies`: **PASS** (`pubspec.yaml contains flutter_riverpod, riverpod, and flutter_rust_bridge.`).

### 1.3 Missing Configuration: `analysis_options.yaml`
While `flutter_lints` is present in `dev_dependencies`, Flutter projects require `analysis_options.yaml` at `apps/aether_app/analysis_options.yaml`.
`tests/test_dart_ui_contract.py` asserts:
```python
def test_analysis_options_config():
    if not ANALYSIS_OPTIONS.exists():
        return {"name": "Static Analysis Configuration", "status": "FAIL", "message": f"analysis_options.yaml missing at {ANALYSIS_OPTIONS}"}
    content = ANALYSIS_OPTIONS.read_text(encoding="utf-8")
    has_lints = "flutter_lints" in content or "include:" in content
    ...
```
**Recommended Solution**: Create `apps/aether_app/analysis_options.yaml` with:
```yaml
include: package:flutter_lints/flutter.yaml

analyzer:
  language:
    strict-casts: true
    strict-inference: true
    strict-raw-types: true

linter:
  rules:
    prefer_const_constructors: true
    prefer_const_literals_to_create_immutables: true
    unnecessary_import: true
```

---

## 2. Bootstrapping & Entry Point (`main.dart`)

### 2.1 Existing Contents
The file `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app/lib/main.dart` contains:
```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'src/features/timeline/timeline_view.dart';

// Import the generated bridge when available:
// import 'src/bridge/api.dart';

void main() {
  // TODO: await RustLib.init();
  runApp(const ProviderScope(child: AetherApp()));
}

class AetherApp extends StatelessWidget {
  const AetherApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Aether Video Editor',
      theme: ThemeData.dark(useMaterial3: true),
      home: const Scaffold(
        body: SafeArea(
          child: Column(
            children: [
              Expanded(
                flex: 2,
                child: Center(child: Text("Preview Area")),
              ),
              Divider(height: 1),
              Expanded(
                flex: 1,
                child: TimelineView(),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
```

### 2.2 Analysis
1. **Riverpod Root Scope**: `ProviderScope` is properly enclosing `AetherApp`. This guarantees all Riverpod providers, including `timelineProvider`, are accessible anywhere in the widget hierarchy.
2. **Native Bridge Initialization**:
   - `RustLib.init()` is generated in `apps/aether_app/lib/src/bridge/frb_generated.dart`.
   - `initEngine()` is declared in `apps/aether_app/lib/src/bridge/api.dart` (which initializes `flutter_rust_bridge::setup_default_user_utils()`, `aether_render::init_render()`, and `aether_media::init_media()`).
   - If `RustLib.init()` is called in a pure Dart / headless testing environment without native dynamic libraries loaded, FRB throws a library load exception unless handled or initialized in mock mode (`RustLib.initMock(...)`).
3. **Recommended `main.dart` Implementation**:
   ```dart
   import 'package:flutter/material.dart';
   import 'package:flutter_riverpod/flutter_riverpod.dart';
   import 'src/bridge/api.dart';
   import 'src/bridge/frb_generated.dart';
   import 'src/features/timeline/timeline_view.dart';

   Future<void> main() async {
     WidgetsFlutterBinding.ensureInitialized();
     try {
       await RustLib.init();
       await initEngine();
     } catch (e) {
       debugPrint('RustLib init skipped or running in test/mock environment: $e');
     }
     runApp(const ProviderScope(child: AetherApp()));
   }
   ```
   This ensures binding initialization, non-blocking fallback if native binaries aren't linked in headless environments, and preserves the dark Material 3 layout.

---

## 3. Design of `TimelineState` and `TimelineNotifier`

### 3.1 FFI Bridge Interface Contract
From `PROJECT.md § Interface Contracts` and `apps/aether_app/lib/src/bridge/api.dart`:
```dart
Future<void> initEngine();
Future<Timeline> createTimeline();
Future<Timeline> addTrack({required Timeline timeline, required TrackKind kind});
Future<Timeline> addClipToTrack({
  required Timeline timeline,
  required UuidValue trackId,
  required UuidValue sourceId,
  required int sourceIn,
  required int sourceOut,
  required int timelineIn,
});

class Timeline {
  final UuidValue id;
  final Rational timebase;
  final int durationPts;
  final List<Track> tracks;
}

class Track {
  final UuidValue id;
  final TrackKind kind;
  final List<Clip> clips;
}

class Clip {
  final UuidValue id;
  final UuidValue sourceId;
  final int sourceIn;
  final int sourceOut;
  final int timelineIn;
  final int timelineOut;
}
```

### 3.2 `TimelineState` Design
The state object must be immutable and provide full introspection into the timeline model:
```dart
import 'package:flutter/foundation.dart';
import '../../bridge/api.dart';

@immutable
class TimelineState {
  final Timeline? timeline;
  final int totalClipCount;
  final int durationPts;
  final List<Track> tracks;
  final bool isLoading;
  final String? errorMessage;

  const TimelineState({
    this.timeline,
    this.totalClipCount = 0,
    this.durationPts = 0,
    this.tracks = const [],
    this.isLoading = false,
    this.errorMessage,
  });

  const TimelineState.initial()
      : timeline = null,
        totalClipCount = 0,
        durationPts = 0,
        tracks = const [],
        isLoading = false,
        errorMessage = null;

  TimelineState copyWith({
    Timeline? timeline,
    int? totalClipCount,
    int? durationPts,
    List<Track>? tracks,
    bool? isLoading,
    String? errorMessage,
    bool clearError = false,
  }) {
    return TimelineState(
      timeline: timeline ?? this.timeline,
      totalClipCount: totalClipCount ?? this.totalClipCount,
      durationPts: durationPts ?? this.durationPts,
      tracks: tracks ?? this.tracks,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TimelineState &&
          runtimeType == other.runtimeType &&
          timeline == other.timeline &&
          totalClipCount == other.totalClipCount &&
          durationPts == other.durationPts &&
          listEquals(tracks, other.tracks) &&
          isLoading == other.isLoading &&
          errorMessage == other.errorMessage;

  @override
  int get hashCode =>
      timeline.hashCode ^
      totalClipCount.hashCode ^
      durationPts.hashCode ^
      tracks.hashCode ^
      isLoading.hashCode ^
      errorMessage.hashCode;
}
```

### 3.3 `TimelineNotifier` Design
The notifier extends `StateNotifier<TimelineState>`:
- **Dependency Injection**: Accepts optional function parameters for `createTimeline` and `addClipToTrack` to support headless testing without loading native dylibs.
- **`init()`**: Requests a new `Timeline` via bridge API (`createTimeline()`), extracts tracks, computes total clip count, extracts `durationPts`, and updates state.
- **`addClip()`**:
  - Automatically initializes timeline if `state.timeline == null`.
  - Resolves target track (defaults to first video track or `tracks.first`).
  - Creates a clip with valid bounds (`sourceIn: 0`, `sourceOut: 60`, `timelineIn: durationPts` so clips abut sequentially).
  - Calls `addClipToTrack(...)` via bridge.
  - Updates state with the returned new immutable `Timeline` from Rust.
  - Recalculates `totalClipCount` and `durationPts`.
  - Handles errors gracefully, recording `errorMessage` without crashing.

```dart
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';
import '../../bridge/api.dart';

class TimelineNotifier extends StateNotifier<TimelineState> {
  final Future<Timeline> Function()? _createTimelineFn;
  final Future<Timeline> Function({
    required Timeline timeline,
    required UuidValue trackId,
    required UuidValue sourceId,
    required int sourceIn,
    required int sourceOut,
    required int timelineIn,
  })? _addClipToTrackFn;

  TimelineNotifier({
    Future<Timeline> Function()? createTimelineFn,
    Future<Timeline> Function({
      required Timeline timeline,
      required UuidValue trackId,
      required UuidValue sourceId,
      required int sourceIn,
      required int sourceOut,
      required int timelineIn,
    })? addClipToTrackFn,
    bool autoInit = true,
  })  : _createTimelineFn = createTimelineFn,
        _addClipToTrackFn = addClipToTrackFn,
        super(const TimelineState.initial()) {
    if (autoInit) {
      init();
    }
  }

  Future<void> init() async {
    state = state.copyWith(isLoading: true, clearError: true);
    try {
      final createFn = _createTimelineFn ?? createTimeline;
      final timeline = await createFn();
      _syncStateWithTimeline(timeline);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Failed to initialize timeline: $e',
      );
    }
  }

  Future<void> addClip({
    UuidValue? trackId,
    UuidValue? sourceId,
    int? sourceIn,
    int? sourceOut,
    int? timelineIn,
  }) async {
    var currentTimeline = state.timeline;
    if (currentTimeline == null) {
      await init();
      currentTimeline = state.timeline;
      if (currentTimeline == null) {
        return;
      }
    }

    state = state.copyWith(isLoading: true, clearError: true);

    try {
      final targetTrackId = trackId ??
          (currentTimeline.tracks.isNotEmpty
              ? currentTimeline.tracks.first.id
              : null);

      if (targetTrackId == null) {
        state = state.copyWith(
          isLoading: false,
          errorMessage: 'No track available to add clip',
        );
        return;
      }

      final clipSourceId = sourceId ?? UuidValue.fromString(const Uuid().v4());
      final inSrc = sourceIn ?? 0;
      final outSrc = sourceOut ?? 60;
      final inTl = timelineIn ?? state.durationPts;

      final addFn = _addClipToTrackFn ?? addClipToTrack;
      final updatedTimeline = await addFn(
        timeline: currentTimeline,
        trackId: targetTrackId,
        sourceId: clipSourceId,
        sourceIn: inSrc,
        sourceOut: outSrc,
        timelineIn: inTl,
      );

      _syncStateWithTimeline(updatedTimeline);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Failed to add clip: $e',
      );
    }
  }

  void _syncStateWithTimeline(Timeline timeline) {
    final count = timeline.tracks.fold<int>(
      0,
      (acc, track) => acc + track.clips.length,
    );
    state = state.copyWith(
      timeline: timeline,
      totalClipCount: count,
      durationPts: timeline.durationPts,
      tracks: timeline.tracks,
      isLoading: false,
      clearError: true,
    );
  }
}
```

### 3.4 Provider Declaration
In `apps/aether_app/lib/src/features/timeline/timeline_provider.dart`:
```dart
final timelineProvider =
    StateNotifierProvider<TimelineNotifier, TimelineState>((ref) {
  return TimelineNotifier();
});
```

---

## 4. UI Specification (`timeline_view.dart`)

### 4.1 Verification Criteria
From `tests/test_dart_ui_contract.py`:
1. `ref.watch(timelineProvider)`
2. `Key('timeline_total_clips_count')` widget displaying `totalClipCount`.
3. `Key('add_clip_button')` widget invoking `ref.read(timelineProvider.notifier).addClip()`.

### 4.2 UI Design
```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'timeline_provider.dart';

class TimelineView extends ConsumerWidget {
  const TimelineView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final timelineState = ref.watch(timelineProvider);

    return Container(
      color: const Color(0xFF1E1E1E),
      child: Column(
        children: [
          // Header Toolbar
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
            color: const Color(0xFF2A2A2A),
            child: Row(
              children: [
                const Icon(Icons.movie_creation_outlined, color: Colors.blueAccent, size: 20),
                const SizedBox(width: 8),
                const Text(
                  'Timeline',
                  style: TextStyle(fontWeight: FontWeight.bold, fontSize: 15),
                ),
                const SizedBox(width: 20),
                // Clips count badge
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: Colors.black45,
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Text(
                        'Total Clips: ',
                        style: TextStyle(color: Colors.white70, fontSize: 13),
                      ),
                      Text(
                        '${timelineState.totalClipCount}',
                        key: const Key('timeline_total_clips_count'),
                        style: const TextStyle(
                          color: Colors.white,
                          fontWeight: FontWeight.bold,
                          fontSize: 13,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 16),
                // Duration PTS display
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: Colors.black45,
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Text(
                    'Duration: ${timelineState.durationPts} PTS',
                    style: const TextStyle(color: Colors.white70, fontSize: 13),
                  ),
                ),
                const Spacer(),
                if (timelineState.isLoading)
                  const Padding(
                    padding: EdgeInsets.only(right: 12.0),
                    child: SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ),
                ElevatedButton.icon(
                  key: const Key('add_clip_button'),
                  onPressed: timelineState.isLoading
                      ? null
                      : () => ref.read(timelineProvider.notifier).addClip(),
                  icon: const Icon(Icons.add, size: 16),
                  label: const Text('Add Clip'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.blueAccent,
                    foregroundColor: Colors.white,
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  ),
                ),
              ],
            ),
          ),
          if (timelineState.errorMessage != null)
            Container(
              width: double.infinity,
              color: Colors.redAccent.shade700,
              padding: const EdgeInsets.all(8),
              child: Text(
                timelineState.errorMessage!,
                style: const TextStyle(color: Colors.white, fontSize: 12),
              ),
            ),
          // Tracks & Clips Canvas
          Expanded(
            child: timelineState.tracks.isEmpty
                ? const Center(
                    child: Text(
                      'No tracks available',
                      style: TextStyle(color: Colors.grey),
                    ),
                  )
                : ListView.builder(
                    itemCount: timelineState.tracks.length,
                    itemBuilder: (context, trackIndex) {
                      final track = timelineState.tracks[trackIndex];
                      return Container(
                        margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                        padding: const EdgeInsets.all(8),
                        decoration: BoxDecoration(
                          color: const Color(0xFF252525),
                          borderRadius: BorderRadius.circular(6),
                          border: Border.all(color: Colors.white12),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              'Track ${trackIndex + 1} (${track.kind.name.toUpperCase()}) — ${track.clips.length} clip(s)',
                              style: const TextStyle(
                                color: Colors.white70,
                                fontSize: 12,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 6),
                            if (track.clips.isEmpty)
                              const Padding(
                                padding: EdgeInsets.symmetric(vertical: 8),
                                child: Text(
                                  'Empty track (Click "Add Clip" to add)',
                                  style: TextStyle(color: Colors.white30, fontSize: 12),
                                ),
                              )
                            else
                              SizedBox(
                                height: 50,
                                child: ListView.separated(
                                  scrollDirection: Axis.horizontal,
                                  itemCount: track.clips.length,
                                  separatorBuilder: (_, __) => const SizedBox(width: 6),
                                  itemBuilder: (context, clipIndex) {
                                    final clip = track.clips[clipIndex];
                                    return Container(
                                      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                                      decoration: BoxDecoration(
                                        color: Colors.blueGrey.shade800,
                                        borderRadius: BorderRadius.circular(4),
                                        border: Border.all(color: Colors.blueAccent.shade200),
                                      ),
                                      child: Column(
                                        mainAxisAlignment: MainAxisAlignment.center,
                                        crossAxisAlignment: CrossAxisAlignment.start,
                                        children: [
                                          Text(
                                            'Clip #${clipIndex + 1}',
                                            style: const TextStyle(
                                              color: Colors.white,
                                              fontSize: 11,
                                              fontWeight: FontWeight.bold,
                                            ),
                                          ),
                                          Text(
                                            '${clip.timelineIn} → ${clip.timelineOut} PTS',
                                            style: const TextStyle(
                                              color: Colors.white70,
                                              fontSize: 10,
                                            ),
                                          ),
                                        ],
                                      ),
                                    );
                                  },
                                ),
                              ),
                          ],
                        ),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}
```

---

## 5. File Modification Checklist for Implementation Worker

| File | Action | Description |
|---|---|---|
| `apps/aether_app/analysis_options.yaml` | **Create** | Add `include: package:flutter_lints/flutter.yaml` and strict linter rules |
| `apps/aether_app/lib/src/features/timeline/timeline_provider.dart` | **Create** | Implement `TimelineState`, `TimelineNotifier`, and `timelineProvider` |
| `apps/aether_app/lib/src/features/timeline/timeline_view.dart` | **Update** | Implement Riverpod-driven UI with `Key('timeline_total_clips_count')` and `Key('add_clip_button')` |
| `apps/aether_app/lib/main.dart` | **Update** | Add `WidgetsFlutterBinding.ensureInitialized()` and robust native initialization |
| `apps/aether_app/pubspec.yaml` | **No change needed** | Verified complete and compatible |
| `apps/aether_app/test/timeline_provider_test.dart` | **Create** (Recommended) | Add unit tests for `TimelineNotifier` and `TimelineState` |
