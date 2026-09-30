import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/features/inspector/clip_properties_provider.dart';
import 'package:aether_app/src/features/inspector/selected_clip_details_provider.dart';
import 'package:aether_app/src/features/media_pool/media_pool_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_provider.dart';
import 'package:aether_app/src/features/timeline/timeline_selection_provider.dart';

/// Headless timeline notifier for unit tests.
class HeadlessTimelineNotifier extends TimelineNotifier {
  HeadlessTimelineNotifier(Timeline timeline)
      : super(autoInit: false, createTimelineFn: () async => timeline) {
    state = TimelineState.fromTimeline(timeline);
  }

  void setTimeline(Timeline timeline) {
    state = TimelineState.fromTimeline(timeline);
  }
}

/// Headless media pool notifier for unit tests.
class HeadlessMediaPoolNotifier extends MediaPoolNotifier {
  HeadlessMediaPoolNotifier(List<MediaItem> items) : super(autoLoad: false) {
    state = MediaPoolState(items: items);
  }

  void setItems(List<MediaItem> items) {
    state = MediaPoolState(items: items);
  }
}

void main() {
  final videoClipId = UuidValue.fromString('c0000000-0000-0000-0000-000000000001');
  final audioClipId = UuidValue.fromString('c0000000-0000-0000-0000-000000000002');
  final unknownClipId = UuidValue.fromString('c0000000-0000-0000-0000-000000000099');
  final videoTrackId = UuidValue.fromString('t0000000-0000-0000-0000-000000000001');
  final audioTrackId = UuidValue.fromString('t0000000-0000-0000-0000-000000000002');
  final videoMediaId = UuidValue.fromString('m0000000-0000-0000-0000-000000000001');
  final audioMediaId = UuidValue.fromString('m0000000-0000-0000-0000-000000000002');
  final orphanMediaId = UuidValue.fromString('m0000000-0000-0000-0000-000000000099');

  final videoClip = Clip(
    id: videoClipId,
    sourceId: videoMediaId,
    sourceIn: 0,
    sourceOut: 600,
    timelineIn: 0,
    timelineOut: 600,
  );

  final audioClip = Clip(
    id: audioClipId,
    sourceId: audioMediaId,
    sourceIn: 0,
    sourceOut: 1200,
    timelineIn: 0,
    timelineOut: 1200,
  );

  final orphanClip = Clip(
    id: UuidValue.fromString('c0000000-0000-0000-0000-000000000003'),
    sourceId: orphanMediaId,
    sourceIn: 0,
    sourceOut: 300,
    timelineIn: 600,
    timelineOut: 900,
  );

  final videoTrack = Track(
    id: videoTrackId,
    kind: TrackKind.video,
    clips: [videoClip, orphanClip],
  );

  final audioTrack = Track(
    id: audioTrackId,
    kind: TrackKind.audio,
    clips: [audioClip],
  );

  final testTimeline = Timeline(
    id: UuidValue.fromString('a0000000-0000-0000-0000-000000000001'),
    timebase: const Rational(num: 60, den: 1),
    durationPts: 1200,
    tracks: [videoTrack, audioTrack],
  );

  final testMediaItems = [
    MediaItem(
      id: videoMediaId,
      filePath: '/media/landscape.mp4',
      fileName: 'landscape.mp4',
      mediaType: MediaType.video,
      metadata: const MediaMetadata(
        durationPts: 600,
        durationSeconds: 10.0,
        fileSizeBytes: 10485760,
      ),
    ),
    MediaItem(
      id: audioMediaId,
      filePath: '/media/voiceover.wav',
      fileName: 'voiceover.wav',
      mediaType: MediaType.audio,
      metadata: const MediaMetadata(
        durationPts: 1200,
        durationSeconds: 20.0,
        fileSizeBytes: 2097152,
      ),
    ),
  ];

  ProviderContainer createContainer({
    Timeline? timeline,
    List<MediaItem>? mediaItems,
  }) {
    final container = ProviderContainer(
      overrides: [
        timelineProvider.overrideWith(
          (ref) => HeadlessTimelineNotifier(timeline ?? testTimeline),
        ),
        mediaPoolProvider.overrideWith(
          (ref) => HeadlessMediaPoolNotifier(mediaItems ?? testMediaItems),
        ),
      ],
    );
    addTearDown(container.dispose);
    return container;
  }

  group('Timeline Selection & Properties Unit Test Suite', () {
    test('1. timelineSelectionProvider starts with hasSelection == false and null IDs', () {
      final container = createContainer();
      final state = container.read(timelineSelectionProvider);

      expect(state.hasSelection, isFalse);
      expect(state.selectedClipId, isNull);
      expect(state.selectedTrackId, isNull);
      expect(state.isClipSelected(videoClipId), isFalse);
      expect(state.isTrackSelected(videoTrackId), isFalse);
    });

    test('2. selectClip updates state and hasSelection becomes true', () {
      final container = createContainer();
      final notifier = container.read(timelineSelectionProvider.notifier);

      notifier.selectClip(videoClipId, videoTrackId);
      final state = container.read(timelineSelectionProvider);

      expect(state.hasSelection, isTrue);
      expect(state.selectedClipId, equals(videoClipId));
      expect(state.selectedTrackId, equals(videoTrackId));
      expect(state.isClipSelected(videoClipId), isTrue);
      expect(state.isClipSelected(audioClipId), isFalse);
      expect(state.isTrackSelected(videoTrackId), isTrue);
      expect(state.isTrackSelected(audioTrackId), isFalse);
    });

    test('3. toggleClip selects unselected clip and deselects currently selected clip', () {
      final container = createContainer();
      final notifier = container.read(timelineSelectionProvider.notifier);

      // Select via toggle
      notifier.toggleClip(videoClipId, videoTrackId);
      expect(container.read(timelineSelectionProvider).hasSelection, isTrue);
      expect(container.read(timelineSelectionProvider).selectedClipId, equals(videoClipId));

      // Deselect via toggle on same clip
      notifier.toggleClip(videoClipId, videoTrackId);
      expect(container.read(timelineSelectionProvider).hasSelection, isFalse);
      expect(container.read(timelineSelectionProvider).selectedClipId, isNull);

      // Select another clip via toggle
      notifier.toggleClip(audioClipId, audioTrackId);
      expect(container.read(timelineSelectionProvider).selectedClipId, equals(audioClipId));

      // Switch selection to video clip via toggle
      notifier.toggleClip(videoClipId, videoTrackId);
      expect(container.read(timelineSelectionProvider).selectedClipId, equals(videoClipId));
      expect(container.read(timelineSelectionProvider).selectedTrackId, equals(videoTrackId));
    });

    test('4. clearSelection clears selection state', () {
      final container = createContainer();
      final notifier = container.read(timelineSelectionProvider.notifier);

      notifier.selectClip(videoClipId, videoTrackId);
      expect(container.read(timelineSelectionProvider).hasSelection, isTrue);

      notifier.clearSelection();
      expect(container.read(timelineSelectionProvider).hasSelection, isFalse);
      expect(container.read(timelineSelectionProvider).selectedClipId, isNull);
      expect(container.read(timelineSelectionProvider).selectedTrackId, isNull);

      // Subsequent call when already unselected is a safe no-op
      notifier.clearSelection();
      expect(container.read(timelineSelectionProvider).hasSelection, isFalse);
    });

    test('5. selectedClipDetailsProvider returns null when selection is empty', () {
      final container = createContainer();
      final details = container.read(selectedClipDetailsProvider);
      expect(details, isNull);
    });

    test('6. selectedClipDetailsProvider correctly reconciles selected clip, parent track, and MediaItem', () {
      final container = createContainer();

      // Video clip reconciliation
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
      final videoDetails = container.read(selectedClipDetailsProvider);

      expect(videoDetails, isNotNull);
      expect(videoDetails!.clip.id, equals(videoClipId));
      expect(videoDetails.track.id, equals(videoTrackId));
      expect(videoDetails.trackKind, equals(TrackKind.video));
      expect(videoDetails.isVideo, isTrue);
      expect(videoDetails.isAudio, isFalse);
      expect(videoDetails.mediaItem, isNotNull);
      expect(videoDetails.mediaItem!.fileName, equals('landscape.mp4'));
      expect(videoDetails.mediaItem!.mediaType, equals(MediaType.video));

      // Audio clip reconciliation
      container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
      final audioDetails = container.read(selectedClipDetailsProvider);

      expect(audioDetails, isNotNull);
      expect(audioDetails!.clip.id, equals(audioClipId));
      expect(audioDetails.track.id, equals(audioTrackId));
      expect(audioDetails.trackKind, equals(TrackKind.audio));
      expect(audioDetails.isVideo, isFalse);
      expect(audioDetails.isAudio, isTrue);
      expect(audioDetails.mediaItem, isNotNull);
      expect(audioDetails.mediaItem!.fileName, equals('voiceover.wav'));
      expect(audioDetails.mediaItem!.mediaType, equals(MediaType.audio));
    });

    test('7. selectedClipDetailsProvider falls back to mediaItem: null when clip sourceId is not in MediaPool', () {
      final container = createContainer();
      final orphanClipId = orphanClip.id;

      container.read(timelineSelectionProvider.notifier).selectClip(orphanClipId, videoTrackId);
      final details = container.read(selectedClipDetailsProvider);

      expect(details, isNotNull);
      expect(details!.clip.id, equals(orphanClipId));
      expect(details.mediaItem, isNull);
      expect(details.isVideo, isTrue);
    });

    test('8. selectedClipDetailsProvider returns null when clip was deleted from timeline', () {
      final container = createContainer();

      // Select a clip ID that does not exist in the timeline
      container.read(timelineSelectionProvider.notifier).selectClip(unknownClipId, videoTrackId);
      final details = container.read(selectedClipDetailsProvider);

      expect(details, isNull);
    });

    test('9. clipPropertiesProvider provides default VideoTransformProperties and AudioClipProperties', () {
      final container = createContainer();
      final state = container.read(clipPropertiesProvider);

      final videoProps = state.videoPropertiesFor(videoClipId);
      expect(videoProps.positionX, equals(0.0));
      expect(videoProps.positionY, equals(0.0));
      expect(videoProps.scale, equals(1.0));
      expect(videoProps.rotation, equals(0.0));
      expect(videoProps.opacity, equals(1.0));

      final audioProps = state.audioPropertiesFor(audioClipId);
      expect(audioProps.volumeDb, equals(0.0));
      expect(audioProps.pan, equals(0.0));
      expect(audioProps.isMuted, isFalse);
    });

    test('10. updateVideoProperties updates positionX and scale while keeping default rotation and opacity', () {
      final container = createContainer();
      final notifier = container.read(clipPropertiesProvider.notifier);

      notifier.updateVideoProperties(
        videoClipId,
        positionX: 150.0,
        scale: 2.0,
      );

      final props = container.read(clipPropertiesProvider).videoPropertiesFor(videoClipId);
      expect(props.positionX, equals(150.0));
      expect(props.scale, equals(2.0));
      expect(props.positionY, equals(0.0));
      expect(props.rotation, equals(0.0));
      expect(props.opacity, equals(1.0));
    });

    test('11. resetVideoScale resets scale to 1.0 without resetting modified position', () {
      final container = createContainer();
      final notifier = container.read(clipPropertiesProvider.notifier);

      notifier.updateVideoProperties(
        videoClipId,
        positionX: 200.0,
        positionY: -50.0,
        scale: 3.0,
        rotation: 45.0,
        opacity: 0.8,
      );

      notifier.resetVideoScale(videoClipId);

      final props = container.read(clipPropertiesProvider).videoPropertiesFor(videoClipId);
      expect(props.scale, equals(1.0));
      expect(props.positionX, equals(200.0));
      expect(props.positionY, equals(-50.0));
      expect(props.rotation, equals(45.0));
      expect(props.opacity, equals(0.8));

      // Also verify individual positionX and opacity resets
      notifier.resetVideoPositionX(videoClipId);
      final props2 = container.read(clipPropertiesProvider).videoPropertiesFor(videoClipId);
      expect(props2.positionX, equals(0.0));
      expect(props2.positionY, equals(-50.0));

      // Verify resetVideoProperties resets all
      notifier.resetVideoProperties(videoClipId);
      final props3 = container.read(clipPropertiesProvider).videoPropertiesFor(videoClipId);
      expect(props3, equals(VideoTransformProperties.defaults));
    });

    test('12. updateAudioProperties and toggleMute correctly manipulate audio clip properties', () {
      final container = createContainer();
      final notifier = container.read(clipPropertiesProvider.notifier);

      notifier.updateAudioProperties(
        audioClipId,
        volumeDb: -12.5,
        pan: 0.6,
      );

      var props = container.read(clipPropertiesProvider).audioPropertiesFor(audioClipId);
      expect(props.volumeDb, equals(-12.5));
      expect(props.pan, equals(0.6));
      expect(props.isMuted, isFalse);

      notifier.toggleMute(audioClipId);
      props = container.read(clipPropertiesProvider).audioPropertiesFor(audioClipId);
      expect(props.isMuted, isTrue);

      notifier.resetVolume(audioClipId);
      props = container.read(clipPropertiesProvider).audioPropertiesFor(audioClipId);
      expect(props.volumeDb, equals(0.0));
      expect(props.pan, equals(0.6));
      expect(props.isMuted, isTrue);

      notifier.resetPan(audioClipId);
      props = container.read(clipPropertiesProvider).audioPropertiesFor(audioClipId);
      expect(props.pan, equals(0.0));

      notifier.resetAudioProperties(audioClipId);
      props = container.read(clipPropertiesProvider).audioPropertiesFor(audioClipId);
      expect(props, equals(AudioClipProperties.defaults));
    });

    test('13. activeClipVideoPropertiesProvider and activeClipAudioPropertiesProvider reactively stream values for selected clip', () {
      final container = createContainer();

      // Initially no clip selected -> returns defaults
      expect(
        container.read(activeClipVideoPropertiesProvider),
        equals(VideoTransformProperties.defaults),
      );
      expect(
        container.read(activeClipAudioPropertiesProvider),
        equals(AudioClipProperties.defaults),
      );

      // Select video clip
      container.read(timelineSelectionProvider.notifier).selectClip(videoClipId, videoTrackId);
      expect(
        container.read(activeClipVideoPropertiesProvider),
        equals(VideoTransformProperties.defaults),
      );

      // Mutate video properties
      container.read(clipPropertiesProvider.notifier).updateVideoProperties(
        videoClipId,
        positionX: 75.0,
        opacity: 0.9,
      );
      expect(
        container.read(activeClipVideoPropertiesProvider).positionX,
        equals(75.0),
      );
      expect(
        container.read(activeClipVideoPropertiesProvider).opacity,
        equals(0.9),
      );

      // Select audio clip
      container.read(timelineSelectionProvider.notifier).selectClip(audioClipId, audioTrackId);
      expect(
        container.read(activeClipAudioPropertiesProvider),
        equals(AudioClipProperties.defaults),
      );

      // Mutate audio properties
      container.read(clipPropertiesProvider.notifier).updateAudioProperties(
        audioClipId,
        volumeDb: -3.0,
      );
      expect(
        container.read(activeClipAudioPropertiesProvider).volumeDb,
        equals(-3.0),
      );

      // Remove clip from properties and clear selection
      container.read(clipPropertiesProvider.notifier).removeClip(videoClipId);
      expect(
        container.read(clipPropertiesProvider).videoPropertiesFor(videoClipId),
        equals(VideoTransformProperties.defaults),
      );

      container.read(timelineSelectionProvider.notifier).clearSelection();
      expect(
        container.read(activeClipVideoPropertiesProvider),
        equals(VideoTransformProperties.defaults),
      );
      expect(
        container.read(activeClipAudioPropertiesProvider),
        equals(AudioClipProperties.defaults),
      );
    });
  });
}
