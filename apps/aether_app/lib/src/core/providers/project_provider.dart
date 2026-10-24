import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';

/// Global provider to hold the currently open project.
final currentProjectProvider = StateProvider<Project?>((ref) => null);

/// Global provider to hold the metadata of the currently open project bundle.
final currentProjectMetadataProvider =
    StateProvider<ProjectMetadata?>((ref) => null);
