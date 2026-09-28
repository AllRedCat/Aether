# Flutter App & UI Survey Report: Aether Timeline & State Integration

## Executive Summary
This report presents the complete survey of the Flutter application codebase located at `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app`. It analyzes the dependencies in `pubspec.yaml`, the existing Riverpod state management setup, the UI widget hierarchy in `TimelineView`, the FFI bridge integration with `flutter_rust_bridge` (FRB v2), and the static analysis baseline (`flutter analyze`).

---

## 1. Codebase Layout & Workspace Inspection

### 1.1 Directory Structure
The Flutter app resides in `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app`:
```
apps/aether_app/
├── lib/
│   ├── main.dart
│   └── src/
│       ├── bridge/                        # Designated location for generated FFI bindings (currently empty)
│       └── features/
│           └── timeline/
│               └── timeline_view.dart     # ConsumerWidget displaying timeline UI
└── pubspec.yaml                           # Flutter package manifest
```

### 1.2 Observed Absences & Scaffolding State
1. **No Generated Bridge**: `lib/src/bridge/` is currently an empty directory. FRB codegen (`make bridge`) has not been executed yet.
2. **No Lockfile or Package Cache**: There is no `pubspec.lock` or `.dart_tool/` directory. `flutter pub get` has not been run in this environment.
3. **No Analysis Configuration**: `analysis_options.yaml` is absent from `apps/aether_app`.
4. **No Platform Directories**: There are no platform-specific folders (`macos/`, `ios/`, `android/`, `linux/`, `windows/`, `web/`). The package is currently structured as an application codebase ready for desktop/mobile integration.

---

## 2. Dependency Analysis (`pubspec.yaml`)

**File**: `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app/pubspec.yaml`

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
  flutter_rust_bridge: ^2.3.0
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

### Key Technical Findings:
1. **Dart SDK Compatibility**:
   - `sdk: '>=3.3.0 <4.0.0'`. Supports all modern Dart 3 features: records, pattern matching, class modifiers, and enhanced enums.
2. **State Management (`flutter_riverpod: ^2.5.1` & `riverpod: ^2.5.1`)**:
   - Riverpod 2.5.1 is modern and stable.
   - Listing both `flutter_riverpod` and `riverpod` is compatible (though `flutter_riverpod` re-exports `riverpod`).
   - Supports both `StateNotifierProvider` and standard Riverpod 2.x `NotifierProvider` / `AsyncNotifierProvider`.
3. **FFI Bridge (`flutter_rust_bridge: ^2.3.0`)**:
   - Aligns exactly with `crates/aether_bridge/Cargo.toml` (`flutter_rust_bridge = "2.3.0"`).
   - FRB v2 generates modern Dart FFI bindings that support asynchronous task dispatch, opaque handles, and serialized struct DTOs without manual C-header bindings.
4. **UUID Package (`uuid: ^4.4.0`)**:
   - Available on the Dart side to generate or parse UUIDs matching the Rust `uuid::Uuid` (version 1.10) in `aether_core`.
5. **Linter (`flutter_lints: ^3.0.0`)**:
   - Declared under `dev_dependencies`, but inactive until `analysis_options.yaml` is added.

---

## 3. Existing Architecture Analysis

### 3.1 `lib/main.dart`
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
**Observations**:
- Root `ProviderScope` is correctly mounted around `AetherApp`.
- Application structure uses Material 3 dark theme (`ThemeData.dark(useMaterial3: true)`).
- Scaffold splits screen into Preview Area (`flex: 2`) and Timeline (`flex: 1`, hosting `TimelineView`).
- `RustLib.init()` and bridge imports are present as commented placeholders pending code generation.

### 3.2 `lib/src/features/timeline/timeline_view.dart`
```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

class TimelineView extends ConsumerWidget {
  const TimelineView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      color: Colors.black87,
      child: const Center(
        child: Text("Timeline DAG / Tracks will render here"),
      ),
    );
  }
}
```
**Observations**:
- `TimelineView` already extends `ConsumerWidget` with access to `WidgetRef ref`.
- Currently contains static placeholder text.
- No Riverpod providers are currently read, watched, or listened to.

---

## 4. FFI Bridge Integration & Data Contracts

### 4.1 Root Makefile & Codegen
Root `Makefile` specifies:
```makefile
bridge:
	flutter_rust_bridge_codegen generate --rust-root crates/aether_bridge --flutter-root apps/aether_app
```

