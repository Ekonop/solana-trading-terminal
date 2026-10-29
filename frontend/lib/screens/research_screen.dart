import 'dart:io';

import 'package:flutter/material.dart';

import '../models/models.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';

/// Research workspace: live strategy-signal feed + one-click CSV exports for
/// offline analysis (pandas, notebooks, spreadsheets).
class ResearchScreen extends StatefulWidget {
  const ResearchScreen({super.key, required this.state});
  final AppState state;

  @override
  State<ResearchScreen> createState() => _ResearchScreenState();
}

class _ResearchScreenState extends State<ResearchScreen> {
  List<SignalFeed> _signals = [];
  bool _loadingSignals = false;
  String? _signalError;

  String? _exportMsg;
  bool _exporting = false;

  @override
  void initState() {
    super.initState();
    _loadSignals();
  }

  Future<void> _loadSignals() async {
    setState(() {
      _loadingSignals = true;
      _signalError = null;
    });
    try {
      final rows = await widget.state.api.signals();
      if (!mounted) return;
      setState(() => _signals = rows.map(SignalFeed.fromJson).toList());
    } catch (e) {
      if (mounted) setState(() => _signalError = e.toString());
    } finally {
      if (mounted) setState(() => _loadingSignals = false);
    }
  }

  /// Fetch one export and write it to `~/stt-exports/<kind>-<ts>.csv`.
  Future<void> _runExport(String kind) async {
    setState(() {
      _exporting = true;
      _exportMsg = 'exporting $kind…';
    });
    try {
      final csv = await widget.state.api.exportCsv(kind);
      final dir = Directory(
          '${Platform.environment['HOME'] ?? '.'}/stt-exports');
      await dir.create(recursive: true);
      final ts = DateTime.now().toUtc().millisecondsSinceEpoch;
      final file = File('${dir.path}/$kind-$ts.csv');
      await file.writeAsString(csv);
      if (!mounted) return;
      setState(() => _exportMsg =
          'saved ${file.path} (${csv.split('\n').length - 1} rows)');
    } catch (e) {
      if (mounted) setState(() => _exportMsg = 'export failed: $e');
    } finally {
      if (mounted) setState(() => _exporting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text('RESEARCH', style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2)),
                const SizedBox(width: 12),
                const SimBadge(),
                const Spacer(),
                if (_exportMsg != null)
                  Flexible(
                    child: Text(_exportMsg!,
                        style: Type.labelCaps.copyWith(color: T.info, fontSize: 9),
                        overflow: TextOverflow.ellipsis),
                  ),
              ],
            ),
            const SizedBox(height: 16),

