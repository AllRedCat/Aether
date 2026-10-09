import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../bridge/api.dart';

/// Global provider to hold the currently open project.
final currentProjectProvider = StateProvider<Project?>((ref) => null);