### 4.2 FFI Contract Analysis (Rust <-> Dart)
From `crates/aether_core/src/timeline.rs` and `survey_explorer_rust/report.md`:
- `Timeline`: `{ id: Uuid, timebase: Rational, duration_pts: i64, tracks: Vec<Track> }`
- `Track`: `{ id: Uuid, kind: TrackKind, clips: Vec<Clip> }`
- `Clip`: `{ id: Uuid, source_id: Uuid, source_in: i64, source_out: i64, timeline_in: i64, timeline_out: i64 }`
- `TrackKind`: `enum { Video, Audio, Overlay }`
- `Rational`: `{ num: i32, den: i32 }`

### 4.3 FFI Functions Needed for "Add Clip" Slice
To allow Flutter to manipulate the timeline and observe the updated DAG:
1. `init_engine()`: Called during startup (`await RustLib.init();` in Dart).
2. `create_timeline()`: Returns an initial `Timeline` instance with at least one default Track (`TrackKind::Video`).
3. `add_clip(...)`: Adds a `Clip` to a specific `Track` within the `Timeline`, recalculates `duration_pts`, and returns the updated `Timeline` (or Result).

In FRB v2 Dart code:
```dart
// Generated bindings import
import 'package:aether_app/src/bridge/api.dart';
// or import 'package:aether_app/src/rust/api/api.dart';
```
The types `Timeline`, `Track`, `Clip`, and `TrackKind` will be available as strongly typed Dart classes with public getters:
- `timeline.id` (`String` or `UuidValue`)
- `timeline.durationPts` (`PlatformInt64` / `int` / `BigInt`)
- `timeline.tracks` (`List<Track>`)
- `track.clips` (`List<Clip>`)
- `clip.timelineIn`, `clip.timelineOut`, etc.

---

## 5. Riverpod State Management Design

To consume the Rust Timeline state and provide an interactive trigger for `add_clip`, a dedicated `TimelineNotifier` and `TimelineState` should be created in `lib/src/features/timeline/timeline_provider.dart`.

### 5.1 Proposed State Model (`TimelineState`)
```dart
import '../../bridge/api.dart'; // Generated FRB types

class TimelineState {
  final Timeline? timeline;
  final bool isLoading;
  final String? errorMessage;

  const TimelineState({
    this.timeline,
    this.isLoading = false,
    this.errorMessage,
  });

  /// Total count of clips across all tracks in the DAG
  int get totalClipCount {
    if (timeline == null) return 0;
    return timeline!.tracks.fold<int>(0, (sum, track) => sum + track.clips.length);
  }

  /// Timeline total duration in PTS
  int get durationPts => timeline?.durationPts.toInt() ?? 0;

  List<Track> get tracks => timeline?.tracks ?? const [];

  TimelineState copyWith({
    Timeline? timeline,
    bool? isLoading,
    String? errorMessage,
  }) {
    return TimelineState(
      timeline: timeline ?? this.timeline,
      isLoading: isLoading ?? this.isLoading,
      errorMessage: errorMessage,
    );
  }
}
```

### 5.2 Proposed Notifier (`TimelineNotifier`)
```dart
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';

class TimelineNotifier extends StateNotifier<TimelineState> {
  TimelineNotifier() : super(const TimelineState()) {
    init();
  }

  Future<void> init() async {
    state = state.copyWith(isLoading: true, errorMessage: null);
    try {
      final timeline = await createTimeline();
      state = state.copyWith(timeline: timeline, isLoading: false);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Failed to initialize timeline: $e',
      );
    }
  }

  Future<void> addClip({
    String? trackId,
    String? sourceId,
    int sourceIn = 0,
    int sourceOut = 60,
    int? timelineIn,
  }) async {
    final current = state.timeline;
    if (current == null) {
      state = state.copyWith(errorMessage: 'Timeline not initialized');
      return;
    }

    if (current.tracks.isEmpty) {
      state = state.copyWith(errorMessage: 'No tracks available to receive clip');
      return;
    }

    state = state.copyWith(isLoading: true, errorMessage: null);
    try {
      final targetTrack = trackId ?? current.tracks.first.id;
      final startPts = timelineIn ?? current.durationPts.toInt();

      final updatedTimeline = await addClipToTimeline(
        timeline: current,
        trackId: targetTrack,
        sourceIn: sourceIn,
        sourceOut: sourceOut,
        timelineIn: startPts,
      );

      state = state.copyWith(timeline: updatedTimeline, isLoading: false);
    } catch (e) {
      state = state.copyWith(
        isLoading: false,
        errorMessage: 'Failed to add clip: $e',
      );
    }
  }
}

final timelineProvider = StateNotifierProvider<TimelineNotifier, TimelineState>((ref) {
  return TimelineNotifier();
});
```

