import 'package:file_selector/file_selector.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Abstract service defining the contract for selecting media files from the filesystem.
abstract class FilePickerService {
  Future<List<String>> pickMediaFiles();
}

/// Production implementation of [FilePickerService] using `package:file_selector`.
class FileSelectorPickerService implements FilePickerService {
  const FileSelectorPickerService();

  static const XTypeGroup mediaTypeGroup = XTypeGroup(
    label: 'Arquivos de Mídia',
    extensions: <String>[
      // Video
      'mp4', 'mov', 'm4v', 'mkv', 'avi', 'webm',
      // Audio
      'wav', 'mp3', 'aac', 'flac', 'ogg', 'm4a', 'opus', 'aiff',
      // Image
      'png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tiff',
    ],
  );

  @override
  Future<List<String>> pickMediaFiles() async {
    final List<XFile> files = await openFiles(
      acceptedTypeGroups: const <XTypeGroup>[mediaTypeGroup],
      confirmButtonText: 'Importar',
    );
    return files.map((f) => f.path).toList();
  }
}

/// Riverpod provider for [FilePickerService].
final filePickerServiceProvider = Provider<FilePickerService>((ref) {
  return const FileSelectorPickerService();
});
