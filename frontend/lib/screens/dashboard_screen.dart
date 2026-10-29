import 'package:flutter/material.dart';

import '../config.dart';
import '../models/models.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/equity_chart.dart';
import '../widgets/widgets.dart';

class DashboardScreen extends StatelessWidget {
  const DashboardScreen({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final stats = state.stats;
    final account = state.account;
    final eq = state.equity;
    final series = state.equitySeries;
        final openPos = state.positions;

    final equityVal = eq?['equity_usd']?.toString() ?? (eq?['equity-usd']?.toString() ?? '0');
    final cashVal = account?.cashLamports ?? 0;
    final realized = eq?['realized_pnl_usd']?.toString() ??
        (eq?['realized-pnl-usd']?.toString() ?? '0');
    final unrealized = eq?['unrealized_pnl_usd']?.toString() ??
        (eq?['unrealized-pnl-usd']?.toString() ?? '0');
    final unreal = double.tryParse(unrealized) ?? 0;

    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Page header
            Row(
              children: [
                Text('SOLANA · PAPER TERMINAL',
                    style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2, color: T.fg)),
                const SizedBox(width: 12),
                const SimBadge(),
                const Spacer(),
                Text(
                  state.lastSync == null
                      ? '—'
                      : 'SYNC ${Fmt.micros(state.lastSync!.millisecondsSinceEpoch * 1000)}',
                  style: Type.labelCaps,
                ),
              ],
            ),
            const SizedBox(height: 16),

            // KPI row
            Row(
              children: [
                Expanded(
                  child: KpiCard(
                    label: 'Equity',
                    value: Fmt.usd(equityVal),
                    delta: stats == null ? null : Fmt.pctD(stats.netReturnPctD, signed: true),
                    deltaColor: stats == null
                        ? null
                        : (stats.netReturnPctD >= 0 ? T.up : T.down),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: 'Cash',
                    value: Fmt.sol(cashVal),
                    delta: '${account?.currency ?? 'SOL'} BALANCE',
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: 'Realized PnL',
                    value: Fmt.usd(realized),
                    delta: null,
                    valueColor: (double.tryParse(realized) ?? 0) > 0 ? T.up : ((double.tryParse(realized) ?? 0) < 0 ? T.down : null),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: 'Unrealized',
                    value: Fmt.usd(unrealized),
                    delta: '${openPos.length} OPEN POS',
                    valueColor: unreal > 0 ? T.up : (unreal < 0 ? T.down : null),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: 'Drawdown',
                    value: stats == null ? '—' : Fmt.pctD(stats.drawdownPctD),
                    delta: stats == null ? null : '${stats.samplePoints} SAMPLES',
                    valueColor: stats == null || stats.drawdownPctD == 0
                        ? null
                        : T.down,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),

            // Equity chart + portfolios side by side
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 3,
                  child: TermPanel(
                    title: 'EQUITY',
                    trailing: stats == null
                        ? null
                        : StatusChip(
                            label: '${Fmt.pctD(stats.netReturnPctD, signed: true)} NET',
                            color: stats.netReturnPctD >= 0 ? T.up : T.down,
                          ),
                    children: [
                      SizedBox(
                          height: 240, child: EquityChart(series: series)),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  flex: 2,
                  child: _OpenPositions(state: state),
                ),
              ],
            ),
            const SizedBox(height: 12),

            // Recent fills + signals
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 3,
                  child: _RecentFills(fills: state.fills),
                ),
                const SizedBox(width: 12),
                Expanded(
                  flex: 2,
                  child: TermPanel(
                    title: 'ACTIVE STRATEGIES',
                    children: [
                      for (final s in state.strategies)
                        Padding(
                          padding: const EdgeInsets.symmetric(vertical: 4),
                          child: Row(
                            children: [
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(s.displayName.toUpperCase(),
                                        style: Type.dataBold.copyWith(fontSize: 11)),
                                    Text(
                                      '${s.signalsTotal} SIG · ${s.signalsRejected} REJ',
                                      style: Type.labelCaps.copyWith(fontSize: 8),
                                    ),
                                  ],
                                ),
                              ),
                              StatusChip(label: s.status, state: s.status),
                            ],
                          ),
                        ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            if (state.lastError != null && state.backendUp)
              Text('LAST ERROR: ${state.lastError}', style: Type.dataDim.copyWith(fontSize: 10)),
          ],
        ),
      ),
    );
  }
}

class _OpenPositions extends StatelessWidget {
  const _OpenPositions({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final positions = state.positions;
    return TermPanel(
      title: 'OPEN POSITIONS',
      trailing: StatusChip(label: '${positions.length}', color: T.info),
      minHeight: 240,
      children: positions.isEmpty
          ? const [
              Padding(
                padding: EdgeInsets.symmetric(vertical: 60),
                child: Center(child: Text('NO OPEN POSITIONS', style: Type.labelCaps)),
              ),
            ]
          : [
              TermTable(
                columns: const [
                  TermColumn('SYM'),
                  TermColumn('QTY', align: TextAlign.right),
                  TermColumn('uPNL', align: TextAlign.right),
                ],
                rows: [
                  for (final p in positions)
                    () {
                      final q = state.markets.cast<MarketQuote?>().firstWhere(
                            (m) => m?.mint == p.mint,
                            orElse: () => null,
                          );
                      return [
                        SymbolCell(symbol: p.symbol, network: q?.network),
                        Text(p.quantityD.toStringAsFixed(p.quantityD.abs() > 100 ? 0 : 4),
                            style: Type.dataBold),
                        DirText.fromJson(p.unrealizedPnlUsd, fontSize: 11, bold: true),
                      ];
                    }(),
                ],
              ),
            ],
    );
  }
}

class _RecentFills extends StatelessWidget {
  const _RecentFills({required this.fills});
  final List<FillView> fills;

  @override
  Widget build(BuildContext context) {
    return TermPanel(
      title: 'RECENT FILLS',
      children: [
        TermTable(
          columns: const [
            TermColumn('T'),
            TermColumn('SYM'),
            TermColumn('SIDE', align: TextAlign.center),
            TermColumn('QTY', align: TextAlign.right),
            TermColumn('PRICE', align: TextAlign.right),
          ],
          emptyText: 'NO FILLS YET',
          rows: [
            for (final f in fills.take(6))
              [
                TimeCell(f.filledAt),
                Text(f.symbol, style: Type.dataBold),
                StatusChip(
                  label: f.side,
                  color: f.side.toLowerCase() == 'buy' ? T.up : T.down,
                ),
                Text(f.quantity, style: Type.data),
                Text(f.price, style: Type.data),
              ],
          ],
        ),
      ],
    );
  }
}