            // --- Export pack -------------------------------------------------
            TermPanel(
              title: 'EXPORT DATA (CSV)',
              trailing: Text('READ-ONLY · ~/stt-exports/', style: Type.labelCaps),
              children: [
                Padding(
                  padding: const EdgeInsets.only(bottom: 10),
                  child: Text(
                    'One-click dumps of the paper-trading data model for offline '
                    'research. Columns mirror the database schema; microsecond '
                    'timestamps; lamports for integers; USD decimals as strings.',
                    style: Type.body.copyWith(fontSize: 10.5),
                  ),
                ),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    _exportBtn('FILLS', Icons.receipt_long_rounded, () => _runExport('fills')),
                    _exportBtn('ORDERS', Icons.book_rounded, () => _runExport('orders')),
                    _exportBtn('LEDGER', Icons.account_balance_wallet_rounded, () => _runExport('ledger')),
                    _exportBtn('SIGNALS', Icons.notifications_active_rounded, () => _runExport('signals')),
                    _exportBtn('POSITIONS', Icons.trending_up_rounded, () => _runExport('positions')),
                    _exportBtn('EQUITY SERIES', Icons.show_chart_rounded, () => _runExport('equity')),
                    _exportBtn('BACKTESTS', Icons.science_rounded, () => _runExport('backtests')),
                    _exportBtn('RISK EVENTS', Icons.gpp_bad_rounded, () => _runExport('risk-events')),
                    _exportBtn('SYSTEM EVENTS', Icons.memory_rounded, () => _runExport('system-events')),
                  ],
                ),
                if (_exporting) ...[
                  const SizedBox(height: 10),
                  Text('EXPORTING…', style: Type.labelCaps.copyWith(color: T.info)),
                ],
              ],
            ),
            const SizedBox(height: 12),

            // --- Signals feed -------------------------------------------------
            TermPanel(
              title: 'SIGNAL FEED',
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  StatusChip(label: '${_signals.length} SIGNALS', color: T.info),
                  const SizedBox(width: 8),
                  GestureDetector(
                    onTap: _loadSignals,
                    child: const Icon(Icons.refresh_rounded, size: 13, color: T.fg3),
                  ),
                ],
              ),
              children: [
                if (_signalError != null)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text('ERROR: $_signalError',
                        style: Type.labelCaps.copyWith(color: T.down, fontSize: 9)),
                  ),
                if (_loadingSignals)
                  const Padding(
                    padding: EdgeInsets.symmetric(vertical: 24),
                    child: Center(
                        child: Text('LOADING…', style: Type.labelCaps)),
                  )
                else
                  TermTable(
                    columns: const [
                      TermColumn('T'),
                      TermColumn('STRATEGY'),
                      TermColumn('SYM'),
                      TermColumn('DIR', align: TextAlign.center),
                      TermColumn('SCORE', align: TextAlign.right),
                      TermColumn('DECISION'),
                      TermColumn('REASON'),
                    ],
                    emptyText: 'NO SIGNALS — START A STRATEGY TO GENERATE DECISIONS',
                    rows: [
                      for (final s in _signals.take(60))
                        [
                          TimeCell(s.generatedAt),
                          Text(s.strategyId.toUpperCase(),
                              style: Type.dataBold.copyWith(fontSize: 10)),
                          Text(s.symbol, style: Type.data),
                          StatusChip(
                            label: s.direction.toUpperCase(),
                            color: s.direction == 'buy' ? T.up : (s.direction == 'sell' ? T.down : T.info),
                          ),
                          Text(s.score, style: Type.data),
                          StatusChip(
                              label: s.decision == 'submitted' ? 'SUBMITTED' : 'REJECTED',
                              state: s.decision == 'submitted' ? 'filled' : 'rejected'),
                          Flexible(
                            child: Text(
                              s.reason,
                              style: Type.body.copyWith(color: T.fg2, fontSize: 9),
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                    ],
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _exportBtn(String label, IconData icon, VoidCallback onTap) {
    return TermButton(
      label: label,
      icon: icon,
      onPressed: _exporting ? null : onTap,
    );
  }
}

/// Signal row as surfaced by `/api/v1/signals`.
class SignalFeed {
  final String id;
  final String strategyId;
  final int strategyVersion;
  final String mint;
  final String direction;
  final String score;
  final String reason;
  final String decision;
  final String? rejectionReason;
  final int generatedAt;
  final String orderId;
  final String symbol;

  SignalFeed({
    required this.id,
    required this.strategyId,
    required this.strategyVersion,
    required this.mint,
    required this.direction,
    required this.score,
    required this.reason,
    required this.decision,
    required this.rejectionReason,
    required this.generatedAt,
    required this.orderId,
    required this.symbol,
  });

  factory SignalFeed.fromJson(Map<String, dynamic> m) {
    final symbol = switch (m['mint']) {
      'so11111111111111111111111111111111111111112' => 'SOL',
      _ => (m['mint']?.toString() ?? '?').substring(0,
          ((m['mint']?.toString() ?? '?').length).clamp(0, 6)),
    };
    final score = m['score'];
    return SignalFeed(
      id: JsonHelpers.s(m, ['id']),
      strategyId: JsonHelpers.s(m, ['strategy-id', 'strategy_id']),
      strategyVersion:
          JsonHelpers.i(m, ['strategy-version', 'strategy_version']),
      mint: JsonHelpers.s(m, ['mint']),
      direction: JsonHelpers.s(m, ['direction']),
      score: score == null ? '—' : score.toString(),
      reason: JsonHelpers.s(m, ['reason']),
      decision: JsonHelpers.s(m, ['decision']),
      rejectionReason: JsonHelpers.sN(m, ['rejection-reason', 'rejection_reason']),
      generatedAt: JsonHelpers.i(m, ['generated-at', 'generated_at']),
      orderId: JsonHelpers.s(m, ['order-id', 'order_id']),
      symbol: symbol,
    );
  }
}