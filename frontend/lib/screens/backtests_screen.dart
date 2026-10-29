import 'package:flutter/material.dart';

import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';

class BacktestsScreen extends StatefulWidget {
  const BacktestsScreen({super.key, required this.state});
  final AppState state;

  @override
  State<BacktestsScreen> createState() => _BacktestsScreenState();
}

class _BacktestsScreenState extends State<BacktestsScreen> {
  String? _strategyId = 'momentum';
  String? _mint = 'So11111111111111111111111111111111111111112';
  final _bars = TextEditingController(text: '200');
  bool _creating = false;

  final _mints = {
    'SOL': 'So11111111111111111111111111111111111111112',
    'BONK': 'DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263',
    'JUP': 'JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN',
    'mSOL': 'mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So',
    'LST': '7dHbWXmci3dT8UFYWYZweBLXgycu7Y3iL6trKn1Y7ARj',
  };

  @override
  void dispose() {
    _bars.dispose();
    super.dispose();
  }

  Future<void> _create() async {
    setState(() => _creating = true);
    final bars = int.tryParse(_bars.text.trim()) ?? 200;
    try {
      await widget.state.api.backtestCreate(_strategyId!, mint: _mint, bars: bars);
      await widget.state.refresh();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('CREATE FAILED: $e'), backgroundColor: T.down),
        );
      }
    } finally {
      if (mounted) setState(() => _creating = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final runs = widget.state.backtests;
    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text('BACKTESTS', style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2)),
                const SizedBox(width: 12),
                const SimBadge(),
                const Spacer(),
                Text('REPLAY ONLY · NO LOOK-AHEAD', style: Type.labelCaps),
              ],
            ),
            const SizedBox(height: 16),
            // Create form
            TermPanel(
              title: 'NEW BACKTEST',
              children: [
                Row(
                  children: [
                    _picker('STRATEGY', ['momentum', 'scalper', 'dip_hunter', 'whale_tail', 'sniper'],
                        _strategyId!, (v) => setState(() => _strategyId = v)),
                    const SizedBox(width: 16),
                    _picker('MARKET', _mints.keys.toList(), _mintKey,
                        (v) => setState(() => _mint = _mints[v])),
                    const SizedBox(width: 16),
                    SizedBox(
                      width: 120,
                      child: TextField(
                        controller: _bars,
                        style: Type.data,
                        decoration: const InputDecoration(labelText: 'HISTORY BARS'),
                      ),
                    ),
                    const Spacer(),
                    TermButton(
                      label: 'RUN BACKTEST',
                      icon: Icons.play_arrow_rounded,
                      onPressed: _creating ? null : _create,
                    ),
                  ],
                ),
              ],
            ),
            const SizedBox(height: 12),
            // Runs table
            TermPanel(
              title: 'RUNS',
              trailing: Text('${runs.length} TOTAL', style: Type.labelCaps),
              children: [
                TermTable(
                  columns: const [
                    TermColumn('ID'),
                    TermColumn('STRAT'),
                    TermColumn('STATUS'),
                    TermColumn('NET RET', align: TextAlign.right),
                    TermColumn('WIN RATE', align: TextAlign.right),
                    TermColumn('TRADES', align: TextAlign.right),
                    TermColumn('QUALIFIED', align: TextAlign.center),
                    TermColumn('CREATED'),
                  ],
                  emptyText: 'NO BACKTEST RUNS YET',
                  rows: [
                    for (final r in runs)
                      [
                        Text(r.id.length <= 8 ? r.id : r.id.substring(0, 8),
                            style: Type.dataDim),
                        Text(r.strategyId.toUpperCase(), style: Type.dataBold),
                        StatusChip(label: r.status, state: r.status),
                        DirText(r.netReturnPct ?? '—'),
                        Text(r.winRate == null ? '—' : '${r.winRate}', style: Type.data),
                        Text(r.trades?.toString() ?? '—', style: Type.data),
                        r.qualified == null
                            ? Text('—', style: Type.dataDim)
                            : StatusChip(
                                label: r.qualified!,
                                color: r.qualified!.toLowerCase() == 'true' ? T.up : T.warn,
                              ),
                        TimeCell(r.createdAt, full: true),
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

  String get _mintKey {
    for (final e in _mints.entries) {
      if (e.value == _mint) return e.key;
    }
    return 'SOL';
  }

  Widget _picker(String label, List<String> items, String selected, ValueChanged<String> onPick) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text('$label ', style: Type.labelCaps),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
          color: T.input,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(selected.toUpperCase(), style: Type.data.copyWith(fontSize: 11)),
              const SizedBox(width: 4),
              PopupMenuButton<String>(
                color: T.panel2,
                icon: const Icon(Icons.arrow_drop_down_rounded, size: 14, color: T.fg3),
                padding: EdgeInsets.zero,
                onSelected: onPick,
                itemBuilder: (_) => [
                  for (final i in items)
                    PopupMenuItem(value: i, child: Text(i.toUpperCase(), style: Type.data)),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}