import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'src/bridge/api.dart';
import 'src/bridge/frb_generated.dart';

import 'src/theme/catppuccin.dart';
import 'src/features/welcome/welcome_screen.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    await RustLib.init(
      externalLibrary: ExternalLibrary.process(iKnowHowToUseIt: true),
    );
    await initEngine();
  } catch (e) {
    debugPrint('RustLib init failed: $e');
  }
  runApp(const ProviderScope(child: AetherApp()));
}

class AetherApp extends StatelessWidget {
  const AetherApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Aether Video Editor',
      debugShowCheckedModeBanner: false,
      theme: aetherTheme,
      home: const WelcomeScreen(),
    );
  }
}