---

## 6. UI Structure & TimelineView Implementation

To fulfill Acceptance Criterion:
> "(Verificação por Agente Juiz) O código Dart da tela de Timeline deve comprovadamente ler a lista de faixas/clipes recebida do Rust via FFI e exibir a quantidade correta em tela."
> "Desenvolver um botão ou interface básica na UI que acione o método FFI gerado para adicionar um clipe e atualize a view (TimelineView) mostrando a contagem de clipes atual."

### 6.1 Recommended UI Hierarchy for `TimelineView`
```
Container (Background: Colors.black87)
└── Column
    ├── Timeline Header & Toolbar (Padding)
    │   ├── Row
    │   │   ├── Title: "Timeline Tracks (DAG)"
    │   │   ├── Spacer
    │   │   ├── Duration Badge: "Duration: ${state.durationPts} pts"
    │   │   ├── Clip Count Badge: "Total Clips: ${state.totalClipCount}" (Key: 'timeline_total_clips_count')
    │   │   └── Add Clip Button (ElevatedButton.icon, Key: 'add_clip_button')
    │   └── if (state.isLoading) LinearProgressIndicator
    ├── if (state.errorMessage != null) Error Banner
    ├── Divider
    └── Expanded
        └── if (tracks.isEmpty)
            ├── EmptyState: "No tracks available"
            └── else ListView.builder (Tracks)
                └── TrackItemCard (Track ID, TrackKind, Track clip count)
                    └── ListView (Horizontal or vertical) of ClipCards
                        └── ClipCard: [Clip ID, PTS in -> out, Duration]
```

