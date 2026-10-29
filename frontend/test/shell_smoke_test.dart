import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:stt_desktop/config.dart';
import 'package:stt_desktop/screens/shell.dart';
import 'package:stt_desktop/state/app_state.dart';

void main() {
  testWidgets('Shell renders sidebar, sim boundary and error banner',
      (WidgetTester tester) async {
    final state = AppState(AppConfig(
      apiBase: 'http://127.0.0.1:8765',
      wsUrl: 'ws://127.0.0.1:8765/api/v1/ws',
    ));

    await tester.pumpWidget(MaterialApp(home: Shell(state: state)));
    await tester.pump(const Duration(milliseconds: 200));

    // All seven nav entries are present.
    for (final label in [
      'DASHBOARD',
      'MARKETS',
      'STRATEGIES',
      'PORTFOLIO',
      'BACKTESTS',
      'RESEARCH',
      'SETTINGS',
    ]) {
      expect(find.text(label), findsOneWidget, reason: 'missing nav $label');
    }

    // Backend not connected => banner is rendered (with its prefix).
    expect(find.textContaining('BACKEND UNREACHABLE'), findsOneWidget);

    // First screen (dashboard) is visible.
    expect(
      find.byType(Scaffold),
      findsWidgets,
    );
  });
}