import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/main.dart';
import 'package:aether_app/src/bridge/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());
  testWidgets('Can render app', (WidgetTester tester) async {
    await tester.pumpWidget(const ProviderScope(child: AetherApp()));
    expect(find.byType(AetherApp), findsOneWidget);
  });
}
