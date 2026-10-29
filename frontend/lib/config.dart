/// Terminal configuration and API endpoint resolution.
library;

class AppConfig {
  AppConfig({required this.apiBase, required this.wsUrl});

  /// REST base URL, override at runtime with `--dart-define=STT_API=...`.
  static const String _apiOverride = String.fromEnvironment('STT_API');

  factory AppConfig.fromEnvironment() {
    final api = _apiOverride.isNotEmpty ? _apiOverride : 'http://127.0.0.1:8765';
    final host = Uri.parse(api).host;
    final port = Uri.parse(api).port;
    final wsScheme = api.startsWith('https') ? 'wss' : 'ws';
    return AppConfig(
      apiBase: api,
      wsUrl: '$wsScheme://$host:$port/api/v1/ws',
    );
  }

  final String apiBase;
  final String wsUrl;
}

/// Preferred format for money amounts.
///
/// All USD amounts arrive as decimal strings; lamports as i64.
class Fmt {
  static String usd(String? raw) {
    if (raw == null || raw.isEmpty) return '\$0.00';
    final v = double.tryParse(raw);
    if (v == null) return '\$$raw';
    if (v.abs() >= 1000) return '\$${v.toStringAsFixed(2)}';
    if (v.abs() >= 1) return '\$${v.toStringAsFixed(4)}';
    if (v.abs() >= 0.0001) return '\$${v.toStringAsFixed(6)}';
    return '\$${v.toStringAsFixed(8)}';
  }

  static String sol(int lamports) {
    final v = lamports / 1e9;
    return '${v.toStringAsFixed(4)} SOL';
  }

  static String pct(String? raw, {bool signed = false}) {
    if (raw == null || raw.isEmpty) return '—';
    final v = double.tryParse(raw);
    if (v == null) return '$raw%';
    final sign = signed && v > 0 ? '+' : '';
    return '$sign${(v * 100).toStringAsFixed(2)}%';
  }

  static String pctD(double v, {bool signed = false}) {
    final sign = signed && v > 0 ? '+' : '';
    return '$sign${(v * 100).toStringAsFixed(2)}%';
  }

  static String micros(int micros) {
    final dt = DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true).toLocal();
    return '${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}:${dt.second.toString().padLeft(2, '0')}';
  }

  static String microsFull(int micros) {
    final dt = DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true).toLocal();
    return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')} '
        '${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
  }
}