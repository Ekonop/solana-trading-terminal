/// Typed domain models for the terminal API.
///
/// The backend serializes with a mix of kebab-case and snake_case keys; every
/// model here parses defensively through [_pick] which accepts both.
library;

class JsonHelpers {
  /// Pick a value by any of several key spellings.
  static dynamic pick(Map<String, dynamic> map, List<String> keys, [dynamic def]) {
    for (final k in keys) {
      if (map.containsKey(k)) return map[k];
    }
    return def;
  }

  static String s(Map<String, dynamic> map, List<String> keys, [String def = '']) =>
      pick(map, keys, def)?.toString() ?? def;

  static String? sN(Map<String, dynamic> map, List<String> keys) {
    final v = pick(map, keys);
    return v == null ? null : v.toString();
  }

  static int i(Map<String, dynamic> map, List<String> keys, [int def = 0]) {
    final v = pick(map, keys);
    if (v == null) return def;
    if (v is int) return v;
    return int.tryParse(v.toString()) ?? def;
  }

  static double d(Map<String, dynamic> map, List<String> keys, [double def = 0]) {
    final v = pick(map, keys);
    if (v == null) return def;
    if (v is num) return v.toDouble();
    return double.tryParse(v.toString()) ?? def;
  }

  static bool b(Map<String, dynamic> map, List<String> keys, [bool def = false]) {
    final v = pick(map, keys);
    if (v == null) return def;
    if (v is bool) return v;
    return v.toString().toLowerCase() == 'true';
  }

  /// Coerce a raw JSON value to a String-keyed map, or empty map.
  /// Never throws: an unexpected shape degrades to an empty map instead of
  /// crashing the whole refresh loop (bug class: raw `as Map` casts).
  static Map<String, dynamic> asMap(dynamic v) {
    if (v is Map<String, dynamic>) return v;
    if (v is Map) return v.map((k, val) => MapEntry(k.toString(), val));
    return <String, dynamic>{};
  }

  /// Coerce a raw JSON value to a list of String-keyed maps, or empty list.
  /// Never throws (bug class: raw `as List` casts).
  static List<Map<String, dynamic>> asMapList(dynamic v) {
    if (v is List) {
      return v.whereType<Map<String, dynamic>>().toList();
    }
    return const [];
  }
}

enum MarketQuality { fresh, stale, bad }

class MarketQuote {
  final String mint;
  final String symbol;
  final String name;
  final String price;
  final String volume24hUsd;
  final String liquidityUsd;
  final bool stale;
  final int observedAt;
  final int quality;
  final String source;

  MarketQuote({
    required this.mint,
    required this.symbol,
    required this.name,
    required this.price,
    required this.volume24hUsd,
    required this.liquidityUsd,
    required this.stale,
    required this.observedAt,
    required this.quality,
    required this.source,
  });

  factory MarketQuote.fromJson(Map<String, dynamic> m) => MarketQuote(
        mint: JsonHelpers.s(m, ['mint']),
        symbol: JsonHelpers.s(m, ['symbol']),
        name: JsonHelpers.s(m, ['name']),
        price: JsonHelpers.s(m, ['price'], '0'),
        volume24hUsd: JsonHelpers.s(m, ['volume-24h-usd', 'volume_24h_usd', 'volume24h_usd'], '0'),
        liquidityUsd: JsonHelpers.s(m, ['liquidity-usd', 'liquidity_usd'], '0'),
        stale: JsonHelpers.b(m, ['stale']),
        observedAt: JsonHelpers.i(m, ['observed-at', 'observed_at']),
        quality: JsonHelpers.i(m, ['quality']),
        source: JsonHelpers.s(m, ['source']),
      );

  MarketQuality get qualityLabel {
    if (stale) return MarketQuality.stale;
    if (quality >= 80) return MarketQuality.fresh;
    return MarketQuality.bad;
  }

  double get priceD => double.tryParse(price) ?? 0;
}

class EquitySnapshot {
  final int ts;
  final String equityUsd;
  final String positionsValueUsd;
  final String exposurePct;
  final int cashLamports;
  final String unrealizedPnlUsd;
  final int realizedPnlLamports;

