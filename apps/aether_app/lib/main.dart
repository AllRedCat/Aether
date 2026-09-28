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
