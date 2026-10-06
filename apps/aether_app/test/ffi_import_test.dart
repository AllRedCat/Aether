import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/src/bridge/api.dart';
import 'package:aether_app/src/bridge/frb_generated.dart';

void main() {
  test('Testa importacao MOV via FFI', () async {
    await RustLib.init();
    
    const projectPath = "/Users/gabrielgenaro/Movies/test_project.aether";
    const movPath = "/Users/gabrielgenaro/Movies/Gravação de Tela 2026-09-30 às 16.44.58.mov";
    
    try {
      final item = await importMediaFile(projectPath: projectPath, filePath: movPath);
      print("SUCESSO: \${item.metadata.videoCodec}");
    } catch (e) {
      print("ERRO CAPTURADO NO DART: \$e");
    }
  });
}