  EquitySnapshot({
    required this.ts,
    required this.equityUsd,
    required this.positionsValueUsd,
    required this.exposurePct,
    required this.cashLamports,
    required this.unrealizedPnlUsd,
    required this.realizedPnlLamports,
  });

  factory EquitySnapshot.fromJson(Map<String, dynamic> m) => EquitySnapshot(
        ts: JsonHelpers.i(m, ['ts', 'timestamp']),
        equityUsd: JsonHelpers.s(m, ['equity-usd', 'equity_usd'], '0'),
        positionsValueUsd: JsonHelpers.s(m, ['positions-value-usd', 'positions_value_usd'], '0'),
        exposurePct: JsonHelpers.s(m, ['exposure-pct', 'exposure_pct'], '0'),
        cashLamports: JsonHelpers.i(m, ['cash-lamports', 'cash_lamports']),
        unrealizedPnlUsd: JsonHelpers.s(m, ['unrealized-pnl-usd', 'unrealized_pnl_usd'], '0'),
        realizedPnlLamports: JsonHelpers.i(m, ['realized-pnl-lamports', 'realized_pnl_lamports']),
      );

  double get equityD => double.tryParse(equityUsd) ?? 0;
  double get positionsD => double.tryParse(positionsValueUsd) ?? 0;
  double get exposureD => double.tryParse(exposurePct) ?? 0;
}

class AccountView {
  final String id;
  final String name;
  final String currency;
  final int cashLamports;
  final int initialLamports;
  final int realizedPnlLamports;

  AccountView({
    required this.id,
    required this.name,
    required this.currency,
    required this.cashLamports,
    required this.initialLamports,
    required this.realizedPnlLamports,
  });

  factory AccountView.fromJson(Map<String, dynamic> m) => AccountView(
        id: JsonHelpers.s(m, ['id']),
        name: JsonHelpers.s(m, ['name']),
        currency: JsonHelpers.s(m, ['currency'], 'SOL'),
        cashLamports: JsonHelpers.i(m, ['cash-balance-lamports', 'cash_balance_lamports']),
        initialLamports: JsonHelpers.i(m, ['initial-balance-lamports', 'initial_balance_lamports']),
        realizedPnlLamports: JsonHelpers.i(m, ['realized-pnl-lamports', 'realized_pnl_lamports']),
      );
}

class DashboardStats {
  final String currentEquityUsd;
  final String peakEquityUsd;
  final String netReturnPct;
  final String dayReturnPct;
  final String drawdownPct;
  final String positionsValueUsd;
  final int samplePoints;

  DashboardStats({
    required this.currentEquityUsd,
    required this.peakEquityUsd,
    required this.netReturnPct,
    required this.dayReturnPct,
    required this.drawdownPct,
    required this.positionsValueUsd,
    required this.samplePoints,
  });

  factory DashboardStats.fromJson(Map<String, dynamic> m) => DashboardStats(
        currentEquityUsd: JsonHelpers.s(m, ['current-equity-usd'], '0'),
        peakEquityUsd: JsonHelpers.s(m, ['peak-equity-usd'], '0'),
        netReturnPct: JsonHelpers.s(m, ['net-return-pct'], '0'),
        dayReturnPct: JsonHelpers.s(m, ['day-return-pct'], '0'),
        drawdownPct: JsonHelpers.s(m, ['drawdown-pct'], '0'),
        positionsValueUsd: JsonHelpers.s(m, ['positions-value-usd'], '0'),
        samplePoints: JsonHelpers.i(m, ['sample-points']),
      );

  double get netReturnPctD => double.tryParse(netReturnPct) ?? 0;
  double get dayReturnPctD => double.tryParse(dayReturnPct) ?? 0;
  double get drawdownPctD => double.tryParse(drawdownPct) ?? 0;
  double get positionsValueD => double.tryParse(positionsValueUsd) ?? 0;
}