### 6.2 Concrete Proposed Implementation of `timeline_view.dart`
```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'timeline_provider.dart';

class TimelineView extends ConsumerWidget {
  const TimelineView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(timelineProvider);

    return Container(
      color: const Color(0xFF1E1E1E),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Toolbar
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
            color: const Color(0xFF2D2D2D),
            child: Row(
              children: [
                const Icon(Icons.timeline, color: Colors.amber, size: 20),
                const SizedBox(width: 8),
                const Text(
                  'Timeline DAG',
                  style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
                ),
                const SizedBox(width: 16),
                Chip(
                  label: Text('Duration: ${state.durationPts} pts'),
                  backgroundColor: Colors.black45,
                ),
                const SizedBox(width: 8),
                Chip(
                  key: const Key('timeline_total_clips_count'),
                  label: Text('Total Clips: ${state.totalClipCount}'),
                  backgroundColor: Colors.blueGrey.shade800,
                ),
                const Spacer(),
                ElevatedButton.icon(
                  key: const Key('add_clip_button'),
                  onPressed: state.isLoading
                      ? null
                      : () => ref.read(timelineProvider.notifier).addClip(),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Add Clip'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.blueAccent,
                    foregroundColor: Colors.white,
                  ),
                ),
              ],
            ),
          ),
          if (state.isLoading)
            const LinearProgressIndicator(minHeight: 2),

          if (state.errorMessage != null)
            Container(
              color: Colors.red.shade900,
              padding: const EdgeInsets.all(8.0),
              child: Text(
                state.errorMessage!,
                style: const TextStyle(color: Colors.white),
              ),
            ),

          // Tracks and Clips display
          Expanded(
            child: state.tracks.isEmpty
                ? const Center(
                    child: Text(
                      'No tracks available in timeline',
                      style: TextStyle(color: Colors.white54),
                    ),
                  )
                : ListView.separated(
                    padding: const EdgeInsets.all(12.0),
                    itemCount: state.tracks.length,
                    separatorBuilder: (_, __) => const SizedBox(height: 8),
                    itemBuilder: (context, trackIndex) {
                      final track = state.tracks[trackIndex];
                      return Container(
                        decoration: BoxDecoration(
                          color: const Color(0xFF252525),
                          borderRadius: BorderRadius.circular(6),
                          border: Border.all(color: Colors.white12),
                        ),
                        padding: const EdgeInsets.all(8.0),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Icon(
                                  track.kind.name.toLowerCase() == 'video'
                                      ? Icons.videocam
                                      : Icons.audiotrack,
                                  size: 16,
                                  color: Colors.lightBlueAccent,
                                ),
                                const SizedBox(width: 6),
                                Text(
                                  'Track: ${track.kind.name.toUpperCase()}',
                                  style: const TextStyle(
                                    fontWeight: FontWeight.bold,
                                    fontSize: 13,
                                  ),
                                ),
                                const Spacer(),
                                Text(
                                  'Clips: ${track.clips.length}',
                                  style: const TextStyle(color: Colors.white70, fontSize: 12),
                                ),
                              ],
                            ),
                            const Divider(color: Colors.white10),
                            if (track.clips.isEmpty)
                              const Padding(
                                padding: EdgeInsets.symmetric(vertical: 8.0),
                                child: Text(
                                  'No clips on this track. Click "Add Clip" to add one.',
                                  style: TextStyle(color: Colors.white38, fontSize: 12),
                                ),
                              )
                            else
                              Wrap(
                                spacing: 8.0,
                                runSpacing: 8.0,
                                children: track.clips.map((clip) {
                                  final duration = clip.timelineOut - clip.timelineIn;
                                  return Container(
                                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                                    decoration: BoxDecoration(
                                      color: Colors.indigo.shade800,
                                      borderRadius: BorderRadius.circular(4),
                                      border: Border.all(color: Colors.indigoAccent),
                                    ),
                                    child: Column(
                                      crossAxisAlignment: CrossAxisAlignment.start,
                                      children: [
                                        Text(
                                          'Clip: ${clip.id.toString().substring(0, 8)}...',
                                          style: const TextStyle(
                                            fontWeight: FontWeight.w600,
                                            fontSize: 12,
                                          ),
                                        ),
                                        Text(
                                          'PTS: [${clip.timelineIn} → ${clip.timelineOut}]',
                                          style: const TextStyle(fontSize: 11, color: Colors.white70),
                                        ),
                                        Text(
                                          'Span: $duration pts',
                                          style: const TextStyle(fontSize: 11, color: Colors.amberAccent),
                                        ),
                                      ],
                                    ),
                                  );
                                }).toList(),
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

## 7. Static Analysis & Environment Baseline

### 7.1 Static Analysis Tool Execution
Command executed: `flutter analyze` in `/Users/gabrielgenaro/Developer/Pessoal/Aether/apps/aether_app`
- Result: Exited with code 127 (`zsh:1: command not found: flutter`).

### 7.2 Environment Tooling Diagnosis
1. Neither `flutter` nor `dart` is currently configured in the session `$PATH`.
2. Homebrew has `flutter` cask available (`brew install --cask flutter`) and `dart-sdk` formula (`brew install dart-sdk`).
3. `cargo` and `rustc` are similarly not in `$PATH`.
4. Automated verification in later phases (e.g. `flutter analyze`, `cargo test`) requires the toolchain to be present in `$PATH` or run in an environment with the SDK installed.

### 7.3 Code Quality & Linter Configuration
1. **Current Code Quality**:
   - `lib/main.dart` and `lib/src/features/timeline/timeline_view.dart` are syntactically valid Dart 3, well-structured, and use `const` constructors where applicable.
2. **Missing `analysis_options.yaml`**:
   - `flutter_lints: ^3.0.0` is declared in `pubspec.yaml`, but `analysis_options.yaml` must be created to enforce them:
   ```yaml
   include: package:flutter_lints/flutter.yaml

   linter:
     rules:
       prefer_const_constructors: true
       prefer_const_literals_to_create_immutables: true
       avoid_print: true
   ```
3. **Bridge Dependency Order**:
   - For `flutter analyze` to pass cleanly after Milestone M2, `make bridge` (Milestone M1) must be generated first so Dart analyzer resolves imports from `src/bridge/api.dart`.

---

## 8. Summary Table of Gaps & Requirements

| Area | Current State | Required State for Slice Acceptance |
|---|---|---|
| **pubspec.yaml** | Declares `flutter_riverpod`, `riverpod`, `flutter_rust_bridge`, `uuid` | Complete and sufficient. |
| **analysis_options.yaml** | Absent | Add standard file including `package:flutter_lints/flutter.yaml`. |
| **Generated Bridge** | Empty `lib/src/bridge/` | Run `make bridge` after R1 is implemented in `aether_bridge`. |
| **Riverpod Provider** | None (only root `ProviderScope`) | Implement `timelineProvider` (`StateNotifier<TimelineState>`). |
| **TimelineView UI** | Static placeholder text | Interactive toolbar, clip count chip, Add Clip button, track & clip lanes. |
| **Host Toolchain** | `flutter` / `cargo` not in PATH | Documented in caveats; needed for execution phase. |

---

## 9. Conclusion
The Flutter application structure is lightweight, clean, and primed for the full-stack slice. Once Milestone M1 exposes `add_clip` in `aether_bridge` and generates Dart bindings via `make bridge`, implementing the Riverpod `TimelineNotifier` and the interactive `TimelineView` will cleanly satisfy all acceptance criteria.
