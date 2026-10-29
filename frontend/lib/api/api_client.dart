import 'dart:convert';

import 'package:http/http.dart' as http;

import '../config.dart';

/// Thin typed REST client for the terminal API.
class ApiClient {
  ApiClient(this.cfg);

  final AppConfig cfg;

  Map<String, String> get _headers => {'Content-Type': 'application/json'};

  Future<Map<String, dynamic>> _get(String path,
      {Map<String, String>? query}) async {
    var uri = Uri.parse('${cfg.apiBase}$path');
    if (query != null && query.isNotEmpty) {
      uri = uri.replace(queryParameters: query);
    }
    final r = await http.get(uri).timeout(const Duration(seconds: 10));
    return _decode(r);
  }

  Future<Map<String, dynamic>> _post(String path,
      [Map<String, dynamic>? body]) async {
    final r = await http
        .post(Uri.parse('${cfg.apiBase}$path'),
            headers: _headers, body: jsonEncode(body ?? const <String, dynamic>{}))
        .timeout(const Duration(seconds: 10));
    return _decode(r);
  }

  Future<Map<String, dynamic>> _put(String path,
      [Map<String, dynamic>? body]) async {
    final r = await http
        .put(Uri.parse('${cfg.apiBase}$path'),
            headers: _headers, body: jsonEncode(body ?? const <String, dynamic>{}))
        .timeout(const Duration(seconds: 10));
    return _decode(r);
  }

  Map<String, dynamic> _decode(http.Response r) {
    dynamic json;
    try {
      json = jsonDecode(r.body);
    } catch (_) {
      throw ApiException('HTTP ${r.statusCode}: non-JSON response');
    }
    if (r.statusCode >= 400 || (json is Map && json.containsKey('error'))) {
      final msg = json is Map
          ? (json['message']?.toString() ?? json['error'].toString())
          : 'HTTP ${r.statusCode}';
      throw ApiException(msg);
    }
    if (json is! Map<String, dynamic>) {
      throw ApiException('unexpected response shape: ${r.runtimeType}');
    }
    return json;
  }

  // ---- endpoint groups -----------------------------------------------------

  Future<Map<String, dynamic>> status() => _get('/api/v1/status');
  Future<Map<String, dynamic>> dashboard() => _get('/api/v1/dashboard');
  Future<Map<String, dynamic>> portfolio() => _get('/api/v1/portfolio');

  Future<List<Map<String, dynamic>>> positions() async {
    final d = await _get('/api/v1/positions');
    final list = d['positions'];
    if (list is List) return list.whereType<Map<String, dynamic>>().toList();
    return const [];
  }

  Future<List<dynamic>> orders() async =>
      (await _get('/api/v1/orders'))['items'] as List<dynamic>? ?? [];

  Future<List<dynamic>> fills() async =>
      (await _get('/api/v1/fills'))['items'] as List<dynamic>? ?? [];

  Future<List<dynamic>> ledger() async =>
      (await _get('/api/v1/ledger'))['items'] as List<dynamic>? ?? [];

  Future<List<Map<String, dynamic>>> markets() async {
    final d = await _get('/api/v1/markets');
    final list = d['markets'];
    if (list is List) return list.whereType<Map<String, dynamic>>().toList();
    return const [];
  }

  Future<List<Map<String, dynamic>>> strategies() async {
    final d = await _get('/api/v1/strategies');
    final list = d['strategies'];
    if (list is List) return list.whereType<Map<String, dynamic>>().toList();
    return const [];
  }

  Future<Map<String, dynamic>> strategyDetail(String id) =>
      _get('/api/v1/strategies/$id');

  Future<Map<String, dynamic>> strategyStart(String id) => _post(
      '/api/v1/strategies/$id/start',
      const {'mode': 'demo', 'execution': 'paper'});

  Future<Map<String, dynamic>> strategyPause(String id) =>
      _post('/api/v1/strategies/$id/pause');

  Future<Map<String, dynamic>> strategyStop(String id) =>
      _post('/api/v1/strategies/$id/stop');

  /// Full strategy-signal decision feed (audit trail).
  Future<List<Map<String, dynamic>>> signals() async {
    final json = await _get('/api/v1/signals', query: {'limit': '200'});
    final items = json['items'];
    if (items is! List) return const [];
    return items.whereType<Map<String, dynamic>>().toList();
  }

  /// Fetch one research export as raw CSV text.
  /// `kind` is one of: fills, orders, ledger, signals, positions, equity,
  /// backtests, risk-events, system-events.
  Future<String> exportCsv(String kind) async {
    final uri = Uri.parse('${cfg.apiBase}/api/v1/export/$kind');
    final r = await http.get(uri).timeout(const Duration(seconds: 30));
    if (r.statusCode >= 400) {
      throw ApiException('export $kind failed: HTTP ${r.statusCode}');
    }
    return r.body;
  }

  Future<Map<String, dynamic>> strategyUpdateConfig(
          String id, Map<String, dynamic> config) =>
      _put('/api/v1/strategies/$id/config', config);

  Future<List<Map<String, dynamic>>> backtests() async {
    final d = await _get('/api/v1/backtests');
    final list = d['items'] ?? d['backtests'];
    if (list is List) return list.whereType<Map<String, dynamic>>().toList();
    return const [];
  }

  Future<Map<String, dynamic>> backtestDetail(String id) =>
      _get('/api/v1/backtests/$id');

  Future<Map<String, dynamic>> backtestCreate(String strategyId,
      {String? mint, int? bars, String? startTime}) async {
    final body = <String, dynamic>{'strategy_id': strategyId};
    if (mint != null) body['mint'] = mint;
    if (bars != null) body['max_bars'] = bars;
    if (startTime != null) body['start_time'] = startTime;
    return _post('/api/v1/backtests', body);
  }

  Future<Map<String, dynamic>> settings() => _get('/api/v1/settings');

  Future<Map<String, dynamic>> submitOrder(Map<String, dynamic> order) =>
      _post('/api/v1/orders', order);

  Future<Map<String, dynamic>> cancelOrder(String id) =>
      _post('/api/v1/orders/$id/cancel');

  Future<void> stopAll() async {
    await _post('/api/v1/stop-all');
  }

  Future<Map<String, dynamic>> demoReset() async {
    return _post('/api/v1/demo/reset');
  }

  Future<Map<String, dynamic>> agentOverview() => _get('/api/v1/agent/overview');

  Future<List<Map<String, dynamic>>> riskEvents() async {
    final d = await _get('/api/v1/risk/events');
    final list = d['items'];
    if (list is List) return list.whereType<Map<String, dynamic>>().toList();
    return const [];
  }
}

class ApiException implements Exception {
  ApiException(this.message);
  final String message;

  @override
  String toString() => message;
}