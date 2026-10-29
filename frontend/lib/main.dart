import 'package:flutter/material.dart';

import 'config.dart';
import 'screens/shell.dart';
import 'state/app_state.dart';
import 'theme.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  final appState = AppState(AppConfig.fromEnvironment())..start();
  runApp(SttApp(state: appState));
}

class SttApp extends StatelessWidget {
  const SttApp({super.key, required this.state});

  final AppState state;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: state,
      builder: (context, _) {
        return MaterialApp(
          title: 'STT · Solana Paper Terminal',
          debugShowCheckedModeBanner: false,
          theme: buildTheme(),
          home: Shell(state: state),
        );
      },
    );
  }
}