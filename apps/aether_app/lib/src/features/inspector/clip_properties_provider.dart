import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import 'selected_clip_details_provider.dart';

/// Client-side transform properties for visual (Video/Overlay) clips.
@immutable
class VideoTransformProperties {
  final double positionX;
  final double positionY;
  final double scale;
  final double rotation;
  final double opacity;

  const VideoTransformProperties({
    this.positionX = 0.0,
    this.positionY = 0.0,
    this.scale = 1.0,
    this.rotation = 0.0,
    this.opacity = 1.0,
  });

  static const VideoTransformProperties defaults = VideoTransformProperties();

  // Constraints & Slider Bounds
  static const double minPositionX = -1920.0;
  static const double maxPositionX = 1920.0;
  static const double minPositionY = -1080.0;
  static const double maxPositionY = 1080.0;
  static const double minScale = 0.1;
  static const double maxScale = 5.0;
  static const double minRotation = -180.0;
  static const double maxRotation = 180.0;
  static const double minOpacity = 0.0;
  static const double maxOpacity = 1.0;

  VideoTransformProperties copyWith({
    double? positionX,
    double? positionY,
    double? scale,
    double? rotation,
    double? opacity,
  }) {
    return VideoTransformProperties(
      positionX: positionX ?? this.positionX,
      positionY: positionY ?? this.positionY,
      scale: scale ?? this.scale,
      rotation: rotation ?? this.rotation,
      opacity: opacity ?? this.opacity,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is VideoTransformProperties &&
          runtimeType == other.runtimeType &&
          positionX == other.positionX &&
          positionY == other.positionY &&
          scale == other.scale &&
          rotation == other.rotation &&
          opacity == other.opacity;

  @override
  int get hashCode =>
      positionX.hashCode ^
      positionY.hashCode ^
      scale.hashCode ^
      rotation.hashCode ^
      opacity.hashCode;

  @override
  String toString() =>
      'VideoTransformProperties(posX: $positionX, posY: $positionY, scale: $scale, rot: $rotation, op: $opacity)';
}

/// Client-side audio properties for audio clips.
@immutable
class AudioClipProperties {
  final double volumeDb;
  final double pan;
  final bool isMuted;

  const AudioClipProperties({
    this.volumeDb = 0.0,
    this.pan = 0.0,
    this.isMuted = false,
  });

  static const AudioClipProperties defaults = AudioClipProperties();

  // Constraints & Slider Bounds
  static const double minVolumeDb = -60.0;
  static const double maxVolumeDb = 12.0;
  static const double minPan = -1.0;
  static const double maxPan = 1.0;

  AudioClipProperties copyWith({
    double? volumeDb,
    double? pan,
    bool? isMuted,
  }) {
    return AudioClipProperties(
      volumeDb: volumeDb ?? this.volumeDb,
      pan: pan ?? this.pan,
      isMuted: isMuted ?? this.isMuted,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AudioClipProperties &&
          runtimeType == other.runtimeType &&
          volumeDb == other.volumeDb &&
          pan == other.pan &&
          isMuted == other.isMuted;

  @override
  int get hashCode => volumeDb.hashCode ^ pan.hashCode ^ isMuted.hashCode;

  @override
  String toString() =>
      'AudioClipProperties(volumeDb: $volumeDb, pan: $pan, isMuted: $isMuted)';
}

/// Immutable state containing per-clip transform and audio properties.
@immutable
class ClipPropertiesState {
  final Map<UuidValue, VideoTransformProperties> videoProperties;
  final Map<UuidValue, AudioClipProperties> audioProperties;

  const ClipPropertiesState({
    this.videoProperties = const {},
    this.audioProperties = const {},
  });

  /// Retrieves the transform properties for a video clip, returning defaults if not yet modified.
  VideoTransformProperties videoPropertiesFor(UuidValue clipId) {
    return videoProperties[clipId] ?? VideoTransformProperties.defaults;
  }

  /// Retrieves the audio properties for an audio clip, returning defaults if not yet modified.
  AudioClipProperties audioPropertiesFor(UuidValue clipId) {
    return audioProperties[clipId] ?? AudioClipProperties.defaults;
  }

  ClipPropertiesState copyWith({
    Map<UuidValue, VideoTransformProperties>? videoProperties,
    Map<UuidValue, AudioClipProperties>? audioProperties,
  }) {
    return ClipPropertiesState(
      videoProperties: videoProperties ?? this.videoProperties,
      audioProperties: audioProperties ?? this.audioProperties,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ClipPropertiesState &&
          runtimeType == other.runtimeType &&
          mapEquals(videoProperties, other.videoProperties) &&
          mapEquals(audioProperties, other.audioProperties);

  @override
  int get hashCode => Object.hash(
        Object.hashAll(videoProperties.entries),
        Object.hashAll(audioProperties.entries),
      );
}

/// StateNotifier managing per-clip property changes.
class ClipPropertiesNotifier extends StateNotifier<ClipPropertiesState> {
  ClipPropertiesNotifier() : super(const ClipPropertiesState());

  /// Sets full video properties for a clip.
  void setVideoProperties(UuidValue clipId, VideoTransformProperties properties) {
    final updated = Map<UuidValue, VideoTransformProperties>.from(state.videoProperties);
    updated[clipId] = properties;
    state = state.copyWith(videoProperties: updated);
  }

  /// Updates specified video properties for a clip, preserving unmentioned fields.
  void updateVideoProperties(
    UuidValue clipId, {
    double? positionX,
    double? positionY,
    double? scale,
    double? rotation,
    double? opacity,
  }) {
    final current = state.videoPropertiesFor(clipId);
    final updated = current.copyWith(
      positionX: positionX,
      positionY: positionY,
      scale: scale,
      rotation: rotation,
      opacity: opacity,
    );
    setVideoProperties(clipId, updated);
  }

  /// Resets individual video properties to defaults.
  void resetVideoPositionX(UuidValue clipId) =>
      updateVideoProperties(clipId, positionX: VideoTransformProperties.defaults.positionX);

  void resetVideoPositionY(UuidValue clipId) =>
      updateVideoProperties(clipId, positionY: VideoTransformProperties.defaults.positionY);

  void resetVideoScale(UuidValue clipId) =>
      updateVideoProperties(clipId, scale: VideoTransformProperties.defaults.scale);

  void resetVideoRotation(UuidValue clipId) =>
      updateVideoProperties(clipId, rotation: VideoTransformProperties.defaults.rotation);

  void resetVideoOpacity(UuidValue clipId) =>
      updateVideoProperties(clipId, opacity: VideoTransformProperties.defaults.opacity);

  /// Resets all video properties for a clip to defaults.
  void resetVideoProperties(UuidValue clipId) {
    final updated = Map<UuidValue, VideoTransformProperties>.from(state.videoProperties);
    updated.remove(clipId);
    state = state.copyWith(videoProperties: updated);
  }

  /// Sets full audio properties for a clip.
  void setAudioProperties(UuidValue clipId, AudioClipProperties properties) {
    final updated = Map<UuidValue, AudioClipProperties>.from(state.audioProperties);
    updated[clipId] = properties;
    state = state.copyWith(audioProperties: updated);
  }

  /// Updates specified audio properties for a clip.
  void updateAudioProperties(
    UuidValue clipId, {
    double? volumeDb,
    double? pan,
    bool? isMuted,
  }) {
    final current = state.audioPropertiesFor(clipId);
    final updated = current.copyWith(
      volumeDb: volumeDb,
      pan: pan,
      isMuted: isMuted,
    );
    setAudioProperties(clipId, updated);
  }

  /// Toggles mute state for an audio clip.
  void toggleMute(UuidValue clipId) {
    final current = state.audioPropertiesFor(clipId);
    updateAudioProperties(clipId, isMuted: !current.isMuted);
  }

  /// Resets audio volume to default (0.0 dB).
  void resetVolume(UuidValue clipId) =>
      updateAudioProperties(clipId, volumeDb: AudioClipProperties.defaults.volumeDb);

  /// Resets audio pan to default (0.0 center).
  void resetPan(UuidValue clipId) =>
      updateAudioProperties(clipId, pan: AudioClipProperties.defaults.pan);

  /// Resets all audio properties for a clip to defaults.
  void resetAudioProperties(UuidValue clipId) {
    final updated = Map<UuidValue, AudioClipProperties>.from(state.audioProperties);
    updated.remove(clipId);
    state = state.copyWith(audioProperties: updated);
  }

  /// Clears stored properties when a clip is permanently removed.
  void removeClip(UuidValue clipId) {
    final updatedVideo = Map<UuidValue, VideoTransformProperties>.from(state.videoProperties);
    final updatedAudio = Map<UuidValue, AudioClipProperties>.from(state.audioProperties);
    updatedVideo.remove(clipId);
    updatedAudio.remove(clipId);
    state = state.copyWith(
      videoProperties: updatedVideo,
      audioProperties: updatedAudio,
    );
  }
}

/// Riverpod StateNotifierProvider for clip properties.
final clipPropertiesProvider =
    StateNotifierProvider<ClipPropertiesNotifier, ClipPropertiesState>((ref) {
  return ClipPropertiesNotifier();
});

/// Convenience computed provider for the active selected clip's VideoTransformProperties.
final activeClipVideoPropertiesProvider = Provider<VideoTransformProperties>((ref) {
  final details = ref.watch(selectedClipDetailsProvider);
  if (details == null) return VideoTransformProperties.defaults;
  final properties = ref.watch(clipPropertiesProvider);
  return properties.videoPropertiesFor(details.clip.id);
});

/// Convenience computed provider for the active selected clip's AudioClipProperties.
final activeClipAudioPropertiesProvider = Provider<AudioClipProperties>((ref) {
  final details = ref.watch(selectedClipDetailsProvider);
  if (details == null) return AudioClipProperties.defaults;
  final properties = ref.watch(clipPropertiesProvider);
  return properties.audioPropertiesFor(details.clip.id);
});