class PositionView {
  final String id;
  final String mint;
  final String symbol;
  final String direction;
  final String quantity;
  final String avgEntryPrice;
  final String currentPrice;
  final String unrealizedPnlUsd;
  final String notionalUsd;
  final String status;
  final int openedAt;
  final int? closedAt;

  PositionView({
    required this.id,
    required this.mint,
    required this.symbol,
    required this.direction,
    required this.quantity,
    required this.avgEntryPrice,
    required this.currentPrice,
    required this.unrealizedPnlUsd,
    required this.notionalUsd,
    required this.status,
    required this.openedAt,
    this.closedAt,
  });

  factory PositionView.fromJson(Map<String, dynamic> m) => PositionView(
        id: JsonHelpers.s(m, ['id']),
        mint: JsonHelpers.s(m, ['mint']),
        symbol: JsonHelpers.s(m, ['symbol']),
        direction: JsonHelpers.s(m, ['direction']),
        quantity: JsonHelpers.s(m, ['quantity'], '0'),
        avgEntryPrice: JsonHelpers.s(m, ['avg-entry-price', 'avg_entry_price'], '0'),
        currentPrice: JsonHelpers.s(m, ['current-price', 'current_price'], '0'),
        unrealizedPnlUsd: JsonHelpers.s(m, ['unrealized-pnl-usd', 'unrealized_pnl_usd'], '0'),
        notionalUsd: JsonHelpers.s(m, ['notional-usd', 'notional_usd'], '0'),
        status: JsonHelpers.s(m, ['status']),
        openedAt: JsonHelpers.i(m, ['opened-at', 'opened_at']),
        closedAt: JsonHelpers.i(m, ['closed-at', 'closed_at']).let((v) => v == 0 ? null : v),
      );

  double get pnlD => double.tryParse(unrealizedPnlUsd) ?? 0;
  double get quantityD => double.tryParse(quantity) ?? 0;
}

class PaperOrder {
  final String id;
  final String symbol;
  final String side;
  final String orderType;
  final String status;
  final String quantity;
  final String limitPrice;
  final String filledQty;
  final String avgFillPrice;
  final int createdAt;
  final String? strategyId;
  final String? rejectionReason;

  PaperOrder({
    required this.id,
    required this.symbol,
    required this.side,
    required this.orderType,
    required this.status,
    required this.quantity,
    required this.limitPrice,
    required this.filledQty,
    required this.avgFillPrice,
    required this.createdAt,
    this.strategyId,
    this.rejectionReason,
  });

  factory PaperOrder.fromJson(Map<String, dynamic> m) => PaperOrder(
        id: JsonHelpers.s(m, ['id']),
        symbol: JsonHelpers.s(m, ['symbol']),
        side: JsonHelpers.s(m, ['side']),
        orderType: JsonHelpers.s(m, ['order-type', 'order_type']),
        status: JsonHelpers.s(m, ['status']),
        quantity: JsonHelpers.s(m, ['quantity'], '0'),
        limitPrice: JsonHelpers.s(m, ['limit-price', 'limit_price'], ''),
        filledQty: JsonHelpers.s(m, ['filled-quantity', 'filled_qty'], '0'),
        avgFillPrice: JsonHelpers.s(m, ['avg-fill-price', 'avg_fill_price'], ''),
        createdAt: JsonHelpers.i(m, ['created-at', 'created_at']),
        strategyId: JsonHelpers.sN(m, ['strategy-id', 'strategy_id']),
        rejectionReason: JsonHelpers.sN(m, ['rejection-reason', 'rejection_reason']),
      );
}

class FillView {
  final String id;
  final String orderId;
  final String symbol;
  final String side;
  final String quantity;
  final String price;
  final int feeLamports;
  final int filledAt;

  FillView({
    required this.id,
    required this.orderId,
    required this.symbol,
    required this.side,
    required this.quantity,
    required this.price,
    required this.feeLamports,
    required this.filledAt,
  });

