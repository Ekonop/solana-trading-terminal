import 'package:flutter/material.dart';

import '../config.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';

class MarketsScreen extends StatelessWidget {
  const MarketsScreen({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final mkts = state.markets;
    final health = state.statusJson?['provider'] is Map<String, dynamic>
        ? state.statusJson!['provider'] as Map<String, dynamic>
        : null;
    final providerName = health?['name']?.toString() ?? '—';
    final providerState = health?['state']?.toString() ?? 'unknown';

    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text('MARKETS', style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2)),
                const SizedBox(width: 12),
                const SimBadge(),
                const Spacer(),
                StatusChip(label: '$providerName · $providerState', state: providerState),
              ],
            ),
            const SizedBox(height: 16),
            TermPanel(
              title: 'QUOTES',
              trailing: Text('${mkts.length} PAIRS', style: Type.labelCaps),
              children: [
                TermTable(
                  columns: const [
                    TermColumn('ASSET'),
                    TermColumn('PRICE (USD)', align: TextAlign.right),
                    TermColumn('24H VOL', align: TextAlign.right),
                    TermColumn('LIQUIDITY', align: TextAlign.right),
                    TermColumn('AGE'),
                    TermColumn('SOURCE', align: TextAlign.center),
                  ],
                  rows: [
                    for (final m in mkts)
                      [
                        SymbolCell(symbol: m.symbol, name: m.name),
                        Text(Fmt.usd(m.price), style: Type.dataBold),
                        Text(_compactUsd(m.volume24hUsd), style: Type.data),
                        Text(_compactUsd(m.liquidityUsd), style: Type.data),
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (m.stale)
                              const Padding(
                                padding: EdgeInsets.only(right: 6),
                                child: Icon(Icons.schedule_rounded, size: 10, color: T.warn),
                              ),
                            TimeCell(m.observedAt),
                          ],
                        ),
                        Text(m.source.toUpperCase(),
                            style: Type.labelCaps.copyWith(color: T.fg3, fontSize: 8)),
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

  String _compactUsd(String raw) {
    final v = double.tryParse(raw) ?? 0;
    if (v >= 1e9) return '\$${(v / 1e9).toStringAsFixed(2)}B';
    if (v >= 1e6) return '\$${(v / 1e6).toStringAsFixed(2)}M';
    if (v >= 1e3) return '\$${(v / 1e3).toStringAsFixed(2)}K';
    return '\$${v.toStringAsFixed(0)}';
  }
}