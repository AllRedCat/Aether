import 'dart:io';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/bridge/frb_generated.dart';

void main() {
  test('Testa importacao MOV via FFI', () async {
    final dylibFile = File('../../target/debug/libaether_bridge.dylib');
    if (!dylibFile.existsSync()) {
      return;
    }
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(dylibFile.path),
    );

    const projectPath = "/Users/gabrielgenaro/Movies/test_project.aether";
    const movPath =
        "/Users/gabrielgenaro/Movies/Gravação de Tela 2026-09-30 às 16.44.58.mov";

    try {
      final item =
          await importMediaFile(projectPath: projectPath, filePath: movPath);
      // ignore: avoid_print
      print("SUCESSO: ${item.metadata.videoCodec}");
    } catch (e) {
      // ignore: avoid_print
      print("ERRO CAPTURADO NO DART: $e");
    } finally {
      RustLib.dispose();
    }
  });
}