  factory FillView.fromJson(Map<String, dynamic> m) => FillView(
        id: JsonHelpers.s(m, ['id']),
        orderId: JsonHelpers.s(m, ['order-id', 'order_id']),
        symbol: JsonHelpers.s(m, ['symbol']),
        side: JsonHelpers.s(m, ['side']),
        quantity: JsonHelpers.s(m, ['quantity'], '0'),
        price: JsonHelpers.s(m, ['price'], '0'),
        feeLamports: JsonHelpers.i(m, ['fee-lamports', 'fee_lamports']),
        filledAt: JsonHelpers.i(m, ['filled-at', 'filled_at']),
      );
}

class StrategyView {
  final String id;
  final String displayName;
  final String description;
  final String status;
  final Map<String, dynamic> config;
  final int configVersion;
  final int signalsTotal;
  final int signalsRejected;
  final String? lastError;
  final int? lastEvalAt;

  StrategyView({
    required this.id,
    required this.displayName,
    required this.description,
    required this.status,
    required this.config,
    required this.configVersion,
    required this.signalsTotal,
    required this.signalsRejected,
    this.lastError,
    this.lastEvalAt,
  });

  factory StrategyView.fromJson(Map<String, dynamic> m) => StrategyView(
        id: JsonHelpers.s(m, ['id']),
        displayName: JsonHelpers.s(m, ['display-name', 'display_name']),
        description: JsonHelpers.s(m, ['description']),
        status: JsonHelpers.s(m, ['status']),
        config: (JsonHelpers.pick(m, ['config']) is Map<String, dynamic>)
            ? (JsonHelpers.pick(m, ['config']) as Map<String, dynamic>)
            : <String, dynamic>{},
        configVersion: JsonHelpers.i(m, ['config-version', 'config_version'], 1),
        signalsTotal: JsonHelpers.i(m, ['signals-total', 'signals_total']),
        signalsRejected: JsonHelpers.i(m, ['signals-rejected', 'signals_rejected']),
        lastError: JsonHelpers.sN(m, ['last-error', 'last_error']),
        lastEvalAt: JsonHelpers.i(m, ['last-eval-at', 'last_eval_at']).let((v) => v == 0 ? null : v),
      );

  bool get isRunning =>
      status == 'paper_trading' || status == 'shadow' || status == 'running';
}

class BacktestRun {
  final String id;
  final String strategyId;
  final String status;
  final String? qualified;
  final int createdAt;
  final int? completedAt;
  final String? error;
  final String? netReturnPct;
  final String? winRate;
  final int? trades;

  BacktestRun({
    required this.id,
    required this.strategyId,
    required this.status,
    this.qualified,
    required this.createdAt,
    this.completedAt,
    this.error,
    this.netReturnPct,
    this.winRate,
    this.trades,
  });

  factory BacktestRun.fromJson(Map<String, dynamic> m) => BacktestRun(
        id: JsonHelpers.s(m, ['id']),
        strategyId: JsonHelpers.s(m, ['strategy-id', 'strategy_id']),
        status: JsonHelpers.s(m, ['status']),
        qualified: JsonHelpers.sN(m, ['qualified']),
        createdAt: JsonHelpers.i(m, ['created-at', 'created_at']),
        completedAt: JsonHelpers.i(m, ['completed-at', 'completed_at']).let((v) => v == 0 ? null : v),
        error: JsonHelpers.sN(m, ['error']),
        netReturnPct: JsonHelpers.sN(m, ['net-return-pct', 'net_return_pct']),
        winRate: JsonHelpers.sN(m, ['win-rate', 'win_rate']),
        trades: JsonHelpers.i(m, ['trades', 'trade-count', 'trade_count']).let((v) => v == 0 ? null : v),
      );
}

/// WS envelope mirroring backend `events::WsEvent` kinds.
class WsEvent {
  final String kind;
  final Map<String, dynamic> data;

  WsEvent({required this.kind, required this.data});

  factory WsEvent.fromJson(Map<String, dynamic> m) => WsEvent(
        kind: JsonHelpers.s(m, ['kind']),
        data: JsonHelpers.pick(m, ['data']) is Map<String, dynamic>
            ? (JsonHelpers.pick(m, ['data']) as Map<String, dynamic>)
            : <String, dynamic>{},
      );
}

extension LetX<T> on T {
  R let<R>(R Function(T) f) => f(this);
}