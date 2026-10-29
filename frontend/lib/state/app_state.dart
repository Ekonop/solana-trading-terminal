import 'dart:async';

import 'package:flutter/foundation.dart';

import '../api/api_client.dart';
import '../api/ws_client.dart';
import '../config.dart';
import '../models/models.dart';

/// Central application state: holds the latest snapshots fetched from the API
/// and merges live WS events on top. Screens listen via [ChangeNotifier].
class AppState extends ChangeNotifier {
  AppState(this.cfg) {
    api = ApiClient(cfg);
    ws = WsClient(
      cfg: cfg,
      onEvent: _onWs,
      onState: (up) {
        wsConnected = up;
        notifyListeners();
      },
    );
  }

  final AppConfig cfg;
  late final ApiClient api;
  late final WsClient ws;

  bool wsConnected = false;
  bool backendUp = false;
  String? lastError;
  DateTime? lastSync;

  // --- snapshot data --------------------------------------------------------
  Map<String, dynamic>? statusJson;
  Map<String, dynamic>? dashboardJson;
  Map<String, dynamic>? settingsJson;
  List<MarketQuote> markets = [];
  List<StrategyView> strategies = [];
  List<PaperOrder> orders = [];
  List<FillView> fills = [];
  List<PositionView> positions = [];
  List<BacktestRun> backtests = [];
  List<RiskEventView> riskEvents = [];

  // --- convenience accessors -------------------------------------------------
  List<EquitySnapshot> get equitySeries {
    final raw = dashboardJson?['series'];
    if (raw is! List) return const [];
    return raw
        .whereType<Map<String, dynamic>>()
        .map(EquitySnapshot.fromJson)
        .toList();
  }

  DashboardStats? get stats {
    final s = dashboardJson?['stats'];
    if (s is! Map<String, dynamic>) return null;
    return DashboardStats.fromJson(s);
  }

  AccountView? get account {
    final a = dashboardJson?['account'];
    if (a is! Map<String, dynamic>) return null;
    return AccountView.fromJson(a);
  }

  Map<String, dynamic>? get equity {
    final e = dashboardJson?['equity'];
    return e is Map<String, dynamic> ? e : null;
  }

  StrategyView? strategyById(String id) {
    for (final s in strategies) {
      if (s.id == id) return s;
    }
    return null;
  }

  // --- lifecycle -------------------------------------------------------------
  Timer? _pollTimer;
  bool _syncing = false;

  void start() {
    ws.start();
    refresh();
    _pollTimer = Timer.periodic(const Duration(seconds: 5), (_) => refresh());
  }

  Future<void> refresh() async {
    if (_syncing) return;
    _syncing = true;
    try {
      final results = await Future.wait<dynamic>([
        api.dashboard(),
        api.markets(),
        api.strategies(),
        api.orders(),
        api.fills(),
        api.positions(),
        api.backtests(),
        api.settings(),
        api.status(),
      ], eagerError: false);
      // All coercion below is defensive: a malformed answer for ONE endpoint
      // degrades to an empty collection instead of crashing the whole refresh
      // (bug class: raw `as Map/List` casts painted the error banner).
      dashboardJson = JsonHelpers.asMap(results[0]);
      markets = JsonHelpers.asMapList(results[1]).map(MarketQuote.fromJson).toList();
      strategies = JsonHelpers.asMapList(results[2]).map(StrategyView.fromJson).toList();
      orders = JsonHelpers.asMapList(results[3]).map(PaperOrder.fromJson).toList();
      fills = JsonHelpers.asMapList(results[4]).map(FillView.fromJson).toList();
      positions = JsonHelpers.asMapList(results[5]).map(PositionView.fromJson).toList();
      backtests = JsonHelpers.asMapList(results[6]).map(BacktestRun.fromJson).toList();
      settingsJson = JsonHelpers.asMap(results[7]);
      statusJson = JsonHelpers.asMap(results[8]);
      backendUp = true;
      lastError = null;
      lastSync = DateTime.now();
      notifyListeners();
    } catch (e) {
      backendUp = false;
      lastError = e.toString();
      notifyListeners();
    } finally {
      _syncing = false;
    }
  }

  Future<void> withCall(Future<dynamic> Function() call) async {
    try {
      await call();
      await refresh();
    } catch (e) {
      lastError = e.toString();
      notifyListeners();
    }
  }

  void _onWs(WsEvent e) {
    switch (e.kind) {
      case 'health':
      case 'equity':
      case 'signal':
      case 'order':
      case 'fill':
      case 'position':
      case 'risk':
      case 'strategy':
      case 'system':
        // Any of these invalidate cached snapshots; refetch a light subset.
        unawaited(refresh());
        break;
      default:
        break;
    }
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    ws.close();
    super.dispose();
  }
}

/// Compact risk event model for the dashboard/system screens.
class RiskEventView {
  final String id;
  final String level;
  final String category;
  final String message;
  final int ts;

  RiskEventView({
    required this.id,
    required this.level,
    required this.category,
    required this.message,
    required this.ts,
  });

  factory RiskEventView.fromJson(Map<String, dynamic> m) => RiskEventView(
        id: JsonHelpers.s(m, ['id']),
        level: JsonHelpers.s(m, ['level']),
        category: JsonHelpers.s(m, ['category']),
        message: JsonHelpers.s(m, ['message']),
        ts: JsonHelpers.i(m, ['created-at', 'created_at', 'ts']),
      );
}